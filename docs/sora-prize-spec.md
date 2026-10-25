# Sora port: prizes (the orbs an enemy drops)

2026-10-12. What Kingdom Hearts III does when an enemy dies and bursts into HP / MP / munny / focus
orbs and item boxes, from the game's code and data; what the port builds from it
(`crates/kh-prize`, sheet `sheets/port/sora-prizes.json`), and the one rule the Elden Ring host uses.

Verification marks: **[D]** read from data (an asset, or static data of the exe); **[C]** decompiled
C only (none of the prize functions has the verdict "same": nothing here was checked at instruction
level); **[UE]** stock UE 4.17 behaviour assumed from the class identity (the Tres subclass was not
read for overrides); **[?]** not determined. Evidence scripts: `build/sora_tmp/prize/`
(`enemy_drops.js`, `prize_bps.js`, `rd.js`, `callers.js`).

## 1. The actors

| Class | What | Evidence |
|---|---|---|
| `ATresPrizeMan` (one per game state, `ATresGameState.m_PrizeMan`) | the manager: takes drop requests into a queue (max 256, 0x38 bytes each), spawns from it in its tick | [C] 0x141480420, 0x141480FB0, 0x141489120 |
| `ATresPrizeBase` | an orb: root `UTresRootComponent` (collision profile `TresPrize`, one sphere), `UTresPrizeMovementComponent` (a projectile movement component), a four-state machine Default / Appear / Get / Disappear, object type `SQEX_OBJ_TYPE_PRIZE` (never time-dilated) | [C] ctor 0x14146EFA0 |
| `ATresPrize` | the nine stat orbs (same code as the base) | [C] |
| `ATresPrizeBox` | the four item boxes (their own Appear begin and Get flight) | [C] 0x141473590, 0x1414738C0, 0x14147C0A0 |
| `ATresPrizeCrab`, `ATresPrizeMovableCrab` | the Caribbean's crab prizes (a mesh; not Sora's stat orbs: not ported) | [D] |

`PRIZE_KIND` (the kind an orb is spawned with; the class per kind is a field of the game mode's
`TresGameModeLoadAsset`, 0x140ECA640 [C]) and its blueprint under `/Game/Blueprints/Prize/` [D]:

| Kind | Blueprint | Effect (scale) | Sphere cm | Get / bounce sound | `ApplyPrize` 0x1405AB950 [C] |
|---|---|---|---|---|---|
| 0 HpPrizeS | PrizeHpS | vs_plz_hpb0 (0.5) | 13 | se00000_022 / 021 | HP +1 |
| 1 HpPrizeL | PrizeHpL | vs_plz_hpb0 (1) | 26 | 022 / 021 | HP +5 |
| 2 MpPrizeS | PrizeMpS | vs_plz_mpb0 (0.5) | 13 | 028 / 027 | MP +3; while the MP gauge recharges: recharge timer -2 s |
| 3 MpPrizeL | PrizeMpL | vs_plz_mpb0 (1) | 26 | 028 / 027 | MP +5; recharging: -6 s |
| 4 MunnyPrizeS | PrizeMunnyS | VS_PLZ_MNY0 (0.5) | 17 | 024 / 023 | munny +1 |
| 5 MunnyPrizeM | PrizeMunnyM | VS_PLZ_MNY0 (0.75) | 25.5 | 024 / 023 | munny +5 |
| 6 MunnyPrizeL | PrizeMunnyL | VS_PLZ_MNY0 (1) | 34 | 024 / 023 | munny +10 |
| 7 FocusPrizeS | PrizeFocusS | vs_plz_fop0 (0.5) | 13 | 041 / 040 | focus +3 |
| 8 FocusPrizeL | PrizeFocusL | vs_plz_fop0 (1) | 26 | 041 / 040 | focus +5 |
| 9 / 10 crab | PrizeCrab / PrizeMovableCrab | mesh amd_crb_vat_00 | 15 | se_g_ca01_prizecrab_* | a counter (not ported) |
| 11 / 12 light | PrizeLightS / L | ve_ex301_light (0.5 / 1) | 30 / 60 | 022 / 021 | a boss mechanic (not ported) |
| 13 HealPrizeBox | PrizeBoxHeal | vs_plz_prb00 | 26 | 026 / 025 | the item (`+0x18A8(item, 1)`) |
| 14 MatePrizeBox | PrizeBoxMate | vs_plz_prb02 | 26 | 026 / 025 | the material |
| 15 FstfPrizeBox | PrizeBoxFstf | vs_plz_prb03 | 26 | 026 / 025 | the ingredient |
| 16 RarePrizeBox | PrizeBoxRare | vs_plz_prb01 | 26 | 026 / 025 | the item |

Sora has focus prizes (kinds 7, 8): they are a normal source of focus. There is no "drive" prize.
A prize is applied to players only (team 0).

## 2. What an enemy drops: its table [D] + [C]

Every enemy's base parameter row (`FTresChrBaseParamDataTable`, the 218 tables under
`/Game/DataTable/Enemy/Base/`, 271 rows) carries:

- `m_DropPrize1` + `m_NumDropPrize1`, `m_DropPrize2` + `m_NumDropPrize2`: two prize kinds with counts.
  BOTH are always dropped.
- `m_DropItemID` / `2` / `3` + `m_DropItemRate` / `2` / `3`: three items with a rate in percent.

The ids are `ETresDropItemID`: 1 HpS, 2 HpL, 3 MpS, 4 MpL, 5 FpS, 6 FpL, 7 MunnyS, 8 MunnyM, 9 MunnyL,
10 .. 19 and 22 heal items (item ids 0x101 ..), 23 .. 82 materials, 83 .. 141 ingredients, 142 / 143
light prizes [C 0x141489120]. Examples [D]: the Shadow (e_ex001) HpS x2 + MunnyS x1, materials at
8 % and 4 %; e_ex002 HpL x2 + MunnyL x2. Of the 271 rows 116 drop prizes; first slot:
HP 67 rows, MP 36, focus 10, munny 3; second slot: munny 76, focus 24, HP 3, MP 2; counts 1 .. 4.

**The drop does not depend on Sora's state.** Nothing in the path reads his HP or MP: a hurt Sora
does not get more HP orbs. [C]

The path [C]: the dying pawn's status component calls 0x1405A6220 (226 callers: the enemies' death
handlers) with the pawn's location (or a socket of it) and the packed table; `ATresPrizeMan`
0x141480420 queues one request per slot:

- the two prize slots: `count`; with PRIZE_FEEVER (175) on any party member `count += (count + 1) / 2`
  (stat orbs only);
- the three item slots: each rolled by itself: `(int)(rand x 99 + 1) <= rate`; with LUCK_UP (39) the
  rate of a heal item / material / ingredient is `round(rate x (1 + 0.3 x summed LUCK_UP count))`.
  With bit 0 of the packed table's flag byte ONE roll over the three (at least 100 wide) picks at
  most one;
- the "magic draw" flag of the killing hit (a magic hit of a Sora with MAGIC_DRAW) rides on every
  request: such orbs are collected from any distance.

When an orb is spawned (0x141472DC0) the party's converters change its kind, in this order:
HP_CONVERTER 0 -> 1, MP_CONVERTER 2 -> 3, FP_CONVERTER 7 -> 8; then MILLIONAIRE (Rags to Riches)
0 / 2 / 7 -> 4, 1 / 3 / 4 / 8 -> 5, 5 / 6 -> 6; then MUNNY_CONVERTER 4 -> 5, 5 -> 6. So the orb on
screen IS the converted kind. (`kh_core::abilities::passive::convert_prize` has another order and
converts at pick-up; the port's orbs use `kh_prize::Converters` and `StatusRequest::PrizeCollected`,
which does not convert again.)

[?] How many queue entries the manager spawns per tick (its tick takes a budget argument that was
not traced): the port spawns a drop at once.

## 3. The throw [D] + [C]

Each orb of a table drop gets a velocity from the request's velocity block; the default block is
static data at 0x1486DD5C0 [D]: **speed 900 cm/s, no speed spread, pitch 50 .. 65 degrees above the
horizon, yaw 0 .. 360 degrees**, each uniform (pitch drawn first, then yaw), air resistance 0.955.
A gimmick may pass its own block (`TresGimmickPrizeVelocityData`). An item box (0x141473110):
`X = Y = (rand - rand) x rand x 300`, `Z = 600 + rand x 600` cm/s, life 180 s.

The random numbers are the engine's xorshift128 (state at 0x142CFF460), `(w >> 8) x 2^-24`.

## 4. Flight, bounce, rest [C] + [UE]

- Gravity: `ProjectileGravityScale` 2.3 x the world's -980 = **-2254 cm/s2** [C ctor; UE].
- Horizontal damping: velocity X and Y x `pow(0.955, dt x 60)` every tick of the Appear state
  (halved about every 15 frames; an orb thrown at 900 cm/s lands roughly 1.5 to 2 m from the body) [C 0x14147B760].
- Bounce: `bShouldBounce`, **Bounciness 0.7, Friction 0** (set at init 0x14147B200): the velocity along
  the hit normal is reversed x 0.7, the rest kept [UE `ComputeBounceResult`]; the sphere is swept, so
  it does not pass through the level [UE].
- The bounce handler 0x14147EDF0: plays `m_SoundBound` at the orb for the **first 4 bounces**; from
  the 4th bounce on, when the orb is at least **1.0 s** old and the surface normal's Z is above 0.2
  (a floor), gravity scale and velocity become 0: it rests there.
- The engine also stops it when a bounce leaves less than 5 cm/s (`BounceVelocityStopSimulatingThreshold`).
- Water (the world's water height above the orb): once, gravity scale x 0.1, falling speed x 0.1
  (rising speed 0), bounciness x 0.7.

## 5. Life [C]

- **Appear**: timer 0.8333 s (50 frames; 0.5 s while the player flies the small airplane, state
  119). Until it runs out the orb cannot be collected. Then the timer becomes the living time,
  **15 s** (every table request carries 15.0; negative = for ever) and the orb is collectible.
- When the living time runs out the state becomes **Disappear**, whose tick removes the actor at
  once (its particle components are deactivated, so what is in the air runs out). **There is no
  blink state in the code**; any fading is the effect's own.
- An item box is collectible from its first frame and lives 180 s.

## 6. Attraction and pick-up [C], radii [D]

There is no separate pick-up radius: an orb that finds a getter flies to him and is applied on arrival.

- **FindGetter** 0x1414794C0, every tick once collectible: among the player and the party (7 slots)
  the nearest pawn that can take the prize (player: HP above 0 and not in the die state,
  0x14142C600) and is inside the radius. Radius by the pawn's PRIZE_DRAW (Treasure Magnet) count,
  table 0x1469AB6D8 (squared) [D]: **300 / 600 / 900 cm for 0 / 1 / 2 and more copies**. No limit with
  MASTER_DRAW (Master Treasure Magnet), or for a magic-draw drop. Special player states: 10000 cm in
  state 138 (rail slide MI), 5000 cm in state 119. (The Kingdom Key carries one Treasure Magnet: a
  level-99 Sora collects from 6 m.)
- **Get** 0x1414736D0 / 0x14147BC90: gravity and velocity off; timer **0.6667 s** (40 frames);
  `dir` = unit vector from the getter's point to the orb, `len` = that distance. Each tick:
  `s = timer x 1.5`; `dir` is turned about the vertical by `min(100 / (len x s) x 722.29, 1020)`
  degrees per second; position = getter point + `dir x len x s`; actor scale = `s`. So the orb
  spirals in on a radius that shrinks linearly to 0 while it shrinks to nothing, following the
  getter as he moves. At timer <= 0.2 s (12 frames before the end) `m_SoundGet` plays once, at the
  orb. At 0 the prize is applied (0x141479360 -> `ApplyPrize`) and the actor is removed.
  [?] the point of the getter it flies to (his virtual +0x6E0): the port uses his capsule centre.
- An item box (0x1414738C0): flight time `clamp(distance / 1200, 0.5, 1.0)` s along an arc with a
  sine ease (arc constant 600): section 11.5 has the arc as it is ported.

## 7. What a prize gives, effects, sounds, HUD

- Values: section 1's last column (stats spec 6.3). HP / MP / focus go through `Recover(.., kind 2)`:
  the pawn then shows the recover effect of cure group `TRES_CURE_GRP_PRIZE` [C]; **which effect that
  group is was not read [?]** (the port shows none on Sora).
- Effects: the orb IS its blueprint's particle system (section 1), attached to the root, for as long
  as the actor lives; no separate spawn or get effect exists in the prize code.
- Sounds: `m_SoundBound` (first four bounces) and `m_SoundGet` (0.2 s before arrival), both 3D at
  the orb. Sheet `sora-sounds` rows `code.prize`; seam names "<class>.bound" / "<class>.get".
- HUD: munny raises the "OBTAINED MUNNY!" counter (0x141587530; the UI spec's popup, which adds up
  while it is open). [?] any flash of the HP / MP gauge on an orb: none was found in the prize code;
  the gauges simply show the new values.

## 8. The port

- `sheets/port/sora-prizes.json` (written by `tools/sora_prize_sheet.js`; generated into
  `build/gen/rust/sora_prizes.rs`): every number above, plus the test level's tables (`test.*`) and
  the Elden Ring rule (`er.*`).
- `crates/kh-prize` (engine-free): `Prizes` (drop table / orbs / box, the throw, one step against the
  `World` trait with swept bounces, life, FindGetter, the Get spiral), `PrizeEvent` (Spawned, Bounced,
  GetStarted, GetSound, Collected, Expired), `Converters`, the game's xorshift128 seeded by the host;
  `host::apply_to_sora` (sounds through the core's audio seam, the prize through
  `StatusRequest::PrizeCollected`). 9 tests.
- `crates/kh-host/src/prize.rs`: `PrizeHost` = the orbs of a host with Sora as the getter and one
  free effect per orb (`FreeEffects`: `free_spawn / free_move / free_stop` of both effect hosts).
- Test app (`crates/kh-app/src/prizes.rs`): a dummy at 0 hit points bursts into its table (rows
  `test.dummy1..5`: pairs real KH3 enemies drop, assigned to the dummies) and is back after
  `test.respawn_time`; script word `dummy-hp N [HP]`; `--no-prizes`.
- NOT PORTED: the manager's per-tick spawn budget, crab and light prizes (the box's arc IS ported since 2026-10-13: section 11), the pawn's
  recover effect for a prize, party members as getters, the two special player states' radii.

## 9. The Elden Ring host's rule (a host choice, not KH3)

An Elden Ring enemy has no KH3 table. `crates/kh-er/src/prizemap.rs` builds one, by rows `er.*`
(all marked MARKED SCALING in the sheet):

1. tier by the enemy's maximum HP: below 300 weak, below 1500 normal, else strong;
2. first slot: HP, MP or focus orbs, the family drawn per kill with weights 67 : 36 : 10 (the share of
   KH3's own enemy rows, section 2); 2 / 3 / 2 orbs; small in the first two tiers, large in the third;
3. second slot: munny, 1 / 2 / 2 orbs (small / medium / large by tier) that TOGETHER are worth the
   enemy's rune value x `er.munny_per_rune` (1), split evenly; without a rune value
   `ceil(max HP x er.munny_per_max_hp)` (0.4, a guess). An orb then carries its amount instead of
   KH3's 1 / 5 / 10, because munny has to buy what runes buy.

Changed 2026-10-13 (IMPL_ER19, from the user's first long session): the munny of a kill is WHAT THE GAME ITSELF AWARDS
for it (read from its rune counter); the row's rune value is only the fallback when the game adds nothing within 5 s.
A row WITHOUT a rune value (bosses: the game awards their runes by an event of its own 6 - 8 s after the blow; and
enemies it pays nothing for) has NO fallback: the kill waits 20 s for the award and gives no munny without one (the
max-HP guess of point 3 had paid 1354 munny for nine enemies the game paid nothing for, and split a boss's award into
orbs of the guess and a remainder without orbs). The guess remains only behind `prize.runes = 0`. Host rows
`prize.*` of sheets/port/sora-er-host.json: spawn height, brightness and size factors of the orbs' effects, the two
waits, whether a vanished munny orb still pays (KH3: no).

KH3 itself has no rule that favours HP orbs when Sora is hurt or MP orbs when he is low, so the host
has none either.

## 10. What only the running game can settle

1. The point of the getter the orb flies to (virtual +0x6E0), and the per-pawn spawn offset
   (status component virtual +0x1D8).
2. The manager's per-tick spawn budget (do the orbs of a kill appear in one frame?).
3. The recover effect of cure group PRIZE on Sora; any gauge flash.
4. Whether `UTresProjectileMovementComponent` changes the stock bounce / sub-step rules.
5. A capture of a kill to compare the burst's width and bounce heights with the port's.

Changed 2026-10-13 (IMPL_ER20): the orbs' picture. `SQEX_DepthOffset` is cm TOWARD the camera (it had been read the other
way round: the HP orb's outline disc stood in front of its ball, its glint behind); the HP orb's ball (a sphere sized
3.6 x 1 x 3.6, camera-facing) shows the camera its round side (the "lens rule" of kh-vfx: a judgement, the game's transform
builder was not traced). By their data: HP = a ball textured with the prism texture jtx_pzm_00c (pastel rainbow, yellow
underside) in a pink outline with a glint; MP = a blue ball (0, 0, 1.2) with a lighter rim, a pink inner ring, a light blue
glow behind and a glint; munny = one gem mesh coloured by its vertex colours, 32 / 48 / 64 cm; no prize effect has a trail.
Elden Ring host rows: `prize.glow` 3.27 (the emissive of these materials is exposure-independent in KH3), `prize.hp.small` 4 /
`prize.hp.large` 20 (the user's choice; KH3's +1 / +5 everywhere else). Orbs still out at Sora's death, at a cutscene's
start or when the host stops are removed and their munny given at once (KH3 loses only a prize whose living time ran out);
at a load nothing is given.

## 11. Item drops: the prize box (added 2026-10-13, IMPL_ER25)

Marks as above; **[I]** = an immediate or instruction read in the disassembly (build/sora_tmp/loot/dis_exe.js); **[A]** = read in
the HUD movie's script. Evidence scripts: build/sora_tmp/loot/ (items.js, fx.js, dis_exe.js). NO CAPTURE OF THE RUNNING GAME
exists for any of this (build/sora_tmp/ref/ has none of an item drop): section 11.8 lists the three that would settle the doubts.

### 11.1 What drops: one box per item, its kind by the item's category
An item prize is an `ATresPrizeBox` (`PRIZE_KIND` 13 .. 16). The spawn function 0x141473110 takes the ITEM ID and chooses the
class by the id's high byte (= `ETresItemCategory`) [C; the compares `cmp cl, 2 / 7 / 8` and the four kinds 0x0D / 0x0E / 0x0F /
0x10 I]: 1 battle item and 2 camp item -> **PrizeBoxHeal**; 7 material -> **PrizeBoxMate**; 8 foodstuff -> **PrizeBoxFstf**;
everything else (weapon, armour, accessory, key item ...) -> **PrizeBoxRare**. So the "rare" box is the box of EQUIPMENT; an enemy
row's three item slots (`ETresDropItemID`) can only name heal items, materials and foodstuff, so enemies never drop it: other
callers of the function (gimmicks) do. There is no further rarity in the drop: no colour, no glow, no size by the item's worth.
The function makes nothing when a count read from the getter's pawn is above 127 [C; what it counts was not read].

### 11.2 What a box looks like [D]
Each blueprint carries ONE particle system at the root (relative location z -3), a 26 cm sphere, bounce sound `se00000_025`,
get sound `se00000_026`. The four systems have the same build: TWO mesh emitters ("OneMesh_lp", one looping mesh particle
each), `SQEXBaseScale` 2, no sprite, no glow, no trail, no light:

| box | system | meshes | material (base colour texture) | by its textures |
|---|---|---|---|---|
| Heal | vs_plz_prb00 | o_ex154_0 + o_ex155_0 | o_ex154_mat0_kari (o_ex154_0d00) | silver-grey egg with dark purple panels, arrow emblems, pale cyan fittings |
| Rare | vs_plz_prb01 | o_ex156_0 + o_ex157_0 | o_ex156_mat0_kari (o_ex156_0d00) | tan / gold egg with blue panels, cream wings and horns |
| Material | vs_plz_prb02 | o_ex158_0 + o_ex159_0 | o_ex156_mat0_kari of o_ex158 (o_ex158_0d00) | orange egg with red flames and a teal crown |
| Foodstuff | vs_plz_prb03 | o_ex160_0 + o_ex161_0 | o_ex160_mat0_kari (o_ex160_0d00) | pale blue-grey with maroon bands and a knife-and-fork emblem |

Per emitter: size 1 x `SizeScale` over the emitter's time (0 -> 1 in 0.1 s with an overshooting tangent, held, 1 -> 0 between
0.15 and 0.25 s: THE POP-IN and, when the system is deactivated, THE SHRINK-OUT; there is no other spawn or vanish effect);
colour (1, 1, 1, 1); the first mesh gets a random yaw (`LocalAddRotation` uniform +-180 degrees, direct), the second a random
start yaw and `LocalAddRotation` (0, 0, 100) [the evaluator's reading of the fork's flags governs whether that spins];
`MeshPivotOffset` z 0.5 / 4.4. The materials are CHARACTER materials (root `Metal_Nuno_Leather_EnemyColorSkin_Basemat`: masked,
two-sided, subsurface, base colour + normal + mask + AO textures, 120 constant rows), not effect materials.

### 11.3 The roll [C] (section 2 in detail)
Each of an enemy row's three item slots is rolled by itself: `(int)(rand x 99 + 1) <= rate`, so a rate r drops with r / 99 (99
and above: always), and a kill can drop none, one, two or three items. LUCK_UP ("Lucky Strike" in the menus; ability 39; the
copies of the whole party summed) turns the rate into `round(rate x (1 + 0.3 n))` for heal items, materials and foodstuff.
With bit 0 of the packed table's flags one roll over the three (at least 100 wide) picks at most one. No difficulty factor is
in the path. Item grades in the data: `TresItemData.m_ItemRank` is set for MATERIALS only: RANK03 = Shard, RANK02 = Stone,
RANK01 = Gem, RANK00 = Crystal (13 families x 4) and the eight single materials (Fluorite, Damascus, Adamantite, Electrum,
Orichalcum, Orichalcum+, Evanescent Crystal, Illusory Crystal: RANK00). Battle items, armour and accessories have no rank (the
"+" pieces are separate rows). Nothing in the drop or the popup reads the rank.

### 11.4 The throw, the bounce, the life [C; D]
Velocity X and Y each `(rand - rand) x rand x 300`, Z `600 + rand x 600` cm/s; the init call passes living time 180 s and
DAMPING 0.98 (an orb: 0.955), so a box keeps its sideways speed [C: both are loaded as constants, not checked in the
disassembly]. Gravity, bounce (0.7, friction 0), the bounce sound for the first four bounces, rest after four bounces and 1 s
on a floor: the base class's (section 4). Appear begin 0x141473590: the box can be collected FROM ITS FIRST FRAME and its
timer is its living time at once; after 180 s it is removed (no blink state; the system's own shrink-out). No idle effect, no
loop sound: the system is only the two meshes.

### 11.5 Attraction and the arc [C; the arc constants I]
FindGetter is the base class's: Treasure Magnets pull item boxes exactly as orbs (300 / 600 / 900 cm, unlimited with Master
Treasure Magnet or a magic-draw drop). A box thrown inside the radius flies to the getter at once, from the air.
Get begin 0x1414738C0: time T = clamp(distance / 1200, 0.5, 1.0) s; direction = normalize(getter - box + (0, 0, **600**)).
Get tick 0x14147C0A0, with p = sin((1 - timer / T) x 90 degrees) and r = 1 - p: the direction turns toward the getter by the
fraction clamp((**1** + p x **15**) x dt, 0, 1) of the angle between them (slerp; flag +0x4BC = 1), then the box moves along it
by dt x (distance left) / r: it sets off upward and slowly, curves in and closes ever faster. Its yaw is p x 30 degrees; when
r < 0.1 its scale is r x 10. The get sound plays once at timer <= 0.2 s; at 0 the prize is applied. (Constructor 0x14146F420
writes 600.0, 1.0, 15.0 and the flag at +0x4B0 .. +0x4BC [I]; the tick reads +0x4B8 and +0x4B4 in that formula [I].)
Measured in the port: a box lying 2.5 to 2.9 m from him leaves the straight line upward by about 40 cm and arrives in 0.5 s.

### 11.6 Pick-up [C; A]
`ApplyPrize` kinds 13 .. 16: the pawn's virtual +0x18A8 (item id, 1): one of the item into the stock (`m_MaxNum` 99 for every
row a Sora can hold [D]). No effect on Sora and none on the box is started by the prize code. HUD: 0x141587510 (cockpit, item
id, stock full) -> `HudLeftGetItem.setParam(icon, name, stockFull)` of the movie HudLeft (clip `getItemS`) [A]: `m_icon.load(icon)`
= the item's picture `TresItemData.m_IconAsset` (/Game/UI/Icon/Item/...) drawn by `tres.controls.Icon` scaled to the clip's 80 x
80; the name; with stockFull the clip `itemFullMes` goes to "on" and `getCol` / `getItemLineCol` to "col1". There is NO count in
the popup ("x N" does not exist), NO rare variant, one popup at a time (further items queue and each opens as the last closes);
the open / hold / close timing is the movie's own. [?] what +0x18A8 does at a full stock; the popup's sound (the port plays the
static SE it already used for the item popup).

### 11.7 The port
- kh-prize: `BoxKind::of_item`, the box damping, the arc as above (`box.*` rows), `Prizes::drop_item_box` / `drop_items`
  (KH3's roll on item ids), `lucky_rate` / `item_chance`, `Prize::yaw`, `Prizes::set_amount` (a host's count on a box);
  tests/boxes.rs. NOT MODELLED: the "above 127" refusal.
- sheets/port/sora-item-table.json (tools/sora_item_table.js): the 240 items a Sora can hold. kh-host `items.rs`: the table
  and `give(item id, count)`, the one door; `Hud::take_given_items` stocks (battle items in the pouch's stock, everything else
  in the camp menu's stock, which the save keeps), shows the popup with the item's picture, stock-full variant at the
  maximum. kh-ui: `tres.controls.Icon` (`controls::icon_init` / `icon_load`), `HudLeft::show_item_icon`. UI pack 1.5.0: 180
  item pictures + data/item_icons.json.
- Test app: rows `test.dummyN.items` (item id, percent); the Items screen lists materials and camp items.
- SEEN IN THE TEST APP (build/sora_tmp/loot/shots): e_sheet.png (burst, landing, lying, pick-up), e_crops.png (the three box
  kinds close up), p1_sheet.png (pick-up), p1_popup.png (the popup with pictures, queueing), c_sheet.png (the stock).
- NOT 1:1, SAID PLAINLY: the boxes' material is drawn UNLIT with its base colour texture (both hosts' "generic variant"): the
  game's own shader for it (a character material: lighting, normal map, metal / cloth / leather masks, subsurface) is not
  ported; the effect shader translator cannot take it (three stops: TEXCOORD9, more than 64 constant rows, a screen-space
  derivative), the character renderer (kh-look) would have to draw effect meshes. The camp items and materials are kept
  and shown but nothing uses them. The item help pane's picture stays empty. The foodstuff box was not looked at.

### 11.8 What only the running game can settle
Three captures: (1) an item box lying on the ground next to Sora in daylight (shading of the cloth and metal, size against
him, whether the upper part turns); (2) the moment of a pick-up, a few frames (the arc's height, the shrink, any flash on Sora);
(3) the "obtained" popup with a second item arriving while it is up (queue or stack, timing, the stock-full variant's text).
