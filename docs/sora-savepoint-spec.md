# Sora port: the save point

2026-10-12. Kingdom Hearts III's save point from the game's code and data, what the port builds
from it, and what is open. Marks: **[D]** read from an asset; **[C]** decompiled C only; **[?]** not
determined; INFERRED where the port had to choose.

## 1. The actor [D]

Blueprint `/Game/Blueprints/Gimmick/ex/g_ex_SavePoint` (`g_ex_SavePoint_C`), native class
`ATresSavePointActor` (super `ATresPlayerStart`: a save point IS a player start), system "save".
Its event graph is empty in the cooked game: everything is native.

| Part | Value |
|---|---|
| Root | `CollisionCapsule` (a default `CapsuleComponent`, no property set) |
| `MyReactor` (`TresReactorComponent`) | capsule radius 110, `Size.Z` 180, at the root -40 cm in Z: where the Save command is offered |
| `MyRecover` (`TresSavePointRecoverComponent`) | capsule radius 50, `Size.Z` 140, at the root -40 cm: where he recovers |
| `MyMapMarker` (`TresMapMarkerComponent`) | the map icon |
| `m_BaseEffect` / `m_BodyEffect` | `/Game/Effects/level/ex/gimmick/par/vg_com_svp_nml00` / `nml01` |
| `m_LimitBaseEffect` / `m_LimitBodyEffect` | `vg_com_svp_lmt00` / `lmt01` (for `m_bLimitedSavePoint`) |
| `m_HealEffect` | `vg_com_svp_in00` |

**No mesh, no light component, no animation or timeline, no sound asset.** The save point is five
particle systems: nml00 (7 emitters: ring, two base glows, stream, gradient, centre glow, a ground
decal), nml01 (the body: emitters `dmy_lit`, a decal, stream, gradient, centre glow, `kira`, tails), the two "limited" twins, and the heal burst. Its light is the effect's own; its sounds, if
any, are effect sounds [?] (none is named by the actor).

**The sound (added 2026-10-13) [D].** The actor names no sound, but its heal burst does: the particle system
`vg_com_svp_in00` lists `ParticleSeAssets` = `/Game/Sound/SE/_Data/SeAttach/common/vg_com_svp_in00`, whose sound is
`/Game/Sound/SE/common/00000/se00000_016` (4.5 s, one-shot, 3D with `Common_Attenuation`; sheet sora-sounds row
"Effect vg_com_svp_in00 starts", route `code.effect_se_common`). A sound a particle system carries starts with the
effect, at it (audio spec 5.5). The other four effects (nml00, nml01, lmt00, lmt01) carry none. So the save point
sounds EACH TIME THE HEAL BURST STARTS: when he steps on it needing recovery, and at the first visit of a save point
not accessed yet; an accessed save point entered with full gauges is silent.

## 2. Showing it [C]

Actor tick 0x140F9CA00; effect switches 0x140F8DCF0 (base), 0x140F8E280 (body), 0x140F8E910 (heal).

- Base and body are spawned attached to the root (at the actor-relative offset +0x4E4, zero by
  default) while the save point is active (`m_bIsActive`; `m_bAutoActivate`, `BP_ActivateGimmick`)
  and "in view": with `m_ClipOutDistance` above 0 they are removed when the player's camera is
  farther than that or the line to it is blocked (default 0 = always shown). A limited save point
  uses the lmt pair.
- Switching off deactivates the component (it stops emitting; what is in the air runs out).
- **(2026-10-13, IMPL_ER52) The offset +0x4E4 is NOT zero in play [I]:** `BeginPlay` 0x140F9AA40 (after `AActor::BeginPlay`
  0x143EA6430) traces a line (0x143FD41F0, channel 2) from the root straight down 140 cm (0x145D785B0); with a blocking
  hit that did not start inside something the offset = hit location - root, else (0, 0, -90) (0x145C9C5A4). All three
  effects (base 0x140F8DCF0, body 0x140F8E280, heal 0x140F8E910) are attached at that offset: the glow lies on the
  floor under the root and the column of light rises from it (the root itself stays where the level put it; the
  capsules hang from the root). Port: `kh_host::savepoint::ground_offset`, `SavePoint::effect_offset`; the Elden Ring
  host traces through its own collision (savepoint.rs `ground_phase`, rows `savepoint.ground_*`). The effect assets
  agree: their emitter origins are 0 to -10 cm (nml00 / nml01 `EmitterOrigin`), i.e. built around the floor point.
- It also reacts to battle mode, cinematic mode and the player's "arts" mode through
  `OnChangeBattleMode` / `OnChangeCinematicMode` / `OnChangePlayerArtsMode` and to
  `OnChangeSavePointDispType` (bodies not read [?]: in the game a save point cannot be used in battle).

## 3. Standing on it: the recover rule [C]

`UTresSavePointRecoverComponent` (a pawn-overlap component): begin 0x140F8FCD0, tick 0x140F93590,
end 0x140F93980.

- While a pawn overlaps the recover capsule and its "needs recovery" virtual (+0x1880; player
  0x141436260) is true: `accumulator += dt x 180`; when it is above 1, `n = floor(accumulator)` and
  `Recover(n, n, n, kind 1)` is called on the pawn and on each of its six party members, then
  `accumulator -= n`. So **HP, MP and focus each rise by 180 points per second**, in whole points.
  Kind 1: while the MP gauge is recharging the recharge timer loses n seconds instead (a recharge
  ends in a fraction of a second).
- "Needs recovery" (player): not dying, and HP below max, or MP below max (when the MP gauge is
  enabled), or focus below max (when enabled), or a party member below max HP or MP.
- The heal effect is on while the checked pawn needs recovery (and the screen is not faded), and for
  **3 s** (`+0x4E0`, ctor 0x140F8B810) at the first visit of a save point that was not accessed yet.
  It is spawned at the root; when the condition ends it is deactivated.
- `Recover(.., kind 1)` also raises the pawn's own recover effect of cure group
  `TRES_CURE_GRP_SAVEPOINT`; which effect and sound that is was not read [?].

**The item pouch.** The save point has NO pouch call. Used auto-reload pouch slots are refilled from
the stock by `RefillAll` (0x141446500) on the first unpaused frame after a pause (game-instance tick
0x140EAE5A0; magic spec J 7.5) and when the pawn is set up. Opening the save menu pauses the game, so
**closing the save menu refills the pouch**; merely standing on the save point does not. In the port:
`Sora::on_unpause()` when the save menu closes.

## 4. The command

Inside the reactor capsule the reactor offers its command: `ETresReactorCommandID::TRES_RCID_SAVE`,
command kind `TRES_CMD_KIND_SAVE` (76); carried out, the player enters `TresPlayerState_SaveMenu`
(state id 105, fields `m_pTargetActor`, `m_pComponent`) and the save menu opens [C names; the
reactor's and the state's bodies were not read: how the command is shown (its slot in the command
menu, its button) is [?]]. The port: a prompt of its own and the Y button.

## 5. The menu [D]

Movie `17_saveload/SaveLoad` (`UTresUIP_SaveLoad`), clip **`menuSavePoint`** (labels off, in, inBack,
loop, outNext): a `tres.rsl.StageCategory` title plate and `savePointObj` with six
`tres.rsl.TopMenuCmd` buttons `cmd1` .. `cmd6` (x 170, y 132 .. 612, 96 apart). Strings
`kh3_menu.savepoint.*`:

| Command | Help line |
|---|---|
| Save | Save your game progress. |
| Select an Area | Move to another save point. |
| World Map | Return to the World Map. |
| Visit the Bistro | Visit Little Chef's restaurant. |
| Tutorials | Go hands-on and learn how to play the game. |
| Title Screen | Exit the game and return to the Title Screen. |

Title "SAVE". Which of the six a save point shows is decided per save point by game progress:
`/Game/UI/Data/SaveMenuRestrictionData` (rows by save point name with the sections for area select,
world map and restaurant), through `/Game/UI/Data/SaveMenuDataAsset`. Save itself leads to the slot
list of the same movie (`menuSaveData`: thumbnails, level, munny, play time).
[?] the order of the six commands (the port: the order of their help strings), the labels played at
open / close, the menu's sounds (the port: the menus' cursor / decide / close static SE).

## 6. The port

- Pack: `tools/sora_savepoint_pack.js` -> the five effects (+ 7 materials new to the pack) in
  `build/sora_pack/vfx`, `build/sora_pack/savepoint/savepoint.json` (effects by role, the two
  capsules, the recover numbers), rows in `sheets/port/sora-assets.json`. The new materials have no
  translated shader yet (`tools/sora_vfx_shaders.js` and, for Elden Ring, `kh-shaderc --vfx` were not
  re-run: they rewrite files the running hosts read): they draw with the generic variant.
- `crates/kh-host/src/savepoint.rs`: `SavePoint` (the recover rule, the heal effect's condition, the
  reactor test, the effects as free effects); 2 tests. INFERRED: `Size.Z` = the capsule's full
  height; overlap = his capsule against the shape; the actor root 92 cm above the floor (a default
  player-start capsule's half height).
- `crates/kh-ui/src/savepoint.rs`: `SavePointMenu` on the game's clip; `kh_host::hud::Hud`:
  `open_savepoint`, `savepoint_open`, `take_savepoint_result`; the menu holds the engine pause.
- Test app (`crates/kh-app/src/savepoint.rs`): one save point 3 m behind and 6 m left of the start;
  prompt "Y Save" inside the reactor; Y opens the menu (D-pad / stick, A, B); closing it refills the
  pouch; Save writes nothing (the test level has no save file); `--no-savepoint`.

**The sound in the port (2026-10-13).** The effects are free effects of the host's effect side, which do not pass the
core's effect seam (where the audio side starts effect-carried sounds), so `SavePoint::step` asks for the sound itself
whenever the heal burst starts: a seam sound named `effect:vg_com_svp_in00` at the actor root, through Sora's own
event list (`kh_audio::Routes::resolve` turns "effect:<name>" into the first sound the effect carries). Both hosts
get it (the test app and the Elden Ring mod). `SavePoint::set_accessed` is for a host whose save points are made anew
(the mod: one per site of grace in reach; a DISCOVERED grace counts as accessed). Host switch: `SavePointSound`
(Off / WithHeal = KH3 / EveryEntry = not KH3).

**KH3 shows no message at a new save point** (checked 2026-10-13 for the Elden Ring host's unlock): the actor names none,
`kh3_menu.savepoint.*` holds only the menu's title, commands, help lines and questions, and the first visit's whole
feedback is the 3 s heal burst with its sound (section 3). A host that unlocks its save point when he runs into the reactor
sets `SavePoint::first_in_reactor`: the burst then starts in the reactor capsule instead of only in the recover capsule.

**The Elden Ring host's menu (2026-10-13, user decisions, sheet decisions dec022)**: `kh_host::savepoint_menu`: Save /
Select an Area / Pass Time / Respawn Enemies / Title Screen (+ Site of Grace with `savepoint.sit = 1`). "Pass Time" and
"Respawn Enemies" are COMPOSED labels (no KH3 string), drawn with the menu's own entry clip and font; their icon cells are
`camp8` (the cogwheel) and `camp3` (sparkles in a ring) of the category icon clip. Pass Time asks Morning / Noon / Nightfall
in the Dialog window (`SavePointMenu::host_choices`: three buttons, no text line, B = back). Notes:
build/sora_tmp/er/IMPL_ER26_STATE.md.

**The Elden Ring host's story events (2026-10-13, user decision, sheet decisions dec023; notes build/sora_tmp/er/IMPL_ER33_STATE.md)**:
a Save is never followed by the other game's rest. What Elden Ring ties to resting (Melina's accord, her invitation to
the Roundtable Hold, her talks) is a table of the game's own conditions, `sheets/port/sora-er-story.json`, parsed from
its event and talk scripts; `crates/kh-er/src/story.rs` makes the game's real rest for the hidden body, under a black
cover and without Sora taking the body's pose, the moment a row is met at the save point he stands in. The menu gets a
sixth entry, "Talk to Melina" (COMPOSED label, the ring-of-light cell `save2`, the KH3 command slot of Select an Area),
only while a row says the game offers a talk (`kh_host::savepoint_menu::GraceMenu::talk`). In a cutscene the scene's own
site of grace is hidden (`cutgrace.rs`) and the save point stays drawn. (IMPL_ER36) Only the accord is made unasked; every story rest is driven to its end (`talkdrive.rs`): the other game's grace menu is never shown, "Talk to Melina" selects her entry in it under the cover, uncovers her talk and leaves the rest for the player afterwards.

## 7. What only the running game can settle

1. How high the root sits above the floor, and so where the ring is (a screenshot of a save point). (The ring: SETTLED by the BeginPlay trace, section 2: the effects sit on the floor whatever the root height.)
2. The pawn's recover effect and sound of cure group SAVEPOINT; any sound on approach.
3. How the Save command is presented (command-menu slot, button) and the menu's command order.
4. The reactions to battle / cinematic mode and `ETresSavePointDispType`.

**(SP2, 2026-10-20, user decisions, sheet decisions dec033; notes build/sora_tmp/party/IMPL_SP2_STATE.md)** Elden Ring host:
(1) the grace's own invisible character (c1000) is not offered to the core (kh-er targets.rs `offered_as_body`): no Run
sway (`RunSwayL/R`, the "trip") and no push at a save point, for every leader; (2) `kh_host::savepoint::EntryBurst`
(`SavePoint::entry`, key `savepoint.entry`): every entry into the save point's area replays KH3's first-visit burst
(in00, 3 s) and the sound it carries (se00000_016) - KH3 has no other save-point effect or sound asset, so idle = nml00 /
nml01, enter = in00, stay = in00 while recovering; the mod plays the sound through the host queue
(`SavePoint::host_sounds`), so it is heard with a friend leading and Sora out of the party; (3) the prompt is KH3's own
reaction command "Save" (command kind 76, `kh3_system.situation.save`) in the default HudCommand's event button (the green
bar over the command menu with the Y icon: `Hud::set_host_command`), shown exactly while Y opens the menu
(`savepoint_rules::prompt_shows`, key `savepoint.prompt`); the test app uses the same (its old text prompt:
`KH_SAVEPOINT_TEXT_PROMPT=1`); (4) the slot rows' world picture is Elden Ring's own title logo (save-load pack world "er").
