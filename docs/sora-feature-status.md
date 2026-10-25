# Sora port: feature status (2026-10-12)

One table of everything Sora can do in the test app `khgame.exe`, compiled from the implementers' notes
(`build/sora_tmp/IMPL_{ACTIONS,COMBAT,MAGIC,HUD,AUDIO,VFX,KEYCHAIN,STATUS,INTEGRATION}_STATE.md`) and their audits
(`impl_actions/ABILITY_AUDIT.md`, `impl_combat/ABILITY_AUDIT.md`, `impl_audio/SOUND_COVERAGE.md`,
`vfx/RENDER_COVERAGE.md`). In the app, **F1** shows the pad inputs of this table as a help page.
Brought up to date after the fourth combat pass and the leftovers pass (`IMPL_CLEANUP_STATE.md`), same day.

## How to try it (Xbox pad)

1. Double-click **`play.bat`** in the project folder (it starts `target\release\khgame.exe`, and builds it first only
   when that file does not exist). The pad is read directly; keyboard and mouse work beside it.
2. The screen starts with a debug text. **F1** steps through: debug text -> **PAD HELP** (every pad input of every
   feature) -> nothing -> debug text. `play.bat --hide-debug-text` starts without it.
3. Sora stands at the origin at level 99 with everything equipped. **LB + View (Back)** (key L) steps through the test
   levels 1 -> 20 -> 99 (base spells / -ra / -ga, stats and abilities of the level, gauges full; shown on screen). Five training dummies stand 5 to 8.5 m **behind**
   the start (two standing, a small, a large and a floating one); the sixth is the bystander in the yard. **View
   (Back)** on the pad puts every dummy back to full, fills his HP / MP / focus and refills the items, at any time.
4. **The traversal yard is to the right of the start** (the red floor line points forward, the blue one to the right:
   follow the blue line for 14 m; every prop has a label). In metres (forward, right): the rail from (3, 14) to
   (29, 14); the 3 m ledge block at (20 .. 23, 16 .. 19); the bystander at (10, 22); the pole at (14, 24); the bar at
   (14, 30 .. 34); the tall runnable wall with its face at forward 30, right 16 .. 40; the hop points at (6 .. 15, 43);
   the slide ramp at (36 .. 46, 42 .. 48). The table "Traversal" below says what to press at each. In front of the
   start are the stage-1 lanes (boxes, stairs, ramps, a 3 m wall).
5. **Menu (Start)** opens the camp menu when no dummy was hit in the last 3 s, the pause screen otherwise.
6. Your options are the default. To get the game's own behaviour instead, add to `play.bat`:

   | Switch | The game's rule instead of your option |
   |---|---|
   | `--game-exact-turning` | turning as read from the game (the turn-around at 75 degrees) instead of continuous turning |
   | `--game-exact-speed` | the binary walk / run speed limit instead of the speed ramp |
   | `--game-exact-anim` | the walk / run clip swap at play rate 1 instead of the speed-matched animation |
   | `--game-exact-jump` | the game's short-hop rule instead of "jump cut on release" |
   | `--game-exact-weapon` | the Keyblade's appear sequence on the first attack too (the mesh 0.22 s after the effect) instead of "in the hand at once" |
   | `--game-exact-slide` | the game's slide start (he must face downhill; the ramp can be walked up) instead of "slides from any direction" |
   | `--game-exact-camera-stick` | right stick up = the camera rises and looks down (the camera code with the engine's stick convention; the game's own raw sign is an open probe item) instead of "stick up = look up" |
   | `--game-exact-shotlock-camera` | no camera of the port's own while a shotlock plays and no reset behind him afterwards (the game's code as read) |
   | `--glide-ends-on-ground` | not one of your options: an open question (off = only releasing B ends a glide, as the code reads) |

   Other useful ones: `--level N` (1 .. 99), `--voice-language ja`, `--mute`. All switches are listed at the top of
   `crates\kh-app\src\main.rs`.

## How to read it

- **Status**: *works* = built from the spec and it ran; *partly* = runs, with the gaps named; *missing* = not built.
- **Pad**: Xbox labels, the game's default layout (A attack / confirm, B jump / cancel, X guard / dodge, Y situation
  command, LB shortcuts, RB lock-on tap / shotlock hold, RT next target, LT next situation command, D-pad command
  menu, R3 camera reset, Menu pause / camp menu, View = test-level reset).
- **How tested**: **U** = unit tests of the rule in the engine-free crates; **A** = at least one scripted run of the
  app through the pad path with a screenshot somebody looked at; **A-** = it ran in the app in a scripted run but no
  picture of it was looked at; **none** = never run in the app.
- WHAT NO ROW CLAIMS: nobody has played any of this by hand in these passes. "Pad path" means scripted button and
  stick events that enter the same `Input::pad` a real pad fills; the XInput device code itself is exercised only when
  you play. Nobody has listened to the sound (it was checked by logs and numerical renders), and no picture was
  compared with the real game.
- Out of scope by your decision: **Attraction Flow** (no states, commands, HUD entries, effects or sounds).

## Movement

| Feature | Status | Pad | How tested | Known gaps |
|---|---|---|---|---|
| Walk, run, turn, stop, run start (stage 1) | works | left stick | U (golden hash, 16 traces), A, played by you | your options (continuous turning, speed ramp, speed-matched animation) are the default; the game-exact switches exist |
| Field camera | works | right stick (up = look up since 2026-10-12, your option), R3 | U, A (trace: stick up -> pitch +50, stick right -> yaw up) | wall run / rail / slope branches added 2026-10-12 (U, A- by trace numbers; their constants are in the code, not yet sheet rows); camera shakes only as the additive offset effects ask for (Effects table), no level camera volumes, no airstep or dive camera |
| Attack cameras (the game's "attack camera" request: Buzz Saw and the pole spin pull the camera back, Sonic Rave and Risk Dodge look at the target, the burn run loosens the follow) | partly (since the fourth combat pass) | - | U (kh-core combat4, kh-camera), A (pole spin -> Buzz Saw: on / off in the log, pictures looked at); risk dodge A- (event in the log); Sonic Rave and burn never looked at in the app | modes 1 / 2 / 8 (other Keyblades, shooter mode) not built; the distance scale after the camera (controller+0xAE0) is a probe item |
| Camera Work: Auto / Manual (Config) | works | Config screen | U, A- (trace: no follow in Manual) | - |
| Jump, High Jump (every ground jump at level 99) | works | B | U, A | height cut on release is your option |
| Doubleflight | works | B in the air | U, A | - |
| Glide | works | hold B while falling | U, A | by the code as read only releasing B ends it (`--glide-ends-on-ground` is off): to confirm in the game |
| Landings (land pose, roll landing after a long fall) | works | - | U, A | - |
| Floating mode (a script mode of set pieces) | works | Home key toggles it (test fixture) | U, A | only reachable by the debug key |
| Swim, dive | missing | - | none | needs water volumes; the seam exists |
| Climbing / vault | missing | - | none | dead code in the retail game |
| Talk, examine, treasure chest, shop | missing | - | none | no such actors in the test level |
| Save point (docs/sora-savepoint-spec.md) | partly (new 2026-10-12) | walk onto it (3 m behind, 6 m left of the start); **Y** inside it opens the save menu; D-pad / stick, A, B | U (kh-host savepoint 2), A (scripted `tap y`: pictures of the point, the heal effect, the menu looked at) | drawn with its five particle systems (7 of their materials on the generic shader: the shader tools were not re-run); +180 HP / MP / focus per second while he stands on it; the pouch is refilled when the save menu closes (the game's rule); Save writes nothing in the test level; the prompt is the port's own; the menu's command order and its sounds are assumed; the height of the ring above the floor is a probe item |
| Prize orbs (docs/sora-prize-spec.md) | works (new 2026-10-12) | defeat a dummy (script `dummy-hp N` leaves it 60 HP) | U (kh-prize 9, kh-host 1), A (scripted `tap a`: burst, bounce, flight to Sora, "OBTAINED MUNNY!" looked at) | a TEST-LEVEL fixture: each dummy has the table of real KH3 enemies and is back 4 s after it burst (`--no-prizes`: stays down); item boxes fly straight (the game's arc is not ported); no recover effect on Sora at pick-up (not read); bounce / get sounds were checked in the audio log, not listened to |

## Defence

| Feature | Status | Pad | How tested | Known gaps |
|---|---|---|---|---|
| Guard (ground and air), guarded hit, Block Replenisher | works | X with the stick neutral | U, A | - |
| Counter after a guard (Counter Slash; Counter Impact / Kick when equipped) | works | A in the 1 s window (the command shows in the situation slot) | U, A- | the three reprisals exclude each other, as in the game |
| Final Blow (chase after a counter) | works | A | U | never looked at in the app |
| Dodge Roll | works | X + stick | U, A | - |
| Air Slide (3 levels), Aerial Dodge in battle | works | X + stick in the air | U, A | - |
| Aerial Recovery, Payback Strike | works | B 0.2 s into being thrown; then A | U | never looked at in the app |
| Risk Dodge (under-slide, over-slide, slide-turn, warp) and its counter | partly | B while **DODGE!** shows; then Y for the counter | U, A (prompt and under-slide looked at; the other three types ran without a picture of the prompt) | attacker bounds, warp destination and the slide moves are simplified; needs an enemy attack that warns of it: the test level's `dummy-risk` + `dummy-auto` script words; where the game draws the prompt was not read (the port: above the attacker) |
| Being hit: flinch, blow, toss, launch, parabola, beat; wall bounce; death; Second Chance, Withstand Combo, auto-revive | works | - | U, A (flinch, blow, toss, wall) | fixture hits only (script words `dummy-attack`, `dummy-auto`) |
| Bad statuses on Sora: stun (stick flick), freeze, burn, sneeze, stop, HP break ... | works | left stick to shake off stun | U; A- (the third combat pass ran stun / freeze / burn / sneeze in the app, no record that the pictures were looked at); sneeze A (leftovers pass, picture looked at) | SNEEZE as corrected by the fourth pass: he attacks normally and the sneeze takes the place of the combo's finisher; from two steps before the end of the combo the Attack command reads "Sneeze" (the return to "Attack" is unit-tested, not looked at); cloud / discharge have no adhere actors |

## Traversal (flowmotion and terrain; the yard to the right of the start)

| Feature | Status | Pad | How tested | Known gaps |
|---|---|---|---|---|
| Superslide | works | X out of a wall kick, a flow jump, a pole, a rail | U, A | - |
| Wall Kick | works | Air Slide / Superslide into a wall; X in a wall run | U, A | every stage-1 wall is kickable (no surface data in the test level) |
| Wall Run (up and along) | partly | run at the flagged wall with the stick held; B super jump, X wall kick | U, A | no actor pitch on leaning walls, no corner rolls; wall camera added 2026-10-12 |
| Pole spin, bar swing | partly | Air Slide into it, or airstep to its point; B jump off, X Superslide | U, A | the grab pose is simplified; pole camera = attack camera mode 5 |
| Rail Slide (level rail and the Blizzard spell's ice rail) | partly | walk onto it; stick back turns round, B jumps off, X Superslide | U, A | straight segments, no rail-to-rail transfer, no slope acceleration |
| Ledge hang and climb | works | jump at the ledge, or airstep to its point; stick toward = climb, away = let go, B = drop | U, A | a ledge must be about 210 cm above what is under it (the probe as specified); no moving ledges |
| Slope Slide | partly | step onto the slide ramp from any side, also from below (your option since 2026-10-12: it cannot be walked up); stick steers within 50 degrees of the fall line; B jumps | U, A (walking at it from below: he gets 1 m up, slides back, again) | no body pitch; the platform on top can no longer be walked to (jump onto the ramp's side to slide); the game's rule (`--game-exact-slide`): he must face downhill |
| Side-step round a pawn | partly | run into the bystander | U, A | the bump is a body-circle test, not the engine event |
| Hop points | partly | walk near; stick fully = next point; B = off | U, A | ordered / closing point kinds not built |
| Attacks out of wall run, rail, Superslide / wall kick, pole spin | works (since the third / fourth combat pass) | A | U (combat3, combat4), A (wall run -> M_1144, rail -> 1181, pole spin -> Buzz Saw 1183: state sequences in the logs, pictures looked at) | the wall-run attack windows got their data rows in the fourth pass |
| Attacks off a hop point and out of a slope slide (kind 0x2F); Somersault out of a bar swing | works by its tests only | A | U (combat4); **never run in the app** | without the Somersault ability the bar-swing attack is an ordinary air attack (tested); the bar swing's own exit into Somersault at the end of the swing is not built; attacks out of climbing: dead code |

## Melee

| Feature | Status | Pad | How tested | Known gaps |
|---|---|---|---|---|
| Ground combo, air combo, finishers; every level-99 attack ability (Quick Slash, Flash Step, Radial Blaster, Speed Slash, Triple Rush, Magic Flash, Last Charge, Aerial Sweep / Dive, Falling Slash with its shockwave ...) | works | A | U (the 99-loadout decision table, damage numbers of the spec), A (incl. the air-down finisher with its shockwave, fourth pass) | hit stop does not exist in the game either; fourth pass: a hit window sweeps in the frame that opens it (after the state's own tick, as the game's manager does), the shockwave shot starts at the Keyblade's tip (socket ATK_01), the advance stops while his body touches the target's (both unit-tested only) |
| Berserk Charge (endless combo at 0 MP) | works | A | U, A (nine normal steps in a row seen in the log) | - |
| Launch out of a combo (Rising Spiral, Groundbreaker) | works | B during a combo | U | not looked at in the app |
| Soft target, lock-on, target switch, lock-on camera | works | RB tap, RT tap | U, A | - |
| Keyblade drawn / put away, battle stance | works | drawn by the first attack, in its first frame (your option since 2026-10-12; the game's appear sequence shows the mesh 0.22 s later: `--game-exact-weapon`); put away after the battle ends | U, A (picture of the attack's first step) | test-level rule: a passive dummy counts as "in battle" for 3 s after a blow; until 2026-10-12 the attack itself did not draw it at all (the game's Draw call of Attack Begin was missing) |
| Hits on guarding / armoured / reflecting bodies, weapon recoil, clash, hits on walls | works | - | U, A (guard recoil, wall) | driven by the dummies' script switches |
| Hit effects, hit sounds, damage to the dummies, EXP | works | - | U, A | no damage numbers (the game has none) |
| Team attacks, links, summons | missing | - | none | no party in the port |

## Formchange (Kingdom Key: Second Form)

| Feature | Status | Pad | How tested | Known gaps |
|---|---|---|---|---|
| Formchange gauge, the offer, Second Form with its abilities and timer | works | Y when the command shows; LT steps through the situation commands | U, A | the form logo movie is not in the pack (the title plate keeps "COMMANDS") |
| Finish commands: Stun Impact, Sonic Rave, Ars Arcanum | works | Y (the finish command); A / Y inputs inside Sonic Rave | U, A (one finisher) | Sonic Rave's first clip follows the code literally (probe item); Last Arcanum counts every window and its "finish end" flag is never set, as the code reads (fourth pass) |
| Auto Finish, Formchange Extender | works | - | U; Auto Finish A (Stun Impact used by itself at 0.5 s left, log and pictures looked at); Extender U only | Stun Impact sends the game's force-feedback event; nothing plays it (see Vibration) |
| Forms of other Keyblades | missing | - | none | only the Kingdom Key has data, a model and a chain in the port |

## Magic and items

| Feature | Status | Pad | How tested | Known gaps |
|---|---|---|---|---|
| Fire, Blizzard, Thunder, Water, Aero, Cure with their -ra / -ga tiers; MP, MP charge, MP Haste, combo cost | works | D-pad to **Magic**, D-pad right or A, pick with D-pad, A; or LB + B / Y / X / A (shortcut Set A: Firaga, Blizzaga, Thundaga, Curaga) | U (the spec's numbers), A | the obstacle test between two ice-rail nodes is not ported; after a ground cast he is "falling" for 3 steps as the code reads (flag to flip); Cure targets Sora only (no party) |
| Grand magic (situation command after repeated casts) | works | Y | U, A | - |
| Casting out of glide, dodge, air slide, a guarded hit | works | as above | U | not looked at in the app |
| Items from the command menu and shortcuts, auto-reload | works | D-pad to **Items**, A; LB + face button (Set B: Wataga, Aeroga, Potion, Ether); LB + D-pad up / down changes the set | U, A | which item heal effect plays is inferred from asset names |
| Under-water spell variants, Sneeze magic | partly | - | U (sneeze attack) | no water |

## Shotlock and airstep

| Feature | Status | Pad | How tested | Known gaps |
|---|---|---|---|---|
| Aim mode, locks, King of Hearts, Ragnarok at MAX, focus gauge, Focus Syphon | works | hold RB (right stick aims, up = look up), A fires, release RB cancels | U, A (King of Hearts and Ragnarok looked at, 2026-10-12) | Focus Syphon not looked at in the app |
| Shot camera while a shotlock plays (close orbit during the lift / charge, wide shot on Sora + the locked targets for the volley, then behind him) | works (new 2026-10-12; a design of the port, NOT the game's camera) | - (the sticks do not reach it) | U (kh-camera shot.rs), A (King of Hearts, Ragnarok: pictures across the move and 1 s after) | the game as read shows the move with the field camera (`--game-exact-shotlock-camera`); Focus Syphon gets the same camera, never looked at; a hit that breaks the move ends it without a blend test |
| Scope, lock counter, lock marks, their sounds | works (since 2026-10-12) | - | U (kh-ui), A | after a shot the marks now stay on their targets until the shotlock has ended, as the game's calls say (looked at only on the frames right after firing: the camera then turns away from the dummies); the full-screen colour filter is not drawn; the blue aim tint that stayed after an airstep is gone (2026-10-12: the scope clip never got its second leave state `out2`; picture 1 s after an airstep looked at) |
| Airstep to an enemy, to level points, chains with the kick, abort | works | while aiming, X; B aborts | U, A | the airstep camera is the plain field camera, eased behind him when the airstep has ended (2026-10-12, with the shot camera option); the enemy-turn swing is not built |
| Flow attack after an airstep (Sonic Slash, Buzz Saw, Sonic Cleave) | works | A when **ATTACK!** shows | U, A (Sonic Slash, Buzz Saw) | Sonic Cleave (target below): fourth-pass fix (no height change inside its traction window; a cleave that hits nothing spawns no shockwave), U only, **never run in the app** |
| Airstep onto a pole (spins) and to a ledge (hangs, climbs) | works (since 2026-10-12) | aim at the point in the yard, X | U, A | as an intermediate hop the pole stop is simplified |

## HUD and menus

| Feature | Status | Pad | How tested | Known gaps |
|---|---|---|---|---|
| HP / MP / focus gauges, red damage lag, low-HP alarm, portrait | works | - | U (pixel comparison with the offline render of the game's movie), A | portrait form frames fixed; story flags that hide gauges not modelled |
| Command menu, sub lists, shortcut panel, situation commands with timers, style gauge, shotlock panel | works | D-pad, A, B, LB, Y, LT | U, A | world skins and form logos not used; "Sneeze" on the Attack slot added by the leftovers pass (U, A) |
| Target cursors (soft, lock-on), enemy HP gauge (Scan), EXP numbers, level up, Next LV, munny / item popups | works | - | U, A | munny and item popups are fed by test words only |
| Prompts: DODGE!, ATTACK!, reaction commands | works (since 2026-10-12) | B / A / Y | U, A | prompt position is the port's choice |
| Pause screen (in battle) / camp menu (out of battle) | works | Menu | U, A | - |
| Camp menu: Equipment, Abilities, Items, Customize (magic slots, shortcut sets), Stats, Game Help, Config | partly | D-pad or left stick, A, B, Y, X, LB / RB pages, LT / RT tabs | U, A | the character step of Equipment / Abilities / Stats now carries the game's title and help line (Equipment looked at; Abilities and Stats not); each row of the Abilities list is its own copy since 2026-10-12 (the lower of two equal abilities toggled the upper one: U, A); item and equipment help texts empty; Keyblade slots 2 and 3 are menu-only; Abilities "Show Next List" missing; cursor repeat is the game's since the leftovers pass (first after 0.6 s, then every 0.06 s; unit-tested, not felt on a pad) |
| Config saved and loaded | works (since 2026-10-12) | Config screen | U, A- (file written, read back in a second run) | `khgame_config.txt` beside the exe |
| Config options that act | partly | Config screen | U / A- | act: Camera Work, both camera flips and sensitivities, Command Selection (slow), cursor position, situation command priority, Control Type B, Master Volume; do not act: Vibration (switch only, see Sound), brightness, subtitles, maps, combo cam, cinematics, command menu skin |
| Minimap, mission / score / timer HUD, gummi and photo HUDs | missing | - | none | not Sora in a test level |

## Stats

| Feature | Status | Pad | How tested | Known gaps |
|---|---|---|---|---|
| Level 99 loadout (stats, abilities, magic, items), any level with `--level N`, EXP awards and level ups, damage formula, difficulty, equipment, AP | works | F4 gives EXP (fixture) | U, A | meals, buffs, Hidden Potential are stubs |
| Test levels 1 / 20 / 99 at run time (base spells / -ra / -ga; level, stats, the level's abilities, HP / MP, Magic list and shortcut sets, gauges full, note on screen) | test-level choice (new 2026-10-12) | **LB + View (Back)**; key L; script word `test-level [N]` | U, A (Magic list of level 1 and 20 looked at) | sheet `sora-test-levels`: not game data; level 1 has no story bonus (HP 105, MP 110), level 20 keeps all of them (which a level-20 save holds is not determined by the level); not while a shotlock aim or a menu is open; switching while in Second Form drops the form's abilities until the next form |
| Spare equipment and item stock | test-level choice | View refills | A | sheet `sora-test-stock`: not game data |

## Sound

| Feature | Status | Pad | How tested | Known gaps |
|---|---|---|---|---|
| Footsteps by surface, jump / land, cloth, swings, hits, attack and damage voices (English / Japanese), spell and shotlock sounds, item, prize, menu and HUD sounds, low-HP alarm | partly | - | U (47), logs and numerical renders of scripted runs; **nobody has listened** | 1574 of the 2834 sound rows of the sheet play (55.5 %); the rest belong to clips and features that never run in the port; reverb send not rendered; low-pass mapping provisional; airstep, superslide, wall kick, doubleflight and guard-hit sounds were never run with the audio log |
| Master Volume | works (since 2026-10-12) | Config screen | U (config), wired in the app, not listened to | slider-to-gain curve is linear (the game's was not read) |
| Vibration | missing (switch only) | Config screen | none | the core now sends the game's force-feedback event for Stun Impact (table index 1), but the effect tables' contents were not read and nothing calls `xinput::rumble` |

## Effects

| Feature | Status | Pad | How tested | Known gaps |
|---|---|---|---|---|
| Particle effects of melee, magic, shotlock, items, hits, Keyblade appear / trail | partly | - | U (kh-vfx: 22 + 11 + 6 tests), A (third pass: Firaga, Blizzaga, Thundaga, Waterga, Aeroga, Curaga, level-1 and level-99 combo, King of Hearts; pictures looked at, near and from 12 - 28 m) | of 351 effects: 49 fully, 212 partly, 8 not at all, 82 dropped (other Keyblades). Drawn with the game's own pixel shaders (559 of 657 translated), HDR + bloom, particle lights (6), decals. The node chain, the spawn shapes (Range, Ring, Quadrangle), the noise and the local-particle rule now follow the game's code (third pass). Not drawn: distortion / refraction (53 effects), 9 effects have a material on the generic shader. Waterga (6 m) and Aeroga (12 m) are as large as their data says: cast at a target 4.6 m away they surround the camera. The Keyblade trail shows on the 26 clips that carry it (the level-1 combo); the level-99 combo's clips (Flash Step, Quick Slash, Radial Blaster, finisher 1305) carry none in the game's data. No picture was compared with the real game |
| Camera shake of effects (`SQEX_CameraShake`: grand magic, dive attack) | works (new 2026-10-12) | - | U (kh-camera shake.rs, kh-vfx request), A (`--effect-test vm_fir_han_025b`: the view moves) | an additive offset on the drawn view; the field-of-view part of a shake is not applied; no spell of the command menu plays one (the grand magic and dive effects do) |
| Screen effects of effects: scene darkening (`SQEX_ChangeSceneColor`: Thunder, grand spells) | works (new 2026-10-12, fourth pass) | - | U (kh-vfx scene light), A (Thunder: the scene goes dark while it strikes; calibrated on your capture) | the light value is squared to match the capture; which engine term it scales was not traced. `KH_VFX_SCENE_LIGHT=0` turns it off |
| Screen effects of effects: distortion / refraction of water and shockwaves | works, approximated (new 2026-10-12, fifth pass) | - | A (Wataga, Aeroga, Blizzaga, Firaga: the scene behind the surface is displaced) | each material's own distortion shader is translated (79 of 79) and drawn in the transmissive phase: it refracts the opaque scene (level, Sora, dummies), not the other effect layers, and overlapping surfaces do not add up as in the game. `KH_VFX_DISTORT=0` turns it off. Additive filter and radial blur: still missing |
| Effects that stay after their action (the glow on the Keyblade after a shotlock) | fixed 2026-10-12 | F1 text, line "effects": live systems / open persistent ones | U (every effect of the pack ends after loop end; kh-core vfx_live: 6 sequences), A (Ragnarok run: 0 effects left) | cause: particles without a lifetime were never removed when their emitter ended |
| Spell effects against your captures (Thunder, Water) | partly (fifth pass) | - | U (kh-vfx 16 fidelity tests), A (pictures in build/sora_tmp/impl_vfx5/shots: all 18 casts, Water at the six captured moments, Thunder at the two) | new in the fifth pass: the game's VERTEX shaders run for mesh particles (252 of 265 materials): Thundara / Thundaga bolts are jagged, the water burst is torn and wobbling instead of a clean glass sphere; ground decals draw when the camera is inside them (the water burst's mist swirl was never seen); refraction. Still different from the captures: the burst is darker blue and glassier, with far less white foam, than your capture; the cast band is thinner; Thundaga is blue-white by its data (your capture shows Thunder / Thundara: warm, as here) |
| Vertex stage of effect materials (world position offset, UV turn / reverse of meshes) | works for mesh particles (new 2026-10-12, fifth pass) | - | U (mesh UV mode), A (Thunder tiers, Water, Blizzard) | translated from the cooked vertex shaders (tools/sora_vfx_shaders.js); 13 mesh materials (sub-UV meshes ...) and every sprite-like material with an offset (20 materials: Aero rock dust, smoke sheets) keep the old path. `KH_VFX_VS=0` = old path |

## Sora's look (character materials)

| Feature | Status | Pad | How tested | Known gaps |
|---|---|---|---|---|
| Sora and the Kingdom Key shaded by the game's own material shaders (12 + 1 material slots: the compiled base-pass pixel shaders of the cooked materials translated to WGSL, their parameters, 49 textures) | partly (since 2026-10-12) | - | U (5: uniform expressions, light rig, collections, bone table, reflection cube), A (front, back, face, Keyblade: `build/sora_tmp/impl_charmat2/shots/s_fb.png`, `f_face_035.png`, `s_key.png`) | lit by the test level's own sun (direction, shadow, colour x illuminance through the camera's exposure) and its own ambient light since the third pass of 2026-10-12 (before: a key light and a constant ambient of his own, 2.9 x the level's ambient: he stayed bright in shadow). Measured lit / shadow ratio of his cloth 4.3 - 5.2 (before 1.8 - 2.2; the floor 13.5, an upright surface of the level about 6), skin 1.6 (before 1.16; skin follows the ambient term, stock subsurface behaviour): `build/sora_tmp/impl_charmat3/shots/cmp_open_shadow.png`, `cmp_facehalf.png`. He darkens with the scene during Thunder. Still not a game level's lights (the test level's sun is 22 x its ambient; no bounce light); effect point lights do not light him; eye highlight / eyelashes are shaded by their own base pass alone; nothing was compared with the running game except the menu screenshot |
| The game's character post-process material `KHS` (what composes a character's pixel from the G-buffer and the fork's light buffers: shadow colour, rim "boards", colour-table regrade) | works with stand-in inputs (new 2026-10-12, second pass) | - | U (preset -> collections), A (`s_k3.png`: presets twilight, olympus, blueprint, collection, off) | its pixel shader is the game's (translated, run per pixel on Sora's own surface values). Exact: G-buffer values, depth, colour table, both parameter collections and the worlds' controller values. With no direct light it only scales the indirect buffer (the shadow colour is white in every preset; the rim boards multiply the indirect term: skin x 1.7 for `twilight`), checked 2026-10-12. Stand-ins: the lights (the test level's), ambient-occlusion (none), the ambient-colour buffer (empty), eye adaptation (1), blend volumes (not applied). `--khs-preset NAME`: `twilight` (Twilight Town's controller, the default), `olympus`, `blueprint`, `collection`, `camp`, `off`. Worlds of the CG / Real / Pooh types run other post-process materials (`KHS_CG` ...), which are not translated |
| Tone: one filmic curve and one exposure for the frame | fixed 2026-10-12 (third pass: one exposure) | - | A (clothes are charcoal with fabric detail; `cmp_open_shadow.png`) | the near-black clothes were two tone curves on top of each other (the shader's and the camera's). Sora had the exposure 1 / 0.6 in front of the camera's curve and the level did not; now both have the camera's own (`KH_CHAR_EXPOSURE` overrides). The curve is Bevy's fitted ACES, not the engine's film curve with the world's colour table. The camp menu's picture keeps 1 / 0.6 (a stand-in for the capture's own film settings; measured against the capture of the real menu: skin 229,179,143 against 246,179,143) |
| Reflections on metal (Keyblade, buckles) | partly | - | U, A | a small generated environment cube (sky, horizon, floor of the test level; the menu: the six face colours of its ambient cube map) through the engine's roughness-to-mip rule; not the game's reflection captures, no screen-space reflections |
| Hair tips (dithered opacity) | works (second pass) | - | A (`f_face_035.png`) | the dither is evaluated at four offsets and written as multisample coverage (the game averages eight frames with temporal anti-aliasing); fine strand lines remain visible close up |
| `--plain-materials` (the look before: Bevy's standard material) | works | - | A | - |
| Run-time material parameters: the Keyblade's appear / disappear dither (`OpacityFade`), the mesh asset's bone-to-parameter table (head position and axes, ankles, actor scale, eyelid masks), setters for `InLight` / `Aura` | partly | - | U (bone table), A (Keyblade appear) | the table is the mesh asset's own (`m_BonePoseToMaterialParam`, 8 rows); its value remap is read from the data, the engine's evaluator was not traced. The effect modules that drive InLight / Aura do not call the setters yet (`char_material::drive_scalar` / `drive_vector` / `drive_clear` are there for them) |
| Second Form's costume: the wear-form texture slide (`FormChangeSlide`, sparkle transition into the form's textures and back) | works (new 2026-10-12) | Y at a full formchange gauge | A (`s_form.png`: `fill-style 100; tap y`) | 1 s on, 0.2 s off (from the combat spec's calls); the ramp is linear (the game's interpolator was not read); the camp menu's picture stays in the normal costume |
| Sora's model in the camp menu's character panel (the game's UI actor: camp idle `Idle_CM`, after 2 - 5 loops `IdleBreak_CM`; capture FOV 30 over the picture's height at 250 cm, two spot lights, ambient cube, the controller's camp values for `KHS`) | works (since 2026-10-12) | Menu opens the camp menu | U (1), A (`m_top_150.png` next to the capture of the real menu: same head size, cut at the hips, arms cut by the panel) | no local placement numbers any more (the picture lies on the movie's placeholder). No blinking: the UI actor layers a face clip (`Idle_CM_Face`) over the body, which the pose evaluator cannot do. The capture's film settings and colour table are not reproduced |
| The model in the Abilities and Stats screens (and the character select before them) | works (new 2026-10-12) | D-pad, A | A (`s_menu2.png`, opened with `tap start`, `tap down`, `tap a`) | Equipment has no picture clip of its own in the movie (its character select shows the top panel) |
| Menus: no world labels or debug text over an open menu; the settings guide in the top left corner shows the pad's Menu glyph when the pad was used last, the "Esc" picture after a key or mouse press | works (new 2026-10-12) | any pad input switches to the pad glyph | A (`m_top_150.png`) | which clip the game shows for a pad on PC is read from the clip names (`settingGuideIconSD`), not seen in the game; the glyph is dim. The button-guide texts of all screens already draw pad glyphs only (the pack holds no keyboard glyph set) |

## Keychain

| Feature | Status | Pad | How tested | Known gaps |
|---|---|---|---|---|
| Kingdom Key chain physics (the game's Bonamik solver on the weapon's own asset) | works | - | U (420 frames equal to the reference evaluator), A | frozen while the Keyblade is hidden (the game's behaviour there was not determined) |
| Wind and speed-rate notifies of Sora's clips reaching the chain | works (since 2026-10-12) | - | U, A (debug text of a played clip) | the clips that blow on the chain are the underwater selfie clips, which no state plays; the wind's per-tick scale curve is not in the pack |
| Weapon's own clips (Strike Raid throw, shotlock) on the chain | missing | - | none | the weapon holds its reference pose |

## Test level conveniences (not game behaviour)

| What | Input |
|---|---|
| Every dummy back to full, Sora's HP / MP / focus full, items refilled | **View (Back)** on the pad; F5 / F11 on the keyboard |
| Test level 1 -> 20 -> 99 (base spells / -ra / -ga) | **LB + View (Back)** on the pad; L on the keyboard |
| Pad help page / debug text / nothing | F1 |
| Events page, game HUD on / off, audio page | F2, F3, F12 |
| EXP from dummy 1, hurt 30, floating mode | F4, F10, Home |
| Dummy behaviour (guard, armour, attacks, bad status, risk-dodge warning) | script words only (`dummy-*`, listed in `crates/kh-app/src/script.rs`) |

## What only the running game can settle

The open probe items are listed in `docs/sora-runtime-probe.md` (8b) and `docs/sora-port-plan.md` (section 6); the ones
that change what you would feel on the pad: whether a glide ends on the ground, the first frames after a ground cast,
Sonic Rave's first clip, the jump out of a guarded hit, where the DODGE! / ATTACK! prompt sits on screen, and the
combat items C3-1 .. C3-9 and C4-1 .. C4-6 (among them the camera distance after an attack camera and Sonic Cleave's
descent).
