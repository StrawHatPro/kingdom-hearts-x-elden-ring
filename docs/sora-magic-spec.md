# Sora: magic, shotlock (focus), airstep and items — specification

Research only (no Rust). Source: `base/KH3.exe` (decompiled C under `re/decomp`, constants and control flow
re-read in the disassembly where marked), the cooked packages under
`assets_converted/packages/KINGDOM HEARTS III/Content`, the game tables under `sheets/entities`, the converted
animations with their notify sidecars. Written 2026-10-11. Nothing here was run in the game; what only the
running game can settle is listed per part ("Open questions", "Runtime probe requests").

The document is assembled from research strands: a first pass (parts A–E) and a second pass (parts F–J) that
brings the items the first pass only mapped to implementation depth and corrects it in places — **where a
second-pass part differs from parts A–E, the second-pass part governs** (the corrections are listed in section
1.1 and at the head of parts D and E). Part K is the implementation order and test list. Each part has its own
section numbers;
this front section gives the model in one place, the cross-checks between the strands, the consolidated host
interface, and the data inventory. Parameters are the sheets `sheets/port/sora-magic-*.json`; the asset list
for the pack is `build/sora_tmp/magic/asset_requests.json`.

Tags on statements (a tag in a heading covers the section):

- **[I]** verified at instruction level (address given)
- **[C]** decompiled C only
- **[U]** stock UE 4.17 by function identity
- **[A]** read from an asset, data table, ini file or reflection data
- **[D]** derived by arithmetic from [I] / [A] values
- **[?]** not determined statically

Units: cm, s, degrees; X forward, Y right, Z up. "Slot n" = vtable index (offset / 8). Objects: pawn
`TresPlayerPawnSora` (vtable 0x14696B930), its request interface at pawn+0x450 (vtable 0x14685EB18, `CanX` /
`TryX` pairs), its animation interface at pawn+0x440 (0x14685E3B0) which forwards notifies to the current
state's sub-object at state+0x40, controller `TresPlayerControllerSora` (vtable 0x146969B98). State objects:
Reset / Begin / Tick / End = vtable slots 95–98; `m_nSubStep` at state+0x78; flags at state+0x84 (bit 0
`m_bEnableChange`).

How far each part goes: A (cast) and C (MP / damage) are read at instruction level for the gates, the state
tick, the cost and the damage formula. B (spell actors) is decompiled C with instruction-level checks of the
level-hit handler, the homing rotation and the constructor constants, plus complete blueprint data; I closes
its open items (Thunder, Aero, Water barrier, Blizzard rail, Cure timing, row table, grand-magic production).
D and E are the first-pass reading of shotlock / focus / airstep / items; F (aim mode), G (shotlock attacks,
focus syphon), H (airstep) and J (menu, shortcuts, situation slots, pouch) replace or extend them with
constants and comparisons at instruction level and control flow mostly from decompiled C (each part says which).

---

## 0. The model in one page

### 0.1 Commands
Everything the player triggers from the command menu or a shortcut is an `ETresCommandKind` (row id of
`Load/Common/TresCmdData`, 254 rows: Type, Cate, Attr, Sp, Mp, Fp, Param0–3, text key). The controller's
`ExecCommand` (slot 678, 0x14141B470) switches on it and calls the pawn's request interface: `TryMagic`
(slot 53) for the magic rows 29–56 and the grand-magic situation rows 110–125, `TryUseItem` (slot 228) for
rows of category ITEM (58–68). There is no input buffer in the controller: a refused command is dropped; the
only queueing is the pawn's state queue. (A 2.1, E I1.)

### 0.2 Magic
- One state, `TresPlayerState_FireMagic` (id 18), casts every spell; the spell is the command id. The clip is
  chosen from 16 `Magic*` names by (cast type / shot type of the magic-set unit, ground / air, moving,
  finisher). The notify `TresAnimNotify_Shot` releases the spell: projectile class from
  `p_ex001_MagicSet` (the Kingdom Key replaces nothing), spawned at the Keyblade tip (socket `ATK_01`), aimed
  with the actor yaw and a pitch clamped to ±45°; **MP is deducted at the shot** (A 1.4).
- Chaining shares the attack combo counter: `MaxCombo` 4 (+ Combo Plus), the last `NumFinish` casts are
  finishers with the `_FIN` attack rows, other clips (`MagicShoot` / `MagicChant`), invincibility until the
  shot; a queued Fire-family follow-up starts immediately after the shot, other follow-ups at
  `TresEnableChange` (A 5).
- In the air the cast hovers: gravity disabled and scale 0.1 until the shot, slow fall until
  `TresEnableChange` (A 6). A moving ground cast keeps running at max walk speed in the direction of the
  request.
- The spell actors are all `TresProjectileBase` actors configured by blueprint data (B): Fire and Blizzard
  homing bolts (2000 cm/s default, turn 180–360°/s) that respawn an explosion / burn actor on a character hit,
  Blizzard additionally lays a grindable ice rail (rail-slide actors every 300 cm on the floor, 3 s each);
  Thunder one widening static cylinder per cast at the target, hitting every enemy inside once; Water a
  barrier sphere attached to the caster (it blocks nothing: the caster is invincible from the shot to the end
  of the state) that then turns into a homing bolt and a splash; Aero a static cylinder that hits each enemy
  once with `SWIRL_UP` for the first 3 / 8 / 14 enemies and `BLOW` after (the victim-side swirl is given in
  part I); Cure an actor that heals the target and same-team pawns in range. Status
  effects (FREEZE, DISCHARGE) come only from the attack rows (chance and duration). Grand magic = the `S_*`
  command ids with the `*_S` rows and the "-gan" classes, no MP, flagged through the same path; it is offered
  by a roll when the formchange gauge (fed by each hit's `m_FormPoint`) reaches 100, with a chance per element
  from the points that element contributed (part I).
- MP (C): current MP int at pawn+0x604; max = 100 + victory bonuses: 135 / 145 / 155 after the base game by opening choice (Vitality / Balance / Wisdom), 159 / 169 / 179 with the Re Mind rows (Critical 72 / 77 / 82) — **no level term**. Cost = `Mp` column (Fire 10 / 14 / 20,
  Blizzard 12 / 17 / 24, Thunder 14 / 20 / 28, Water 11 / 15 / 22, Aero 13 / 18 / 26, Cure 255 = all), reduced
  by MP_SAVE and MAGIC_COMBO_SAVE. A cast needs only "not in MP charge" (MP_SAFETY adds "enough MP"); a cost
  above the remaining MP empties the gauge and starts the **MP charge**: 50 s (+20 on Critical), ticking at
  `1 + 0.1·MP Haste + 0.2·MP Hastera + 0.3·MP Hastega` per second, shortened by MP prizes and Ethers, after
  which MP is full.
- Damage (C 4.2, read whole at instruction level): `base = max(MagicPower − Defense, 1)`, clamped by the
  victim's damage min / max rules, `dmg = ceil(base · power)`, then attribute resistance, rapid-fire
  resistance, difficulty, and the attacker's abilities (Magic Combo Up +10 % per combo step and copy, element
  Boost +20 % per copy, Full MP Blast +50 %); no random spread and no critical hits. Magic power at level 99 is
  64 before equipment. Cure heals `m_AppendCure (80 / 120 / 160) + max(1, MagicPower)` on the target and
  `m_BonusAppendCure (40 / 60 / 80) + MagicPower` on allies in range (B 3.6, C 5).

### 0.3 Shotlock and focus (part D)
- Focus: int at pawn+0x614, max 100 for Sora. It is **spent by aiming**: the aim time available is
  `focus · 0.05 s`, it runs down in real time, and at release the focus is set to what is left (20 points
  per second of aiming). Refill: focus prizes (+3 / +5, C 3), Refocusers (40 % / 100 %), Elixirs, the focus-syphon shotlock; not by
  hits (`MyFocusPointAddHit` = 0 for Sora). A cancelled aim costs nothing.
- Aim mode (controller slot 800, part F): hold `Gamepad_RightShoulder` 0.267 s (a shorter press is the
  lock-on tap) → per-object-type slow motion 0.4 (Sora, enemies, map; not projectiles, camera, UI), a
  `TresCameraShootFlow` camera 150 cm behind and 100 cm right of Sora; no state and no aim animation: the
  input dispatch is simply skipped; every lock-on target with shotlock permission within 5000 cm is a candidate;
  a candidate inside the 150 px circle (at 1280×720) around the screen centre with line of sight is locked
  once per `2.24 / Param0` s of real time. The attack button releases, the airstep button dashes.
- Kingdom Key: kind 1 (`KC_SHOT`, text key `sf_magic_0_1`), maximum 24 locks; at 24 locks the release becomes
  kind 2 (`KC_RAGNAROK`, `sf_ragnarok_0`).
- Release (part G): `TresPlayerState_ShootFlow` (id 41), invincible throughout: start clip `M_1473` (2.5 s)
  with a 500 cm lift, then one homing projectile per lock, seven per tick, on a rotating cone around the blade
  tip (distance to 200 cm in 0.5 s) and launched (`TresProjKC_Shot`: sphere r 30, constant 1500 cm/s, homing
  180 → 360°/s, life 5 s, row `SF_KCSHOT` power 0.8), fire clip `M_1474`, end clip `ShootFlowEnd`. Ragnarok:
  24 `TresProj_Ragnarok` (helix, 1000 + 500·t cm/s, homing 720°/s, power 0.9) in one volley. Focus syphon:
  no damage, focus += trunc(rate · locks).
- Airstep (part H): `TresPlayerState_AthleticFlow` (id 42), unlocked by a player-state bit, no focus cost,
  invincible, launch speed `max(2·dist / 1.0 s, min(60·dist, 6000 cm/s))`, then a straight chase to enemies or
  level points (chains of points, wall kick, pole, ledge), camera `ATresCameraAthleticFlow`.

### 0.4 Items (part E)
`TresPlayerState_UseItem` (id 111): clip `ItemUse` / `ItemAirUse` (0.8333 s); the `TresTiming` notify at 0.5 s
consumes the pouch slot and queues the effect on the target with a 0.5 s delay; leaving is allowed from the
same moment. Pouch: 8 entries of (item id, flags), of which 3 + one per ITEM_SLOT_UP1 bonus are usable (6 after
the base game, 8 with Re Mind); used auto-reload slots refill from stock when the game leaves a pause (part J). Amounts are constants of one function: Potion 80 HP,
Hi-/Mega-Potion 160 HP, Ether 100 MP (or half the MP charge), Hi-/Mega-Ether full MP (or the whole charge),
Elixir everything, Refocuser 40 % focus, Hi-Refocuser 100 %, Panacea status cure + 60 s immunity; each copy of
ITEM_UP adds 50 %.

---

## 1. Cross-checks between the strands

| Fact | Found by | Agreement |
|---|---|---|
| `ExecCommand` 0x14141B470, `TryMagic` slot 53 0x14143AEB0, magic command sets, Cure target from the menu | cast, items | same reading of the switch |
| Shot notify handler = state+0x40 slot 69 0x14137BB30 | cast; vtable comparison of the state sub-objects (items strand) | same slot and function |
| MP deducted at the shot through 0x14140D560 → pawn vt+0x1860 | cast, mp | same call chain |
| Gate 0x14142F330 (`Mp == 0xFF`, MP_SAFETY, MP charge) | cast, mp; found independently by a scan for `[base+idx*8+0x1F]` | same |
| Item function 0x1405BE820 (Ether 100 / half charge, ITEM_UP ×(1 + 0.5 n)) | items, mp | same amounts |
| MP-charge flag pawn+0x7C0 bit 1, timer +0x7C4, `MyMpChargeTime` | mp, items | same; the same byte's bit 0 is the focus-empty flag (shotlock) |
| Prizes: MP +3 / +5 or charge −2 / −6 s; focus +3 / +5 | mp (0x1405AB950) | fills the gap of part D: focus prizes are the normal focus source |
| Game option set B (0x140EAB160) | mp; shotlock (bit 6 → +7 focus per second) | same accessor; the option names are open in both |
| Projectile setters called at the shot (vt+0x1100, 0x1120, 0x1130, 0x1140, 0x1160, 0x1210, 0x1220, 0x11F8, 0x1270) | cast (caller), proj (slots 548, 578, 580 …), mp (combo values into the hit record), shotlock (same setters on the shotlock projectiles) | consistent roles: command, finish / grand, combo index, combo counters, full-MP flag, homing target, fire position, start |
| Cure heal uses 0x1405836E0 | proj (caller), mp (identifies it as MagicPower) | settled |
| Attack rows are looked up in the owner's `p_ex001_AttackData` | proj, mp, shotlock (`SF_KCSHOT`) | same |

### 1.1 Corrections made by the second pass (the later parts govern)

| First-pass statement | Correction | Where |
|---|---|---|
| C: max MP 165 / 189 with all bonuses | the opening-choice rows are alternatives: 135 / 145 / 155 after the base game, 159 / 169 / 179 with Re Mind (found by the stats agent; C is corrected in place) | C 1 |
| A 1.4 step 8: sixth spawner argument is a yaw | it is the caster's team id | I |
| B: Aero multi-hit, "swirl limit 3 / 8 / 14 hits" | no re-hit: each enemy is hit once; the limit is the number of enemies that receive SWIRL_UP, later ones get BLOW | I |
| B / first summary: Thunder strike count open | one column per cast; every enemy overlapping it is hit once | I |
| D S1: int16 focus bonus, FOCUS_BOOST | no writer / no reader: max focus is 100, FOCUS_BOOST has no effect in code | G |
| D S2: `0x4D9E` is a time-dilation id; `FUN_14140E500` is the release handshake; entry +0x14 is the move-flow type | it is an object-type mask; the handshake is controller slot 809; +4 is the move-flow type and +0x14 the shot-lock range | F |
| D S2.4: who plays `ShootFlowStart` / `ShootFlowIdle` | nobody during aiming | F |
| D S3: horizontal ring, `KC_SHOT` accelerates to 4500, Ragnarok is a `TresProjectileBase` | cone around the blade-tip axis; constant 1500; native class `TresProj_Ragnarok` | G |
| D S4: range 3750; speed formula for the whole flight; `AthleticSlideEnd` as an attack | 15000 unless the component has its own; launch only; arrival hop without damage | H |
| E: pending-command writer, pouch size, refill, stock counter, restriction bit 3 = Link | see part J (bit 3 = situation commands, bit 4 = Link) | J |
| D / E: grand magic production unknown | formchange gauge roll | I, J |

Differences to keep in mind: part B documents only the base magic set (`p_ex001_MagicSet`), which is the one
the Kingdom Key uses; the `_bz_`, `_dw_`, `_hm_` sets belong to formchanges. Part A's `MagicTurn` / `MagicSpin`
clips and the form {2, 4, 16} shot handlers are likewise formchange-only.

---

## 2. What the host must provide (consolidated)

| Need | Used by | Detail |
|---|---|---|
| "execute command id" events, stick magnitude and world yaw | A, E | from the command menu / shortcuts (UI layer); for Cure and party items a target handle |
| buttons: shotlock hold, attack (release), airstep | D | plus the viewport size |
| lock-on targets | A, B, D | per target component: look-at position each tick, alive, team, owner handle, permissions (lock, shotlock, move-flow + type); the controller's current target and auto-target |
| screen projection and line of sight | D | project a world point with the game camera; a line-of-sight query eye → target |
| line trace | A | spawn wall guard: from `P − D·L` to the muzzle `P` |
| sphere sweep against level geometry | B | root sphere r 25 (profile `TresProjectile`: blocks world and gimmicks, ignores pawns), returns impact normal and start-penetration; floor test below a Blizzard bolt (rail) |
| overlap of attack shapes with enemy bodies | B, D | swept spheres / capsules / cylinder (growing sizes), one hit per actor per activation unless the shape has an interval |
| hit delivery | B, C, D | (damage from C 4.2 or the inputs to compute it: victim defence, damage min / max, attribute resistances, kind; attack row: attribute, damage kind, power scale, angle, effect time, knockback, bad status kind / chance / time, armour, reaction power, force level; attacker, finish / grand flags) |
| pawn services | A, D, E | on ground, swimming, velocity, max walk speed, `Turn(yaw, alpha)`, gravity scale and enable, movement mode, direct XY velocity, invincible flag, muzzle socket `ATK_01` (bone `wep`, +79.04 cm), sockets `L_te` |
| time dilation hook | D | 0.4 while aiming, 0.1 at the airstep start; the aim timers divide dt by the dilation |
| animation | all | play by name with blend-space input (pitch), animation time, is-end, notify dispatch: Shot, Timing, EnableChange, PlayVoice, AttachEffect, swing SE |
| audio / VFX | all | effects by path, cast effect groups, voice groups (`sora_magic` index, `sora_item`, `sora_atk3`), SE assets |
| HUD read-outs | C, D, E | MP, max MP, MP-charge fraction; focus, aim time left, lock count / max, current shotlock kind, can-shoot / can-airstep; pouch counts |
| game state | C, E | ability counts, equipment stat sums, difficulty, option sets, pouch contents and stock |

---

## 3. Data inventory

| Sheet (`sheets/port/`) | Rows | Content |
|---|---|---|
| `sora-magic-spells.json` | 533 | cast path: gates, state constants, animation and notify times, magic-set units, combo rules |
| `sora-magic-projectiles.json` | 5770 | per spell and tier: class chain, class-default and component values, hit shapes, respawn chain, attack rows (all fields) |
| `sora-magic-mp.json` | 5135 | MP rules, cost table, charge, damage and cure formula constants, level table of magic power, abilities, status rows, food and victory-bonus values |
| `sora-magic-shotlocks.json` | 614 | focus, aim mode, Kingdom Key shotlocks and projectiles, timing gauge, airstep |
| `sora-magic-items.json` | 378 | item state timing, pouch, effect amounts |

`build/sora_tmp/magic/asset_requests.json`: 735 entries (101 animation, 3 camera, 51 data, 166 effect, 6 mesh, 99 projectile_blueprint, 60 sound, 20 ui_icon, 188 ui_text, 41 voice).
Columns of every sheet: id, value, unit, source, evidence, ref_addr, ref_conf, note; an unchecked value
(`null`, `ref_conf` guess) is a quantity the code needs that was not determined: 87 (proj 82, mp 1, shot 4) rows.

Working files (scripts, dumps, per-strand notes): `build/sora_tmp/magic/` — `cast/`, `proj/`, `mp/`,
`shotlock/`, `airstep/`, `items/`; the sheets and this document are rebuilt with `merge.js` and `assemble.js`
there.

---

## 4. What still needs the running game (short list; probe tables in each part)

Nothing in the list blocks an implementation; each item is a value or an order of events that static reading
leaves open, with the assumption the specification makes.

1. **Frame order**: UI tick against the controller input update (J); player state against projectile
   components (3 or 4 frames between the last formation placement and the launch, G); whether the Cure heal
   lands in the shot frame or the next (I).
2. **Aim camera FOV** (assumed the component default) and the two `ULocalPlayer` fields used for the screen
   test (assumed last view-projection matrix and view rectangle) (F).
3. **Object-type slow motion**: which actor types the masks cover in practice, particles and sound of slowed
   actors (F, H, J).
4. **Airstep**: start value of the unlock bit, the sweep shape (assumed spheres of 35 / 60 cm), the measured-
   speed vector of the stuck test, whether the kick projectile can spawn on a wall kick, per-level data of the
   placed airstep points (H).
5. **Spells**: which enemies accept SWIRL_UP, whether DISCHARGE chains on the victim side, riding a Blizzard
   rail and node expiry under Sora, the Water barrier attachment (I). The rail-slide state itself belongs to
   the actions strand.
6. **MP**: `MyMpChargeTime` at pawn+0x60C (50 s from the constructor), names of the game option sets, the
   conditions of the attacker ×1.5 and victim ×0.25 multipliers (C).
7. **Focus**: confirm that FOCUS_BOOST leaves the aim drain unchanged; menu name of game option B6 (G).
8. **Menu**: how long the HUD freezes situation timers during a command; the refill block
   `gameInstance+0x8D4 & 0x14` (J).
9. Pawn virtuals whose visible effect was not read and are called symmetrically in Begin / End of the
   shotlock and airstep states: vt+0x1388, vt+0x19E0, vt+0xDC8 (G, H).

---

# Part A — Cast: request path and the cast state

_Source of this part: `build/sora_tmp/magic/cast/spec.md` (section numbers are local to the part)._

Scope: button press → spell actor spawned, chained casts, cancel windows. Projectile behaviour after spawn, MP / damage
formulas, shotlock, airstep and items are other strands. Tags: [I] instruction level (address), [C] decompiled C only,
[U] stock UE 4.17, [A] asset, [D] derived, [?] not determined statically. Units cm, s, deg.

### 0. Architecture

```
HUD command menu / shortcut (UI strand)                                    [C] callers 0x1416220D0, 0x14161D450, ...
  └─ controller vt+0x1540 ExecuteCommand(cmdKind, UICommandInfo*)  0x14140EAA0   (slot 680)
       ctrl+0x87C = cmdKind; m_pUICommandInfo = info
       └─ controller vt+0x1530 ExecCommand()                      0x14141B470   (slot 678; switch on cmdKind)
            target comp  = controller vt+0x1690()  (weak ptr ctrl+0x994)        0x1413FED90
            pawn+0xE00   = cmdKind                                              (pending magic command)
            └─ request interface (pawn+0x450, vtable 0x14685EB18)
                 slot 53 vt+0x1A8 TryMagic(targetPawn, targetComp, stickMag, stickYaw, bGrand)   0x14143AEB0
                   ├─ slot 52 vt+0x1A0 CanMagic(cmd)                                             0x1414286F0
                   │     └─ slot 51 vt+0x198 CanUseCommand(cmd)  (MP / restriction gate)         0x14142F330
                   └─ SetState(18) 0x1405B9FE0  or  PushUnique(18) 0x1405B4F80 (queued behind Attack / FireMagic)
state TresPlayerState_FireMagic (id 18 = TSID_STATE_FIREMAGIC, vtable 0x14691D4D0, size 0xE8)
  Reset 0x14136ECF0 · Begin 0x14136A8E0 · Tick 0x1413794B0 · End 0x14136CA50 · debug HUD 0x14136AF80
  animation notify TresAnimNotify_Shot → state sub-object +0x40 (vtable 0x14691D8E0) slot 69 = OnShot 0x14137BB30
       ├─ projectile class: pawn vt+0x1438 → equipment vt+0x5A0 0x141780F20 → TresPlayerMagicSet 0x14147AA70
       ├─ spawn: pawn vt+0x900 (spawner) → vt+0x788(class, pawn, &pos, &targetPos, &dir, yaw, 0, 0)
       └─ MP use: 0x14140D560(controller, cmd) → pawn vt+0x1860 (0x14144A600)       ← at the SHOT notify
```

It is the single state for every spell and tier (Fire/Blizzard/Thunder/Water/Aero/Cure, -ra, -ga, the "-gan" and
`S_*` Grand Magic situation commands, and the four underwater variants) [I]: `TryMagic` only ever creates state 18
(0x14143AFE4, 0x14143B033, 0x14143B162) and the spell is nothing but the command id stored in state+0xB0.
`MyStateID` = 0x12 and name "FireMagic" are set by the parent constructor 0x141505900 [C].

### 1. The state object

#### 1.1 Fields [I] (offsets from the state; written by TryMagic 0x14143B1BE..0x14143B33F unless noted)

| off | type | meaning |
|---|---|---|
| +0x78 | i32 | `m_nSubStep`: 0 = start animation next tick, 1 = running |
| +0x84 | bits | bit0 `m_bEnableChange` (notify `TresAnimNotify_EnableChange`), bit5 `m_bEnableInput` |
| +0x88 | ptr | `m_pTargetPawn` (reflected) |
| +0x90 | ptr | `m_HomingTargetComponent` (reflected, `TresLockonTargetComponent`) |
| +0x98 | f32 | stick magnitude of a moving cast (1.0 when forced from Glide) |
| +0x9C | f32 | world yaw of the moving cast (stick yaw; actor yaw when forced) |
| +0xA0 | f32 | copy of +0x9C made when the animation starts (Tick) |
| +0xAC | i32 | copy of the command id made in Begin |
| +0xB0 | i32 | command id = `ETresCommandKind` = row id of `TresCmdData` (Fire 29 … see 3.3). Rewritten to the underwater id in Tick |
| +0xB4 | i32 | combo index of this cast (debug label "Combo:%d/%d") |
| +0xB8 | i32 | combo counter A (pawn+0xE1C chain; see 5.3) |
| +0xBC, +0xC0 | i32 | Begin-time copies of +0xB8 and +0xC4 (handed to the projectile) |
| +0xC4 | i32 | combo counter B (pawn+0xE20 chain) |
| +0xC8 | f32 | yaw Sora turns to (target yaw) |
| +0xCC | i32 | weapon form of the active weapon slot (pawn vt+0x12E0(-1) → equipment vt+0x440 0x141780380) |
| +0xD0 | i32 | "BulletCount": shots fired by this cast (debug label) |
| +0xD4 | f32 | aim pitch (clamped), used as blend-space input and as projectile pitch |
| +0xD8 | u32 | flags, below |
| +0xDC/+0xE0 | weak ptr | an object notified in Begin (0x140F6F670) and End (0x140F74110); no writer found, null by construction [?] |

Flags +0xD8:

| bit | set by | meaning |
|---|---|---|
| 0x1 | TryMagic | cast started in the air (`!OnGround` 0x1405AD850; forced 1 from Glide/Floating) |
| 0x2 | Tick sub 0 | moving cast (copy of 0x8; cleared when swimming, for some weapon forms, and for finish / grand casts) |
| 0x4 | TryMagic | swimming (pawn vt+0x1C30) |
| 0x8 | TryMagic | move request (see 2.3) |
| 0x10 | TryMagic / TryAttack on the *current* cast | a follow-up (magic or attack) is queued behind this cast |
| 0x20 | Tick sub 0; cleared by OnShot | turn to the target each tick |
| 0x40 | Tick sub 0 | this cast is a combo finisher ("Finish") |
| 0x80 | Tick sub 0 | last finisher of the combo ("Finish End"): Shoot/Chant animation, invincible, no chain |
| 0x100 | OnShot | shot released (opens `m_bEnableInput` after 0.2 s of animation time) |
| 0x200 | Tick sub 0 | hover: gravity scale 0.1 + gravity disabled (air or swim start) |
| 0x400 | Begin | Grand Magic (cmd in the -gan / `S_*` set, or request flag 0x2000): Shoot / Chant animation, invincible until the shot, no MP use |
| 0x800 | Begin | Cure family (cmd 49..51 or 125) |
| 0x1000 | Begin | Cure with ability LEAF_VEIL (43): invincible, damage reaction refused |
| 0x2000 | TryMagic | request came with bGrand = 1 |
| 0x4000 | Begin | ability AUTO_LOCK_MAGIC (73): keep the target given at the request |
| 0x8000 | Begin | ability ENDLESS_MAGIC (82): no combo limit, no finisher |
| 0x10000 | End helper 0x141367F70 | counters already advanced |
| 0x20000 | OnShot | "MP was full when the first shot left" → projectile vt+0x1160 |
| 0x40000 | OnShot | 0x20000 already evaluated (multi-shot animations) |
| 0x80000 | OnShot | Water ring spawned (invincible until End) |
| 0x100000 | debug HUD | debug draw request |
| 0x200000 | OnShot | at least one shot was released |
| 0x400000 | Tick sub 0; cleared by OnShot | keep re-aiming the pitch each tick |
| 0x800000 | 0x14137DF10 | debug |

`Reset` 0x14136ECF0 [C]: base reset, flags &= 0xFF048000, +0x88..+0xD4 zeroed. The base constructor clears
`m_bEnableRecycle` (0x141506C90) [C], so a cast normally gets a fresh object and the preserved bits are 0.

#### 1.2 Begin 0x14136A8E0 [C, constants I]

1. pawn vt+0x1340(1, 1, 1.0): draw the Keyblade. pawn vt+0x1780(1): last-anim-pose = 1 (pawn+0x6E4).
2. `SetActorRotation(Rot(0, actorYaw, 0))` (0x143EAD160): pitch and roll removed.
3. pawn vt+0x16D8(1): movement component +0xA3C bit 2 = 1 (the CalcVelocity skip flag of movement spec 4.2).
4. +0xAC = cmd, +0xBC = +0xB8, +0xC0 = +0xC4; pawn+0xE1C = 0, pawn+0xE20 = 0 (vt+0x1D68 / +0x1D70).
5. +0xCC = weapon form. Effect: `m_EffectGroup` of the spell's unit (vt+0x398 0x14136E220); if ≥ 0 →
   0x1405B8C30(pawn+0x440, slot 1, group): the pawn's effect slot 1 (cast effect) is set to that group.
6. Flag bits 0x4000, 0x8000, 0x400, 0x800, 0x1000 as in the table (HasAbility = pawn vt+0x14E0 0x1414358B0;
   helpers 0x14136F020, 0x14136F000, 0x14136F130). If (flags & 0x1400) → pawn vt+0x2068(1) = `m_bIsInvincible` on.

#### 1.3 Tick 0x1413794B0 [I]

Every tick, first: pawn vt+0xDC8(dt·12) and vt+0xDB8(dt·12) (0x14137956C: mesh / actor pitch-roll levelling at rate 12).

Target refresh (0x1413795B7): unless flag 0x4000 (AUTO_LOCK_MAGIC) or 0x800 (Cure):
`m_HomingTargetComponent = controller vt+0x16A0()` (0x1413FEF10: the locked-on component if locked, else the
controller's auto-target ctrl+0x9A4) and `m_pTargetPawn = null`. A homing component whose vt+0xA80() is false is dropped.

**Sub-step 0 (first tick):**

```
flags.bit1 = flags.bit3;  if flags&4: flags &= ~2                              // 0x14137988A
if flags&0x800:           pawn vt+0x2740(1)            // Cure: pawn+0xE48 bit5 "no further chain"
elif !(flags&0x8000):                                                           // 0x1413798DA
    max = pawn vt+0x1D00()  (MaxCombo 0x141433570);  nfin = pawn vt+0x2728() (NumFinish 0x141433A60)
    idx = state+0xB4
    if idx+1 == max-1: pawn vt+0x28C0()                 // friend "combo finish" roll 0x141430A60
    if idx < max - nfin: pawn+0xE14 = 0
    else:
        f = pawn+0xE14;  flags |= 0x40
        if f >= nfin-1 || idx >= max-1:
            pawn vt+0x2740(1);  flags |= 0x80;  pawn vt+0x2068(1)   // invincible
        pawn+0xE14 = f+1
if pawn status byte (+0x85D) == 13 && !(flags&0x8000) && idx >= max-2: 0x14140DF30(controller, 1)   // HUD hint [C]
if flags&4 (swimming): cmd 29..32 → 53, 33..36 → 54, 37..40 → 55, 45..48 → 56          // 0x141379A.. switch [C]
unit = pawn vt+0x1440(cmd)  (equipment vt+0x5A8 0x141780ED0)
isCast = unit.m_bIsCastType; isTurn = unit.m_bIsTurnType
if vt+0x3C0() (weapon form in {3,5,6,7,12,14,15,17,18,20,21}): flags &= ~2
if flags & 0x480: flags &= ~2
voice = unit.m_VoiceGroup;  if flags&0x80: voice = unit.m_VoiceGroupFinish
if flags&0x800 && m_pTargetPawn: voice = table by (caster id, target id)   // Sora→Sora 31, →Donald 32, →Goofy 33 … (4.5)
```

Animation name (0x141379D3A.. ; strings in the function) — `B` = flags & 0x480, `air` = flags & 5:

| | not moving (flag 2 clear): StopMovement first | moving (flag 2 set) |
|---|---|---|
| Cure (0x800) | `MagicCast` / `MagicAirCast` | (cast type) `MagicCastM` / `MagicAirCastM` |
| cast type | !B `MagicCast` / `MagicAirCast`; B `MagicChant` / `MagicAirChant` | `MagicCastM` / `MagicAirCastM` |
| shot type, turn type | !B `MagicTurn` / `MagicAirTurn`; B `MagicSpin` / `MagicAirSpin` | as below |
| shot type | !B `MagicFire` / `MagicAirFire`; B `MagicShoot` / `MagicAirShoot` | δ = norm(+0x9C − actorYaw): \|δ\| < 50 → `MagicFireMF`, δ < 0 → `…ML`, else `…MR` (`MagicAirFireM?` in the air) |

Ground / air side effects in the same step:
- not moving: pawn vt+0x16E8 StopMovement. On the ground and not swimming (flags & 5 == 0): pawn vt+0x1728(1)
  (gravity enabled). Otherwise: `SetGravityScale(0.1)` (0x141379EDC / 0x14137A057), flags |= 0x200, pawn vt+0x1728(0)
  (movement +0xA3C bit 1 = gravity disabled).
- moving (0x141379D51): v = GetVelocity(); if v.Z > 0 → StopMovement; pawn vt+0x1668((v.X, v.Y, 0), 0) (movement
  vt+0x500 0x140592BC0: requested velocity); +0xA0 = +0x9C;
  ground: vt+0x1728(1); air: gravity scale 0.1, flags |= 0x200, vt+0x1728(0).

Play (0x14137A144): `play(name, slot 0, blendIn −1 (asset value, 0 for these assets), mode 2, effectGroup =
unit.m_EffectGroup, playRate 1.0, rootScale (1,1))`; if it returns > 0: anim interface vt+0x1F0(voice) = the voice
group the `TresPlayVoice` notify of the clip will use. flags |= 0x400000; +0xD4 = ClampPitch(PitchToTarget());
+0xC8 = actor yaw; flags |= 0x20; sub-step = 1. In this tick the turn alpha factor is 1.0 instead of 0.2 (0x14137A10D).

**Sub-step 1:**

```
if flags&0x400000: +0xD4 = ClampPitch(PitchToTarget())                 // until the shot
if flags&0x10 and next queued state is FireMagic with cmd 29..31 (Fire/Fira/Firaga)
   and !vt+0x3B0(cmd):  m_bEnableChange = 1                             // 0x141379780: Fire chains at once
if IsAnimEnd() || (m_bEnableChange && flags&0x10):  finish (vt+0x270 → next queued state or default)
if !(flags&0x10) && !(flags&4) && (pawn swimming vt+0x1C30 || vt+0x1A80(0)):   // fell into water
    StopMovement; pawn vt+0x2740(1); if m_bEnableChange && PushUnique(SWIM 102): finish; return
```

**Every tick (both sub-steps), tail:**

```
if !m_bEnableInput && flags&0x100 && animTime > 0.2: m_bEnableInput = 1        // 0x14137A24C, strict >
if flags&0x20:                                                                  // turn to target
    if homing comp valid:   d = comp.vt+0xB88() − pawn location (XY)
    elif target pawn valid && != self: d = targetPawn location − pawn location (XY)
    if d found: +0xC8 = yaw(normalize(d))
    pawn vt+0xDB0(Rot(0, +0xC8, 0), k · dt · 60)        // k = 1.0 in the first tick, 0.2 afterwards (0x14137A459)
if flags&2:  v = Rot(0,+0x9C,0).Vector() · GetMaxWalkSpeed() (pawn vt+0x1580);  pawn vt+0x16C0(v)   // XY velocity set
if !swimming && m_bEnableChange && flags&0x200:
    flags &= ~0x200; ResetGravityScale (vt+0x1658); vt+0x1728(1); SetMovementMode(Falling=3) (vt+0x17A0(3))
```

`PitchToTarget` 0x141367C00 [C]: pitch of normalize(targetPos − pawnPos) with the 3-D positions used above; 0 with no
target. `ClampPitch` 0x14137DF10 [I]: feeds the pitch to the blend space (anim interface vt+0x120(pitch,0,0)), reads the
blend range (vt+0x128): with a range → clamp(pitch, min, max); with none (plain sequence) → |pitch| > 30 ? ±45 : 0
(0x14137DFA5, 0x14137DFB3).

#### 1.4 The shot: `TresAnimNotify_Shot` → OnShot 0x14137BB30 [C, constants I]

The notify object's fields from +0x50 are passed as the parameter block: `m_ProjectileName` (+0), `LocationIndex` (+8),
`m_SocketName` (+0x10), bits at +0x18: 1 `m_bCastEffectContinue`, 2 `m_bAttachFireEffect`.

1. flags = (flags & ~0x400020) | 0x100. If it had 0x480 → invincible off (pawn vt+0x2068(0)).
2. First shot only (bit 0x40000): bit 0x20000 = !Grand && cost(cmd) != 0 && MP ≥ MaxMP (0x141367E80).
3. Turn-type unit → vt+0x3F0 (0x141380470). Weapon forms {2,4,16} for non-Cure, non-underwater → vt+0x3F8
   (0x14137E110). Neither applies to the Kingdom Key (form 1) with `p_ex001_MagicSet`.
4. **Water** (cmd 41..44, 119..121): for a character id (pawn vt+0xD70 = `m_ChrUniqueID`) ≠ 7 with weapon form in {1, 8, 9, 10, 22, 23, 24} (Kingdom Key = 1),
   or Roxas(7) without Grand → `ShotWater` 0x14137FFA0, then flags |= 0x80000 and invincible on (0x14137BD30):
   spawn at the muzzle; target vector (50, 0, 100); direction = Rot(pitch 60, 0, 0).Vector(); yaw = actor yaw; the
   projectile is attached to the pawn's root; homing flag vt+0x1220(1) if a homing component exists; rest as step 8.
5. Aero special: pawn status byte == 15 and cmd 45..48 / 122..124 → 0x1405A5C40(pawn, 15) and the projectile gets
   vt+0x11C0(1); its target position is the pawn location.
6. Class = pawn vt+0x1438(cmd, 0) → `m_AssetData00` of the unit (3.1). Requires pawn vt+0x900() (spawner) and
   pawn vt+0x1330() (weapon out).
7. Spawn position P: `LocationIndex ≥ 0` → pawn vt+0x1410(LocationIndex) = weapon[LocationIndex] mesh socket
   `MuzzleAttachPoint` (0x1417812E0); `< 0` → pawn mesh socket `m_SocketName` (0x144037430), or the pawn location.
   P.z is limited to actorZ + 163 (0x14137BF51). Aim: yaw = actor yaw; D = Rot(+0xD4, actorYaw, 0).Vector().
   - `m_bIsDirectPos` = 0 (Fire, Blizzard, Water, Cure): wall guard — L = max(120, distXY(actor, P)); line trace from
     P − D·L to P (0x14063C1C0); on a hit P = hit point (0x14137C3B3). Target position T handed to the spawn = P.
   - `m_bIsDirectPos` = 1 (Thunder, Aero): D = Rot(0, actorYaw, 0).Vector(); T = homing component location, else target
     pawn location, else actor location + D·500 (0x14137C216); with a target actor, T.z = max(T.z, its root Z).
8. Spawn: `proj = spawner.vt+0x788(class, pawn, &P, &T, &D, yaw, 0, 0)`; then
   `proj.vt+0x1100(cmd, −1)`; homing component → `vt+0x1220(1)`, else target pawn → `vt+0x1210(1, pawn, 0, zero, 0)`;
   `vt+0x1130(comboIdx)`; `vt+0x1140(+0xB8, +0xBC, +0xC0)`; `vt+0x1160(flag 0x20000)`;
   `vt+0x1120(flag 0x80, flag 0x400)`; statistics 0x140E66FE0(cmd); `m_bAttachFireEffect` →
   `vt+0x1200(weapon mesh (vt+0x1418), socket (name or vt+0x1420), zero)` else `vt+0x11F8(&P)`; `vt+0x1270()`;
   BulletCount (+0xD0) += 1.
9. Cure (0x800): if pawn vt+0x1740() (battle mode) and n = AbilityCount(POWER_CURE 103) > 0:
   controller vt+0x1A80(round(((15n + 20) · MP) / MaxMP), 1, cmd) [C; MP strand]. cmd 125 → pawn+0xE60 bit 20 = 1.
10. **MP is deducted here**: if !Grand → 0x14140D560(controller, cmd) (0x14137C8.. after the spawn block): cost =
    `TresCmdData[cmd].Mp` (0xFF = all) through 0x1405A4B90(cost, pawn+0xE18) → pawn vt+0x1860 (0x14144A600) [C].
11. flags |= 0x200000; pawn+0xE28 = 1 (vt+0x1D80); pawn vt+0x1728(1) (gravity enabled again; the scale stays 0.1 until
    EnableChange); if not swimming: `SetMovementMode(Falling)` (vt+0x17A0(3), 0x14137C9FE).
12. cmd in {32, 36, 40, 44, 48, 112, 115, 118, 121, 124} and weapon form ∉ {2, 16}: pawn vt+0x1048(1, 0, 0) → controller
    interface vt+0x20 (0x1405D33C0) [?] (the only call that could be a camera / rumble hint).

#### 1.5 End 0x14136CA50 [C; branch constants I]

pawn vt+0x1728(1), ResetGravityScale, vt+0x16D8(0), vt+0x22E8(), effect slot 1 stopped (pawn+0x440 vt+0x1D0(1,0));
if flags & 0x81480 → invincible off. Then by the id of the next queued state:

- Attack family (12, 13, 14, 75..90, 156, 157, 159..161): counters advance (0x141367F70: +0xC4 += 1; +0xB8 += 1 unless
  ENDLESS_MAGIC — pawn vt+0x1D20(1)); a next `TresPlayerState_Attack` gets +0x12C = +0xB8 and +0x138 = +0xC4.
- FireMagic (18): counters advance; next.+0xB8 = +0xB8, next.+0xC4 = +0xC4.
- WeaponChange (32): counters advance; pawn+0xE1C = +0xB8, pawn+0xE20 = +0xC4.
- anything else: counters advance; pawn vt+0x1EA8(flag 0x80) = combo reset 0x141432780 (pawn+0xE14 = 0, +0xE48 bit5 = 0,
  …; with flag 0x80 the command cool-down pawn+0xE34 = `MyCommandCoolDownTime`); then pawn+0xE1C / +0xE20 as above.
- next id ≠ 12: pawn vt+0x28B8() (0x141431A80).
- Shot fired (0x200000) and not Finish End: pawn+0xE34 = 0 (0x14136CD5F).
- No shot fired (interrupted) and (swimming, or not on the ground): pawn+0xE38 = `MyAirSlideCoolDownTime`,
  pawn+0xE3C = `MyAirGuardCoolDownTime` (0x14136CD7F..0x14136CDC1).

### 2. The request

#### 2.1 Controller [C]

`ExecCommand` 0x14141B470 first requires controller vt+0x1548(cmd) (0x1413EC9F0). Common values: target component =
controller vt+0x1690() (the controller's current target, weak pointer ctrl+0x994); yaw = actor yaw, or the stick's world
yaw ctrl+0x85C when the stick magnitude ctrl+0x854 ≥ 0.1. By command:

| commands | handling |
|---|---|
| 29..31, 33..35, 37..39, 45..47, 53..56 | target pawn = owner (comp+0x1B8) if it is a `TresCharPawnBase`; pawn+0xE00 = cmd; TryMagic(pawn, comp, mag, yaw, 0) |
| 41..43 (Water) | refused while swimming (pawn vt+0x1C30), else as above |
| 32, 36, 40, 44, 48, 110..124 (Grand Magic) | refused while swimming; TryMagic(…, 1); on success controller vt+0x1520(), ctrl+0x8F4 = 0.1, situation-command lists cleared |
| 49..51 (Cure) | target = `m_pUICommandInfo->m_Actor` / `m_Component` (the party member chosen in the menu); refused if there is no command info; TryMagic(…, 0) |

On success 0x1414158F0(controller, cmd, 0) and, in `ExecuteCommand`, every actor tagged "Enemy" is told the command id
(+0x448 interface vt+0x1E8) and the use is counted (0x140E9E2B0). **There is no buffer on the controller side**: a
refused command returns false and is dropped; the only queueing is the state queue (2.3).

#### 2.2 Gates [I]

`CanMagic(cmd)` 0x1414286F0 — all of:
1. controller exists (pawn vt+0x948); pawn vt+0x29C0() true (0x141427690: not pawn+0x868 bit1, status byte ∉ {12, 17},
   current and next state ∉ {61, 62}); pawn vt+0x2540() false; interface vt+0x618() false (0x141436AA0: not in
   Jump-with-+0xF8, states 21..25, 138); `CanUseCommand(cmd)`; pawn vt+0x2788() == 0 (pawn+0xE60 bit 17).
2. Grand Magic commands (32, 36, 40, 44, 48, 110..126, 231) take their own exit (0x1414287B2): pawn+0xE62 bit0 == 0,
   not swimming, nothing queued, pawn vt+0x29F0(cmd) (0x141425510) → true. They ignore the table below.
3. Next queued state ∉ {19, 32, 40, 59, 61, 62, 67, 118}.
4. By current state (jump table 0x141428FF0):

| current state | condition |
|---|---|
| IDLE 1, RUN 2, GLIDE 4, FLOATING 5, SWAYING 11 | always |
| JUMP 3 | Jump+0xF8 == 0 |
| ATTACK family 12, 13, 14, 75..90, 156, 157, 159..161 | next queued is not FireMagic; Attack+0x143 == 0 (no magic already queued); Attack+0x164 != 0; Attack+0x128 (combo) < MaxCombo; pawn+0xE48 bit5 == 0; (Attack+0x1EC & 0x80) == 0 |
| FIREMAGIC 18 | next queued is not FireMagic; flag 0x10 clear; BulletCount > 0; pawn+0xE48 bit5 == 0; flag 0x8000 **or** +0xB4 < MaxCombo |
| GUARD 15 | nothing queued; Guard+0xDC != 0; `m_bEnableInput` |
| DODGEROLL 16, USE_ITEM 111, BADSTAT_STUN 122 | `m_bEnableChange` (122: also nothing queued) |
| AIR_SLIDE 20 | (state+0xA8 & 0x80) == 0 and `m_bEnableChange` |
| DAMAGE 19 | nothing queued; Damage+0x90 == 0; Damage+0x88 ∈ {1, 2}; `m_bEnableChange` |
| WEAPONCHANGE 32 | sub-step != 0 |
| OPEN_TREASURE_BOX 74 | state vt+0x388() |
| GIMMICK_ATTACH 97 | its sub-object vt+8() |
| SWIM 102 | Swim state vt+0x3A0(cmd) |
| COVER 142 | on the ground; Cover+0xA0 == 0; !`m_bEnableChange`; sub-step != 10 |
| every other state | refused |

`CanUseCommand(cmd)` 0x14142F330 for the magic commands (29..31, 33..35, 37..39, 41..43, 45..47, 49..51, 53..56):
1. game instance (pawn vt+0x8C8) with (+0x95B0 & 4) set; current state ∉ {6, 8, 9};
2. Cure (49..51, 125): refused if 0x1405AD320(pawn, 5);
3. pawn vt+0x1EF8() false (0x141436820: pawn+0x1130 bit 0x10, pawn+0xE60 & 0x8002, or restriction 1);
4. command cool-down pawn+0xE34 ≤ 0 (vt+0x1E10, 0x14110E6F0);
5. player id (vt+0xD80) == 11 → allowed, else: not in MP charge (pawn+0x7C0 bit 1, vt+0x17E0) and
   - cost byte == 0xFF (Cure) → allowed;
   - without ability MP_SAFETY (25) → allowed **whatever the MP amount** (0x14142F4B6);
   - with it: need = 0x1405A4B90(cost, pawn+0xE18); allowed if need ≤ MP, or MP > 0 and ability MP_LEAVE (76).

So the MP rule of the gate is "not in MP charge"; "MP > 0" follows from the charge state starting at 0 (MP strand).

#### 2.3 TryMagic 0x14143AEB0 [I]

`cmd = pawn+0xE00`; fails if `CanMagic(cmd)` fails. `air = !OnGround`. With bGrand: SetState(18), idx = 0. Else by
current state (jump table 0x14143B368):

| current | combo index / counters | queueing |
|---|---|---|
| 1, 2, 3 | pawn+0x10E4 (combo keep timer, vt+0x1770) > 0 ? idx = pawn+0xE18 + 1, A = pawn+0xE1C, B = pawn+0xE20 : idx = 0, A = B = 0 | SetState(18) (queue cleared) |
| 4 GLIDE | same; move flag forced: flags \|= 8, +0x9C = actor yaw, +0x98 = 1.0; air = 1 | SetState(18) |
| 5 FLOATING | same; air = 1 | SetState(18) |
| 11, 15, 16, 19, 20, 74, 97, 102, 111, 122, 142 | idx = 0, A = B = 0 | SetState(18) |
| 32 | idx = pawn+0xE18 + 1 (no timer test), A, B from the pawn | SetState(18) |
| Attack family | idx = Attack+0x128 + 1; Attack+0x143 = 1; A = B = 0 | PushUnique(18): queued behind the attack |
| 18 | current.flags \|= 0x10; idx = current+0xB4 + 1; A = B = 0 (End of the current cast fills them) | PushUnique(18) |

New state: flag 1 = air; +0x88 / +0x90 targets; +0xB0 = cmd; +0xB4 = idx; +0xB8 = A; +0xC4 = B; swimming → flag 4;
move request (not forced): cmc Velocity.X² + Velocity.Y² > 40000 (200 cm/s, 0x14143B2F3) **and** stick magnitude > 0.5
(0x14143B2FC) → flag 8, +0x9C = stick yaw, +0x98 = magnitude; bGrand → flag 0x2000. Finally pawn+0xE18 = idx.

A queued cast starts when the running state finishes: an Attack at its own `m_bEnableChange` chain point (combat
strand), a FireMagic per 1.3 (EnableChange notify, or immediately for Fire-family follow-ups).

#### 2.4 Target

The request passes the controller's current target (2.1); the state replaces it every tick with controller vt+0x16A0()
unless AUTO_LOCK_MAGIC or Cure (1.3). Facing: first tick the actor is turned with alpha 1·dt·60 (a snap at 60 Hz),
then 0.2·dt·60 per tick, until the shot; no angle limit in the state [I]. Pitch: −45..+45 from the blend space (4.1).
What `Turn(rot, alpha)` (pawn vt+0xDB0, 0x1405DDD90) does with alpha is in the movement spec.

### 3. Data

#### 3.1 `TresPlayerMagicSet` [A, I]

`m_Units` (+0x48): array of `TresPlayerMagicAssetUnit`, 0x28 bytes: `m_Command` u8 (+0), bits at +4:
1 `m_bIsCastType`, 2 `m_bIsTurnType`, 4 `m_bIsDirectPos`; `m_EffectGroup` i32 (+8); `m_VoiceGroup` i32 (+0xC);
`m_VoiceGroupFinish` i32 (+0x10); `m_AssetData00` class (+0x18); `m_AssetData01` class (+0x20, null in all four
assets). Lookup = linear search on `m_Command` (0x14147AA30); class = `m_AssetData01` only when asked with alt = 1
and valid, else `m_AssetData00` (0x14147AA70).

Which set: `TresEquipmentComponent.m_ReplaceMagicSet` (+0x298) first, then `m_MagicSet` (+0x280)
(0x141780ED0, 0x141780F20). Sora's component: `m_MagicSet` = `p_ex001_MagicSet` (`p_ex001_PawnBase`).
`m_ReplaceMagicSet` comes from the active weapon form's `TresEquipValiableSet` in `w_so0x0_Equip`: the Kingdom Key
(`w_so010_Equip`: one form `TRES_WEAPON_FORM_01`, two grow levels) has it null → **always `p_ex001_MagicSet`**.
Other Keyblades: form 2/3 entries name `_dw_` (w_so020, 040, 060), `_bz_` (020, 040, 050, 090, 110), `_hm_` (050, 060,
080, 090, 110). `_bz_` / `_dw_` = same flags all 0 with `…BZ` / `…DW` projectile classes; `_hm_` = turn type with
`…CL` classes; none of the three has Cure or underwater units (30 units each).

`p_ex001_MagicSet` (38 units; full dump `magicset_dump.json`; class path
`/Game/Blueprints/Projectile/Magic<Elem>/TresProj_Mag<Name>Nml`):

| cmd ids | names | cast | direct | effect group | voice / finish | class suffix |
|---|---|---|---|---|---|---|
| 29, 30, 31 | Fire, Fira, Firaga | 0 | 0 | 1301, 1302, 1303 | 20 / 2 | Fire, Fira, Figa |
| 110, 111, 112 | S_FIRA, S_FIRAGA, S_FIRAGAN | 0 | 0 | 1302, 1303, 1304 | 3 / 0 | Fira, Figa, Figan |
| 33, 34, 35 | Blizzard, -ra, -ga | 0 | 0 | 1320, 1321, 1323 | 20 / 4 | Blizzard, Blizzara, Blizzaga |
| 113, 114, 115 | S_BLIZZARA.. | 0 | 0 | 1321, 1323, 1325 | 5 / 0 | Blizzara, Blizzaga, Blizzagan |
| 37, 38, 39 | Thunder, -ra, -ga | 1 | 1 | 1340, 1341, 1342 | 21 / 6 | Thunder, Thundara, Thundaga |
| 116, 117, 118 | S_THUNDARA.. | 1 | 1 | 1341, 1342, 1343 | 7 / 0 | Thundara, Thundaga, Thundagan |
| 41, 42, 43 | Water, -ra, -ga | 1 | 0 | 1360, 1361, 1362 | 20 / 10 | Water, Watara, Wataga |
| 119, 120, 121 | S_WATARA.. | 1 | 0 | 1361, 1362, 1363 | 11 / 0 | Watara, Wataga, Watagan |
| 45, 46, 47 | Aero, -ra, -ga | 1 | 1 | 1400, 1401, 1402 | 21 / 8 | Aero, Aerora, Aeroga |
| 122, 123, 124 | S_AERORA.. | 1 | 1 | 1401, 1402, 1403 | 9 / 0 | Aerora, Aeroga, Aerogan |
| 49, 50, 51 | Cure, Cura, Curaga | 1 | 0 | 1420, 1422, 1424 | 12 / 0 | Cure, Cura, Curaga |
| 125 | S_CURAGAN | 1 | 0 | 1424 | 12 / 0 | Curagan |
| 53, 54, 55 | WaterFire, WaterBlizzard, WaterThunder | 0 | 0 | 501, 502, 504 | 20/2, 20/4, 21/6 | `Projectile/Swim/TresProj_MagWaterFire`, `…WaterBlizzard`, `…WaterThunder` |
| 56 | WaterAero | 1 | 1 | 503 | 21 / 8 | `Projectile/Swim/TresProj_MagWaterAero` |

Turn type is 0 in the whole set. The set has **no unit for the plain -gan ids 32, 36, 40, 44, 48** (they exist in
`TresCmdData` with Mp 0): Grand Magic uses the `S_*` ids.

#### 3.2 Enums

There is no separate "magic kind + level" enum on this path: the spell is the `ETresCommandKind` value [I]. Magic rows
of `TresCmdData` have Type 3 (`TRES_CMD_TYPE_COMMAND`), Cate 11 (`TRES_CMD_CATEGORY_MAGIC`) [A].

#### 3.3 Spawn socket [I, A]

`ATresWeaponBase.MuzzleAttachPoint` (+0x5A0) default = FName "ATK_01" (static 0x140183A40, constructor 0x141777AE0);
not overridden by `w_so_KeybladeBase` / `w_so010_Wep`. `w_so010_0_Skeleton` socket `ATK_01`: bone `wep`, relative
location (79.03675, 0, 0), rotation roll 180 — the Keyblade tip. Every cast clip's Shot notify leaves `LocationIndex`
and `m_SocketName` at the class defaults [A]; the constructor 0x14055ACE0 sets `LocationIndex` = 0 and both names
None [C] → **every cast spawns at the muzzle of weapon 0** (the Keyblade tip).

### 4. Animations (`p_ex001_AnimSet`, `Character/pc/p_ex001/anm/rt/body/`) [A]

#### 4.1 Table

All are played once, mode 2, rate 1.0, blend-in 0, no root motion (`bEnableRootMotion` absent). The Fire / Shoot names
are `BlendSpace1D` assets, axis −45..+45 (pitch), samples (0, −45, +45) = (N, D, U) clips, notify mode
`SQEX_AllAnimation`; the Cast / Chant names are plain sequences (pitch only snaps the projectile to 0 / ±45).

| AnimSet name | asset(s) | length | voice | **Shot** | EnableChange | other |
|---|---|---|---|---|---|---|
| MagicFire | BS 1321 / 1321D / 1321U | 1.2 | 0.06958 | 0.3 | 0.73333 | swing SE 0.17429 (SWING1), 0.40629 (SWING2) |
| MagicFireMF | BS 1328 (+D, U) | 0.8 | 0.06958 | 0.36667 | 0.73333 | LandAnimPose LIGHT_L @0; SE 0.32571 (SWING4) |
| MagicFireMR | BS 1329 | 0.8 | 0.06958 | 0.36667 | 0.73333 | LandAnimPose LIGHT_L; SE 0.3021 |
| MagicFireML | BS 1330 | 0.8 | 0.06958 | 0.36667 | 0.73333 | LandAnimPose LIGHT_L; SE 0.28229 |
| MagicAirFire | BS 1331 | 1.2 | 0.06958 | 0.3 | 0.73333 | LandAnimPose (default) @0; SE 0.12629, 0.34114 |
| MagicAirFireMF | BS 1338 | 0.8 | 0.06958 | 0.36667 | 0.73333 | SE 0.27543 |
| MagicAirFireMR | BS 1339 | 0.8 | 0.06958 | 0.36667 | 0.73333 | SE 0.28 |
| MagicAirFireML | BS 1340 | 0.8 | 0.06958 | 0.36667 | 0.73333 | SE 0.27314 |
| MagicShoot | BS 1322 | 1.53333 | 0.48831 | 0.7 | 1.43333 | SE 0.64911 |
| MagicAirShoot | BS 1332 | 1.26667 | 0.47005 | 0.66667 | 1.16667 | SE 0.39146, 0.7039 |
| MagicCast | 1341 | 1.13333 | 0.01982 | 0.33333 (attach) | 0.86667 | SE 0.23368; AttachEffect notifies of group 3400 (other Keyblades) |
| MagicCastM | 1348 | 1.13333 | 0.01982 | 0.33333 (attach) | 0.86667 | LandAnimPose LIGHT_L; SE 0.27362 |
| MagicAirCast | 1351 | 1.03333 | 0.05263 | 0.33333 (attach) | 0.66667 | SE 0.22389 |
| MagicAirCastM | 1358 | 1.13333 | 0.01982 | 0.33333 (attach) | 0.86667 | SE 0.2434 |
| MagicChant | 1342 | 1.56667 | 0.24081 | 0.56667 (attach) | 1.46667 | SE 0.47522; Bonamik wind 0.26476 for 1.30 s |
| MagicAirChant | 1352 | 1.2 | 0.24081 | 0.5 (attach) | 1.1 | SE 0.42 |

"(attach)" = `m_bAttachFireEffect` true. `MagicTurn`, `MagicAirTurn`, `MagicSpin`, `MagicAirSpin` are **not** in
`p_ex001_AnimSet` [A] (turn type is only in `_hm_` sets, whose forms replace the anim set). The numbered variants
(`MagicFire2`, `4`, `5`, `6`, `9`, `10`, `11`, `14`, `23` …) belong to other weapon forms' anim sets.

#### 4.2 Which clip for which spell with the Kingdom Key [D from 1.3 and 3.1]

| spell | normal cast | finish-end / Grand cast |
|---|---|---|
| Fire, Blizzard (shot type) | `Magic[Air]Fire`; moving `Magic[Air]FireMF/ML/MR` | `Magic[Air]Shoot` (never moving) |
| Thunder, Water, Aero (cast type) | `Magic[Air]Cast`; moving `Magic[Air]CastM` | `Magic[Air]Chant` (never moving) |
| Cure | `Magic[Air]Cast`; moving `Magic[Air]CastM` | — (no finisher logic) |
| underwater (53..56) | the `Air` names (flag 4), never moving | `MagicAirShoot` / `MagicAirChant` |

#### 4.3 Timeline of a standing Fire (example) [D]

t = 0 request accepted, state queued; next MainProc: Begin; first Tick: stop, play `MagicFire`, snap-turn. t = 0.07
voice. t = 0.3 Shot: projectile spawned, MP deducted, facing / aiming stop. `m_bEnableInput` in the first Tick after the shot (the test
"shot done and animation time > 0.2" is already true then). t = 0.733 EnableChange: dodge / air slide / follow-ups.
t = 1.2 animation end → state finishes. A queued Fire/Fira/Firaga follow-up starts on the first tick after the shot.

#### 4.4 Effects [A, C]

Begin sets pawn effect slot 1 to `m_EffectGroup`; the animation is also played with that group. The Keyblade's cast
effect table is `m_CmnMagicCastEffect` = `/Game/Effects/wep/w_com/ead/ead_wep_cal0` (`w_so_KeybladeBase` default),
"基本キー詠唱", whose groups map to: 1301..1306, 501 → `vm_fir_cal0`; 1320..1325, 502 → `vm_blz_cal0`; 1340..1345 →
`vm_thd_cal0`; 1360..1363 → `vm_wat_cal0`; 1400..1405, 503 → `vm_air_cal0`; 1420, 1422, 1424 → `vm_cur_cal0`
(`/Game/Effects/magic/<elem>/par/`). Which slot asset the pawn binds (pawn+0x8E8) and the attach sockets were not
decoded (VFX strand) [?].

#### 4.5 Voice [A, C]

Notify `TresAnimNotify_PlayVoice`, `m_VoiceGroupName` = "sora_magic"; the index is the group set by the state:
`m_VoiceGroup` (20 = Fire/Blizzard/Water, 21 = Thunder/Aero, 12 = Cure, 3/5/7/9/11 Grand) or `m_VoiceGroupFinish`
on a finish-end cast (2 Fire, 4 Blizzard, 6 Thunder, 8 Aero, 10 Water). Cure by Sora replaces it by the target's id:
Sora 31, Donald 32, Goofy 33, Kairi 46, Ventus 50, Roxas 48, Lea 49, Xion 47, Mickey 45, Jack 37, Woody 34, Buzz 35,
Hercules 36, Rapunzel 38, Flynn 39 (40 after flag `gameflow_RA` / `RA_END`), Sulley 41, Mike 42, Baymax 43,
FRD_RIKU 44, FRD_AQUA 51 [C].

### 5. Combos, finishers, cancels

#### 5.1 Chaining [I]

Magic shares the attack combo: index pawn+0xE18, finisher count pawn+0xE14, "combo over" pawn+0xE48 bit 5, keep timer
pawn+0x10E4 (combat strand owns them). A cast pressed during a cast is accepted only after the shot left
(BulletCount > 0), once (flag 0x10), while the combo index is below MaxCombo and the combo is not over; it is queued
and starts at EnableChange — or immediately when the follow-up is Fire / Fira / Firaga (1.3).

`MaxCombo` 0x141433570 for Sora in the base form [C]: 4 + AbilityCount(on the ground ? COMBO_PLUS 28 : AIRCOMBO_PLUS 29);
4 while swimming. `NumFinish` 0x141433A60: 1, +1 with TRIPPLE_SLASH (13) or CHARGE_THRUST (14), +1 with MAGICFLUSH (15)
or FINAL_HEAVEN (110); 1 while swimming. (Both functions have other values for formchanges — combat strand.)

#### 5.2 The finisher [I]

A cast whose index ≥ MaxCombo − NumFinish is a finisher (flag 0x40). It is the **last** one ("Finish End", flag 0x80)
when pawn+0xE14 ≥ NumFinish − 1 or index ≥ MaxCombo − 1. With default abilities: casts 0, 1, 2 normal, cast 3 = Finish
End. Finish End: Shoot / Chant animation, never moving, finish voice group, invincible from the first tick to the shot,
projectile told `vt+0x1120(finish = 1, grand)`, combo closed (no chain; `CanMagic` refuses until the reset), and on
leaving to a non-combat state the command cool-down `MyCommandCoolDownTime` starts.
ENDLESS_MAGIC (82): no finisher, no index limit. Cure: closes the combo at once (pawn+0xE48 bit 5) and has no finisher.

Grand Magic (architecture only): the situation command ids 110..126 arrive through the same controller switch with
bGrand = 1; gate 0x141425510; the state sets flag 0x400: Shoot / Chant animation, invincible until the shot, no MP use,
projectile `vt+0x1120(…, grand = 1)`. Which casts award the situation command is outside this path [?].

#### 5.3 Counters A / B [C]

+0xB8 (A) and +0xC4 (B) travel pawn+0xE1C / +0xE20 → state → next state. Each cast adds 1 to B and, without
ENDLESS_MAGIC, 1 to A, at End; the projectile receives (A, A at Begin, B at Begin) through vt+0x1140. Their use
(damage / MP scaling with MAGIC_COMBO_SAVE 56, MP_SAVE 75) is in the projectile and MP strands [?].

#### 5.4 Cancel windows out of a cast [C unless noted]

| action | gate | condition in FireMagic |
|---|---|---|
| another spell | CanMagic [I] | 5.1 |
| attack | slot 44 0x141426330, request slot 45 0x141437330 | next queued is not Attack; flag 0x10 clear; BulletCount ≥ 1; +0xB4 < MaxCombo; interface+0x9D8 != 0; combo not over. The request sets flag 0x10 and queues the attack → starts at EnableChange |
| guard | slot 54 0x1414294C0 | flag 0x10 clear, `m_bEnableInput`, not a finisher (0x40) |
| dodge roll | slot 57 0x141427CC0 | `m_bEnableChange`, flags & 0x50 == 0, combo not over |
| air slide | slot 68 0x141425780 | same as dodge |
| run | slot 38 0x14142CE00 | only from a moving cast: (flags & 0x12) == 2, `m_bEnableChange`, combo not over |
| jump | slot 41 0x14142A3D0 [I] | never (state 18 → default target 0x14142AA45 = false) |
| swim | Tick | on entering water: after EnableChange → SWIM |

Damage: the damage system replaces the state (End runs: gravity restored, cool-downs if no shot was fired). Not
interruptible while invincible (Finish End before the shot, Grand before the shot, Water ring after the shot, Cure with
LEAF_VEIL for the whole cast); request-interface slot 91 (0x141427300) additionally refuses when flag 0x1000 is set.

### 6. Movement during the cast [I]

- Ground, standing: velocity zeroed once (StopMovement, first tick); the CalcVelocity skip flag is on from Begin to
  End, gravity stays enabled. No root motion.
- Ground, moving cast (speed > 200 and stick > 0.5 at the request): each tick XY velocity = direction(+0x9C) ×
  `GetMaxWalkSpeed()`; the direction is fixed at the request; animation picked by the angle to the facing (50°).
- Air (or swimming) start: velocity zeroed (kept XY for a moving cast, upward Z removed), gravity scale 0.1 **and**
  gravity disabled → Sora hangs. At the shot: gravity enabled again (scale still 0.1), movement mode Falling. At
  EnableChange: gravity scale reset. So the hover lasts until the Shot time (0.3 / 0.367 / 0.333 / 0.667 / 0.5 s) and
  the slow fall until EnableChange (0.733 / 0.667 / 0.867 / 1.167 / 1.1 s).
- Landing during an air cast: no code in the state; the air animation keeps playing to its end (the state has no
  on-ground test after the start) [I by absence]. Glide / Floating starts are treated as air and as a forced moving cast
  (Glide).
- Facing: section 2.4. Actor pitch / roll are zeroed in Begin.

### 7. Camera

No call into the camera from Begin, Tick, End or OnShot [C by inspection of all callees listed here]. The only
controller-side calls are the HUD hint 0x14140DF30 and, for the five -gan ids, the controller-interface call of 1.4
step 12 [?].

### 8. Cure

Same state, same request path. Differences: target = the party member chosen in the menu (2.1), kept for the whole cast
(no per-tick target refresh); voice by target; `MagicCast` family; closes the combo; LEAF_VEIL → invincible; the "spell"
is a `TresProj_MagCure*` actor spawned at the muzzle like the others (homing component → vt+0x1220, else target pawn →
vt+0x1210) and the heal is done by that actor (projectile strand); cost byte 0xFF = all MP. Self-target: the turn-to-
target step skips a target pawn equal to the caster.

### Open questions

1. (settled) Default `LocationIndex` = 0 (constructor 0x14055ACE0).
2. Meaning of pawn status byte +0x85D values 13 and 15 (0x1405A9DD0) used for the HUD hint and the Aero special.
3. Controller-interface vt+0x20(1, 0, 0) after a -gan shot.
4. Weak object state+0xDC (Begin / End callbacks 0x140F6F670 / 0x140F74110).
5. Exact semantics of pawn vt+0xDC8 (0x1405AEED0 → vt+0x21B0) and of `Turn` alpha; of pawn vt+0x22E8, vt+0x28B8.
6. Projectile setters vt+0x1100 / 0x1120 / 0x1130 / 0x1140 / 0x1160 / 0x11C0 / 0x1210 / 0x1220 / 0x1270 (projectile
   strand) and the spawner vt+0x788 argument roles for T.
7. Counters A / B consumers; MP cost adjustments (MP strand).
8. Is a FireMagic object ever recycled (`m_bEnableRecycle`)? If so flags 0x8000 / 0x40000 survive `Reset`.
9. Turn-type (`_hm_`) and form {2, 4, 16} shot handlers 0x141380470 / 0x14137E110 were not decoded (not Kingdom Key).

### Runtime probe requests

| object | offset | type | settles |
|---|---|---|---|
| FireMagic state (queue current, id 18) | +0xD8 | u32 | flag timeline of a cast (air, moving, finish, hover) |
| FireMagic state | +0xB4, +0xB8, +0xC4, +0xD0 | i32 ×4 | combo index / counters / BulletCount per cast |
| FireMagic state | +0xCC | i32 | weapon form value for the Kingdom Key (expected 1) |
| FireMagic state | +0xC8, +0xD4 | f32 ×2 | target yaw and clamped pitch |
| pawn | +0xE00, +0xE14, +0xE18, +0xE1C, +0xE20, +0xE34, +0xE38, +0xE3C | i32 ×5, f32 ×3 | pending command, combo fields, cool-downs |
| pawn | +0x10E4 | f32 | combo keep timer at the request |
| pawn | +0x630 | u32 | bit 2 `m_bIsInvincible` during finish / Water |
| pawn | +0x85D | u8 | status byte |
| movement component | +0x2F8, +0xA3C, +0x214 (3 f32), +0x314 | f32, u8, vec, u8 | gravity scale 0.1, gravity-disabled bit, velocity, mode during an air cast |
| equipment component | +0x298, +0x280 | ptr ×2 | active magic sets |

### What the host must provide

- Input: "execute command id" events (the command menu / shortcut UI is another strand), stick magnitude and world yaw.
- The current target (a point that moves: location each tick, validity, owner) and, for Cure, the chosen party member.
- Pawn queries: on ground, swimming, velocity, max walk speed; `Turn(yaw, alpha)`; movement component with gravity scale,
  gravity enable, movement mode, CalcVelocity skip, direct XY velocity.
- A line trace (spawn wall guard) and the weapon muzzle socket transform (`ATK_01` on bone `wep`).
- Animation: play by name with blend-space input, animation time, is-end, notify dispatch (Shot, EnableChange,
  PlayVoice, swing SE, LandAnimPose).
- Spawning of spell actors with (class, position, target position, direction, yaw) and the setters of 1.4 step 8.
- MP: current, max, charge state, `UseMp(cost)`; abilities (HasAbility / count); effects and voices by group id.

---

# Part B — Spell actors (projectiles and area effects)

_Source of this part: `build/sora_tmp/magic/proj/spec.md` (section numbers are local to the part)._

What each spell actor does from the moment it is spawned. The cast state / request path up to the spawn is strand
`cast`; MP cost and the damage formula are strand `mp`. Units cm, s, deg. Tags: [I] instruction level (address given),
[C] decompiled C only, [U] stock UE 4.17 by function identity, [A] asset (file and property), [D] derived, [?] not
determined statically. Every number below is a row of `params.json` (same folder); `assets.json` lists the assets.

### 0. Summary of the architecture

- All six spells are `ATresProjectileBase` actors (a `TresPawnBase`, vtable 0x145FF41F8, 613 slots, ctor 0x141470210,
  sizeof 0x808) with three default sub-objects: `MyMovement` (`UTresProjectileMovementComponent`, +0x598),
  `MyParticle` (+0x5A0), `MyAtkColl` (`UTresAtkCollComponent`, +0x5A8), root `TresRoot` (`TresRootComponent`). [C 0x141470210]
- The magic classes add almost no code [I: vtable comparison, `vtdiff.js`]:

  | class | vtable | overridden slots (index: function) |
  |---|---|---|
  | TresProj_MagicBase | 0x1469ADDC0 | none (only `m_AutoStuckShutdownTime = 0.25` in ctor 0x14146FB90) |
  | TresProj_MagicFire | 0x1469B5598 | 541 OnPutEffect 0x141476AC0 |
  | TresProj_MagicBlizzard | 0x1469B2808 | 96 BeginPlay 0x141471FC0, 134 Tick 0x141483FD0, 532 Shutdown 0x1414755C0, 541 OnPutEffect 0x1414757F0, 609 DoFire 0x141474C80, 613 (new) CloseTrail 0x141472810 |
  | TresProj_MagicThunder | 0x1469B7F68 | none; notify interface slot 15 → 0x14147D0F0 |
  | TresProj_MagicWater | 0x1469B9898 | 134 Tick 0x1414841E0; notify slot 15 → 0x14147D400 |
  | TresProj_MagicAero | 0x1469AF3D0 | 570 0x141483D50, 613 (new) SetSwirlCount 0x141482570; notify slot 15 → 0x14147D060 |
  | TresProj_MagicAeroCylinder | 0x1469B0D08 | as Aero + 609 DoFire 0x141474C40 |
  | TresProj_MagicCure | 0x1469B3B38 | 134 Tick 0x1414840A0, 578/579/580/581 target accessors, 613 (new) DoCure 0x1414742A0; created without movement, particle and attack-collision components (ctor 0x14146FD60) |

  So the behaviour of Fire, Blizzard, Thunder, Water, Aero is the generic projectile + the blueprint data; the element
  specific parts are: Fire ground scorch (cosmetic), Blizzard ice rail, Water bolt steering, Aero hit counter, Cure heal.
- `FTresProj_MagicReaction m_Reaction[8]` (+0x808, class + same-team flag) is flagged deprecated and is set in no cooked
  blueprint [A: no `m_Reaction` in any dump]; no code path that reads it was found in the projectile unit [C]. Unused.
- Spell → class: `Blueprints/Player/p_ex001/p_ex001_MagicSet.m_Units` [A] (rows `magicset_*`). The fourth tier
  ("...gan" assets: Figan, Blizzagan, Thundagan, Watagan, Aerogan, Curagan) belongs to the commands
  `TRES_CMD_KIND_S_FIRAGAN` etc.: the top grand-magic situation command. The `S_` commands of the lower tiers reuse the
  normal tier classes (S_FIRA → Fira class, S_FIRAGA → Figa class) and differ only by the attack row (`*_S`). [A]
  The form sets `_bz_`, `_dw_`, `_hm_` map to the `*BZ`, `*DW`, `*CL` classes and the `MAG_B_/MAG_D_/MAG_C_` attack
  rows; they are not documented here (base set only).

### 1. Life cycle of a projectile (generic, all spells)

Public interface used by the spawner (vtable slot: function) [C]:
`TresProjectileManager` slot 241 0x141483480 Spawn(class, owner, firePos, pos, dir, team, bFireNow, flag):
SpawnActor at `pos` with rotation of `dir`; SetTeam (0xD88); slot 577 SetDirection(dir) 0x14147B190:
`Velocity = dir * InitialSpeed`; FinishSpawning; when bFireNow: slot 575 SpawnFireEffects(firePos) 0x141482ED0 (every
`MyFireEffects` entry: particle at firePos with the actor rotation + sound) then slot 590 StartFire.
Other setters: 548 SetFinish(bFinish, bSituation) 0x1414827B0; 578 SetHomingTarget(bOn, actor, part, offset, bLocal)
→ movement 0x1414825F0; 580 SetHomingTargetComponent(bOn, lock-on component) → 0x1414826C0; 584 SetMoveStartDelay;
585 SetDelayFire; 562 SetOwnerParams(data table set, base param); 592 SetParabolaVelocity 0x141472470 (not used by magic data).

1. **PostInitializeComponents** 0x14147FD10 [C]: binds movement `OnTickMove` → `HandleTickMove`, `OnProjectileStop` →
   `HandleOnImpact`; makes the root ignore the instigator while moving unless `m_bEnableOwnerHit`; copies the owner's
   base param and data-table set into the attack collision (the attack rows are looked up in the OWNER's
   `p_ex001_AttackData`).
2. **StartFire** slot 590 0x141474970 [C]: remembers the fire position (+0x6C4, +0x6DC); optional random fire delay;
   then **DoFire** slot 609 0x141475120: movement `SetActive(1)` (flag +0x388 bit0) and `start` (bit1); attack
   collision activated (0x141486890: one activation per `m_CollAutoActivate` entry, or one for all shapes with interval 0;
   attack id = override id (+0x6B0) else the shape's `m_DefaultAttackDataIDName`); state flag +0x6C0 bit0 "fired".
3. **Tick** slot 134 0x141484B10 [C], while fired and not exploded:
   - pending level impact of the previous frame is processed first (step 5);
   - `timeSinceFire` (+0x6D4) += dt, `distance` (+0x6D8) += |pos − lastPos|;
   - slot 538 PutEffect stepping 0x1414863C0: when `m_bCallPutEffectEventCall`, calls OnPutEffect(pos) every
     `m_fPutEffectDuration` cm of travel from the fire position (XY distance when `m_bCallPutEffectDistCheckXY`),
     interpolated along the segment; the first event is at the fire position, or `m_fFirstPutEffectDuration` cm after it
     when that is > 0;
   - stuck test: if `m_AutoStuckShutdownTime > 0` and the movement stuck timer (+0x3A4) ≥ it → life over (flag bit8);
   - life over when `m_MainLife > 0 && timeSinceFire > m_MainLife`, or `m_MaxLifeDistance > 0 && distance > it`, or stuck.
     Then, without `m_bExplodeOnTimeOut`: `bExploded = true`; Shutdown (532), attack collision off (533), detach (534),
     Respawn(class, hit = none) (535), SpawnResidues(none) (536). With `m_bExplodeOnTimeOut`: Explode with a synthetic hit
     at the own position, normal = −forward (so a HIT_ANY respawn happens).
   - after Shutdown the actor lives `m_ExplodeLife` more seconds (0 for all magic) and is then destroyed (+0x758).
4. **Movement component tick** 0x1414854E0 [C] (UE runs it after the owner's tick [U]); skipped while the root does
   not simulate / the actor is paused. Order: move-start delay (0x141487FE0: movement inactive until
   `m_fMoveStartDelay` has elapsed); `OnTickMove`; scale animation; spin; **homing** (§2.2); roll-follows-velocity
   (unused); then the stock `UProjectileMovementComponent::TickComponent` 0x144044AC0 [C, stock structure]:
   `v' = ComputeVelocity(v, dt)` (override 0x1414729E0: gravity·scale on z, `+ normalize(v)·m_Accel·dt`, clamp to
   `MaxSpeed`), `delta = v·dt + (v' − v)·dt/2` (0x141472920), sweep of the root shape, rotation follows velocity
   (`bRotationFollowsVelocity`), blocking hit → `HandleImpact`. No sub-stepping without gravity [U].
   Stuck timer: accumulates while the grazing flag (bit4) is set and the frame displacement ≤ `speed·0.1·dt`, else 0.
5. **Level hit** — `HandleImpact` 0x14147AD80 [I]: with `bShouldBounce` off (all magic): if
   `m_bEnableMapHitNormalCheck` and the hit is not start-penetrating: `d = −(moveDir · ImpactNormal)`;
   `d ≥ cos(m_CheckMapHitNormal)` → `StopSimulating(hit)`; otherwise only flag bit4 is set and the stock code deflects
   the projectile along the surface (`SQEX_bNoReduceVelocityOnDeflect` = 1: speed kept) [U]. Without the normal check
   every blocking hit stops. `StopSimulating` fires `OnProjectileStop` → slot 527 0x14147B120 stores the hit; on the next
   actor Tick slot 528 0x14147ED80: attack collision plays the map hit effect (0x140625170), then **Explode(hit)**.
6. **Character hit** — the attack collision reports through the notify interface (sub-object at +0x448, slot 15,
   base 0x14147D600 [C]): blueprint event `TresBpNotifyAttackHit`; owner notified (unless
   `m_bDisableNotifyParentOnAttackHit`); then `m_bNoDestroyOnHit` ? SpawnResidues(hit) : **Explode(hit)** immediately
   (slot 530 0x14147ED30). Hit results 3..5 (reflected by the victim) instead clear the hit list, switch the team,
   turn the velocity back and re-target the previous owner (not needed unless the host has reflecting enemies).
7. **Explode(hit)** slot 573 0x141479100 [C]: Shutdown (532: notify owner once, deactivate the particle, flag bit4,
   stop the movement), attack collision off, detach children, **Respawn(m_ReSpawnProjectileClass, hit)**,
   SpawnResidues(hit), AttachToHit (only with `m_bAttachToHitActor`), `bExploded = true`.
8. **Respawn** slot 535 0x141477100 [C]: runs once, only when `m_bReSpawnProjectileOnTimeOut` (it is the master enable
   for hits as well) and a class is set, and the hit satisfies `m_RespawnCheckType`: HIT_ALWAYS always; HIT_ANY a hit
   exists; HIT_PAWN a hit exists and it is not level geometry (0x140643CC0); HIT_MAP it is level geometry; NO_HIT no hit
   (time-out). The successor is spawned through the manager at the actor location (slot 611), optionally pushed out of
   walls by `m_ReSpawnProjectileRange` (sphere overlap, channel mask 0xF; only floors with normal.z ≥ 0.71 when
   `m_bReSpawnProjectileOnGround`), with direction per `m_ReSpawnRotInheritType` (NORMAL: forward; YAW_ONLY: forward
   flattened to XY; CLEAR: zero vector), the same owner and team, and it inherits per flag: homing target
   (`TakeOverAtkTarget`), the list of actors already hit (`TakeOverAtkCollHitList`), finish/situation flags
   (`TakeOverAtkCollFinishFlag`), no-shutdown-message, effect colour/alpha. It is fired immediately.
   Residues (`m_ResidueList`, 0x1414882F0) use the same mechanism, once per projectile (flag bit7).
9. **Finish / situation attack rows** 0x1414827B0 [C]: SetFinish(bFinish, bSituation): situation → all shapes use
   `m_OverrideSituationAttackDataID`; else finish → `m_OverrideFinishAttackDataID`; `None` = keep the default row
   (the 4th tier has no finish row: its ids are `None` [A]).

### 2. What the host must answer

#### 2.1 Collision
- Root shape of the moving bolts: sphere radius 25 (`TresRoot.CollisionShapesSrc`, [A]); profile `TresProjectile`
  (`Config/DefaultEngine.ini` line 820 [A]): query only; **blocks** WorldStatic, WorldDynamic, PhysicsBody, Vehicle,
  Destructible, TresGimmick, TresBlockDetect; overlaps TresTrigger; **ignores** Pawn, TresEnemyPawn, TresBodyCollision
  and everything else. Host query: sphere sweep (r 25) from the old to the new position against level geometry,
  returning impact normal and start-penetration. Static spells (Thunder, Aero, burns, barrier) have speed 0: no sweep.
- Attack shapes (`TresAtkColl0.CollisionShapesSrc` [A]; update 0x140647950 [C]): each shape keeps its previous location
  and is swept from it to the new one (`m_bDisableSweep` is false everywhere). Shape meaning [C 0x1406302A0 bounds
  code]: SPHERE radius `size.x`; CAPSULE1 segment along the shape's local Z, half length `size.z`, radius `size.x`;
  CONVEX = mesh × scale (`Collision/tres_colltest01` [A]: 20-sided cylinder radius 100, z −50..+50). Growth:
  `size(t) = max(0, m_Size + m_IncSize·t)`, capped per axis by `m_IncMaxSize` when > 0 [C 0x140649E40];
  a `m_SizeVectorCurve` replaces it (linear keys); `scale(t) = max(0.01, m_Scale + m_IncScale·t)` capped by
  `m_IncMaxScale` when > 0 [C 0x140649BF0]; t = time since activation. For non-convex shapes the size is multiplied by
  the smallest scale component.
  Host query: overlap of that swept shape against enemy bodies → hit actor, hit point, normal.
- `m_MapHitType` NOHIT (all bolts): the attack shape itself never hits level geometry. HITMAP (Fira burn, Thunder base
  capsule that the tiers replace, WaterFire burn, WaterThunder burn): it also reports level contact (hit effect only).
- Hit rules: `m_AttackInterval` is 0 for every magic shape → the periodic re-hit flag is never set
  [C 0x140625500], an actor is hit once per activation; successors inherit the hit list where flagged (so a Blizzard or
  Fire burn does not hit the actor the bolt already hit; Water burns and Fira/Figa/Figan burns — `HitList` true from
  MagFireBase — likewise). Moving bolts die on the first character hit (`m_bNoDestroyOnHit` false) = no pierce.
  Static actors (`m_bNoDestroyOnHit` true) hit every enemy inside once.
  [?] Aero is expected to hit repeatedly (see §3.5): the mechanism that re-arms hits is on the victim side.
- Fire scorch: line trace 200 cm down from the put position (channel mask 0xF) [C]. Blizzard rail: traces down
  100 above → `m_GenerateRailHeight` below the put position, plus wall traces between consecutive rail points [C].

#### 2.2 Homing (steering) — 0x1414751E0 → 0x1414859E0 → 0x141486CB0 → 0x141481160
Runs each movement tick when `m_bEnableHoming`, a target is set (+0x34C bit0) and `m_HomingStartDelay` has run out
(the delay counts down from the first movement tick). Target point: lock-on component → its world aim point (component
vtable+0xB88); else actor → its part position (actor vtable+0x738) + offset; a dead / untargetable target clears the
target flag and the bolt flies straight on. [C]
1. turn rate: `TurnMax += m_HomingTurnAccel·dt`, capped by `m_HomingMaxTurnMax` when > 0 (when no cap and it reaches
   21600 it resets to 0) [C 0x1414859E0];
2. `delta = target − pos`; if `m_HomingMinDist > 0` and `|delta| ≤ m_HomingMinDist` → homing switched off permanently;
3. `Δ = min(TurnMax·dt, 180°)` (if TurnMax = 0: 180°); `v̂`, `t̂` unit vectors; `c = v̂·t̂`;
   `c ≤ cos(m_HomingMaxAngle)` (−1 when the angle is 0) → no change; `c ≥ cos Δ` → `v = t̂·|v|`; else `v` is rotated
   towards `t̂` by `2·atan(sin θ · tan(Δ/2))`, θ = angle(v, t) — the axis `v̂×t̂` is used unnormalised [I 0x141481160:
   no normalisation between 0x141481473 and 0x1414816D0] — speed unchanged.
   Type NORMAL becomes IGNORE_Z (XY only, 0x141486B00) when gravity scale ≠ 0 (never for magic).

#### 2.3 Other services
Targets: the caster passes the homing target; the projectile only needs "aim point of target", "target still valid".
Cure needs "all character pawns of team T" with positions. Effects/sounds by asset name at a position (+ attach for
the actor's own particle), decals for the Fire scorch, the Blizzard trail mesh. Blizzard additionally asks the host for
rail-slide actors (grindable rail segments) — optional gameplay, see §3.2.
No camera call exists in the projectile code read (Tick, Explode, Respawn, notify handlers, element overrides) [C];
camera shake, if any, would be inside the particle systems or the caster state [?].

### 3. The spells (base set). All values [A] unless tagged; see `params.json` for every field.

#### 3.1 Fire — `TresProj_Mag{Fire,Fira,Figa,Figan}Nml` (chain: MagicFire → MagFireBase → tier)
| | Fire | Fira | Firaga (Figa) | grand (Figan) |
|---|---|---|---|---|
| speed / max (no accel, no gravity) | 2000 [I ctor] | 2000 | 2000 | 2000 |
| life | 5 s (≤ 10000 cm) | 5 | 5 | 5 |
| move start delay | 0 | 0 | 0.066666 | 0.066666 |
| homing delay / turn | 0.15 s / 180°/s | 0.15 / 180 | 0.10 / 360 | 0.10 / 360 |
| level hit stops within | 60° of the normal | 60° | 30° | 30° |
| hit sphere radius | 50 | 55 | curve 250 → 60 in 0.07 s, then 60 | curve 250 → 70 in 0.07 s |
| attack row / finish / situation | MAG_FIRE / _FIN / – | MAG_FIRA / _FIN / _S | MAG_FIGA / _FIN / _S | MAG_FIGAN / – / – |
| successor on pawn hit | none | FiraNmlBurn r150, 0.25 s | FigaNmlBurn r250, 0.25 s | FiganNmlBurn r400, 0.25 s |
- One bolt per cast, homing, no pierce, dies on the first enemy or on a level hit steeper than the normal check
  (shallower hits slide along the surface; stuck 0.25 s → time-out). Root sphere r25.
- The explosion ("Burn") is only spawned when the bolt hits a non-level actor (HIT_PAWN) — not on walls, not on time-out
  [C 0x141477100]. It is a static sphere, hits every enemy inside once except the one the bolt hit (hit list
  inherited), uses the same attack row as the bolt (and its finish/situation variant), lasts 0.25 s.
  Fire (tier 1) has no successor class: no explosion.
- Firaga / grand: the bolt is held 0.0667 s at the spawn point while its hit sphere shrinks from 250 → a point-blank
  burst [D].
- `OnPutEffect` 0x141476AC0 [C], every 100 cm of XY travel, first at the fire position: line trace 200 down; on ground
  (and no water volume) spawn `vm_fir_zan1` (scale 1..1.5, life 3 + rand·1 s) and decal `tdc_fir_zmn00a`
  (size 100 + rand·50, life 3, fade 0.125 / 3, random yaw). Cosmetic only — no damage, no burn status.
- Status: none (`m_BadStatusKind` NONE in every MAG_FI* row).

#### 3.2 Blizzard — `TresProj_Mag{Blizzard,Blizzara,Blizzaga,Blizzagan}Nml` (MagicBlizzard → MagBlizzardBase)
| | Blizzard | Blizzara | Blizzaga | grand |
|---|---|---|---|---|
| speed | 2000 | 2000 | 2000 | 2000 |
| life | 5 | 5 | 5 | 5 |
| move start delay | 0 | 0 | 0.066666 | 0.066666 |
| homing | delay 0.10, turn 180 +360/s² up to 360°/s, off within 100 cm of the target | same | same | same |
| level hit stops within | 60° | 60° | 60° | 60° |
| hit sphere | 50 | 55 | curve 250 → 60 | curve 250 → 70 |
| rows | MAG_BLIZZARD / _FIN | MAG_BLIZZARA / _FIN / _S | MAG_BLIZZAGA / _FIN / _S | MAG_BLIZZAGAN |
| successor on pawn hit (yaw kept) | BlizzardNmlBurn: no shape (effect only) | BlizzaraNmlBurn r400 | BlizzagaNmlBurn r600 | BlizzaganNmlBurn r800 |
| burn rows | – | MAG_BLIZZARA_BURN / _F / _S | MAG_BLIZZAGA_BURN / _F / _S | MAG_BLIZZAGAN_BURN |
- Single homing bolt, **no pierce** (dies on the first enemy like Fire); burns last 0.25 s and inherit the hit list.
- Freeze: `m_BadStatusKind` FREEZE in the attack rows — chance `m_BadStatusGenRate` 100 % for bolts, 50 % for burns,
  duration `m_BadStatusEffectTime` 5 / 7 / 10 / 10 s. Chance and duration come only from the attack row; the
  projectile has no status property. (Roll and resistances: strand `mp`.)
- **Ice rail** (native) [C]: BeginPlay spawns a `TresSplineMeshBlizzard_C` trail (`TresDecalMeshSplineActor`) with life
  `m_GenerateRailLife` 3 s, extended every Tick to the bolt position. `OnPutEffect` 0x1414757F0 fires every 300 cm of
  travel (first after 35 cm) and at shutdown: it traces down from 100 above to `m_GenerateRailHeight` 200 below the
  bolt; if ground is found and the path from the previous point is clear it spawns a `ATresRailSlideActor` node (slot
  593 0x141479A60) linked to the previous one — a grindable rail that lives 3 s. DoFire 0x141474C80 disables rails for
  the whole flight when a wall lies between the caster and the spawn point. So: yes, a Blizzard bolt flying within
  2 m of the ground leaves a rail Sora can grind. The exact acceptance tests of 0x1414757F0 (4.8 kB) are only
  summarised — [?] slope / spacing limits (constants 60, 50, 70, 100 seen) need a full read if the rail is ported.

#### 3.3 Thunder — `TresProj_Mag{Thunder,Thundara,Thundaga,Thundagan}Nml` (MagicThunder → MagThunderBase)
Static actor (speed 0, no homing, `m_bNoDestroyOnHit`), spawned by the caster at the target position
(`m_bIsDirectPos` units). No native code of its own. One vertical cylinder that widens:
| | Thunder | Thundara | Thundaga | grand |
|---|---|---|---|---|
| life | 0.6 | 0.4 | 0.5 | 2.0 |
| cylinder radius (100·scale.xy) | 100 → 400 at +1200/s | 400 → 800 at +2400/s | 400 → 1200 at +3600/s | 400 → 1400 at +1400/s |
| height (100·scale.z), centre above actor | 600, +150 | 800, +150 | 1000, +100 | 150, 0 |
| second shape | – | – | – | capsule r 100 → 1400 (+700/s), half length 500 → 1400, centre +500 |
| rows | MAG_THUNDER / _FIN | MAG_THUNDARA / _FIN / _S | MAG_THUNDAGA / _FIN / _S | MAG_THUNDAGAN |
- Every enemy inside is hit once per column. Status DISCHARGE 100 %, 5 / 7 / 10 / 10 s (attack rows).
- [?] `strike_count`, target selection radius, delay between strikes, chaining: not in the projectile — one actor is one
  column. They are decided by the caster (state FireMagic; strand `cast`).
- Notify handler 0x14147D0F0 would draw `m_HitTailEffect` from the column to each victim's "Center" socket; the
  property is null in every cooked thunder blueprint → inactive.

#### 3.4 Water — barrier `TresProj_Mag{Water,...}Nml` → bolt `TresProj_{Water,...}Blt` → splash `TresProj_{Water,...}Burn`
1. **Barrier** (spawned at the caster; static): sphere centred 50 above the actor (world offset), radius
   `10 + 280·t`, no cap, life 0.6 s (grand 0.5) → 178 (150) cm at the end [D]; row MAG_WATER_BARRIER (power 1, no
   reaction value) for all tiers; hits each enemy once; `m_bNoDestroyOnHit`. Whether it follows the caster is the
   spawner's business (the projectile has no attach of its own) [?].
2. At the barrier's time-out (respawn type NO_HIT) the **bolt** is spawned at the barrier position, direction = barrier
   forward, inheriting the homing target and the finish flags: speed 1400, life 5, sphere 50 / 55 / 60 / 70, homing delay
   0.066666 s, turn 360 + 720/s² up to 1440°/s, homing off within 50 / 55 / 60 / 70 cm, level hit stops within 60°,
   dies on hit. Native Tick 0x1414841E0 [C]: with a target, the first time the XY distance is < 800 the turn rate is set
   once by distance: < 350 → 1440, < 550 → 1080, else 720; a target that stops being targetable is dropped. Without any
   target (`m_bSetFallingIfNoHoming`): once the homing delay is over it homes to the point 1000 cm along its own down
   axis — it arcs into the ground.
3. On any hit, enemy or level (HIT_ANY; also when entering a water volume: MAPHITEXPLODE), the **splash** is spawned
   (yaw kept, hit list inherited): Water: no shape (effect only, 0.25 s); Watara sphere 100 → 400 at +1200/s, 0.3 s;
   Wataga 200 → 600 at +1200/s, 0.4 s; grand 200 → 800 at +1600/s, 0.4 s; same row as the bolt.
- No bounce (bShouldBounce false). No status. Rows MAG_WATER / WATARA / WATAGA (+_FIN, _S) / MAG_WATAGAN.

#### 3.5 Aero — `TresProj_Mag{Aero,Aerora,Aeroga,Aerogan}Nml` (MagicAero → MagAeroBase) + `TresProj_MagAeroCylinder`
Static actor at the target position (`m_bIsDirectPos`), life 2 s, `m_bNoDestroyOnHit`; one vertical cylinder:
radius 100 → 400 / 800 / 1200 / 1400 (growth 1200, 1200, 2400, 2400 cm/s), height 400 / 600 / 800 / 1200, centre
+100 / +100 / 0 / +200. Rows MAG_AERO, MAG_AERORA, MAG_AEROGA (+_FIN, _S), MAG_AEROGAN: damage kind
`SWIRL_UP` with `m_DamageEffectTime` 2 / 2.5 / 3 / 4 s (finish +0.5) and phys force level 1 / 2 / 3 / 4.
- The pull-in / lift itself is the victim's SWIRL_UP reaction (enemy side), driven by those two row fields; the
  projectile applies no force [C: no force code in the class]. A host without that reaction must implement
  "hold inside the column for `m_DamageEffectTime`, then launch" itself — radii and forces of the original reaction
  are [?] (enemy damage-reaction code, not read).
- Hit counter (notify 0x14147D060 [C]): every reported hit increments +0x890; when it reaches `m_SwirlLimit`
  (3 / 8 / 14 / none) the victim of that hit gets `m_AfterDamageKind` = BLOW written to pawn+0x9D2 and the limit is
  disabled. Since the shape's hit interval is 0, repeated hits must be re-armed elsewhere — [?] who clears the hit list
  during a swirl (probe: break on 0x14147D060 and log time stamps for one Aero cast).
- Residue `TresProj_MagAeroCylinder` (spawned once, at the first hit or at time-out, at the Aero position): life 6 s
  (ctor 0x14146FAF0), particle `vm_air_zan0`, no attack shape, a `TresChrLauncherComponent` (cylinder radius 100,
  height 50, 20 below the actor, mode TCLT_AERO, launch height 0). When a character is launched by it the blueprint
  deactivates the launcher and ends the projectile (`BP_ReqLifeOver`). It registers with the projectile manager
  (0x141474C40). The launch (height, state) is the launcher / player-state code — [?] not in this strand.

#### 3.6 Cure — `TresProj_Mag{Cure,Cura,Curaga,Curagan}Nml` (MagicCure → MagCureBase)
No movement, no attack shape; root uses profile `TresPrize`. The caster sets the target actor (slot 578 0x141482590
stores `m_pTargetActor`). Tick 0x1414840A0 [C]: on the first Tick after firing with a target: DoCure(target), then
shutdown (life 1 s otherwise).
DoCure 0x1414742A0 [C] (strand `mp` has the same function disassembled):
- `S = max(1, FUN_1405836E0(owner base param))` — caster stat getter (0x1405836E0; [?] which stat: formula strand);
- target (a character pawn): heal `m_AppendCure + S` through pawn vtable+0x1878(amount, 0, 0, effectId, team);
- then every OTHER character pawn of the target's team that is an NPC pawn or a player pawn and whose root lies
  within `m_DetectRange` (3D) of the target: heal `m_BonusAppendCure + S`;
- an NPC whose FUN_1405ABE20(pawn+0x450) returns 0x3B gets half (rounded up).
| | Cure | Cura | Curaga | grand (Curagan) |
|---|---|---|---|---|
| m_DetectRange | 350 | 550 | 800 | 5000 |
| m_AppendCure / m_BonusAppendCure | 80 / 40 | 120 / 60 | 160 / 80 | 9999 / 9999 |
| effect group | CURE (ids 7 / 10) | CURA (8 / 11) | CURAGA (9 / 12) | CURAGA |
The attack rows MAG_CURE / CURA / CURAGA (kind RECOVER, power 0.25 / 0.5 / 1) are not referenced by the projectile
(it has no attack shape) [A]; [?] whether anything else reads them.

#### 3.7 Underwater set (`Projectile/Swim`, commands WATERFIRE / WATERBLIZZARD / WATERTHUNDER / WATERAERO)
- WaterFire: Fire-class bolt, speed 500 accelerating 1350 cm/s² to 3000, sphere 60 → 120 (+300/s), explodes on
  time-out too, successor `MagWaterFireBurn` (sphere 100 → 400 at +800/s, 1 s) on any hit; row MAG_WATERFIRE.
- WaterBlizzard: static 0.5 s cone of three growing spheres at 100 / 350 / 800 cm ahead (caps 250 / 400 / 700);
  MAG_WATERBLIZZARD (FREEZE 100 %, 7 s).
- WaterThunder: homing bolt speed 2700, life 10, sphere 80, successor `WaterThunderBurn` (cylinder radius 400 → 1200,
  height 800, 0.25 s) on any hit; MAG_WATERTHUNDER (DISCHARGE 100 %, 7 s).
- WaterAero: static sphere r500 centred +100, life 1 s, no residue; MAG_WATERAERO.

### 4. Status effects — where the numbers live
Only the attack rows carry them: `m_BadStatusKind`, `m_BadStatusGenRate` (%), `m_BadStatusEffectTime` (s),
`m_BadStatusEffectParam` (0 everywhere). Fire, Water, Aero: NONE. Blizzard: FREEZE. Thunder: DISCHARGE. No projectile
property and no native projectile code touches status. [A]

### 5. Open questions
1. Aero multi-hit: what re-arms the hit list (victim swirl reaction?) and at which rate; pull-in radius/forces of the
   SWIRL_UP reaction; exact use of pawn+0x9D2.
2. Thunder: number of columns per cast and their targets/timing (caster side).
3. Attack-collision manager details shared with melee (0x140618940, 0x14062A920): exact overlap test order, the
   hit-list reset for interval > 0, `m_bAbsoluteOffset1` handling (tagged inferred).
4. Blizzard rail acceptance tests in 0x1414757F0; rail riding itself.
5. AeroCylinder launch (TresChrLauncherComponent, TCLT_AERO, height 0).
6. Stock tick 0x144044AC0 is identified by structure only; deflection path with `SQEX_bNoReduceVelocityOnDeflect`
   (0x14401E730 / vtable+0x518) not read.
7. Whether the Water barrier and the Cure actor are attached to / follow the caster (spawner side).
8. Stat returned by 0x1405836E0; heal clamp and the 0x3B halving condition.
9. Camera shake / rumble: none in projectile code; possibly in particle systems.
10. Root shape of Thunder / Water barrier / Cure blueprints is not set in the chain (native TresRootComponent default).

### 6. Runtime probe requests
| object | offset | type | settles |
|---|---|---|---|
| live `TresProj_MagFireNml_C` → MyMovement (+0x598) | +0x248, +0x24C, +0x214 | f32, f32, vec3 | InitialSpeed/MaxSpeed 2000 and the per-frame velocity (homing turn formula) |
| same movement component | +0x308, +0x2F4, +0x388 | f32, f32, u32 | turn rate, remaining homing delay, flags (bit0 active, bit4 grazing) |
| live Aero actor | +0x888, +0x890 | i32, i32 | swirl limit and hit counter over time (multi-hit rate) |
| attack-collision primitive of a Thunder actor (MyAtkColl +0x8F8 array[0]) | +0x888..+0x890, +0x8A0..+0x8A8, +0xA4C | vec3, vec3, f32 | size / scale growth and the activation clock |
| live `TresProj_MagWaterNml_C` | root location each frame | vec3 | whether the barrier follows Sora |
| live projectile | +0x6C0, +0x6D4, +0x6D8 | u32, f32, f32 | state flags, life timer, travelled distance |
| count of live `TresProj_MagThunder*` after one cast | – | int | strike count per tier |

### 7. What the host must provide (summary)
Sphere sweep vs level (r25, blocking set of profile TresProjectile, with impact normal); swept sphere / capsule /
scaled-cylinder overlap vs enemies with a per-activation hit list; "is this hit level geometry"; target aim point +
validity; line trace down (Fire scorch 200 cm, Blizzard rail 100 up / 200 down); list of character pawns by team with
positions (Cure); heal(amount, effect id); damage application from an attack row (strand `mp`); status application
from the row; SWIRL_UP reaction for Aero; particle / sound / decal playback by name; optional: rail-slide segments
(Blizzard), character launcher (Aero residue), water volumes (Water bolt explodes on entering one).

---

# Part C — MP, damage and cure formulas, abilities, status effects

_Source of this part: `build/sora_tmp/magic/mp/spec.md` (section numbers are local to the part)._

Tags: [I] instruction level (address given), [C] decompiled C only, [A] asset (file and property),
[D] derived by arithmetic, [U] stock UE 4.17, [?] not determined statically.
Units: cm, s, deg. All numbers are rows of `params.json` (same folder). Addresses are KH3.exe VAs.
`ceil(x)` below is the engine idiom `-(cvtss2si(-0.5 - 2x) >> 1)`, `floor(x)` is `cvtss2si(2x - 0.5) >> 1`
(round-to-nearest-even conversion); a port must use these two forms or exact integer equivalents.

### 0. Objects and accessors

- Pawn `ATresCharPawnBase` (Sora = `TresPlayerPawnSora`, vtable 0x14696B930): `+0x4D0` `m_pBaseParam`
  (`UTresChrBaseParam`), `+0x5FC` `m_HitPoint` i32, `+0x604` `m_MagicPoint` i32, `+0x60C` `MyMpChargeTime` f32,
  `+0x614` `m_FocusPoint` i32, `+0x630` u32 flags (bit1 read by vt+0x1740, bit2 "damage invalid"), `+0x7C0` u8 flags
  (bit1 = MP charge), `+0x7C4` f32 MP-charge timer, `+0x85D` u8 bad status kind, `+0x860` f32 bad status time,
  `+0xE18` i32 combo count. [I] 0x1405AB180, 0x1405AD8F0, 0x1405B9C60, 0x1405B7C50, 0x141109050
- Pawn virtuals (slot = offset/8) [I]:

| slot | off | function | meaning |
|---|---|---|---|
| 555 | +0x1158 | 0x1405A8080 | TickMpCharge(dt) |
| 559 | +0x1178 | 0x1405B9C60 | SetMpCharge(bool on, float ratio) |
| 560 | +0x1180 | 0x1405B9D30 | ForceMpZero: MP = 0 and start charge |
| 561 | +0x1188 | 0x1405AB4D0 | GetMpChargeRate() |
| 562 | +0x1190 | 0x141446160 (player) | halve the remaining charge time |
| 563 | +0x1198 | 0x1405B58D0 | OnDamaged MP gain (damage, info) |
| 666 | +0x14D0 | 0x1405AF5B0 | walk regeneration tick |
| 668 | +0x14E0 | 0x1414358B0 → 0x140588690 | HasAbility(id) |
| 669 | +0x14E8 | 0x1405A8C60 | ability MP cost (`TresAbilityDataTable.m_UseMP`, row+0x13) |
| 670 | +0x14F0 | 0x1405A4EF0 | can use ability (MP test) |
| 671 | +0x14F8 | 0x1405BCB70 | use ability (spends MP) |
| 672 | +0x1500 | 0x141424130 → 0x140580810 | ability count(id) |
| 758 | +0x17B0 | 0x1405AADB0 | GetHitPoint |
| 763 | +0x17D8 | 0x1405AB180 | GetMagicPoint |
| 764 | +0x17E0 | 0x1405AD8F0 | IsMpCharge |
| 765 | +0x17E8 | 0x1405AB250 | GetMpChargeTime |
| 766 | +0x17F0 | 0x1405AB4A0 | charge timer (0 when not charging) |
| 767 | +0x17F8 | 0x1405AB420 | charge gauge in MP units |
| 772 | +0x1820 | 0x1405AB200 | GetMaxHitPoint |
| 773 | +0x1828 | 0x1405AB230 → 0x140583B40 | GetMaxMagicPoint |
| 776 | +0x1840 | 0x1405B9700 | SetHitPoint (clamp 0..max) |
| 778 | +0x1850 | 0x1405B9800 | SetMagicPoint |
| 779/780 | +0x1858/+0x1860 | 0x1405A4840 / 0x14144A600 → 0x1405BCDC0 | UseMagicPoint(cost) |
| 783 | +0x1878 | 0x141445F40 → 0x1405B57D0 | Recover(hp, mp, fp, kind, team) |
| 787 | +0x1898 | 0x1405AB950 | apply prize (type, value) |
| 954 | +0x1DD0 | 0x141109050 | combo count (pawn+0xE18) |
| 1036 | +0x2060 | 0x141435A90 → 0x1405AD120 | damage invalid? |
| 1037 | +0x2068 | 0x1405B9120 | set damage-invalid bit (pawn+0x630 bit2) |
| 1192 | +0x2540 | 0x141435AF0 | is dead (state id 59) |

- Blueprint-callable natives: `GetMagicPoint` 0x141D83980 (→ vt+0x17D8), `GetMaxMagicPoint` 0x141D83A50 (→ vt+0x1828),
  `SetMagicPoint` 0x141D84B70 (→ vt+0x1850), `BP_UseMagicPoint` 0x141D83160 (→ vt+0x1858). [I]
- Ability set: `bp+0x130`; count(id) = 0x140E8AAB0: u32 at `set+0x160+id*4`, three 2-bit fields (bits 0-1, 4-5, 8-9; value 3
  counts one) plus bits 18..24. The meal object `bp+0x138` adds one more copy when its `+0x30` equals the id. [I] 0x140E8AAB0, 0x140580810
- Controller (`pawn vt+0x8C8`) byte `+0x95B0`: bit2 = MP usable, bit4 = focus usable. Recover drops the MP part when bit2 is
  clear. [I] 0x141445FA9
- Game options: `UTresGameInstance+0x8660` i32 game level (`ETresGameLevelID`: 0 BEGINNER, 1 STANDARD, 2 PROUD, 3 CRITICAL),
  `+0x8679` u8 battle level, option bit sets tested by 0x140EAB0E0(gi, n) (set A) and 0x140EAB160(gi, n) (set B). [I]
  Uses seen: A0 fixed initial stats, A1 defence 0, A4 MP-charge rate x0.5 (with other conditions), A5 Cure commands disabled,
  B0 attack and magic stat x3. [?] names of the option sets (runtime probe: save object + 0x9570AC, 8 bytes).

### 1. MP storage and max MP

- Current MP: `pawn+0x604` i32. [I] 0x1405AB180
- Max MP = `i16 bp[0x6E] + i32 bp[0x54] + meal`, meal = `i32 [bp[0x138]+0x20]` when > 0, replaced by `ceil(meal*1.25)` when
  ability 171 POWER_LUNCH is counted. [I] 0x140583B40
- `bp[0x54]` = `m_MaxMagicPoint` of the base row (Sora: 100) [A] `p_ex001_BaseParamData.json`; on game level >= 3 and
  character kind 0 it is `floor(100/2) = 50` (max HP is halved the same way). [I] 0x140575D73..0x140575DAA
- `bp[0x6E]` = sum over victory-bonus counts (kinds 8..11): MP_UP3 3, MP_UP5 5, MP_UP10 10, MP_UP20 20; on critical 1, 2, 5, 10.
  [I] 0x140E8CE20, 0x1405864C0. HP kinds 1..5: 3, 5, 10, 15, 30 (critical 1, 2, 5, 7, 15).
- Max MP has no level term: `p_ex001_LevelUpData` has columns m_Exp, m_AttackPower, m_MagicPower, m_DefensePower,
  m_AbilityPoint, m_AbilityKind1..3 only. [A] So the level 1..99 table of max MP is flat:

| condition | max MP (Standard) | Critical |
|---|---|---|
| level 1..99, no bonuses | 100 [D] | 50 |
| + the 7 story rows MP_UP5 | 135 | 64 |
| + the opening choice: Vitality (Vbonus_079, HP only) / Balance (080, MP_UP10) / Wisdom (081, MP_UP20) | 135 / 145 / 155 | 64 / 69 / 74 |
| + the 8 Re Mind rows (MP_UP3) | **159 / 169 / 179** | 72 / 77 / 82 |

  Sora MP rows [A] `TresVBonusData.json`: Vbonus_010, 020, 027, 050, 068, 071, 073 (MP_UP5); the opening choice awards
  exactly one of Vbonus_079 "パラメータ選択・体力" (HP_UP30), 080 "…平均" (HP_UP15 + MP_UP10), 081 "…魔力" (MP_UP20) — level
  blueprint `Levels/ew/ew_01/umap/ew_01_gameflow`, see `docs/sora-stats-spec.md`; VBonus_DLC_002..008 and 010 (MP_UP3).
  (Correction 2026-10-11: the first version of this table summed rows 080 and 081 and gave 165 / 189; the stats agent's
  reading is right. The level-99 loadout sheet `sheets/port/sora-loadout-99.json` uses Balance: 169.)
- Equipment adds no max MP in this function; armour/accessory MP would have to arrive through another path. [?] the stats strand
  owns equipment; `TresEquipItemData` was not read here.
- Meals: `TresFoodItemEffectData` m_MaxMPPlus per dish (0..20) and m_MagicPlus (0..2); see `food.*` rows. [A]
- SetMagicPoint(v) [I] 0x1405B9800: ignored while charging; MP = v < 0 ? 0 : min(v, max); when v < 0 and old MP >= 0:
  SetMpCharge(true, 1.0); then notifies `(pawn+0x448)->vt+0x228(old, new)` (HUD listener).

### 2. MP cost

Row struct `TresCommandKindData` (0x48): `Name` FString +0x08, `Type` u8 +0x18, `Cate` +0x19, `Old` +0x1A, `Attr` +0x1B,
`Local` +0x1C, `Flag` +0x1D, `Sp` +0x1E, `Mp` +0x1F, `Fp` +0x20, `Param0..3` i32 +0x24..+0x30, `LocKey` FString +0x38.
[A] reflection header. Magic rows [A] `sheets/entities/Load/Common/TresCmdData.json` (Type 3, Cate 11; Attr =
`ETresDamageAttribute`):

| id | name | LocKey (kh3_system.command.) | Attr | Sp | Mp | Fp | Param0 | Param1 |
|---|---|---|---|---|---|---|---|---|
| 29 | ファイア | fire | 1 | 12 | 10 | 0 | 200 | 220 |
| 30 | ファイラ | fira | 1 | 15 | 14 | 0 | 300 | 320 |
| 31 | ファイガ | firaga | 1 | 15 | 20 | 0 | 540 | 570 |
| 32 | ファイガン | firagan | 1 | 0 | 0 | 0 | 600 | 600 |
| 33 | ブリザド | blizzrd | 2 | 12 | 12 | 0 | 200 | 220 |
| 34 | ブリザラ | blizzara | 2 | 15 | 17 | 0 | 300 | 320 |
| 35 | ブリザガ | blizzaga | 2 | 12 | 24 | 0 | 540 | 570 |
| 36 | ブリザガン | blizzagan | 2 | 0 | 0 | 0 | 600 | 600 |
| 37 | サンダー | thunder | 3 | 20 | 14 | 0 | 100 | 120 |
| 38 | サンダラ | thundara | 3 | 25 | 20 | 0 | 150 | 170 |
| 39 | サンダガ | thundaga | 3 | 16 | 28 | 0 | 320 | 350 |
| 40 | サンダガン | thundagan | 3 | 0 | 0 | 0 | 400 | 400 |
| 41 | ウォータ | water | 4 | 25 | 11 | 0 | 200 | 220 |
| 42 | ウォタラ | watara | 4 | 15 | 15 | 0 | 300 | 320 |
| 43 | ウォタガ | wataga | 4 | 18 | 22 | 0 | 540 | 570 |
| 44 | ウォタガン | watagan | 4 | 20 | 0 | 0 | 600 | 600 |
| 45 | エアロ | aero | 5 | 10 | 13 | 0 | 50 | 50 |
| 46 | エアロラ | aerora | 5 | 10 | 18 | 0 | 90 | 90 |
| 47 | エアロガ | aeroga | 5 | 15 | 26 | 0 | 190 | 190 |
| 48 | エアロガン | aerogan | 5 | 20 | 0 | 0 | 200 | 200 |
| 49 | ケアル | cure | 7 | 0 | 255 | 0 | 40 | 22 |
| 50 | ケアルラ | cura | 7 | 0 | 255 | 0 | 50 | 38 |
| 51 | ケアルガ | curaga | 7 | 0 | 255 | 0 | 60 | 70 |
| 52 | ホーリー | holy | 7 | 8 | 50 | 0 | 600 | 600 |
| 53 | ウォタファイア | waterfire | 1 | 25 | 10 | 0 | 200 | 220 |
| 54 | ウォタブリザド | waterblizzard | 2 | 25 | 12 | 0 | 200 | 220 |
| 55 | ウォタサンダー | waterthunder | 3 | 25 | 14 | 0 | 200 | 200 |
| 56 | ウォタエアロ | wateraero | 5 | 10 | 11 | 0 | 50 | 50 |

Situation-command (grand magic) rows, Type 4, Cate 11, Mp 0, Param0 20: 110 △ファイラ, 111 △ファイガ, 112 △ファイガン,
113..115 △ブリザラ/ガ/ガン, 116..118 △サンダラ/ガ/ガン, 119..121 △ウォタラ/ガ/ガン, 122..124 △エアロラ/ガ/ガン,
125 △ケアルガン (curagan), 126 △ダークファイガ. Links 69..74 have Mp 255 and Param3 20. [A]
[?] meaning of `Sp`, `Param0`, `Param1` for magic rows (cast strand).

Application [I]:
- `ATresPlayerControllerBase` helper 0x14140D560(ctrl, cmd): pawn = `ctrl+1000`; skipped when `pawn vt+0xD80() == 11`
  (a form id; free magic). If `row.Mp == 0xFF`: `UseMagicPoint(-1)`. Else
  `UseMagicPoint(EffCost(pawn, row.Mp, pawn vt+0x1DD0()))`.
- Called from the FireMagic shoot helper 0x14137BB30 (pointer at 0x14691DB08, call at 0x14137C9AC, once per cast unless
  state flag bit10 "grand magic" is set) and from FireMagic slots 125/126/127 (0x14137FFA0, 0x141380470, 0x14137E110).
  So MP is spent at the shot, not at command acceptance. [I]/[C]
- EffCost 0x1405A4B90(pawn, cost, combo) [I]: if cost > 0: n = count(75 MP_SAVE): if n > 0
  `cost = max(cost - floor(0.2*n*cost + 0.9), 1)`; if HasAbility(56 MAGIC_COMBO_SAVE): `cost = max(cost - combo, 1)`.
  `combo` = `pawn+0xE18`, the running combo counter written by the command dispatcher 0x141437330 (attack cases store
  `counter+1`). [?] how a magic cast advances it (cast strand).
- UseMagicPoint 0x1405BCDC0(pawn, cost) [I]: nothing while charging or when cost == 0. cur = MP.
  cost < 0: SetMagicPoint(-1) (MP 0, charge starts), used = cur.
  cur - cost < 0: if HasAbility(76 MP_LEAVE) and cur > 0: SetMagicPoint(0) (no charge); else SetMagicPoint(-1); used = cur.
  otherwise SetMagicPoint(cur - cost), used = cost. Note: landing exactly on 0 does not start the charge; the next cast does.
  Then n = count(38 WIZZARD_STAR): if used > 0, n > 0 and not dead: r = min(int(rand24 * 2^-21), 7); if r < n + 1:
  HP += used (chance (n+1)/8).
- Cure's "all remaining MP" rule is the 255 (-1) in the `Mp` column; no other code path is involved. [A]+[I]
- Availability 0x14142F330(iface = pawn+0x450, cmd) [C]: needs controller `+0x95B0` bit2; false in states 6, 8, 9; Cure
  commands (49..51, 125) false when option A5 is set; then, unless form id == 11: false while IsMpCharge; with ability 25
  MP_SAFETY and `row.Mp != 255`: false when MP < EffCost, unless ability 76 MP_LEAVE and MP > 0. Without MP_SAFETY a spell can
  be cast with any MP > 0 or even 0 (it then triggers the charge). Cure is never blocked by MP_SAFETY.

### 3. MP charge

- Start: SetMpCharge(true, r) 0x1405B9C60 [I]: no-op if already in that state; if `MyMpChargeTime > 0`: set `pawn+0x7C0`
  bit1 and `timer = GetMpChargeTime()`; otherwise (charge time 0) the off path runs at once.
- GetMpChargeTime 0x1405AB250 [I]: `MyMpChargeTime` (+20 s when the game level is >= 3), 0 when the property is <= 0.
  `MyMpChargeTime` = 50 s for player pawns: constructor 0x141423210 writes 0x42480000 at 0x141423BD4; `p_ex001_Pawn` /
  `p_ex001_PawnBase` do not override it [A]. [?] a class-default override in `TresPlayerPawnSora` native CDO was not seen in
  the store scan (pawn range 0x140540000..0x141460000) — runtime probe: pawn+0x60C f32.
- No per-max-MP scaling: the duration is a time, independent of max MP. [I]
- Tick 0x1405A8080(pawn, dt) [I], called from the pawn tick sub 0x1405BC470 after the walk-regeneration tick and before the
  bad-status tick: if charging and `pawn+0x518` bit0 is clear: if timer > 0: if `(pawn+0x450)->vt+0x760()` (0x14142B970:
  false in states 28, 117, 126, 127, 128 = the link summons, and when vt+0x1ED8() and pawn+0x630 bit1 are both true):
  `timer -= GetMpChargeRate()*dt`; when timer <= 0: SetMpCharge(false, 1.0).
- End: SetMpCharge(false, r) [I]: clear flag, timer = 0, `MP = r >= 1 ? max : trunc(max * clamp(r,0,1))` (r < 0 gives 0).
- Rate 0x1405AB4D0 [I]: `1 + 0.1*count(53 MP_HASTE) + 0.2*count(54 MP_HASTERA) + 0.3*count(55 MP_HASTEGA)`; all three add and
  each stacks per equipped copy. Multiplied by 0.5 when vt+0x1548() (form id not in {5, 7, 11, 14, 18, 19}), option A4,
  pawn+0x630 bit1 and vt+0x1550() (state not 96 or 103) all hold. (The table comment of MP_HASTE says x1.25; the code is 1.1.)
- What shortens it:
  - prizes 0x1405AB950 [I] (type, value): type 2: charging ? timer -= 2 s : MP += 3; type 3: charging ? timer -= 6 s :
    MP += 5 (types 0/1 HP +1/+5, 7/8 focus +3/+5, 4/5/6 munny 1/5/10). The subtraction does not clamp; the next tick ends
    the charge.
  - Recover(.., mp, .., kind 1) 0x1405B57D0 [I]: charging ? `timer = max(timer - mp, 0)` (1 MP = 1 s) : MP += mp. Any other
    kind while charging is lost (SetMagicPoint is ignored). [?] which caller uses kind 1.
  - items 0x1405BE820 [C] (items strand owns): 0x104 Ether: charging ? `timer -= T*0.5*m` : `MP += ceil(100*m)`;
    0x105 / 0x10B: charging ? `timer -= T*m` : `MP += ceil(maxMP*m)`; 0x106/0x107/0x201: charge ended, MP full;
    0xA01: `timer -= T*0.5`. `m = (state 59 ? 0.5 : 1) * (1 + 0.5*count(40 ITEM_UP))`, T = GetMpChargeTime().
  - player vt slot 562 0x141446160 [I]: `timer -= 0.5*timer`. [?] trigger (callers 0x1414393D0, 0x141513E30).
- Damage taken does not shorten the charge: slot 563 returns at once while charging. Hits dealt do not either (no code
  path found). [I] 0x1405B58D0
- During the charge: all magic and link commands are unavailable (section 2), `SetMagicPoint` and `UseMagicPoint` are
  no-ops, MP stays 0.
- HUD values: `vt+0x17E0` charging flag; `vt+0x17F8` = `timer / GetMpChargeTime() * maxMP` (MP units, the part still to
  refill); fraction refilled = `1 - timer/T`; MP change notifications go through `(pawn+0x448)->vt+0x228(old, new)`. [I]
- Other MP sources (not charging) [I]/[C]:
  - damaged, slot 563 0x1405B58D0(damage, info) [I]: n = count(52 DAMAGE_ASPIR): `mp = ceil(damage / maxHP * 100 * n)`;
    by `info.m_DamageAttribute` 1..6 → abilities 84..89 (FIRE/BLIZZARD/THUNDER/WATER/AERO/DARK_ASPIR):
    `mp += ceil(p * 3 * count)`, p = `info.m_AttackSrcPower` (must be > 0) when `info+0x65 == 0`, else 1; Recover(0, mp, 0, 0).
  - walking 0x1405AF5B0 [C], constants [I]: while moving (velocity^2 > 1, 0.25 s grace) a timer runs up to 10 s; from 5 s on
    count(59 WALK_HEALING) gives +1 HP every 1/(n+1) s and count(58 WALK_REGENE) +1 MP every 1/(n+1) s.
  - save point / full recover and Kupo-style revive: items function.

### 4. Magic damage

#### 4.1 Hit record and call
The attack collision component holds the row copy: power f32 at `comp+0x980`, `FTresDamageInfo` (0x68) at `comp+0x9C8`
(reflected [A]): `m_AttackDataIDName` +0x00, `m_CommandKind` +0x08, `m_ShootFlowKind` +0x09, `m_DamageKind` +0x0A,
`m_DamageAttribute` +0x0B, `m_AttackSrcPower` +0x0C, `m_DamagePowerScale` +0x10, `m_DamageParabolaAngle` +0x14,
`m_DamageMoveLength` +0x18, `m_DamageBrakeParam` +0x1C, `m_DamageEffectTime` +0x20, `m_KnockbackType` +0x24,
`m_KnockbackDir` +0x28, `m_BadStatusKind` +0x34, `m_BadStatusEffectTime` +0x38, `m_BadStatusEffectParam` +0x3C,
`m_ReactionPower` +0x44, `m_ArmorAttackPower` +0x48, `m_FormPoint` +0x4C, flags +0x50 (0x01 magic, 0x02 finish, 0x04 killer,
0x08 ignore guard, 0x10 rapid fire, 0x20 critical hit, 0x40 armor damage, 0x80 aerial), flags +0x51 (0x01 attraction,
0x02 counter, 0x04 just guard, 0x08 weak-guard, 0x10 full-MP burst, 0x20 magic draw, 0x40 no-react body correction),
unreflected: four i8 combo values +0x52..+0x55 (each clamped 0..99), hit result +0x60/+0x61, calc kind +0x65,
`m_PhysForceLv` +0x64.
Hit dispatch 0x140618940 [C]: per overlapped body: `m_DamageExecRate` roll (rate < 100: skip when `rate < rand[0,100)`),
bad-status roll (`m_BadStatusGenRate < rand[0,100)` removes the status from this hit), `m_bIsEnableMagicDraw` = attacker has
ability 60 and the attack is magic, `m_bIsCriticalHit` = result of a stub that returns 0 (no critical hits),
reaction resolution 0x14061FE20, then
`dmg = 0x140632240(power = comp[0x980], attackerActor, statActor, victimActor, &info, sameTeam, comp[0x984] bit4)` and
`victim->vt+0x570(dmg, damageEvent, instigator, causer)` (TakeDamage 0x1405BB850).
For projectiles the combo values and the full-MP flag come from the projectile: `proj+0x75C/+0x760/+0x764/+0x768` → info
+0x52..+0x55 and `proj+0x76C` bit0 → `m_bIsEnableFullMpBurst` (0x141488160 → 0x140625500) [C]; the FireMagic shoot helper
sets them with `proj vt+0x1130(state+0xB4)`, `vt+0x1140(state+0xB8, state+0xBC, state+0xC0)`, `vt+0x1160(fullMp)` [C]
0x14137BB30. `fullMp` = FireMagic slot 128 0x141367E80 [I]: the command's effective cost != 0 and MP >= max MP, sampled at
the first shot before MP is spent.

#### 4.2 Formula 0x140632240 [I] (whole function read at instruction level)
```
stat  = info.magic ? MagicPower(statParam) : AttackPower(statParam)          // 0x1405836E0 / 0x140580E70
def   = DefensePower(victimParam)                                             // 0x140581830 (0 if no victim param)
if info.m_DamageKind == 13 (RECOVER): return max(stat,0) (0.1 if 0)           // value handed to TakeDamage as a heal
if sameTeam flag set (and not RECOVER) or power <= 0: return 0.1
if info[0x65] == 1: return ceil(power * victim.maxHP)
if info[0x65] == 2: return ceil(power * victimInterface.vt+0xB8())
base = max(stat,0)
if !noAbil and attacker has ability 100 and victim is in the lock list: base += 5
base = max(base - def, 1)
if victim param:
    lo = victim.DamageMin (bp+0xA4), hi = max(victim.DamageMax (bp+0xA8), lo)
    if lo <= 0: lo = victimKind in {1,5} ? 1 + ceil(battleLevel*0.1) : (battleLevel > 70 ? 13 : 1)
    base = max(base, lo)                                                      // skipped only if the computed lo <= 0
    if hi > 0 and base > hi: base = min(base, max(floor((hi + stat - def)*0.5), lo))
    if battleLevel > 70 and victimKind in {1,5}: hi2 = hi * (optionB0 ? 6 : 2); if base > hi2 and hi2 > lo: base = hi2
    if victimKind in {0,3,4} and base > 80 and lo < 80: base = 80
dmg = ceil((base - 1e-6) * (power - 1e-6))
if victim param:
    c = victim.bp[0x114]; if c > 0 and info[0x51] & 0x40: dmg = ceil((100 - min(c,100)) * 0.01 * dmg)
    r = AttrResist(victim, info.attr)                                         // 0x1405850B0
    dmg = ceil(clamp(100 - r, 0, 200) * 0.01 * dmg)
    if info.rapidFire: dmg = ceil(clamp(100 - victim.bp[0xDC], 0, 100) * 0.01 * dmg)
    if victim.bp[0x89] & 1: dmg = ceil(dmg * 0.25)
dmg = ceil(DifficultyMult(attackerActor) * dmg)                               // 0x140642820
if attacker ability param:
    n = (i8) info[0x54]
    if !noAbil and n > 0:
        magic:    k = count(57 MAGIC_COMBO_UP);                   if k > 0: dmg += ceil(k*n*0.1  * dmg)
        physical: k = count(31 COMBO_UP, or 32 if info.aerial);   if k > 0: dmg += ceil(k*n*0.05 * dmg)
    if info.counter:   k = count(69 COUNTER_UP);                  if k > 0: dmg += ceil(k*0.5 * dmg)
    if info.counter and info.justGuard and count(104) > 0: dmg *= 2
    if attr in 1..5:   k = count(32 + attr)  (33 FIRE_UP .. 37 AERO_UP); if k > 0: dmg += ceil(k*0.2 * dmg)
    if victim.bp[0x7D] == 1: k = count(101); if k > 0: dmg = ceil((1 + 0.3k) * dmg)
    if victim.bp[0x7D] == 2: k = count(102); if k > 0: dmg = ceil((1 + 0.3k) * dmg)
    if attackerInterface.vt+0x98(): dmg = ceil(dmg * 1.5)
    if !noAbil and info.fullMpBurst: k = count(77 FULLMP_BURST);  if k > 0: dmg += ceil(k*0.5 * dmg)
if victim has ability 51 and victim HP% <= 25: dmg = ceil(dmg * (count(51) <= 1 ? 0.5 : 1/3))
return dmg == 0 ? 0.1 : dmg
```
- No random variance and no critical hits. [I]
- DifficultyMult 0x140642820 [I]: reads the game level; attacker "player side" (actor byte +0x3B0 in {1, 10}, or kinds
  3/5/11 resolved through the owner at +0x170 / team id 0): BEGINNER x1.25 player side, x0.5 otherwise; STANDARD x1;
  PROUD x1.5 for non-player attackers; CRITICAL x2 for non-player attackers. Sora's own damage is x1 except x1.25 on Beginner.
- MagicPower 0x1405836E0 [I]: `i32 bp[0x60]` (option A0 and kind 0/3/4: `bp[0x168]`) `+ u8 [set+0x981]`; x3 with option B0;
  `+ i32 [bp[0x140]+0x28]` (weapon); `+ sum over bp[0x148][i >= 3] of i32 [+0x28]` (other equipment); with ability 179
  OVERCOME: `max(., battleTable[lvl].[+0x14] + ceil(0.15*lvl + 1.5))`; `+ meal[+0x28]` (ceil x1.25 with POWER_LUNCH).
  `bp[0x60]` is set from the level row `m_MagicPower` (absolute value) by 0x140576E70 [C]:

| levels | m_MagicPower [A] |
|---|---|
| 1-11 | 12 13 13 14 14 15 16 16 17 17 18 |
| 12-22 | 18 19 19 20 20 21 21 22 22 23 24 |
| 23-33 | 25 25 25 26 26 27 27 28 28 29 29 |
| 34-44 | 30 30 31 31 32 32 33 34 34 35 35 |
| 45-55 | 36 36 37 37 38 38 39 39 40 40 41 |
| 56-66 | 41 42 42 43 44 44 45 45 46 46 47 |
| 67-77 | 47 48 48 49 49 50 50 51 51 52 52 |
| 78-88 | 53 54 54 55 55 56 56 57 57 58 58 |
| 89-99 | 59 59 60 60 61 61 62 62 63 64 64 |

  Level 99: 64 before keyblade / equipment / boost items / meals. [?] growth-type adjustments in 0x1405832D0 (stats strand).
- AttrResist 0x1405850B0 [C]: `i32 bp[0xBC + attr*4] + 100 - ceil(trunc(f*1000)*0.1)`,
  `f = product(1 - i32[item + 0x30 + attr*4]*0.01)` over the weapon (`bp[0x140]`) and equipment entries i >= 3 of `bp[0x148]`.
  Attributes [A] `ETresDamageAttribute`: 0 PHYSICAL, 1 FIRE, 2 BLIZZARD, 3 THUNDER, 4 WATER, 5 AERO, 6 DARK, 7 NOTYPE.
  Sora's own row has all `m_AttrResist*` 0 and no weak points. [A]
- DefensePower 0x140581830 [I]: `bp[0x64]` (+ option handling, + `u8 [set+0x982]`, + meal, + equipment, + 2 at low HP per
  count of ability 50 DEFENDER: thresholds 25/50/75/100 % for 1/2/3/4 copies).

#### 4.3 Attack data rows used by magic
Row struct `TresAttackDataTable` (0x68): m_Power +0x08, m_DamageKind +0x0C, m_DamageAttribute +0x0D, m_DamagePowerScale
+0x10, m_DamageParabolaAngle +0x14, m_DamageMoveLength +0x18, m_DamageBrakeParam +0x1C, m_KnockbackType +0x20,
m_DamageExecRate +0x24, m_DamageEffectTime +0x28, m_BadStatusKind +0x2C, m_BadStatusGenRate +0x30, m_BadStatusEffectTime
+0x34, m_BadStatusEffectParam +0x38, m_TeamCheckType +0x3C, m_ReactionPowerValue +0x40, m_bIsScrumAttack +0x44,
m_ArmorAttackPower +0x48, m_FormPoint +0x4C, flags +0x50 (0x01 magic, 0x02 invalid guard, 0x04 rapid fire, 0x08 killer,
0x10 attraction, 0x20 counter, 0x40 weak guard), m_ReflectLevel +0x54, m_ReflectReaction +0x58, m_RevengePoint +0x5C,
m_PhysForceLv +0x60. [A] Role in the damage function: only m_Power (the multiplier), m_DamageKind (RECOVER special case),
m_DamageAttribute (resist + element boost + aspir), the magic / rapid-fire / counter flags. The others drive reaction,
knockback, armour, revenge, form gauge and status (combat strand).

Base rows (all: ExecRate 100, Team NORMAL, RefLv 0, flags magic|killer|attraction; `G` = also invalid-guard) [A]
`p_ex001_AttackData.json`; full field list for all 199 MAG_/UNISON_ rows is in `params.json` (`atk.<ROW>.<Field>`) and
`atk_magic.tsv`:

| row | Power | Kind | Attr | PScale | Angle | EffTime | BadStatus rate% / time s | React | Armor | FormPt | Revenge | Phys |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| MAG_FIRE | 1.0 | SMALL | FIRE | 1 | 0 | 0 | - | 1 | 2 | 15 | 1 | LV_1 |
| MAG_FIRE_FIN | 1.2 | PARABOLA | FIRE | 0.5 | 75 | 0 | - | 5 | 3 | 18 | 0 | LV_1 |
| MAG_FIRA / _FIN / _S | 1.5 / 1.8 / 3.0 | SMALL / PARABOLA / PARABOLA | FIRE | 1 / 0.6 / 0.6 | 0 / 75 / 75 | 0 | - | 1 / 5 / 5 | 2 / 3 / 10 | 15 / 18 / 0 | 1 / 0 / 0 | LV_1 |
| MAG_FIGA / _FIN / _S | 2.2 / 2.6 / 4.4 | SMALL / PARABOLA / PARABOLA | FIRE | 1 / 0.7 / 0.7 | 0 / 75 / 75 | 0 | - | 1 / 5 / 5 | 2 / 3 / 10 | 15 / 18 / 0 | 1 / 0 / 0 | LV_1 |
| MAG_FIGAN (G) | 6.6 | PARABOLA | FIRE | 1 | 0 | 0 | - | 5 | 10 | 0 | 0 | LV_1 |
| MAG_BLIZZARD / _FIN | 0.8 / 0.95 | SMALL / PARABOLA | BLIZZARD | 1 / 0.5 | 0 / 75 | 0 | FREEZE 100 / 5 | 1 / 5 | 2 / 3 | 12 / 14 | 1 / 0 | LV_1 |
| MAG_BLIZZARA / _FIN / _S | 1.2 / 1.4 / 2.4 | SMALL / PARABOLA / PARABOLA | BLIZZARD | 1 / 0.6 / 0.6 | 0 / 75 / 75 | 0 | FREEZE 100 / 7 | 1 / 5 / 5 | 2 / 3 / 10 | 12 / 14 / 0 | 1 / 0 / 0 | LV_1 |
| MAG_BLIZZAGA / _FIN / _S | 1.8 / 2.15 / 3.6 | SMALL / PARABOLA / PARABOLA | BLIZZARD | 1 / 0.7 / 0.7 | 0 / 75 / 75 | 0 | FREEZE 100 / 10 | 1 / 5 / 5 | 2 / 3 / 10 | 12 / 14 / 0 | 1 / 0 / 0 | LV_1 |
| MAG_BLIZZARA_BURN(_F,_S), MAG_BLIZZAGA_BURN(_F,_S), MAG_BLIZZAGAN_BURN | as the non-BURN row | | | | | | FREEZE 50 / same time | | | | | |
| MAG_BLIZZAGAN (G) | 5.4 | PARABOLA | BLIZZARD | 1 | 0 | 0 | FREEZE 100 / 10 | 5 | 10 | 0 | 0 | LV_1 |
| MAG_THUNDER / _FIN | 1.2 / 1.4 | SMALL / BEAT | THUNDER | 1 | 0 | 0 | DISCHARGE 100 / 5 | 1 / 5 | 2 / 3 | 6 / 7 | 1 / 0 | LV_1 |
| MAG_THUNDARA / _FIN / _S | 1.6 / 1.9 / 3.2 | SMALL / BEAT / BEAT | THUNDER | 1 | 0 | 0 | DISCHARGE 100 / 7 | 1 / 5 / 5 | 2 / 3 / 10 | 6 / 7 / 0 | 1 / 0 / 0 | LV_1 |
| MAG_THUNDAGA / _FIN / _S | 2.4 / 2.85 / 4.8 | SMALL / BEAT / BEAT | THUNDER | 1 / 2 / 2 | 0 | 0 | DISCHARGE 100 / 10 | 1 / 5 / 5 | 2 / 3 / 10 | 6 / 7 / 0 | 1 / 0 / 0 | LV_1 |
| MAG_THUNDAGAN (G) | 7.2 | TOSS | THUNDER | 1 | 0 | 0 | DISCHARGE 100 / 10 | 5 | 10 | 0 | 0 | LV_1 |
| MAG_WATER_BARRIER | 1.0 | SMALL | WATER | 1 | 0 | 0 | - | 0 | 1 | 8 | 1 | LV_1 |
| MAG_WATER / _FIN | 0.9 / 1.05 | SMALL / PARABOLA | WATER | 1 / 0.5 | 0 / 75 | 0 | - | 1 / 5 | 2 / 3 | 12 / 14 | 1 / 0 | LV_1 |
| MAG_WATARA / _FIN / _S | 1.4 / 1.65 / 2.8 | SMALL / PARABOLA / PARABOLA | WATER | 1 / 0.6 / 0.6 | 0 / 75 / 75 | 0 | - | 1 / 5 / 5 | 2 / 3 / 10 | 12 / 14 / 0 | 1 / 0 / 0 | LV_1 |
| MAG_WATAGA / _FIN / _S | 2.0 / 2.4 / 4.0 | SMALL / PARABOLA / PARABOLA | WATER | 1 / 0.7 / 0.7 | 0 / 75 / 75 | 0 | - | 1 / 5 / 5 | 2 / 3 / 10 | 12 / 14 / 0 | 1 / 0 / 0 | LV_1 |
| MAG_WATAGAN (G) | 6.0 | PARABOLA | WATER | 1 | 0 | 0 | - | 5 | 10 | 0 | 0 | LV_1 |
| MAG_AERO / _FIN | 0.6 / 0.7 | SWIRL_UP | AERO | 1 | 0 | 2 / 2.5 | - | 1 / 5 | 2 / 3 | 6 / 7 | 1 / 0 | LV_1 |
| MAG_AERORA / _FIN / _S | 1.0 / 1.2 / 2.0 | SWIRL_UP | AERO | 1 | 0 | 2.5 / 3 / 3 | - | 1 / 5 / 5 | 2 / 3 / 10 | 6 / 7 / 0 | 1 / 0 / 0 | LV_2 |
| MAG_AEROGA / _FIN / _S | 1.4 / 1.65 / 2.8 | SWIRL_UP | AERO | 1 | 0 | 3 / 3.5 / 3.5 | - | 1 / 5 / 5 | 2 / 3 / 10 | 6 / 7 / 0 | 1 / 0 / 0 | LV_3 |
| MAG_AEROGAN (G) | 4.2 | SWIRL_UP | AERO | 1 | 0 | 4 | - | 5 | 10 | 0 | 0 | LV_4 |
| MAG_CURE / CURA / CURAGA | 0.25 / 0.5 / 1.0 | RECOVER | NOTYPE | 1 | 0 | 0 | - | 0 | 0 | 0 | 0 | LV_1 |
| MAG_WATERFIRE | 1.5 | SMALL | FIRE | 1 | 0 | 0 | - | 1 | 2 | 0 | 1 | LV_1 |
| MAG_WATERBLIZZARD | 1.5 | SMALL | BLIZZARD | 1 | 0 | 0 | FREEZE 100 / 7 | 1 | 2 | 0 | 1 | LV_1 |
| MAG_WATERTHUNDER | 1.5 | SMALL | THUNDER | 1 | 0 | 0 | DISCHARGE 100 / 7 | 1 | 2 | 0 | 1 | LV_1 |
| MAG_WATERAERO | 1.5 | SWIRL_UP | AERO | 1 | 0 | 0 | - | 1 | 2 | 0 | 1 | LV_1 |
| UNISON_FIRE (G) | 1.6 | SIDESPIN | FIRE | 1 | 0 | 0 | - | 1 | 3 | 8 | 0.5 | LV_2 |
| UNISON_BLIZZARD / _END (G) | 0.6 | SMALL / SIDESPIN | BLIZZARD | 1 | 0 | 0 | FREEZE 100 / 10 | 1 | 3 | 6 | 0.5 | LV_2 |
| UNISON_THUNDER (G) | 1.6 | BEAT | THUNDER | 2 | 0 | 0 | DISCHARGE 100 / 10 | 1 | 3 | 8 | 0.5 | LV_2 |

`_FIN` = combo finisher shot, `_S` = situation-command (grand magic) tier, `MAG_B_/C_/D_*` = 135 variant rows with other
powers (e.g. MAG_B_FIRE 1.3, MAG_B_FIGAN 8.55, MAG_C_FIRE 0.5) referenced by other projectile blueprints
(`atk_magic_variant_diff.txt`). [?] which blueprint uses which variant (proj strand); the unprefixed rows are assumed to be
the Kingdom Key base form.
Example [D]: Firaga at level 99 (magic 64) against a victim with defence 20, no clamps, 0 resist, Standard:
base 44, dmg = ceil(43.999999 * 2.199999) = 97; with one FIRE_UP: 97 + ceil(0.2*97) = 117.

### 5. Cure

- The three commands spend all MP (section 2) and spawn `ATresProj_MagicCure` (blueprints
  `Blueprints/Projectile/MagicCure/TresProj_MagCure{Nml,..}`); the heal is applied by its slot 613 0x1414742A0(this, target):
  [I] `magic = max(MagicPower(proj+0x4D0 param), 1)`; target: `heal = m_AppendCure + magic`; if the target is an NPC in state
  59 (DIE) the heal is halved (`-(cvtss2si(-0.5 - heal) >> 1)`); `target->Recover(heal, 0, 0, kind, team)` with kind 7 / 8 / 9
  (Cure / Cura / Curaga; 14 D_CURE, 16 CURE_SPRAY). If `m_DetectRange > 0`: every other pawn (player or NPC class) of the same
  team id (`vt+0xD90`) whose root location is within `m_DetectRange` (3D) gets `m_BonusAppendCure + magic` (halved in state 59)
  with kind 10 / 11 / 12.
- Values [A]: Cure 80 / 40 / 350 cm, Cura 120 / 60 / 550 cm, Curaga 160 / 80 / 800 cm, Curagan 9999 / 9999 / 5000 cm
  (append / bonus append / range). Level 99 Curaga on Sora: 64 + 160 = 224 HP before equipment. [D]
- Recover → SetHitPoint clamps to max HP. The tier multipliers of the MAG_CURE rows (0.25 / 0.5 / 1.0, kind RECOVER) are not
  used by this path; they only matter if the projectile's attack collision hits something, in which case the damage function
  returns the caster's magic stat and TakeDamage 0x1405BB850 adds `ceil(value)` HP [C]. [?] whether that collision is ever
  active for Sora's Cure (proj strand).
- The target (`m_pTargetActor`, slots 578/580) is chosen by the cast code (cast strand).
- Leaf Bracer [I]: FireMagic Begin 0x14136A8E0 (slot 96) computes state flags `+0xD8`: bit14 = HasAbility(73
  AUTO_LOCK_MAGIC), bit15 = HasAbility(82 ENDLESS_MAGIC), bit10 = slot 122 0x14136F020(cmd) (grand magic / forced-invincible
  commands 32, 36, 40, 44, 48, 110..126, 231), bit11 = slot 123 0x14136F000 (cmd in 49..51 or 125), bit12 = slot 124
  0x14136F130 = HasAbility(43 LEAF_VEIL) and bit11. If bit10 or bit12: `pawn vt+0x2068(1)` sets `pawn+0x630` bit2, which makes
  `vt+0x2060` (0x1405AD120) return true = damage invalid. Cleared with `vt+0x2068(0)` in End 0x14136CA50 (slot 98) and when the
  shot sequence finishes (0x14137BB30, 0x14137E110). So the invulnerability lasts for the whole FireMagic state of a Cure.
- Cure Converter (103 POWER_CURE) [I] 0x14137C8F5: at the Cure shot, when `pawn+0x630` bit1 is set and n = count(103) > 0:
  `controller vt+0x1A80(15 + ceil((15n + 20) * MP / maxMP), 1, cmd)` before the MP is spent (form-gauge points per the table
  comment).

### 6. Status effects

- Kinds [A] `ETresBadStatusType`: 0 NONE, 1 DEATH, 2 CATCH, 3 DRILL_BIND, 4 YO_BIND, 5 RALPH_BIND, 6 ENERGYBURST, 7 FREEZE,
  8 STOP, 9 MAGNET, 10 DRAIN, 11 ASPIR, 12 STUN, 13 SNEEZE, 14 HONEY, 15 CLOUD, 16 DISCHARGE, 17 BURN, 18 HP_BREAK,
  19 TIME_BREAK.
- What Sora's magic inflicts [A]: Blizzard family FREEZE (100 %, 5 / 7 / 10 s; `_BURN` rows 50 %), Thunder family DISCHARGE
  (100 %, 5 / 7 / 10 s), underwater Blizzard / Thunder 7 s; Fire, Water, Aero, Cure none. `m_BadStatusEffectParam` 0 on all.
  Aero rows carry `m_DamageEffectTime` 2..4 s (SWIRL_UP hold time).
- Chance [C] 0x140618940: one roll per hit on the attacker side: if rate < 100 and `rate < rand[0,100)` the status is removed
  from the hit record. Duration = `m_BadStatusEffectTime`, no scaling. Victim side [I]/[C] TakeDamage → 0x1405A2350:
  resist test `vt+0xF50(kind)` (0x1405ADBF0: pawn+0x634 bit0, state-interface checks, BURN refused when vt+0x1990() is true,
  then abilities via 0x140589610: FREEZE 66, STUN 68, SNEEZE 65, CLOUD 64, DISCHARGE 67, BURN 63); if not resisted and kind
  != FREEZE: SetBadStatus 0x1405B7C50(kind, time, info) stores `pawn+0x85D` / `+0x860` (kinds 1..5, 9..11 store 0); FREEZE is
  entered through the damage reaction (state 123).
- Timer [C] 0x1405AFF90(pawn, dt) (after the MP-charge tick): STOP and HONEY count real time, STUN only while state 122,
  BURN only while state 124 (and ends at once when vt+0x1990()), FREEZE is not counted here; at 0 the status is cleared
  (0x1405A5C40). Player states: `TresPlayerState_BadStatBurn` (slot 95 0x141352D30, Begin slot 96 0x14134E6E0, Tick slot 97 0x1413530B0 over 0x14150E960, End slot 98 0x141350220),
  `_BadStatFreeze`, `_BadStatStun` (combat / states strands).
- [?] burn tick damage: none in 0x1405AFF90 or 0x14150E960; runtime probe below.
- Buffs that alter magic / MP [A]: meals `TresFoodItemEffectData` (m_MaxMPPlus 0..20, m_MagicPlus 0..2 per dish; x1.25
  with POWER_LUNCH [I]); full-course bonus ability by level `TresFullcourseAbilityList`: 0 LUNCH_TIME, 1 POWER_LUNCH,
  2 OVER_TIME / PRIZE_FEEVER / FIRAGAN / BLIZZAGAN, 3 BEST_CONDITION / EXP_BARGAIN / THUNDAGAN / WATAGAN / AEROGAN,
  4 MILLIONAIRE / CHARGE_BERSERK, 5 GRAND_MAGIC / CURAGAN. Nothing in these tables changes focus.

### 7. Abilities relevant to magic / MP

AP and ids [A] `TresAbilityData.json` / `ETresAbilityKind`; effect and test address [I] unless marked.

| id | key (table comment name) | AP | effect | tested at |
|---|---|---|---|---|
| 25 | MP_SAFETY (ＭＰセーフティー) | 0 | spells costing more than MP are unavailable (not Cure) | 0x14142F4AE, 0x1405A4F6C, 0x1414040DD |
| 33..37 | FIRE/BLIZZARD/THUNDER/WATER/AERO_UP | 4 | dmg += ceil(0.2 n dmg) for that attribute | 0x1406329BD |
| 38 | WIZZARD_STAR (ウィザードスター) | 0 | chance (n+1)/8: HP += MP spent | 0x1405BCE61 |
| 40 | ITEM_UP | 2 | item effect x(1 + 0.5 n) [C] | 0x1405BE820 |
| 43 | LEAF_VEIL (リーフベール) | 1 | damage invalid during a Cure cast | 0x14136F145 |
| 51 | CRITICAL_HALF | 4 | damage taken x0.5 (x1/3 with 2) at HP <= 25 % | 0x140632B3A, 0x140632B64 |
| 52 | DAMAGE_ASPIR | 3 | MP += ceil(dmg/maxHP*100*n) when hit | 0x1405B5928 |
| 53 / 54 / 55 | MP_HASTE / HASTERA / HASTEGA | 3 / 4 / 5 | charge rate +0.1 / +0.2 / +0.3 per copy | 0x1405AB4EB / 0x1405AB50B / 0x1405AB52F |
| 56 | MAGIC_COMBO_SAVE | 3 | cost -= combo count, min 1 | 0x1405A4C11 |
| 57 | MAGIC_COMBO_UP | 5 | dmg += ceil(0.1 n comboN dmg) | 0x1406328DE |
| 58 / 59 | WALK_REGENE / WALK_HEALING | 1 / 4 | +1 MP / +1 HP every 1/(n+1) s after 5 s of movement | 0x1405AF608 / 0x1405AF5F5 |
| 60 | MAGIC_DRAW | 3 | sets `m_bIsEnableMagicDraw` on magic hits (prize attraction; not damage) | 0x1406195FB |
| 63..68 | BURN/CLOUD/SNEEZE/FREEZE/DISCHARGE/STUN_GUARD | 0 | resist that status | 0x140589610 (0x140589750) |
| 72 | MAGIC_TIME | 3 | situation-command duration of magic commands (count read) [C], magnitude not read | 0x14140DD2D |
| 73 | AUTO_LOCK_MAGIC | 1 | FireMagic flag bit14 (use in the cast strand) | 0x14136AA3E |
| 75 | MP_SAVE | 0 | cost -= floor(0.2 n cost + 0.9), min 1 | 0x1405A4BBC |
| 76 | MP_LEAVE | 3 | insufficient MP with MP > 0: MP goes to 0 without charge; lets MP_SAFETY pass | 0x1405BCE1D, 0x14142F4F6, 0x1414040EF |
| 77 | FULLMP_BURST | 2 | dmg += ceil(0.5 n dmg) when cast at full MP | 0x140632AE3 |
| 82 | ENDLESS_MAGIC | 0 | FireMagic flag bit15 (cast strand) | 0x14136AA69 |
| 84..89 | FIRE/BLIZZARD/THUNDER/WATER/AERO/DARK_ASPIR | 0 | MP += ceil(3 n srcPower) when hit by that attribute | 0x1405B59E7 |
| 101 / 102 | HEARTLESS / NOBODY_BUSTER | 0 | dmg x(1 + 0.3 n) vs that victim type | 0x1406329FF / 0x140632A52 |
| 103 | POWER_CURE | 1 (max 3) | Cure Converter, section 5 | 0x14137C901 |
| 171 | POWER_LUNCH | 0 | meal bonuses x1.25 | 0x140583B8C, 0x140583874 |
| 177 | CURAGAN | 0 | △Curagan offer chance [C] | 0x1413EDD1A |
| 179 | OVERCOME | 0 | stat floor by battle level | 0x1405837D0 |
| 180 | GRAND_MAGIC | 0 | grand-magic roll multiplier x1.3 [C] | 0x1413ED608 |
| 181..185 | FIRAGAN/BLIZZAGAN/THUNDAGAN/WATAGAN/AEROGAN | 0 | +15 points per copy and offer the -gan tier [C] | 0x1413EDB2D / 9F0 / 8C7 / 76A / 691 |
| 186 | MAGIC_ROULETTE | 0 | calls controller vt+0x1A30 before the rolls [C] | 0x1413ED64E |
| 190..192 | UNISON_FIRE/BLIZZARD/THUNDER | 3 | count read in 0x141430A60 (team attacks; not analysed) | 0x141430AD0.. |

Grand magic offer [C] controller 0x1413ED500(ctrl, alt, forced): M = 10 (20 when `alt`), `int(M*1.3)` with GRAND_MAGIC;
per element: `p = int(points[element]) + 15*count(-GAN ability)`; offered when `rand() % 1000 < p*M` and the pawn's
`vt+0xD70()` is 1 or 3 (7 also for water); the offered command is the next tier above the last cast tier (e.g. Aero 45 →
122, Aerora 46 → 123, otherwise 124); Curagan: chance value 25 (50 when vt+0x2538()) + 10 per KO party member, needs
ability 177 (or HP < 51 % in the forced variant); at most 3 offers (2 when one is already forced). [?] how `points[element]`
(floats at `*(ctrl+0xB98)`) accumulate (cast strand).
Listed only (no magic / MP effect found or not Sora's): 15 MAGICFLUSH (finisher count, 0x141433B53), 31/32 COMBO_UP /
AIRCOMBO_UP (physical, x0.05), 69 COUNTER_UP (x0.5), 104 CRITICAL_COUNTER (x2), 100 JOIN_FORCE (+5), 61 MASTER_DRAW,
78 HARVEST, 79 HP_CONVERTER, 80 MP_CONVERTER, 81 MUNNY_CONVERTER, 83 FP_CONVERTER (no call site with an immediate id was
found for 78..81, 83; [?]), 170 LUNCH_TIME, 172 OVER_TIME, 173 BEST_CONDITION, 199..243 friend abilities with `m_UseMP`
(10 / 15 / -1) spent through slots 669..671.
Level-up grants of magic abilities [A]: L2 MAGIC_COMBO_SAVE, L4/6/12 MP_HASTE, L9/14/16 FIRE_UP, L12/18/28 WATER_UP,
L14/26/28 LEAF_VEIL, L18/30/38 BLIZZARD_UP, L20/42/44 MAGIC_TIME, L26/46/50 MAGIC_COMBO_UP, L34/40 THUNDER_UP, L38/48 AERO_UP,
L44/46 MAGIC_COMBO_SAVE (level:column 1/2/3 of m_AbilityKind1..3; which column applies is the stats strand's question [?]; `levelup.tsv`).

### 8. What the host must provide

For each magic hit the simulation delivers a hit record (= `FTresDamageInfo` + power):
`power` (row m_Power), `attack_id` (row name), `command` (ETresCommandKind), `damage_kind` (SMALL, PARABOLA, BEAT, TOSS,
SWIRL_UP, SIDESPIN, RECOVER), `attribute` (0..7), `is_magic`, `is_rapid_fire`, `is_aerial`, `ignore_guard`, `combo_n`,
`full_mp`, `power_scale`, `parabola_angle`, `move_length`, `brake`, `effect_time`, `knockback_type` + direction,
`bad_status` (kind, time, param; already rolled), `reaction_power`, `armor_attack`, `form_point`, `revenge_point`,
`phys_force_lv`, attacker handle, and the attacker-side numbers needed by the formula: magic stat, ability counts
(33..37, 57, 77, 101, 102), difficulty multiplier.
The host answers per victim: defence (int), `DamageMin` / `DamageMax` (0 = none), victim kind (enemy 1/5 vs player-side
0/3/4), resist percent for the attribute (int, 0 default; > 100 nullifies, negative = weakness up to x2), rapid-fire resist
percent, combo-resist value (bp+0x114 analogue, 0 default), "x0.25" flag (bp+0x89 bit0, default false), victim type
(1 Heartless / 2 Nobody / other), current HP % and CRITICAL_HALF count (only for player-side victims), whether the victim
is on the attacker's team, and whether it accepts the bad status (immunity). The simulation then computes the integer
damage with section 4.2 and reports it; the host applies HP loss, reaction and status.
For Cure the host needs nothing except party members' positions, team and KO state if allies are to be healed.
The host must also supply: battle level (u8; 1 when unused), game level (0..3), and a uniform 24-bit random source
(the game uses one xorshift128 stream, 0x142CFF460).

### Open questions
1. Names of the game-option bit sets A/B and of option A4, and the meaning of pawn+0x630 bit1 (gates MP rate x0.5, Cure
   Converter, charge pause).
2. Which projectile blueprint uses the MAG_B_/C_/D_ rows; whether the MAG_CURE collision is ever active.
3. `combo_n` (FireMagic state+0xB8 → +0xBC): how a magic combo advances it and its range; pawn+0xE18 for magic casts.
4. `Sp`, `Param0`, `Param1` of the command rows.
5. Callers of Recover kind 1 and of slot 562 (halve charge).
6. Burn tick damage; duration scaling of FREEZE on the victim side (state code).
7. MAGIC_TIME magnitude; code sites of HARVEST and the *_CONVERTER abilities.
8. Whether all nine base-game Sora MP bonus rows are actually granted; equipment MP.
9. Attacker interface vt+0x98 (x1.5) and victim bp+0x89 bit0 (x0.25) conditions.
10. How `points[element]` for grand magic accumulate.

### Runtime probe requests
| object | offset | type | settles |
|---|---|---|---|
| Sora pawn | +0x604, +0x60C, +0x7C0, +0x7C4 | i32, f32, u8, f32 | MP, base charge time (expect 50), flag bit1, timer countdown and rate with MP Haste |
| Sora pawn `+0x4D0` → bp | +0x54, +0x6E, +0x60, +0x64 | i32, i16, i32, i32 | max MP parts and magic / defence at level 99 |
| bp `+0x138` (meal) | +0x1C..+0x30 | i32 x6 | meal bonuses layout |
| Sora pawn | +0x630 | u32 | bit1 meaning, bit2 during Cure with Leaf Bracer |
| Sora pawn | +0xE18 | i32 | combo counter during a magic combo |
| Sora pawn | +0x85D, +0x860, +0x5FC | u8, f32, i32 | burn duration and any tick damage |
| FireMagic state | +0xB0, +0xB4..+0xC4, +0xD8 | i32 x6, u32 | command id, combo values, flags (bit17 full MP) |
| hit AtkColl component | +0x980, +0x9C8 (0x68 bytes) | f32, FTresDamageInfo | power and info at a magic hit |
| UTresGameInstance | +0x8660, +0x8679, save+0x9570AC (8 bytes) | i32, u8, bytes | game level, battle level, option bits |
| controller | +0x95B0, *(+0xB98) (6 floats) | u8, f32 x6 | MP/focus enable bits, grand-magic points |

---

# Part D — Shotlock, focus gauge and airstep

_Source of this part: `build/sora_tmp/magic/shotlock/spec.md` (section numbers are local to the part)._

> **Superseded statements (second pass, 2026-10-11).** Parts F (aim mode), G (shotlock attacks, focus syphon) and
> H (airstep) re-read this material at instruction level and govern wherever they differ. Known corrections:
> - S1.1: the int16 at BaseParam+0x70 has no writer: max focus is a constant 100. The focus-empty flag clears only when
>   focus reaches the maximum. S1.2: the +7 per second regeneration is game option B6, not an ability; focus prizes give
>   +3 / +5 (FP_CONVERTER turns small ones into large ones). FOCUS_BOOST (42) is read by no code.
> - S2: the shotlock key is `Gamepad_RightShoulder` (a release within 0.2667 s is the lock-on tap); the airstep key is
>   `Gamepad_FaceButton_Left`. No state is pushed and no aim animation is played (`ShootFlowIdle` is unused); the
>   input dispatch is skipped while aiming; in the air Sora falls normally on the slowed clock. List entry +4 is
>   `m_MoveFlowType` and +0x14 is `m_ShootLockOnRange`. `0x4D9E` is an object-type mask of the per-type time dilation
>   (player, enemies, gimmicks, map … at 0.4; projectiles, camera and UI are not slowed), not an id. The flag-2
>   handshake is controller slot 809 (0x14140E720) called from the states' End, not `FUN_14140E500` (a separate 0.1
>   slow motion). Aim time reaching 0 only stops new locks; a cancel costs no focus.
> - S3: the formation is a cone of half-angle 60° around the axis pawn → blade tip (socket `ATK_01`), rotating
>   180°/s, not a horizontal ring; sub-step 3 falls through into sub-step 4; `KC_SHOT` flies at a constant 1500 cm/s
>   (4500 is a cap), its spin fields are dead data; `TresProjKC_Ragnarok` is the native class `TresProj_Ragnarok`
>   (helix around the path), 24 projectiles in one volley, no finishing burst. Sora is invincible from Begin to End.
> - S4: the selection range is `MyAthleticLockonLimit` unless the component carries its own; `MyAthleticLockonRange`
>   3750 is a sentinel. In flight the speed is max(6·dist, MaxFlySpeed) and 3000 cm/s inside the last 600 cm; the
>   formula given here is the launch speed. `AthleticSlideEnd` is the arrival hop at an enemy and deals no damage.
>   The camera is a separate class `ATresCameraAthleticFlow`.


Read by the coordinating agent itself (no sub-agent slot was free). Depth: the controller aim mode, the kind
selection, the focus accounting, the Kingdom Key data and the airstep speed formula are settled; the inner
functions of the projectile class, the focus-syphon shotlock and most of the airstep state are mapped but not
read (list in "Open questions").

Naming, established from the code: `TresPlayerState_ShooterMode` (state id 40) is **not** the shotlock aim
mode — it is the gun mode of the formchanges (animations `ShooterModeStart/…`, projectile names `DW_GATLING`,
`BZ_CRACKER`) [C 0x1413B0DC0]. The shotlock is "ShootLock" / "ShootFlow": the aim mode lives in the player
controller, the attack is `TresPlayerState_ShootFlow` (id 41), the airstep is `TresPlayerState_AthleticFlow`
(id 42) started with shootflow kind 37.

### S1. Focus gauge

#### S1.1 Storage and limits
- `ATresCharPawnBase::m_FocusPoint`, int, pawn+0x614 [A reflection]. Getter pawn vt+0x1808 (0x1405AAD70),
  maximum vt+0x1830 (0x1405AB1E0) = `(int16)[BaseParam+0x70] + (int32)[BaseParam+0x58]`, `BaseParam` =
  `m_pBaseParam` [C]. Base value 100 (`p_ex001_BaseParamData.m_MaxFocusPoint`) [A]. No victory bonus kind
  raises focus (`ETresVictoryBonusKind` has HP, MP, slots and magic only) [A]; the int16 bonus source is not
  traced [?].
- Setter pawn vt+0x1868 (0x1405B94F0) [C]: `new = clamp(v, 0, max)`; notifies the HUD interface
  (pawn+0x448 vt+0x230(old)); if `old >= 1 && v <= 0` and pawn vt+0x1800() (constant true for player pawns,
  false for the base class) → vt+0x1160(1); if vt+0x1170() and `new >= max` → vt+0x1160(0).
  vt+0x1160 (0x14054E980) writes and vt+0x1170 (0x140545D70) reads **pawn+0x7C0 bit 0 = "focus empty"** [C].
  While that flag is set and Sora has the ability FOCUS_ASPIR (46), holding the shotlock button opens the
  focus-syphon mode instead of the normal aim mode (S2.2); a non-Sora character cannot enter the aim mode at
  all while the flag is set.
- The same byte's bit 1 is the MP-charge flag (strand mp).

#### S1.2 Gains
- Controller vt+0x1988 (slot 817, 0x1413E8CA0) `AddFocus(n)` [C]: ignored while the aim mode is active
  (ctrl+0xA08 bit 0); otherwise `Set(Get + n)` for n > 0.
- Per hit dealt: pawn hit handler 0x141442DE0 calls `AddFocus(MyFocusPointAddHit)` when that tunable is > 0
  [C]. **For Sora it is 0** (native constructor 0x141423D45, not overridden by either blueprint class default
  object) [I, A] → no focus per hit.
- Timed regeneration: controller slot 634 (0x1413F8EF0) adds 7 points every 1.0 s while game flag 6 is set
  (`FUN_1405AD350(pawn, 6)` → game instance), the pawn is not restricted and not aiming [C]. Which feature sets
  game flag 6 is open [?] (candidates: an EZ code / the ability FOCUS_BOOST path).
- `MyFpChargeAddTime` / `MyFpChargeInterval` (native 1 / 0.1) are 0 / 0 in `p_ex001_PawnBase` [A]: readers not
  traced.
- Prizes and items: `Recover(hp, mp, fp, kind)` 0x1405B57D0 → `Set(Get + fp)` [C] (items: part "Items").
- A victim's damage does not appear in the focus path [C: no other caller of the setter: callers are
  0x1405B57D0, 0x1413E8CA0, 0x1413EAAA0 (enable/disable focus: 0 or max), 0x14140CCB0, 0x14140F1F0].

#### S1.3 Cost
The focus cost of a shotlock is **the time spent aiming** [C 0x1413EC850, 0x1413FD3D0, 0x14140F1F0; I for the
constant]:
```
at aim start:   focus0 = focus;  aimTime = focus0 * MyFocusPointConvertRate        // 0.05 s per point
each tick:      aimTime -= dt / actorTimeDilation                                   // real time, floor 0
at release:     focus = int(aimTime / 0.05)        // = focus0 - consumed            (kinds 36 and 37: unchanged)
```
100 focus = 5.0 s of aiming; aiming burns 20 points per real second; releasing immediately costs almost
nothing. When `aimTime` reaches 0 no further lock can be added (S2.3) but the mode stays open while the button
is held. The `Fp` column of `TresShootFlowData` (20 for both Kingdom Key rows) is not read by the controller,
pawn or state code (scan for `[base + index*8 + 0x20]` in 0x141280000–0x141470000 and 0x140540000–0x1405C0000
[I]); only the HUD function 0x1415D1CE0 tests it against 0.

### S2. Aim mode (player controller)

`ATresPlayerControllerSora` vtable 0x146969B98, slot 800 (vt+0x1900) = 0x1414165E0 `(bHold, dt)`, called from
the controller tick [C; every constant below I]. Controller fields:

| Offset | Meaning |
|---|---|
| +0x8E8 | pointer to the shootflow command table (`TresShootFlowData`, rows of 0x48 bytes: Type +0x18, Cate +0x19, Fp +0x20, Param0..3 +0x24..+0x30) |
| +0x980 | lock range, 5000.0 (reset 0x1414162A0 @0x141416419; setter 0x141411290 = `BP_SetShootLockonLimit`) |
| +0xA08 | flags: 1 aim active · 2 release pending (cleared by the state) · 8 ignore the aim timer · 0x10 exited, wait for button release · 0x20 pad to maximum · 0x40 MAX reached · 0x80 special lock routine (kinds 29 and 39) |
| +0xA0C | hold timer |
| +0xA10 | mode: 0 off, 1 aiming, 2 shotlock fired, 3 airstep requested, 4 focus-syphon aiming |
| +0xA18 / +0xA1C | total locks / number of locked targets |
| +0xA24 | lock interval timer |
| +0xA28 | maximum locks = `Param0` of the current kind |
| +0xA38 | start delay (0.1 s) |
| +0xA3C | aim time left (S1.3) |
| +0xA40 | slow-motion rate 0.4 (reset 0x1414162A0 @0x1414164DA) |
| +0xA4C | current `ETresShootFlowKind` (+0xA34: debug override) |
| +0xB28 / +0xB2C | focus consumed (−1 = none) / focus at entry |
| +0xAE8 (+0xAF0 count) | candidate list; +0xAF8 (+0xB00) airstep candidates; +0xB08 (+0xB10) locked list; +0xB18 (+0xB20) airstep targets |
| +0x868 | `m_CameraShootFlow` (spawned at aim start) |

List entry (0x34 bytes): +0 distance (−1 until measured), +4 byte from component vt+0xAD0, +8 state (1
candidate, 2 locked), +0xC lock count, +0x10 marker timer, +0x14 move-flow type (component vt+0xAF8),
+0x1C weak pointer to the `TresLockonTargetComponent`.

#### S2.1 Entry (mode 0)
The hold flag passed in is forced false when [C]: controller vt+0x1B78(); request interface vt+0x618(); pawn
swimming-type checks (vt+0x1C30 / !vt+0x15D0 when `FUN_1405AD820`); pawn vt+0x1F88() (restriction SHOOTLOCK,
pawn+0xE60 bit 10) or vt+0xA48(); !pawn vt+0x29C0(); controller vt+0x1A70() == 0x8B; current state id in
{6, 22, 31..40, 122..124, 138, 142, 168}; JUMP with sub-step 9; DAMAGE with sub-step ≠ 1; some flags of states
97 and 102; next queued state id 32; controller vt+0x1B48(). Then, with hold true, pawn vt+0x1E10() false and
vt+0x2788() == 0: return if flags & 4, ctrl+0x878 & 8, flags & 0x10; return if the focus-empty flag is set and
the character is not Sora; return if controller vt+0x1B78 / 0x1BF0 / 0x1BD0 / 0x1C30.
`holdTimer += dt`; when it exceeds **0.26666668 s** [I 0x14141908D] the mode opens:

1. flags |= 1; candidate list = every lock-on target component of the world (manager vt+0x9D0) with component
   vt+0xAB0() true (shotlock permission), owner team ≠ own team, not tagged `Gimmick` when controller
   vt+0x17C0(), and `|pos − eye| < range` (pos = component vt+0xB88, eye = pawn vt+0x6E0). The owner pawn gets
   vt+0xBC8(2, 1.0).
2. airstep candidate list = components with their move-flow permission bit and vt+0xAC0() true.
3. kind selection `FUN_141411B80` (S2.2); `maxLocks = table[kind].Param0`.
4. normal case: mode = 1; S1.3 start (slot 821 0x1413EC850); startDelay = 0.1; flag 0x80 for kinds 29 / 39;
   if aimTime > 0: world slow motion `FUN_1405DA460(pawn, 1, 0x4D9E, 0.4, 0)`.
   Focus-empty case (Sora): mode = 4; `aimTime = maxFocus * 0.05`.
5. camera: spawn `TresCameraShootFlow` (`FUN_1413E6790`), vt+0x788(pawn, controller), `FUN_140627E30(cam,
   {…, 50.0}, 0)`, `FUN_140626FC0(cam, (50, 100, 0), 0)`, vt+0x790(1, {0.5, 3, 2.0, 2}) [C; meaning of the
   blocks: camera strand]. HUD: `FUN_141587480(hud, aimTime, bSyphon, kind)`, `FUN_141584140(hud, 1)`.

If hold is false in mode 0: holdTimer = 0, flags &= ~0x11.

#### S2.2 Kind selection (0x141411B80) [C]
- focus-empty flag and ability 46 → kind 36 (focus syphon);
- character Aqua → 38, Riku → 43, Kairi → 39; Sora: debug override ctrl+0xA34, else by weapon kind
  (pawn vt+0x12C8(−1)): **1 (Kingdom Key) → 1**, 2 → 3, 4 → 8, 5 → 10, 6 → 12, 7 → 14, 8 → 20, 9 → 18,
  10 → 16, 11 → 23, 12 → 25, 13 → 40, 14 → 41, 15 → 29, 16 → 30, 17 → 32, 18 → 27, 19 → 34, others → 5;
- when `total locks >= maxLocks > 0`: flag MAX, and for Sora the kind becomes the second shotlock of the
  weapon: **1 → 2**, 2 → 4, 3 → 6 or 7, 4 → 9, 5 → 11, 6 → 13, 7 → 15, 8 → 21 or 22, 9 → 19, 10 → 17,
  11 → 24, 12 → 26, 13/14 → 42, 16 → 31, 17 → 33, 18 → 28, 19 → 35; the HUD is told (`FUN_14158C380`).

Kingdom Key rows of `TresShootFlowData` [A]: kind 1 `KCショット`, text key `kh3_system.command.sf_magic_0_1`,
Param0 24; kind 2 `KCラグナロク`, `kh3_system.command.sf_ragnarok_0`, Param0 24. So: locking fewer than 24
times fires kind 1; reaching 24 locks turns the release into kind 2 (Ragnarok). (English names "King of
Hearts" / "Ragnarok" are the localisation of those keys; the localisation tables were not opened.)

#### S2.3 Aiming tick (mode 1)
```
if startDelay > 0: startDelay -= dt / actorTimeDilation; return
aimTime -= dt / actorTimeDilation (slot 825 0x1413F9930)
drop locked entries whose component died, lost permission, changed team or left the range  (total -= its count)
if (aimTime > 0 or flags&8) and not restricted:
    scale = min(W * 0.00078125, H * 0.0013888889)                    // viewport / (1280 x 720)
    if CanAddLock (0x1413F0320: focus != 0 — in syphon mode: ability 46 — and not MAX):
        lockTimer = max(0, lockTimer - dt / 0.4)
        for each candidate, round robin from index ctrl+0x964:
            if lockTimer > 0: continue
            if entry.dist == -1: d = |pos - eye|; skip if d > range; entry.dist = d
            (sx, sy, w) = project(pos)  (FUN_140ECF670);  skip if w <= 0
            if (sx - cx)^2 + (sy - cy)^2 <= scale^2 * 22500 and LineOfSight(component) (0x1413F0A80):
                entry.state = 2; entry.lockCount += 1; entry.markerTimer = 0.08
                lockTimer = Param0 >= 1 ? 2.24 / Param0 : 0.08
                first lock on this target → append to the locked list
                total += 1; HUD markers (FUN_1415759C0, FUN_1415895F0(total, max)); round-robin index advances
canShoot = total > 0;  canAirstep = AirstepTargetUpdate(dt) (0x14141FDC0);  HUD FUN_14158B420(0, canShoot), (1, canAirstep)
if not bHold: Exit (slot 813 0x1413FAD90); return
if attack key pressed (FKey 0x1487D7760) and total >= 1:
    mode = 2; flags |= 2; (flag 0x20: pad the last candidate up to max and set MAX)
    remove airstep markers; if TryShootFlow (request interface slot 95 0x1414413A0): StartCommand(kind) else Exit
else if airstep key pressed (FKey 0x149C70748) and airstep targets >= 1:
    mode = 3; flags |= 2; clear locks and markers;
    if TryAthleticFlow (slot 97 0x1414372F0): kind = 37; StartCommand(37) else Exit
```
[C for the structure; I for 0.00078125, 0.0013888889, 22500 (0x1414184CB), 2.24 (0x14141871C), 0.08
(0x1414186F0, 0x141418733), 0.1 (0x141419705), division by ctrl+0xA40 (0x141418486).] The reticle is a circle
of radius 150 px (at 1280×720) around the viewport centre. Kingdom Key: one lock every 2.24/24 = 0.0933 s of
real time while any candidate is inside the circle; a target may be locked repeatedly.

Focus-syphon mode (mode 4) is the same loop with: per-target timer `dist * 0.0002` s instead of the global
interval [I 0x1414171F9], MAX at 18 locks, marker type 3, and release requires the focus-empty flag [C].

`StartCommand(kind)` = `FUN_14140F1F0` [C]: focus update of S1.3 (not for 36 / 37); controller vt+0x1520(1.0);
pawn vt+0x1D98(0xFD, kind) (command-begin event; 0xFD is the command id of shootflow actions); ctrl+0x880 =
0xFD, +0x884 = kind, +0x8F4 = 0.1; `FUN_1405DA4E0(pawn, 1, 1, 1.0, 0)` (slow motion off); for kind 37 a new
slow motion of 0.1 (`FUN_1405DA460(pawn, 1, 0x4D9E, 0.1, 0)`); camera end vt+0x798(1, {0.5, 3, 2.0, 2}).

`Exit` (0x1413FAD90) [C]: flags = (flags & ~1) | 0x10, mode = 0, counters 0, slow motion off
(`FUN_1405DA3E0(pawn, 1, 0)`), all markers removed, owners vt+0xBC8(2, 0), lists emptied, camera ended.
Mode 2 / 3 wait until the state clears flag 2 (`FUN_14140E500(ctrl, 0)` at the end of the fire animation),
then do the same clean-up.

Gate of the release, `CanShootFlow` (slot 94, 0x14142D800) [C]: pawn vt+0x29C0(); vt+0x2770() == 0;
!vt+0x2540(); !vt+0xA48(); !interface vt+0x618(); (!`FUN_1405AD320(pawn, 8)` or world id 11);
vt+0x2788() == 0; current state not 6 and not 40; controller has ≥ 1 locked entry whose component is alive.

#### S2.4 Pose while aiming
`ShootFlowStart` (body/1451, 0.6667 s), `ShootFlowIdle` (1452, 0.8 s loop), `ShootFlowEnd` (1453, 0.8 s,
`TresEnableChange` at 0.300) are in the main anim set [A]. `ShootFlowEnd` is played by the shotlock state
(S3). Who plays Start / Idle during the aim mode (controller vt+0x18B8 / vt+0x1780(0) / pawn vt+0x2110 at
entry) was not read [?]; movement and gravity while aiming likewise [?].

### S3. Shotlock attack: `TresPlayerState_ShootFlow` (state 41)

Vtable 0x14693CA58 (super `TresCharState_Attack`): Reset 0x1413AD3E0, Begin 0x1413A5CD0, Tick 0x1413B0D10,
End 0x1413ABA70, 55 more virtuals (slots 113..167). Anim-interface vtable (state+0x40) 0x14693CF98:
`OnTiming` 0x1413B57D0.

- **Begin** [C]: pawn vt+0x1780(1), vt+0x16E8(), vt+0x1738(1) (battle mode), vt+0x1340(1, 1, 1.0),
  vt+0x2068(1), vt+0x19E0(1); state+0x88 = command (0xFD), +0x8C = kind; +0x94 = total locks (controller
  vt+0x1918); no locked entry → finish; `m_pComponent` = first locked component; +0xB8 = projectile name;
  the per-kind update function is stored as a delegate at state+0x170:
  kind 1 → vt+0x408 (slot 129, 0x1413B8C90), name `KC_SHOT`; kind 2 → vt+0x410 (slot 130, 0x1413B88F0),
  name `KC_RAGNAROK`; kind 36 → vt+0x448 (slot 137, 0x1413B78B0). (43 kinds are wired there; names such as
  `HE_DRAIN`, `SF_SHOOTINGSTAR`, `UW_ULTIMA` give the keyblade.)
- **Tick** [C]: pawn vt+0xDC8(dt·12), vt+0xDB8(dt·12); delegate(dt); vt+0x388 (0x1413C51D0: faces the target
  component while flag 8 of state+0xCC is set).

#### S3.1 Kind 1 (King of Hearts) — 0x1413B8C90 [C]
| Sub-step | Action |
|---|---|
| 0 | pawn vt+0x1728(0), vt+0x17A0(5, 1), vt+0x1388(1, 0, 0); play `M_1473` (body/1473, 2.5 s) once; `m_bFaceAt` = 1; → 4, or → 3 when vt+0x398(500.0) returns true |
| 3 | vt+0x3A0() true → 4 |
| 4 | animation ended: vt+0x3F0 (0x1413C3330: volley set-up), play `M_1474` (body/1474, 0.7 s); → 5 |
| 5 | vt+0x400 (0x1413C3A80: ring formation); animation ended: controller `FUN_14140E500(ctrl, 0)` (clears release-pending), play `ShootFlowEnd`; `m_bEnableChange` = 0; → 2 |
| 2 | animation ended: pawn vt+0x1388(0, 0, 0); finish (DoChain) |
| every tick | vt+0x3F8 (0x1413C1350: spawning) |

Volley [C]: set-up stores `remaining = total locks`, `perTick = 7`, origin = pawn vt+0x1410() (a point on the
keyblade side: not resolved [?]), ring timer 0.5 s. Spawning: while `remaining > 0`, up to 7 projectiles per
tick; projectile i is created with pawn vt+0x1450(name `KC_SHOT`, origin, origin, direction, team, 0) where the
direction has yaw = (direction origin→pawn) + i·360/N and pitch 60°; its target is
`locked[(spawnIndex) mod lockedTargets]` (round robin over the locked **targets**, not weighted by per-target
lock count); projectile vt+0x1100(command), vt+0x1220(1), vt+0x11F8(origin), vt+0x1270().
Ring formation (0.5 s): each projectile is placed at origin + dir_i · (0.5 − t)·400 (radius grows to 200 cm),
oriented outward, vt+0x1208(dir), vt+0x1240(t + 0.066666) (launch delay).

Projectile `TresProjKC_Shot` (`Blueprints/Projectile/ShootFlow`, class `TresProjectileBase`) [A]: life 5 s;
attack collision sphere r 30, profile `TresProjectile`, no map hit, attack row `SF_KCSHOT`; root sphere r 10;
movement: start delay 0.533333 s, initial speed 1500, max speed 4500, homing on (start delay 0.033333 s,
minimum distance 100, turn max 180, turn acceleration 360, maximum turn max 360), spin roll 30 for 0.1 s;
trail `vw_sor_rbs_blt4`; hit effect `vw_sor_rbs_hit0` + `se02001_020`; map hit `vw_sor_rbs_wht0` +
`se02001_021`. Attack row `SF_KCSHOT` [A]: power 0.8, attribute NOTYPE, damage kind SIDESPIN, magic attack,
rapid fire, killer attack, armor attack 1, revenge 0.1, force level 2.

#### S3.2 Kind 2 (Ragnarok) — 0x1413B88F0 [C]
Same shape with `M_1481` (2.3333 s) and `M_1482` (1.0 s), set-up vt+0x418 (slot 131 → the same 0x1413C3330),
spawning vt+0x420 (0x1413C0D60), formation vt+0x428 (0x1413C34D0); for weapon kinds 1, 0x16, 0x17 the weapon
glow pawn vt+0x1498(5, 1.0) is on from sub-step 0 until `m_bEnableChange` of the end animation.
Projectile `TresProjKC_Ragnarok` [A]: life 8 s; attack sphere r 50, row `SF_KCRAGNAROK` (power 0.9, NOTYPE,
damage kind TOSS, magic, rapid fire); movement: start delay 0.533333, initial speed 1000, acceleration 500,
homing turn max 720, spin roll 30; trail `vw_so010_rgn_bst00`, hit `vw_so010_rgn_hit00`, map hit
`vw_so010_rgn_wht00`. Slots 132 / 133 were not read: count and pattern of the Ragnarok volley are open [?].

#### S3.3 Timing gauge
Controller 0x1413F86C0 (from the controller tick) [C]: while ctrl+0x8F9: `g += dt * ctrl+0x900`; rank 1 below
0.7, 2 below 0.9, 3 below 1.0, 4 from 1.0, 5 from 1.1 (flag 0x20 forces 3); on the attack key or rank 5 the rank
is shown (`FUN_141577490`) and written to state+0x160. In 0x1413C3330 the rank selects `Param1..3` of the row
as the shot count (rank 3 → Param1, 2 → Param2, else Param3) — only rows 38 and 43 have those columns, so the
Kingdom Key shotlocks do not use the gauge [A].

### S4. Airstep: `TresPlayerState_AthleticFlow` (state 42)

- Request (S2.3): airstep key in the aim mode with ≥ 1 airstep target → `TryAthleticFlow` (slot 97,
  0x1414372F0) → `CanAthleticFlow` (slot 96, 0x141426280): pawn vt+0x29C0(); vt+0x2770() == 0; !vt+0x2540();
  character is Sora; vt+0x2788() == 0; state not 6 / 40; controller airstep-target count > 0 → PushState(0x2A)
  [C]. No focus is consumed (S1.3). `MyFocusPointNeedAF` (10) has no reader on this path [?].
- Targets: lock-on target components with the move-flow permission (`m_bPermissionMoveFlow`,
  `m_MoveFlowType`, `m_bMoveFlowDestination`), searched by the controller within `MyAthleticLockonLimit`
  15000 (getter 0x141108E50, used by 0x141410E50) / `MyAthleticLockonRange` 3750 (0x141410F00) [I for the
  values; search not read].
- State: vtable 0x146901158: Reset 0x14133CFB0, Begin 0x141332F90, Tick 0x141341270, End 0x141334B60,
  virtuals 113..128. Begin [C]: pawn vt+0x16E8(), vt+0x17A0(5, 1), vt+0x1340(1, 1, 1.0), vt+0x1388(1, 0, 0),
  vt+0x1728(0), vt+0x2068(1), vt+0x19E0(1); target list = controller +0xB18; start position = actor location;
  state+0x12C = `MyAthleticAttackSlowRate` 0.25, +0x130 = `MyAthleticAttackSlowTime` 0.05; vt+0x398(0)
  selects target 0. Tick: finish when the target list is empty; pawn vt+0xDC8(dt·12), vt+0xDB8(dt·12);
  delegate at state+0x1E0.
- Travel (slot 116, 0x141348BA0) [I 0x141348D01..0x141348D9D, 0x141349A60..0x141349B42]:
  at launch: slow motion off; state+0x8C = actor yaw; `T` = `MyAthleticSlideTime` (1.0); pose variant by the
  pitch of (target − start): > 45° → 3, < −30° → downward variant, else 1;
  each tick: `dist = |target − ref|` (target = vt+0x3D8, ref = pawn vt+0x2110),
  **`speed = max(2·dist / T, min(dist · 60, MyAthleticSlideSpeed))`**, `MyAthleticSlideSpeed` = 6000 cm/s.
  So 6000 cm/s up to 3000 cm, faster beyond (the trip never takes more than about 0.5 s plus the approach),
  and `60·dist` inside the last 100 cm.
  Animations [A]: `AthleticSlide` (1156, 0.7333 s), `AthleticSlide2` (1153, 1.0 s), `AthleticSlide3` (1150),
  `SlideEnd` (1157, 0.7667 s), `AthleticSlideEnd` (1163, 0.6333 s, `TresEnableChange` 0.2667), `WallKickF`
  (1169, 0.5 s, `TresEnableChange` 0.4674), `PoleTurn(Loop)` (1172 / 1173), `PoleSwing(Loop)` (1174 / 1175).
  Arrival kinds (debug strings in the function): END, ENEMY, KICK, NEXT, POLE.
- Effects [A]: `m_AthleticRoot` = `Blueprints/Player/AthleticFlowRootEffect`, `m_pAthleticHitEffect` =
  `Effects/PC/Free_f/par/vp_ffa_arv0`, trail `vp_ffa_rad0` on the slide animations, lens effect
  `Effects/PC/shootLock/bp/BP_slc_camatch00`.

### Open questions (shotlock / focus / airstep)
1. Source of the int16 focus bonus at BaseParam+0x70; what sets game flag 6 (7 focus per second).
2. Aim-mode pose, movement and gravity (who plays `ShootFlowStart` / `ShootFlowIdle`); camera parameter blocks.
3. `LineOfSight` 0x1413F0A80 (trace channel, start point) and the airstep target search 0x14141FDC0 /
   0x141410E50 / 0x141410F00.
4. `TresProjectileBase` motion model with the `TresProjectileMovementComponent` fields (homing formula) —
   see the projectile part for the magic classes; the shootflow projectiles use the base class.
5. Ragnarok volley (slots 132, 133), focus syphon (slot 137), what `vt+0x398(500)` / `vt+0x3A0` do in
   sub-step 3, pawn vt+0x1410 (origin).
6. The whole airstep state apart from the speed formula: arrival distances, the ENEMY / KICK / NEXT / POLE
   branches (0x141347750, 0x141346600), invulnerability flags (pawn vt+0x2068 / vt+0x1388), collision during
   travel, the 0.25 × 0.05 s hit slow motion.

### Runtime probe requests
| Object | Offset | Type | Settles |
|---|---|---|---|
| pawn | +0x614 | i32 | focus value; watch while aiming and at release (S1.3) |
| pawn | +0x7C0 | u8 | bit 0 focus-empty, bit 1 MP charge |
| pawn → m_pBaseParam | +0x58 i32, +0x70 i16 | | max focus parts |
| controller | +0xA08 u32, +0xA10 i32, +0xA18 i32, +0xA24 f32, +0xA28 i32, +0xA3C f32, +0xA40 f32, +0xA4C i32 | | aim-mode state, lock cadence, slow rate, kind |
| controller | +0x980 | f32 | lock range in the level under test |
| controller | +0xB10, +0xB20 | i32 | locked targets / airstep targets |
| state ShootFlow | +0x78 (sub-step), +0x94, +0x9C, +0xA0 | i32, i32, i32, f32 | volley progress |
| state AthleticFlow | +0x78, +0x90, +0x98, +0x140 (vec3) | | travel parameters and target point |
| pawn | velocity during airstep | vec3 | speed formula |

### What the host must provide (shotlock / airstep)
- The list of lock-on target components: position (the component's look-at point), team, alive flag,
  permissions (lock, shotlock, move-flow + type), owner pawn handle.
- Camera eye position, view-projection and viewport rectangle (screen test), and a line-of-sight query between
  the eye and a target point.
- A world time-dilation hook with an id (0.4 while aiming, 0.1 at the airstep start, off otherwise); the aim
  timers run in real time (dt divided by the dilation).
- Two extra buttons: shotlock hold (R1) and airstep (Square); the attack button releases.
- Projectile hosting: spawn by name from the weapon's projectile set, per-projectile target component, sweeps
  of a 30 / 50 cm sphere against pawns only (no map hit), hit delivery with the attack row.
- For the airstep: target point of a component, sweep or teleport of the pawn along a straight line (to be
  confirmed), arrival callbacks.

---

# Part E — Items and the command-execute path

_Source of this part: `build/sora_tmp/magic/items/spec.md` (section numbers are local to the part)._

> **Superseded statements (second pass, 2026-10-11).** Part J (command menu, shortcuts, situation slots, pouch) governs
> wherever it differs. Known corrections: `ExecuteCommand` 0x14140EAA0 is the only writer of ctrl+0x87C (press frame,
> no buffer); usable pouch slots = 3 initial + 1 per ITEM_SLOT_UP1 (6 after the base game, 8 with Re Mind); flag bits:
> 0 exists, 1 fixed, 2 auto-reload, 3 used; `FUN_140E86A80` at save+0x12C2670 is a usage counter, the stock is a u8 per
> item at save+0x86B4 and pouch items are already out of stock; the refill runs when the game leaves a pause and at pawn
> set-up, not at battle end; the sub-menu slow motion (0.3, object-type mask 0x4F9E) exists only with the config option
> "Command Selection: Slow". Restriction bits of pawn+0xE60: 0 attack, 1 magic, 2 items, 3 situation, 4 link.


Read by the coordinating agent itself. The item state, the gate, the effect amounts and the pouch model are
settled from decompiled C with the constants checked in the disassembly; the command-menu widget
(`TresUIP_HudCommand`), shortcut storage and the pouch refill after battle were not read.

### I1. From a selected command to the pawn

`ATresPlayerControllerSora` slot 678 (vt+0x1530) = 0x14141B470 `ExecuteCommand` [C]. The selected command is
`ctrl+0x87C` (an `ETresCommandKind` = row id of `Load/Common/TresCmdData`, 254 rows of `FTresCommandKindData`,
0x48 bytes: Name +8, Type +0x18, Cate +0x19, Old +0x1A, Attr +0x1B, Local +0x1C, Flag +0x1D, Sp +0x1E,
Mp +0x1F, Fp +0x20, Param0..3 +0x24..+0x30, LocKey +0x38; table pointer at `[[ctrl+0x8D8]]`). Menu and
shortcut both end here (the callers that write +0x87C were not traced [?]).

1. gate: controller vt+0x1548(kind); facing yaw = `ctrl+0x85C` (camera-relative stick yaw) when the stick
   magnitude `ctrl+0x854 >= 0.1`, else the actor yaw; lock target = controller vt+0x1690().
2. by kind:

| Kinds | Call on the request interface (pawn+0x450, vtable 0x14685EB18) |
|---|---|
| 29–31, 33–35, 37–39, 45–47, 53–56 (Fire, Blizzard, Thunder, Aero ×3; the four water-zone spells) | `pawn.cmdKind = kind`; slot 53 (vt+0x1A8, 0x14143AEB0) `TryMagic(targetPawn, lockComponent, stickMag, yaw, 0)`; targetPawn = owner of the lock component when it is a `TresCharPawnBase` |
| 41–43 (Water ×3) | same, refused when pawn vt+0x1C30() |
| 32, 36, 40, 44, 48 (…gan) and 110–124 (△ grand magic) | need !pawn vt+0x1C30(); `TryMagic(…, 1)`; on success controller vt+0x1520(), `ctrl+0x8F4 = 0.1`, grand-magic bookkeeping cleared (ctrl+0xB40, +0xB4C, +0xB98.., +0xBA8..), HUD `FUN_14158C410` |
| 125 (△ Curagan) | target = self, component = none, `TryMagic(…, 1)` |
| 49–51 (Cure ×3) | target pawn and component from `m_pUICommandInfo` (`m_Actor`, `m_Component`: the party target picked in the menu); `TryMagic(…, 0)` |
| any row with Cate 12 (ITEM: 58–68) | target = `m_pUICommandInfo->m_Actor` when it is a `TresCharPawnBase`, else the own pawn; `pawn.cmdKind = kind`; slot 228 (vt+0x720, 0x141441FF0) `TryUseItem(target, kind, 0)` |

3. on success `FUN_1414158F0(ctrl, kind, 0)`: pawn vt+0x1D98(kind, 0) (0x1414488A0: fires
   `OnPlayerBeginCommandAction`, the state-event component, stores pawn+0xE2C = kind), `ctrl+0x880 = kind`.

(Magic details: part "Cast". The magic rows of the table with MP costs: part "MP".)

### I2. `CanUseItem` (slot 227, vt+0x718, 0x14142EBD0) [C]

Refuse when: pawn vt+0x1F08() (restriction ITEM, pawn+0xE60 bit 2); vt+0xA48(); !vt+0x29C0(); vt+0x2540();
interface vt+0x618(). The **next queued** state id must be one of: none, 12–14, 61, 62, 75–90, 156, 157,
159–161. Controller present and controller vt+0x1A70() ≠ 0x8B. Unless the caller skips the stock check:
`item = ItemOfCommand(kind)` and the pouch must hold ≥ 1 of it (I4). Then by **current** state:

| Current state | Allowed when |
|---|---|
| IDLE 1, RUN 2, GLIDE 4, FLOATING 5 | always |
| JUMP 3 | Jump+0xF8 == 0 |
| attack / combo states (12–14, 75–90, 156, 157, 159–161) | Attack+0x234 bit 7 set |
| GUARD 15 | Guard+0xDC ≠ 0 and Guard+0x84 bit 5 set |
| DODGEROLL 16, AIR_SLIDE 20 (and +0xA8 bit 7 clear), USE_ITEM 111, BADSTAT_STUN 122 | state+0x84 bit 0 (`m_bEnableChange`) set |
| FIREMAGIC 18 | (Magic+0xD8 & 0x50) == 0, Magic+0x84 bits 5 and 0 set |
| DAMAGE 19 | +0x84 bit 0 set, +0x90 == 0, sub-step in {1, 2, 9} |
| GIMMICK_ATTACH 97, SWIM 102, COVER 142 | their own virtual / flags |
| anything else | refused |

and finally pawn vt+0x2788() == 0.

`TryUseItem` (0x141441FF0): `CanUseItem`; if an attack / combo state is queued it is dropped
(`FUN_1405A7FF0`); `PushState(111)`; `state+0x88 = target`, `state+0x90 = kind`.

`ItemOfCommand` (0x1411270C0) [C]: 58 → 0x101 Potion, 59 → 0x102 Hi-Potion, 60 → 0x103 Mega-Potion,
61 → 0x104 Ether, 62 → 0x10B Hi-Ether, 63 → 0x105 Mega-Ether, 64 → 0x106 Elixir, 65 → 0x107 Megalixir,
66 → 0x108 Refocuser (`FOCUSSUPPLY`), 67 → 0x109 Hi-Refocuser, 68 → 0x10A Panacea (`ALLCURE`). Item id =
category << 8 | index (`ETresItemCategory` 1 = BATTLEITEM, `ETresItemDefBattleItem`).

### I3. State `TresPlayerState_UseItem` (id 111)

Vtable 0x14694D0F8 (base `TresCharState_UseItem` 0x146A15960): Reset 0x14150E2C0, Begin 0x141509E40, Tick
0x141511E10, End 0x14150C2D0, slot 113 animation name 0x1413D0540, slot 114 voice 0x1413DE130; anim-interface
(state+0x40) vtable 0x146851668 with `OnTiming` (vt+0x348) 0x141512C80. Fields: +0x84 flags (bit 0
`m_bEnableChange`), +0x88 `m_pTargetPawn`, +0x90 command kind, +0x94 bit 0 "was airborne".

- **Begin** [C]: (weapon handling when pawn vt+0x1330() / vt+0x12E0(−1): vt+0x1358(1, 1.0)); actor rotation
  set to yaw-only; pawn vt+0x16D8(1), vt+0x16E8(); target defaults to self; if the pawn is not on the ground
  (`FUN_1405AD850` false): pawn vt+0x1728(0) (gravity off) and flag "airborne"; play `ItemAirUse` when
  airborne else `ItemUse` (play once: `Play(name, 0, −1.0, 2, 0, 1.0)`); slot 114 picks the voice variant from
  the pair (user character, target character) through pawn+0x440 vt+0x1F0(index).
- **Tick** [C]: pawn vt+0xDC8(dt·12), vt+0xDB8(dt·12); animation ended → finish.
- **OnTiming** [C; delay I]: if the target is alive: `UseItem(user, kind, target, consume = 1, delay = 0.5)`
  (0x1405BCBE0); then `m_bEnableChange` is set (interface vt+0x238) and pawn vt+0x1728(1), vt+0x1658()
  (gravity scale reset), vt+0x16D8(0).
- **End** [C]: the same three restores plus pawn vt+0x22E8().
- Animations [A]: `ItemUse` = body/0391, `ItemAirUse` = body/0392, both 0.8333 s (26 frames): `TresPlayVoice`
  t = 0 (group `sora_item`), `TresAttachEffect` t = 0.3127 (ground) / 0.3449 (air)
  (`/Game/Effects/system/item/par/vs_itm_trw0`, socket `L_te`), **`TresTiming` t = 0.5**, `TresEnableChange`
  t = 0.5667.

So: 0.5 s after the press the item is consumed, 0.5 s later the effect lands (queue below), and the state can
be left from 0.5 s on (the timing handler enables change itself).

### I4. Pouch and consumption [C]

The battle pouch is 8 entries of 8 bytes at `[[pawn+0x4D0]+0x130] + 0x118`: `{ int32 itemId; uint8 flags }`,
flags bit 0 = slot valid, bits 1–2 = refill kinds (auto-reload), bit 3 = used. Count of an item
(`FUN_1405AA910`, `FUN_140E8CCF0`) = entries with bit 0, matching id, bit 3 clear.

`UseItem` (0x1405BCBE0): needs count ≥ 1; takes the first matching usable slot: with no refill bits the id is
zeroed, otherwise bit 3 is set (`FUN_140E98B90`); if the slot's bit 1 is clear and it is now empty, the item
counter of the save data is updated (`FUN_140E9FC50` → `FUN_140E86A80(table, id, +1)`: a per-item int array:
battle items index < 16; meaning — stock or usage statistic — not settled [?]); then
`ApplyItem(target+0x460, id, userCharacter, 0, delay 0.5, ITEM_UP count, 1)` (0x1405BD190): with a positive
delay the effect is queued on the target (pawn vt+0x1200 = 0x1405A0550 appends `{delay, id, …}` to the array at
pawn+0x7E8) and executed later by `ItemEffect` (0x1405BE820); party items (Mega-Potion, Mega-Ether,
Megalixir, Tent) go through 0x1405BEF00, which calls `ItemEffect` for the player and every party member.

How many of the 8 slots Sora owns: victory bonuses `ITEM_SLOT_UP1` (Sora: `vbonus_038`, `_062`, `_069`,
`dlc_001`, `dlc_015`) [A]; the initial count and where the limit is stored were not found [?]. Stock limit of
every battle item: 99 (`TresItemData.m_MaxNum`) [A]. `TresItemData` has no effect fields: all amounts are
constants of `ItemEffect`.

### I5. `ItemEffect(target, id, userChar, itemUpCount, bShow)` (0x1405BE820) [C; constants I]

`mult = 1.0`; ×0.5 when the target's current state is DIE (59) [I 0x1405BE88D]; `× (1 + 0.5·itemUpCount)` when
the count of ability ITEM_UP (40) is > 0 [I 0x1405BE8A1]. `round(x)` = round half up.

| Item | Effect |
|---|---|
| Potion 0x101 | HP + round(mult · 80) [I int 0x146713E64] |
| Hi-Potion 0x102 | HP + round(mult · 160) [I 0x146713F0C] |
| Mega-Potion 0x103 | same 160 [I 0x146714004], whole party |
| Ether 0x104 | not in MP charge: MP + round(mult · 100) [I 0x1467140C4]; in MP charge: `chargeLeft −= chargeTotal · 0.5 · mult`, ≤ 0 ends the charge (pawn vt+0x1178(0, 1.0)) |
| Hi-Ether 0x10B / Mega-Ether 0x105 (party) | not in MP charge: MP + round(maxMP · mult); in MP charge: `chargeLeft −= chargeTotal · mult` |
| Elixir 0x106 / Megalixir 0x107 (party) | HP + max HP (computed with the override pawn+0x61C cleared), MP + max MP or MP charge ended, focus-empty flag cleared, focus + max focus, bad statuses cured |
| Refocuser 0x108 | focus-empty flag cleared; focus + round(maxFocus · 0.4 · mult) [I 0x1405BEB7C] |
| Hi-Refocuser 0x109 | focus + round(maxFocus · 1.0 · mult) [I 0x1405BEBDF] |
| Panacea 0x10A | bad statuses cured (`FUN_1405A5C40`); if not already immune: pawn+0x868 bit 0 set and pawn+0x864 = 60.0 (s) [I 0x1405BEC21] |
| Tent 0x201 (camp) | HP, MP, focus to max, party |
| 0x202..0x205 (camp: Power / Magic / Guard / AP up) | permanent stat counters of the character (`FUN_140E86A40`, `_A00`, `_9C0`, `_980`) |

Nothing is applied when the target is in state DIE or GAMEOVER (59, 118) unless its byte +0x3B0 is 10. The
gains are delivered through pawn vt+0x1878 `Recover(hp, mp, fp, effectKind, userChar)` (player override
0x141445F40: plays the heal effect `sora_kaihuku` on the mesh unless the user is the same character class, and
ignores MP / focus when the player state has MP / focus disabled (bits 2 / 4 of `[playerState+0x95B0]`); base
0x1405B57D0 adds to HP / MP / focus through the clamping setters and shows the HUD effect kind:
3 Potion, 4 Hi/Mega-Potion, 5 Ether, 0x12 Hi/Mega-Ether, 6 Elixir, 0x13 / 0x14 Refocusers, 0x15 Panacea).
MP-charge terms used here: charge flag pawn+0x7C0 bit 1 (vt+0x17E0), time left pawn+0x7C4, total = pawn
vt+0x17E8() (`MyMpChargeTime`, +20 s when vt+0x8D0()) — owned by part "MP".

### Open questions (items / command path)
1. Who writes `ctrl+0x87C` (menu confirm and L1 shortcut press), the three shortcut sets and their storage,
   the menu's time behaviour (controller 0x14140E610 sets pawn+0xE60 bit 16 and a time dilation with id 0x4F9E
   while the sub-menu is open: value not read).
2. Pouch size at level 99 and the initial slot count; the after-battle refill (auto-reload) and the meaning of
   flag bits 1–2; whether `FUN_140E86A80` is the stock.
3. `m_pUICommandInfo` target selection for party items (host has no party at first: target = self).
4. The weak pointer at state+0x98 / +0x9C notified in Begin / End (`FUN_140F6F670` / `FUN_140F74110`).

### Runtime probe requests
| Object | Offset | Type | Settles |
|---|---|---|---|
| `[[pawn+0x4D0]+0x130]` | +0x118 .. +0x157 | 8 × {i32, u8} | pouch contents, flags before / after use and after battle |
| pawn | +0x7E8 (TArray, 16-byte entries) | | queued item effects (delay) |
| controller | +0x87C, +0x880, +0x884 | i32 | command selected / executed |
| state UseItem | +0x78, +0x84, +0x90 | i32, u32, i32 | sub-step, enable-change, kind |

### What the host must provide (items)
- The command to execute (kind) and, for party items, the target; with no party the target is Sora.
- Pouch contents (up to 8 entries) and stock counts; the HUD needs the per-item counts and the "usable" gate.
- Effect and voice playback (`vs_itm_trw0` on `L_te`, voice group `sora_item`, heal effect `sora_kaihuku`).

---

# Part F — Second pass: shotlock aim mode in full (supersedes D S2)

_Source of this part: `build/sora_tmp/magic/aim2/spec.md` (section numbers are local to the part)._

Replacement for sections S2.1–S2.4 of Part D of `docs/sora-magic-spec.md`, plus the camera and the slow
motion of the aim mode. Research only. Tags: [I] instruction level (address), [C] decompiled C, [U] stock UE 4.17
by identity, [A] asset, [D] derived, [?] open. Units cm, s, deg. `ctrl` = `ATresPlayerControllerSora` (vtable
0x146969B98), `pawn` = `TresPlayerPawnSora` (vtable 0x14696B930), `iface` = the request interface at pawn+0x450
(vtable 0x14685EB18), `comp` = `UTresLockonTargetComponent` (vtable 0x146628970), `hud` = `ATresHUD`
(`FUN_141401180(ctrl)`), `cam` = `ATresCameraShootFlow` (vtable 0x145D69D48). Camera names and conventions are
those of `docs/sora-camera-spec.md`.

Corrections to the first pass are marked **(corr.)**.

---

### A0. The answers in one page

1. **Button**: `bHold = IsInputKeyDown(Gamepad_RightShoulder)` (R1). The same key tapped (released within
   0.26666668 s) is the lock-on toggle; held longer than 0.26666668 s it opens the aim mode. Release fires
   nothing: the attack button (`Gamepad_FaceButton_Bottom` by default) fires, `Gamepad_FaceButton_Left` starts the
   airstep, letting R1 go cancels.
2. **Sora has no aim animation and no aim state.** No state is pushed; he stays in the state he is in. While the
   mode is open the controller dispatches **no** movement, jump or attack request, so on the ground Run brakes
   into Idle (Idle shows `IdleLockon` when in battle stance) and in the air he keeps falling ballistically. He
   does not hover and he does not turn; only the camera turns. `ShootFlowIdle` (body/1452) is referenced by no
   native code; `ShootFlowStart` (1451) is played only by three other keyblades' attack routines.
3. **Slow motion is per object type, not global**: rate 0.4, no blend, for the types PLAYER, ENEMY, GIMMICK,
   AVATAR, MAP, BG, FRIEND, VEHICLE, PHYSX. **Sora himself runs at 0.4**; projectiles, prizes, the camera, events
   and the UI run at full speed. All aim timers are in real time. Sound gets the state `STATE_SLOW_START`.
4. **Camera**: a second camera actor of the native class `TresCameraShootFlow` (no blueprint) is put on top of the
   camera stack with a 0.5 s ease-out blend. It is the field camera's update with: distance 200 (constant), look
   point shifted 50 cm forward and 100 cm right in view space (over the right shoulder), start rotation = behind
   Sora at pitch −10, pitch limits ±70, no automatic follow, no wall-avoiding rotation, no reset, look-point lag
   60 /s, FOV = the camera component default. The view is turned by the **right stick, or the left stick while
   the right stick is at rest**, at 120 °/s per unit deflection with no sensitivity setting applied.
5. **Locking**: candidates are collected once at entry (shotlock permission, other team, within 5000 cm). Each
   tick at most one lock: round robin over the candidates, the first one whose component position projects inside
   a circle of radius 150 px (at 1280×720, scaled by min(W/1280, H/720)) around the view-rectangle centre and
   passes a line trace gets +1. Interval between locks 2.24 / Param0 s of real time (Kingdom Key: Param0 = 24 →
   0.09333 s; 24 locks in 2.147 s + start delay 0.1 s). One target can take any number of locks. Nothing depends
   on level or abilities.
6. **Cost** = real time spent aiming × 20 focus/s, frozen when MAX is reached.
7. **Kingdom Key** never uses the special routine `FUN_141421190` (flag 0x80 is set only for kinds 29 and 39).

---

### A1. Call path and the button [I]

```
ctrl Tick 0x14141E950 → slot 631 0x1412875B0 (if ctrl+0x878 bit 0) → slot 675 (vt+0x1518) input update 0x1413F1AB0
```
(`docs/sora-movement-spec.md` 1.2 for the chain [C]; the vt+0x1518 call at 0x1412875C5 [I].)

In 0x1413F1AB0 [I unless noted]:

```
bHold  = IsInputKeyDown(FKey 0x149C70790)            // 0x1413F1C94 → FUN_1444369D0; FKey = "Gamepad_RightShoulder"
                                                     // (static initialiser 0x1402D5110, string 0x14725C3A8)
bR2    = IsInputKeyDown(FKey 0x149C707C0)            // "Gamepad_RightTrigger" (slot 713, not shotlock)
attackJustPressed = WasInputKeyJustPressed(0x1487D7760)   // FaceButton_Bottom unless GamePadDecideType >= 1 (input notes)
...stick read...
if ctrl.vt+0x888():            bHold = 0             // AController::IsMoveInputIgnored thunk 0x141403640  [C]
if ctrl+0x878 & 0x40:          bHold = 0             // 0x1413F2378
if state ∈ {95, 96, 103}:      bHold = 0             // AF_SHOOTINGRIDE, GIGAS, SPECIAL_SHIP   0x1413F23BB..23DF
if state ∈ {144, 150, 145}:    bHold = 0             // HOLD_CAMERA, ONE_ACTION, FESTIVAL_DANCE 0x1413F23E2..240F
ctrl.vt+0x1640(bHold, dt)                            // slot 712 0x1413F0700: R1 tap detector (below)
if pawn.vt+0xD70() == 1:                             // character id (pawn+0x5F0) == TRES_CHR_UID_SORA
    ctrl.vt+0x1900(bHold, dt)                        // slot 800 0x1414165E0   call at 0x1413F2443
    ctrl.vt+0x1648(bR2, dt)
ctrl.vt+0x1650(dt); vt+0x1668(dt); vt+0x1688(); target = vt+0x1690()
if ctrl+0xA10 (aim mode) == 0:                       // 0x1413F2495
    ... the whole normal dispatch: attack, jump, guard, MoveInput, TryRun, command input ...
else:
    s = current state id
    if (s == 41 || (s == 42 && !(squareJustPressed && iface.vt+0x240(1)) && !(jumpJustPressed && iface.vt+0x150(0))))
       && attackJustPressed:
        iface.vt+0x168(0, 0)                         // attack request during the shotlock / airstep states
```

`dt` is the controller's tick delta, i.e. already multiplied by the controller's time dilation (A6).

R1 tap detector, slot 712 0x1413F0700 [I, whole function]:
```
ctrl+0x934 = 0
if bHold: ctrl+0x938 += dt
else:     if ctrl+0x938 > 0 and ctrl+0x938 <= 0.26666668: ctrl+0x934 = 1     // "R1 tapped" → lock-on
          ctrl+0x938 = 0
```

Other callers of slot 800: the Riku controller's update 0x1413CA610 @0x1413CAEDC only [I, call scan over
0x141330000–0x141470000]. The Aqua / Kairi updates (0x1412847C0 / 0x141314D20) do not call it.

Keyboard: the key table / emulation of `docs` input notes maps keyboard keys onto the pad FKeys; which table slot
feeds `Gamepad_RightShoulder` was not traced [?].

---

### A2. Controller state

| Offset | Type | Meaning | Tag |
|---|---|---|---|
| +0x8E8 | ptr | → pointer to the `TresShootFlowData` rows (0x48 bytes each; `Param0` at row+0x24) | I 0x141418703 |
| +0x868 | obj | `m_CameraShootFlow` | A |
| +0x878 | u32 | bit 0 input update on · bit 2 a post-effect flag (`FUN_14140DF80`) · bit 3 "attack slow motion" (`FUN_14140E500`) · bit 6 input lock · bit 8, 9 situation-command flags | C |
| +0x934 / +0x938 | u8 / f32 | R1 tapped / R1 hold time (lock-on) | I |
| +0x960 | u8 | lock-on active | C |
| +0x964 | i32 | round-robin start index | I |
| +0x980 | f32 | lock range; 5000.0 at reset (0x141416419), = `pawn.MyShootLockonLimit` (pawn+0xC98, 5000) via slot 744 0x141411290, script setter `BP_SetShootLockonLimit` | I |
| +0x994 / +0x9A4 | weak | lock-on target / the lock-on target saved at aim entry | C |
| +0xA08 | u32 | flags: 0x01 aim mode active · 0x02 release pending (cleared by the state's End) · 0x04 (blocks entry; set elsewhere) · 0x08 debug "ignore aim timer" · 0x10 exited, wait for R1 release · 0x20 debug "pad to max" · 0x40 MAX · 0x80 special routine (kinds 29, 39) · 0x100 command-name banner shown | I |
| +0xA0C | f32 | R1 hold timer (entry) | I |
| +0xA10 | i32 | mode: 0 off · 1 aiming · 2 fired · 3 airstep requested · 4 focus-empty aiming · 5 scripted start (slot 812, not Sora's R1 path) | I |
| +0xA18 / +0xA1C | i32 | total locks / number of locked targets | I |
| +0xA24 | f32 | lock interval timer | I |
| +0xA28 | i32 | maximum locks = Param0 of the kind | C |
| +0xA30 / +0xA50 | i32 | banner data / kind shown in the banner | C |
| +0xA34 | i32 | debug kind override (0 = none) | C |
| +0xA38 | f32 | start delay | I |
| +0xA3C | f32 | aim time left | I |
| +0xA40 | f32 | slow rate 0.4 (reset 0x1414164DA; `DebugSetShootLockSlowRate`) | I |
| +0xA48 | f32 | charge of the special routine | C |
| +0xA4C | i32 | `ETresShootFlowKind` | I |
| +0xA68, +0xA70, +0xA78, +0xA7C | | airstep route marker (enable, actor, segment, phase): airstep strand | C |
| +0xA88 | obj | camera lens effect instance | C |
| +0xAE8 / +0xAF0 | TArray | lock candidates (data / count) | I |
| +0xAF8 / +0xB00 | TArray | airstep candidates | I |
| +0xB08 / +0xB10 | TArray | locked list | I |
| +0xB18 / +0xB20 | TArray | airstep targets | C |
| +0xB28 / +0xB2C | i32 | focus consumed (−1 = not computed) / focus at entry | I |

Entry (0x34 bytes) [I 0x141419377..0x14141939D for the layout]:

| +0 f32 | distance, −1 until measured |
|---|---|
| +4 u8 | `comp.m_MoveFlowType` (comp vt+0xAD0) **(corr.)** |
| +8 i32 | state: 1 candidate, 2 locked |
| +0xC i32 | lock count |
| +0x10 f32 | per-entry timer (0.08 after a lock in mode 1; `dist × 0.0002` in mode 4) |
| +0x14 f32 | `comp.m_ShootLockOnRange` (comp vt+0xAF8) — written only for **airstep** candidates; 0 in lock candidates **(corr.)** |
| +0x18 u32 | bit 0 |
| +0x1C weak | the component; +0x24, +0x2C two more weak references (actors), null for entries made here |

`m_ShootLockOnRange` of the component is therefore **not** used by the shotlock range test: the only range is
ctrl+0x980 [I: the five readers of +0x980 in slot 800; comp vt+0xAF8 is called only at 0x14141955F in the airstep
candidate branch].

---

### A3. Slot 800 `ShootLockUpdate(bHold, dt)` 0x1414165E0

#### A3.1 Preamble and gate (every tick) [I for each test: 0x1414165E0..0x141416B02; state list C]

```
world = ctrl.vt+0x140(); if not a valid world: return
gi = ctrl.vt+0x1418()                                  // UTresGameInstance (0x140F27C60)
if gi and gi+0x84C (u8) != 0: return                   // meaning not decoded [?]
if ctrl.Pawn is not an ATresPlayerPawnBase: return
if !pawn.vt+0x2938(): return                           // pawn+0xD80 bit 3 (0x14110E7B0), meaning not decoded [?]
mgr = pawn.vt+0x8E0(); if null: return                 // lock-on manager (0x1405CF480)
hud = FUN_141401180(ctrl); if null: return

hold = bHold
if ctrl.vt+0x1B78():            hold = 0               // ctrl+0xBB8 (u8)   context action pending [C: meaning]
if iface.vt+0x618():            hold = 0               // slot 195 0x141436AA0: state JUMP with Jump+0xF8 != 0,
                                                       //   or state 21..25, or 138
swim = false
if FUN_1405AD820(pawn):                                // movement component vt+0x540(): swimming [U]
    if pawn.vt+0x1C30() or !pawn.vt+0x15D0(): hold = 0 // dive (0x141436BB0) / movement+0x3CC bit 0 clear
    swim = true
mapBlock = FUN_1405AD320(pawn, 8) && pawn.vt+0xD80() != 11     // game-instance flag 8, not in world 11
if pawn.vt+0x1F88():            hold = 0               // 0x1414368A0: pawn+0xE60 & 0x8400 (restrictions SHOOTLOCK
                                                       //   bit 10, WEAPON bit 15) or pawn.vt+0x1FE8(10)
if pawn.vt+0xA48():             hold = 0               // pawn+0x518 bit 0 (no player control)
sys = ctrl.vt+0x1420(); if null: return                // (GameInstance+0x870)
if !(sys+0x95B0 & 0x10): return                        // ability switch "ShootFlow" (BP_SetAbilityShootFlow)
if !pawn.vt+0x29C0():           hold = 0               // 0x141427690: pawn+0x868 bit 1, bad status 12 / 17,
                                                       //   current or next state 61 / 62
if ctrl.vt+0x1A70() == 0x8B:    hold = 0               // ctrl+0xB38 = formchange command CHANGE_AN (139)
switch current state id (FUN_1405ABE20(iface)):
    3  JUMP:           if Jump.m_nSubStep(+0x78) == 9: hold = 0          // 0x141416A55 (dive start)
    6, 22, 31..40, 122, 123, 124, 138, 142, 168:        hold = 0          // DANGLE, RAIL_SLIDE, STYLECHANGE,
                       // WEAPONCHANGE, STYLEFINISH*, SHOOTERMODE, BADSTAT_STUN/FREEZE/BURN, RAILSLIDE_MI, COVER,
                       // QUICKBATTLE
    19 DAMAGE:         if Damage+0x88 (i32) != 1:       hold = 0          // 0x14141698B
    97 GIMMICK_ATTACH: if !state.vt+0x388():            hold = 0
    102 SWIM:          if Swim+0xB4 & 0x40:             hold = 0          // 0x1414169F2
if next state id (FUN_1405AB6E0(iface)) == 32:          hold = 0          // WEAPONCHANGE queued
if ctrl.vt+0x1B48():            hold = 0               // ctrl+0xB78 bit 3
myTeam = pawn.vt+0xD90()                               // pawn+0x514
```

"hold = 0" in mode 1 / 4 means **cancel** (A3.4): this is how damage, a formchange, a rail, a dive etc. end the
aim mode — there is no separate damage hook.

#### A3.2 Mode 0: entry [I for constants and order: 0x141416B0B, 0x141418FBB..0x1414198F3]

```
if mode == 0:
    if !pawn.vt+0x1E10() && hold && pawn.vt+0x2788() == 0:        // command cool-down pawn+0xE34 <= 0;
                                                                   // pawn+0xE60 bit 17 clear
        if flags & 0x04: return
        if ctrl+0x878 & 0x08: return                               // attack slow motion still on
        if flags & 0x10: return                                    // must release R1 first
        if pawn.vt+0x1170() && pawn.vt+0xD70() != 1: return        // focus-empty flag and not Sora
        if ctrl.vt+0x1B78() || ctrl.vt+0x1BF0() || ctrl.vt+0x1BD0() || ctrl.vt+0x1C30(): return
                                                                   // ctrl+0xBB8, +0xC09, +0xC08, +0xC18 (u8 each)
        flags &= ~0x46;  aimTime = 0;  consumed = -1;  focus0 = 0
        holdTimer += dt
        if holdTimer > 0.26666668:  Enter(); return                // comiss / jbe at 0x14141908D
    else:
        holdTimer = 0;  flags &= ~0x11
    FUN_14141F6F0(ctrl, dt)                                        // airstep start-point display (airstep strand)
    return
```

Note: `holdTimer` accumulates the controller's (dilated) `dt`; outside slow motion that is real time.

```
Enter():
    FUN_1414145C0(ctrl, 0.0, null, 0, 0)        // drops a pending situation / reaction command prompt [C]
    FUN_1414142D0(ctrl, 0)                      // ctrl+0x878 bit 9 = 0; pawn.vt+0x2840(0) [C]
    ctrl+0x878 &= ~0x100
    if ctrl+0xA54: remove the airstep start marker (hud FUN_14157A7A0(comp, 6, -1, 0)); ctrl+0xA60 = null; +0xA54 = 0
    flags |= 0x01
    ref = pawn.vt+0x2110()                      // 0x141448550: actor location + (0, 0, 100); for the riding states
                                                //   it is the mount's lock-on point [C]
    range2 = (ctrl+0x980)^2
    for c in mgr.vt+0x9D0():                    // every registered lock-on target component (weak list)
        if !valid(c): continue
        if !c.vt+0xAB0(): continue              // 0x140F51A50: (c+0x868 & 1) && m_bPermissionShootFlow (c+0x878 bit 0)
        if Team(c) == myTeam: continue          // 0x140F75F30: owner team (owner.vt+0xD90 / vt+0x838) when
                                                //   c+0x868 bit 1, else c+0x86C
        owner = c+0x1B8
        if ctrl.vt+0x17C0() && owner.ActorHasTag("Gimmick"): continue       // ctrl+0x98F (BP_SetDisableGimmickLockon)
        pos = c.vt+0xB88()                      // 0x140ACF250: component world location (c+0x2B0)
        if |ref - pos|^2 < range2:              // strict
            candidates.push({dist: -1, type: c.vt+0xAD0(), state: 1, lockCount: 0, timer: 0, comp: c})
            if owner is a TresCharPawn: owner.vt+0xBC8(2, 1.0)     // "aimed at by a shotlock" notification
    for c in mgr.vt+0x9D0():                    // airstep candidates (airstep strand): (c flag bit) && c.vt+0xAC0()
        ...                                     //   && Team(c) != myTeam → airstepCandidates.push({..., +0x14: c.vt+0xAF8()})
    ctrl.vt+0x18B8()                            // 0x1413FA990: ends a multi-lock (link / attraction) mode if one is on
    ctrl+0x9A4 = ctrl+0x960 ? ctrl+0x994 : null // remember the lock-on target
    ctrl.vt+0x1780(0)                           // 0x1413FA580: clears the lock-on (HUD markers, camera lock target,
                                                //   ctrl+0x994, +0x99C, +0x960, +0x964, +0x94C, +0x944)
    SelectKind(reset = true)                    // A3.5
    empty = pawn.vt+0x1170()                    // focus-empty flag, pawn+0x7C0 bit 0
    ctrl+0xA68 = 1; ctrl+0xA78 = 0              // airstep route marker on
    total = 0; lockedTargets = 0; rr = 0; holdTimer = 0
    if !empty:
        mode = 1
        focus0 = pawn.vt+0x1808();  consumed = -1                           // slot 821 0x1413EC850
        aimTime = focus0 * pawn.MyFocusPointConvertRate                     // pawn+0xD0C = 0.05
        startDelay = 0.1;  lockTimer = 0;  flags &= ~0x80;  ctrl+0xA48 = 0
        if kind == 29 || kind == 39: flags |= 0x80
        if aimTime > 0:
            SetTypeRates(CH_PLAYER, 0x4D9E, ctrl+0xA40 = 0.4, blend 0)      // FUN_1405DA460  (A6)
            SoundState("STATE_SLOW_START")                                  // FUN_141515D60(2)
    else:                                       // Sora only (other characters returned above)
        mode = 4
        focus0 = pawn.vt+0x1830();  aimTime = focus0 * 0.05                 // max focus
        startDelay = 0.1;  lockTimer = 0;  consumed = -1
        // no slow motion, no sound state
    LensEffect(respawn = true)                  // FUN_141415C30 (A7)
    if m_CameraShootFlow is not valid:          // A5
        cam = spawn ATresCameraShootFlow (FUN_1413E6790: owner ctrl, cam+0x468 |= 0x44)
        cam.vt+0x788(ctrl.Pawn, ctrl)
        FUN_140627E30(cam, (0, 0, 50), 0)       // m_TargetInfo.m_TargetOffset
        FUN_140626FC0(cam, (50, 100, 0), 0)     // m_CameraLocalOffset
        cam.vt+0x790(1, {BlendTime 0.5, VTBlend_EaseOut, BlendExp 2.0, bLockOutgoing 0, interp 2})
    hud.FUN_141587480(aimTime, empty, kind)     // aim-time gauge start
    hud.FUN_141584140(1)                        // aim HUD on
```

Constants [I]: 0.26666668 (0x14141908D), start delay 0.1 (0x141419742 mode 1, 0x141419705 mode 4), mask 0x4D9E
(0x14141979C), kind test 29 / 39 (0x14141976D..0x141419777), the `aimTime > 0` test (0x141419782), (0, 0, 50) (0x14141981D..0x141419848),
(50, 100, 0) (0x14141984D..0x14141987B), blend block 0.5 / 3 / 2.0 / word 2 (0x141419880..0x1414198AB).

#### A3.3 Modes 1 and 4: aiming

```
if mode == 1 or mode == 4:
    if startDelay > 0:
        startDelay -= dt / ActorTimeDilation(ctrl)          // FUN_143E98A80 (A6): real time
        if aimTime <= 0: aimTime = 0
        return                                              // nothing else, not even the cancel test
    if aimTime > 0: aimTime = max(0, aimTime - dt / ActorTimeDilation(ctrl))     // slot 825 0x1413F9930
```

Mode 1, normal routine (flag 0x80 clear) [I: 0x141418095..0x141418920]:

```
    canLockPhase = true
    if lockedList.count >= 1:
        if swim || mapBlock: Exit(); return
        eye = pawn.vt+0x6E0(0)                              // 0x1405BAC00: Sora's own lock-on point (his lock-on
                                                            //   target component's location; fallback bone
                                                            //   MyLockOnSetBoneName, then the root)
        removed = false
        for i, e in lockedList:                             // stops at the first removal
            c = e.comp
            if !valid(c) || !c.vt+0xAB0() || Team(c) == myTeam || !EntryPos(e, &p)       // 0x1413FE8F0
               || |eye - p|^2 > range2:
                total -= e.lockCount
                lockedList.removeAt(i)                      // order kept
                removed = true; break
        if removed:
            lockedTargets = lockedList.count;  rr = 0
            SelectKind(reset = true)
            hud.FUN_1415895F0(total, maxLocks)
            if flags & 0x40: hud.FUN_14158C380(kind)
            flags &= ~0x40
            canLockPhase = false
        else if flags & 0x40:                               // MAX held: keyblade kinds 3 and 8 only re-pick
            wk = pawn.vt+0x12C8();  if wk == 3 || wk == 8: kind = (wk == 3 ? 0 : 15) + (FUN_1413ECC80(ctrl) ? 6 : 7);
                                                            hud.FUN_14158C380(kind)
    else if swim: canLockPhase = false

    if canLockPhase && (aimTime > 0 || flags & 0x08) && !mapBlock:
        lp = ctrl.Player as ULocalPlayer                    // view rectangle and matrix, A4.2
        (x0, y0, x1, y1) = i32 × 4 at lp+0x420
        w = x1 - x0;  h = y1 - y0
        cx = float(x0 + w / 2);  cy = float(y0 + h / 2)     // integer halves, truncated toward zero
        scale = min(float(w) * 0.00078125, float(h) * 0.0013888889)
        eye = pawn.vt+0x6E0(0)
        if CanAddLock():                                    // A3.6
            lockTimer -= dt / ctrl+0xA40;  if lockTimer < 0: lockTimer = 0       // 0x141418486
            n = candidates.count
            for k in 0 .. n-1:                              // n re-read every iteration
                idx = (rr + k) % n
                e = candidates[idx];  c = e.comp
                if valid(c) && c.vt+0xAB0() && Team(c) != myTeam && EntryPos(e, &p) && lockTimer <= 0:
                    if e.dist == -1:
                        d2 = |eye - p|^2
                        if d2 > range2: continue
                        e.dist = sqrt(d2)
                    s = Project(lp+0x1B0, p, lp+0x420)      // A4.2
                    if s.z > 0
                       && (s.x - cx)^2 + (s.y - cy)^2 <= scale * scale * 22500.0
                       && LineOfSight(ctrl, null, c):       // A4.3
                        e.state = 2;  e.lockCount += 1;  e.timer = 0.08
                        P0 = rows[kind & 0xFF].Param0
                        lockTimer = (P0 >= 1) ? 2.24 / float(P0) : 0.08
                        if e.lockCount > 1:
                            j = first index in lockedList with the same component     // FUN_14042D9A0
                            if found: lockedList[j] = copy(e)                         // else nothing (see note)
                        else:
                            lockedList.push(copy(e));  lockedTargets += 1
                        total += 1
                        hud.FUN_1415759C0(c, 2, 1, -1)      // lock marker on the component
                        hud.FUN_1415895F0(total, maxLocks)  // lock counter
                        if c.owner is a TresCharPawn: hud.FUN_14158A860(owner, 0, -1)   // enemy gauge target
                        rr = idx + 1
                        break
            SelectKind(reset = false)                       // sets MAX (A3.5)
    canShoot = total > 0
```

Mode 1, flag 0x80 set: `canShoot = FUN_141421190(ctrl, dt)` (A8) instead of the block above.

Then, both routines [I 0x141418925..]:

```
    canAirstep = FUN_14141FDC0(ctrl, dt)                    // airstep target update (airstep strand)
    hud.FUN_14158B420(0, canShoot);  hud.FUN_14158B420(1, canAirstep)      // the two button prompts
    if !hold: Exit(); return
    if WasInputKeyJustPressed(attack key 0x1487D7760):
        if total < 1: return
        mode = 2;  rr = 0;  flags |= 0x02;  ctrl+0xA20 = 0;  ctrl+0xA2C = 0
        if flags & 0x20 && candidates.count > 0:            // debug only
            candidates.last.lockCount += maxLocks - total;  total = maxLocks;  flags |= 0x40
        SelectKind(reset = false)
        for e in airstepCandidates: hud.FUN_14157A7A0(e.comp, 8, -1, 0)
        hud.FUN_141577520(2)
        ctrl.vt+0x1930(kind)                                // slot 806 0x141415B20: command-name banner, flags |= 0x100
        if flags & 0x80: hud.FUN_141577ED0(0)
        FUN_141412250(ctrl, 0)                              // airstep route marker off
        LensEffect(respawn = false)
        SoundState("STATE_SLOW_END")                        // FUN_141515C90(2)
        airstepCandidates.clear();  airstepTargets.clear()
        if iface.vt+0x2F8(): StartCommand(kind)             // slot 95 TryShootFlow 0x1414413A0 (A3.7)
        else:                Exit()
        return
    if WasInputKeyJustPressed(FKey 0x149C70748 "Gamepad_FaceButton_Left"):
        if airstepTargets.count < 1: return
        mode = 3;  rr = 0;  flags |= 0x02;  lockedTargets = 0;  ctrl+0xA20 = 0;  total = 0
        for e in candidates: hud.FUN_14157A7A0(e.comp, 8, -1, 0);  owner.vt+0xBC8(2, 0)
        hud.FUN_141577520(4)
        FUN_141415E20(ctrl, 0);  FUN_141412250(ctrl, 0);  LensEffect(respawn = true);  SoundState("STATE_SLOW_END")
        candidates.clear();  lockedList.clear()
        if iface.vt+0x308(): kind = 37; StartCommand(37)    // slot 97 TryAthleticFlow 0x1414372F0
        else:                Exit()
```

The attack key is tested before the airstep key; when both are pressed in one tick the attack wins unless
`total < 1`, in which case the function returns without looking at the airstep key [I: `jle` to the function end at 0x1414189B8].

Notes that follow from the order of operations:

- **At most one new lock per tick** (the loop breaks), and none while `lockTimer > 0`. At frame rates below
  1 / interval (10.7 fps for Kingdom Key) the lock rate is frame-limited.
- A candidate's distance is measured **once** (when `dist == -1`) against the range; locked entries are re-tested
  every tick with their current position.
- The candidate list is **not rebuilt** while aiming: enemies that appear later cannot be locked [C].
- No per-target maximum. `lockedList` holds one entry per target (a copy of the candidate entry at its last lock).
- Quirk: a target dropped from `lockedList` keeps `lockCount` and `state 2` in its candidate entry. If it is locked
  again its `lockCount` is already > 1, the search in `lockedList` finds nothing, and the lock is counted in
  `total` without an entry in `lockedList` [C: 0x14141873E..0x141418864].
- When `aimTime` reaches 0 the mode stays open; only the lock phase stops. Releasing R1 cancels, the attack
  button still fires with the locks already made.
- `dt / ctrl+0xA40` is real time only while the controller is actually slowed to 0.4 (A6). If the mode was entered
  with `aimTime == 0` (no slow motion) the lock timer would run 2.5× fast — not reachable with a non-empty gauge.

Mode 4 (focus-empty flag; Sora) [C, constants I]: the same skeleton with these differences — the locked-list
re-test removes with `FUN_141410C10` and calls `hud.FUN_1415895F0(total, 0)`; the lock phase needs `aimTime > 0`
only; there is no global `lockTimer`: an entry is eligible when its own `timer <= 0`, otherwise `timer -= dt`;
on a lock `e.timer = e.dist * 0.0002` (0x1414171F9), marker type 3, `rr += 1`; `CanAddLock` requires the ability
FOCUS_ASPIR (46) and sets MAX at 18 locks; the release test is `hold && pawn.vt+0x1170()` (else Exit); firing
needs `lockedList.count >= 1`; kind is 36 only with ability 46 (A3.5). Without that ability the view opens (no
slow motion) and nothing can be locked.

#### A3.4 Modes 2 and 3: waiting for the state

```
if mode == 2:
    if flags & 0x02: return                         // cleared by ctrl slot 809 (vt+0x1948) 0x14140E720, which the
                                                    //   ShootFlow state's End 0x1413ABA70 calls (@0x1413ABC9E) (corr.)
    holdTimer = 0;  mode = 0;  flags &= ~0x01;  total = 0;  lockedTargets = 0
    ResetTypeRates(CH_PLAYER, blend 0)              // FUN_1405DA3E0(pawn, 1, 0)
    for e in candidates: hud.FUN_14157A7A0(e.comp, 8, -1, 0);  owner.vt+0xBC8(2, 0)
    hud.FUN_141577520(pawn.vt+0xA48() ? 6 : 3);  hud.FUN_141584140(0)
    ctrl.vt+0x1938(0, kind ∈ {1, 12, 20, 30, 32, 34} ? 3.0 : 0.0)      // banner off after 3 s
    FUN_141412250(ctrl, 0)
    candidates.clear();  lockedList.clear();  (array ctrl+0x9E8).clear()
    FUN_14140DF80(ctrl, 0, 1.0, 1.0)
    if m_CameraShootFlow valid: cam.vt+0x798(1, same blend block);  m_CameraShootFlow = null
    return
if mode == 3: the same shape for the airstep (airstep strand); slot 809 is called from AthleticFlow End 0x141334B60.
```

Slot 809 [I, whole function 0x14140E720]: `hud.FUN_14158A860(null, 0, -1)`; `if mode == 2 || mode == 3: flags &= ~0x02`.

`FUN_14140E500(ctrl, b)` is **not** this handshake **(corr.)**: it switches a second slow motion
(`SetTypeRates(CH_PLAYER, 0x419C, 0.1, 0)` = ENEMY, GIMMICK, AVATAR, MAP, BG, PHYSX at 0.1 / reset) and keeps
ctrl+0x878 bit 3; that bit blocks a new aim entry. It belongs to the attack state (S3).

Mode 2 does not set flag 0x10, so R1 still held after the shotlock starts a new 0.26666668 s count.

#### A3.5 `SelectKind(reset)` 0x141411B80 [C; table index and MAX test I]

```
if pawn.vt+0x1170() && pawn.vt+0x14E0(46):  kind = 36;  reset = true          // focus empty + FOCUS_ASPIR
else switch pawn.vt+0xD70():  6 (Aqua) → 38;  2 (Riku) → 43;  3 (Kairi) → 39
     else: kind = ctrl+0xA34 (debug) if != 0, else by weapon kind pawn.vt+0x12C8(-1):
           1→1  2→3  4→8  5→10  6→12  7→14  8→20  9→18  10→16  11→23  12→25  13→40  14→41  15→29  16→30
           17→32  18→27  19→34  other→5
ctrl+0xA4C = kind;  maxLocks = rows[kind].Param0
if !reset:
    if maxLocks <= total && maxLocks > 0:
        ctrl.vt+0x19B0()                    // GetConsumed (A3.6): stores the value before MAX freezes it
        flags |= 0x40
    if flags & 0x40:
        if Sora: kind = by weapon kind: 1→2  2→4  3→(FUN_1413ECC80 ? 6 : 7)  4→9  5→11  6→13  7→15
                 8→(FUN_1413ECC80 ? 21 : 22)  9→19  10→17  11→24  12→26  13,14→42  16→31  17→33  18→28  19→35
        hud.FUN_14158C380(kind)             // command name of the MAX shotlock
```

Kingdom Key = weapon kind 1 (first pass) → kind 1 `KCショット` while `total < 24`, kind 2 `KCラグナロク` once
`total >= 24`. Because the first branch re-derives the base kind every call, the kind falls back to 1 when a
locked target is lost (the caller clears flag 0x40).

#### A3.6 `CanAddLock` 0x1413F0320 and `GetConsumed` (slot 822) 0x1413FD3D0 [I]

```
CanAddLock():
    v = pawn.vt+0x1170() ? pawn.vt+0x1500(46) : pawn.vt+0x1808()       // ability count / focus
    if v == 0: return false
    if maxLocks <= total && maxLocks > 0: flags |= 0x40
    if mode == 4 && total > 17:           flags |= 0x40
    return !(flags & 0x40)

GetConsumed():
    if pawn.vt+0x1170(): return consumed                   // focus-empty: stored value
    if ctrl.vt+0x1958(): return consumed                   // slot 811 0x14128C0A0: flags & 0x40 (MAX) → frozen
    consumed = focus0 - trunc(aimTime / pawn.MyFocusPointConvertRate)       // cvttss2si, 0x1413FD499..B5
    return consumed
```

With `aimTime = focus0 × 0.05 − t` (t = real seconds after the start delay):
`consumed = focus0 − trunc(focus0 − 20 t)`; e.g. focus0 = 100, t = 2.14 s → 43.
Waiting at MAX costs nothing more; losing MAX (a locked target dies) un-freezes it and the full elapsed time
counts again.

#### A3.7 `StartCommand(kind)` 0x14140F1F0 [C]

```
if kind == 36 || kind == 37: consumed = 0; focus0 = 0
else: c = GetConsumed();  pawn.vt+0x1868(pawn.vt+0x1808() - c);  consumed = -1;  focus0 = 0      // SetFocus (clamped)
aimTime = 0
ctrl.vt+0x1520(1.0)                                 // slot 676 0x141415A50
pawn.vt+0x1D98(0xFD, kind)                          // command-begin event, 0xFD = TRES_CMD_KIND_SHOOTFLOW
ctrl+0x880 = 0xFD;  ctrl+0x884 = kind;  ctrl+0x8F4 = 0.1
pawn.vt+0x1D30(0xFD, kind)
SetTypeRate(CH_PLAYER, PLAYER, 1.0, blend 0)        // FUN_1405DA4E0(pawn, 1, 1, 1.0, 0): Sora back to full speed,
                                                    //   every other slowed type stays at 0.4 until mode 2 ends
if kind == 37: FUN_141411FD0(ctrl, 1, 0);  if ctrl+0xA40 != 0.1: SetTypeRates(CH_PLAYER, 0x4D9E, 0.1, 0)
else if kind != 36 && gi: FUN_140E9E390(gi, kind, 1);  FUN_140EB2770(gi, kind)        // usage record
FUN_14140DF80(ctrl, 1, 1.0, 1.0)
if m_CameraShootFlow valid: cam.vt+0x798(1, {0.5, VTBlend_EaseOut, 2.0, interp 2});  m_CameraShootFlow = null
if ctrl.vt+0x1828(): ctrl.vt+0x1830(0)
ctrl.vt+0x1820(0)
```

So the aim camera is released the moment the attack starts (0.5 s blend back to the field camera, or to whatever
camera the attack state pushes).

`TryShootFlow` (iface slot 95, 0x1414413A0) [C]: `CanShootFlow` (slot 94, 0x14142D800: first pass S2.3) → push
state 41 (`FUN_1405B9FE0(iface, 0x29)`); in world 11 it also fires the event `Tutorial_ShootFlow`.

#### A3.8 `Exit()` = slot 813 (vt+0x1968) 0x1413FAD90 [C]

```
flags = (flags & ~0x01) | 0x10;  holdTimer = 0
ctrl.vt+0x1938(1)                                   // banner off at once; flags &= ~0x100
if mode == 0: return
flags &= ~0x02;  mode = 0;  rr = 0;  total = 0;  lockedTargets = 0;  aimTime = 0;  consumed = 0;  focus0 = 0
if ctrl+0x8F9 (timing gauge): hud.FUN_141577490(0);   ctrl+0x8F9 = 0;  ctrl+0x8FC = 0
if flags & 0x80: hud.FUN_141577ED0(0)
flags &= ~0x80;  ctrl+0xA48 = 0
SoundState("STATE_SLOW_END")
if no player pawn: return
ResetTypeRates(CH_PLAYER, blend 0)                  // FUN_1405DA3E0(pawn, 1, 0)
hud.FUN_14158A860(null, 0, -1)
for e in candidates:        hud.FUN_14157A7A0(e.comp, 8, -1, 0);  owner.vt+0xBC8(2, 0)
for e in airstepCandidates: hud.FUN_14157A7A0(e.comp, 8, -1, 0)
hud.FUN_141577520(pawn.vt+0xA48() ? 6 : 1);  hud.FUN_141584140(0)
ctrl+0xA68 = 0;  destroy actor ctrl+0xA70
LensEffect(respawn = true)                          // sic: the effect is re-created on a cancel (A7)
candidates, lockedList, airstepCandidates, airstepTargets, (array ctrl+0x9E8) .clear()
FUN_14140DF80(ctrl, 0, 1.0, 1.0)
if m_CameraShootFlow: cam.vt+0x798(1, {0.5, VTBlend_EaseOut, 2.0, interp 2});  m_CameraShootFlow = null
ctrl.vt+0x1788()                                    // slot 753 0x1414110A0 (lock-on restore; not read)
ctrl+0x9A4 = null
```

No focus is spent by a cancel (`consumed` is discarded, the pawn's focus is untouched).

**All callers of the exit slot** [I: call scan `call [reg+1968h]` over 0x140500000–0x141700000, each site read]:

| Caller | When |
|---|---|
| slot 800 @0x14141776F | `hold` false in mode 1 / 4 (R1 released, or any gate of A3.1 — damage, formchange, rail, dive, restriction, loss of control …); swimming or map block with ≥ 1 locked target |
| slot 800 @0x141418C3C | `TryShootFlow` / `TryAthleticFlow` refused at the release |
| ctrl slot 379 0x141413460 @0x14141349E | `SetCinematicMode(true, …)` |
| ctrl slot 157 0x1413FACB0, 0x1413FACF0 | controller end-of-play / teardown |
| 0x14140E610 @0x14140E6D3 | the HUD command sub-menu opens (it also sets type rates with mask 0x4F9E) |
| pawn slot 986 0x141447570 @0x1414475CA | weapon restriction switched on (pawn+0xE60 bit 15) |
| iface slot 26 0x1414393D0 @0x14143949D.. | start of the cinematic state (state 67) |
| iface slot 129 0x14143A190 @0x14143A1EC | start of the die state (state 59) |
| `TresPlayerState_Bind` 0x14135FA40 @0x14135FCE4 | bound / caught |

(The two `call [rax+1968h]` sites in the FireMagic state, 0x141368D14 and 0x14136DAE7, call the **pawn's** slot
813, a different function.)

Other requests refused while `IsShootLockMode()` (ctrl slot 801, flags bit 0): iface slot 29 0x14142BA80, pawn
slot 621 0x1414300E0, helper 0x141435C60 [C; which requests these are was not resolved [?]]; `AddFocus`
(slot 817) is ignored (first pass S1.2); the preview camera ends (0x14060D590).

---

### A4. Lock tests

#### A4.1 Component accessors [I, whole functions]

| Virtual | Function | Returns |
|---|---|---|
| vt+0xAB0 | 0x140F51A50 | `(comp+0x868 & 1) && m_bPermissionShootFlow` (comp+0x878 bit 0) |
| vt+0xAC0 | 0x140F51A30 | `(comp+0x868 & 1) && m_bPermissionMoveFlow` (comp+0x878 bit 1) |
| vt+0xAD0 | 0x140F50B00 | `m_MoveFlowType` (comp+0x87C, u8) |
| vt+0xAF0 | 0x140F50950 | `m_AutoLockOnRange` (comp+0x888) |
| vt+0xAF8 | 0x140F50C30 | `m_ShootLockOnRange` (comp+0x88C) |
| vt+0xB88 | 0x140ACF250 | component world location (comp+0x2B0, 3 × f32) |
| vt+0xA80 | 0x140F96570 | `(comp+0x868 & 1) && m_bPermissionLockOn (comp+0x870 bit 0) && owner.vt+0x698()` |

comp+0x868 bit 0 is the component's "enabled" switch, bit 1 "take the team from the owner"; comp+0x86C a fixed
team id [C 0x140F75F30].

`EntryPos(e, out)` 0x1413FE8F0 [C]: the component's world location when the weak reference is alive; otherwise
(entries of other systems) an actor's `vt+0x6E0` point; false when nothing is alive.

#### A4.2 Screen test [I 0x1414183DF..0x1414186AF; projection C 0x140ECF670]

`lp` = `ctrl->Player` cast to `ULocalPlayer`. `lp+0x1B0` is a 4×4 float matrix M (row-major as used below) and
`lp+0x420` an integer rectangle (x0, y0, x1, y1): SQEX additions to `ULocalPlayer`, taken to be the last view's
view-projection matrix and view rectangle (the letter-boxed 16:9 area when the aspect ratio is constrained)
[?: semantics inferred from use; probe].

```
Project(M, p, rect):
    w  = M[3][3] + p.z*M[2][3] + p.y*M[1][3] + p.x*M[0][3]
    wi = 1 / (|w| <= 1e-8 ? 1e-6 : w)
    out.x = ((M[3][0] + p.z*M[2][0] + p.y*M[1][0] + p.x*M[0][0]) * wi + 1) * float(x1 - x0) * 0.5 + float(x0)
    out.y = (1 - (M[3][1] + p.z*M[2][1] + p.y*M[1][1] + p.x*M[0][1]) * wi) * float(y1 - y0) * 0.5 + float(y0)
    out.z =  (M[3][2] + p.z*M[2][2] + p.y*M[1][2] + p.x*M[0][2]) * wi
    if w <= 0: out.z = 0
```

Reticle: circle of radius `150 × min(w/1280, h/720)` pixels around the rectangle centre; inclusive (`<=`);
behind-the-camera points are rejected by `out.z > 0`. The tested point is the component's location only (no
bounds). With the aim camera's FOV (A5) a 150 px radius at 1280 px width is a cone of half-angle
atan(150/640 × tan(FOV/2)): 13.2° at FOV 90, 15.6° at FOV 100 [D].

#### A4.3 Line of sight `FUN_1413F0A80(ctrl, fromComp = null, targetComp)` [I, whole function]

```
pawn = ctrl.Pawn;  tOwner = targetComp+0x1B8;  if !pawn || !tOwner: return false
start = pawn.vt+0x2110()                 // actor location + (0, 0, 100)   (not the camera, not the eye point)
end   = targetComp.vt+0xB88()            // component world location
params = FCollisionQueryParams(TraceTag "ShootLock", bTraceComplex = false, IgnoreActor = pawn)
for a = tOwner; a valid; a = a.GetAttachParentActor(): params.AddIgnoredActor(a)        // 0x143E98D00
params byte +0x10 = 1                    // bTraceComplex = true [U: field identity]
resp = FCollisionResponseParams();  resp.SetAllChannels(ECR_Ignore)
resp.SetResponse(ECC_WorldStatic 0, ECR_Block);  resp.SetResponse(ECC_WorldDynamic 1, ECR_Block)
resp.SetResponse(18 = TresGimmick, ECR_Block)
return !world.LineTraceTestByChannel(start, end, channel 15 = TresProjectile, params, resp)       // 0x143FD43D0
```

Channel names from `Config/DefaultEngine.ini` [A]: `ECC_GameTraceChannel2` (15) = `TresProjectile`,
`ECC_GameTraceChannel5` (18) = `TresGimmick`. So a lock needs a clear segment from 100 cm above Sora's origin to
the target point through everything that is of object type WorldStatic / WorldDynamic / TresGimmick and blocks
the `TresProjectile` channel; pawns never block it.

#### A4.4 Cadence table [A `sheets/entities/Load/Common/TresShootFlowData.json`; formula I 0x141418711..0x141418733]

`interval = Param0 >= 1 ? 2.24 / Param0 : 0.08` seconds of real time; `maxLocks = Param0`; time from the first
possible lock to MAX with a target always in the circle = `(Param0 − 1) × interval` (the first lock is
immediate, `lockTimer` starts at 0).

| kind | Param0 | interval | Fp | | kind | Param0 | interval | Fp | | kind | Param0 | interval | Fp |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 | 0 | 0.08 | 0 | | 15 | 24 | 0.09333 | 30 | | 30 | 24 | 0.09333 | 20 |
| **1** | **24** | **0.09333** | 20 | | 16 | 32 | 0.07 | 30 | | 31 | 24 | 0.09333 | 20 |
| **2** | **24** | **0.09333** | 20 | | 17 | 32 | 0.07 | 40 | | 32 | 24 | 0.09333 | 20 |
| 3 | 16 | 0.14 | 20 | | 18 | 24 | 0.09333 | 40 | | 33 | 24 | 0.09333 | 20 |
| 4 | 16 | 0.14 | 30 | | 19 | 24 | 0.09333 | 20 | | 34 | 24 | 0.09333 | 20 |
| 5 | 18 | 0.12444 | 30 | | 20 | 28 | 0.08 | 20 | | 35 | 24 | 0.09333 | 20 |
| 6 | 18 | 0.12444 | 30 | | 21 | 28 | 0.08 | 30 | | 36 | 18 | (per target) | 0 |
| 7 | 18 | 0.12444 | 30 | | 22 | 28 | 0.08 | 20 | | 37 | 0 | 0.08 | 25 |
| 8 | 12 | 0.18667 | 30 | | 23 | 32 | 0.07 | 20 | | 38 | 28 | 0.08 | 40 |
| 9 | 12 | 0.18667 | 40 | | 24 | 32 | 0.07 | 30 | | 39 | 1 | (special) | 20 |
| 10 | 28 | 0.08 | 40 | | 25 | 16 | 0.14 | 40 | | 40 | 32 | 0.07 | 20 |
| 11 | 28 | 0.08 | 20 | | 26 | 16 | 0.14 | 50 | | 41 | 32 | 0.07 | 20 |
| 12 | 18 | 0.12444 | 30 | | 27 | 24 | 0.09333 | 20 | | 42 | 32 | 0.07 | 20 |
| 13 | 18 | 0.12444 | 20 | | 28 | 24 | 0.09333 | 20 | | 43 | 28 | 0.08 | 20 |
| 14 | 24 | 0.09333 | 20 | | 29 | 1 | (special) | 20 | | 44 | 24 | 0.09333 | 20 |

(Rows 38 and 43 also have Param1..3 = 21 / 14 / 3 and 21 / 14 / 4: timing-gauge shot counts, S3.3. The `Fp`
column has no reader in the aim mode, first pass S1.3.) All rows give `maxLocks × interval = 2.24 s`.

**Level / abilities**: nothing in slot 800, `SelectKind`, `CanAddLock`, the entry or the tests reads the level or
an ability other than FOCUS_ASPIR (46) [I: the only `HasAbility(46)` sites are 0x1413F03C1, 0x141411C37,
0x141433C99 and two HUD functions; no site queries ability 42 (FOCUS_BOOST) through pawn vt+0x14E0 / vt+0x1500
with an immediate id anywhere in 0x140400000–0x141800000]. What FOCUS_BOOST changes was not found here [?].

---

### A5. The aim camera

#### A5.1 Class and creation

`ATresCameraShootFlow` — reflected class `TresCameraShootFlow`, parent `TresCameraNormal`, no reflected
properties of its own, **no blueprint**: `FUN_1413E6790(ctrl, null)` spawns the native class
(`GetPrivateStaticClass`), owner = the controller, and sets `cam+0x468 |= 0x44` (bit 0x04: the actor destroys
itself when it has ended, [C 0x1405CA980]; bit 0x40 not decoded) [C]. Nothing under `Blueprints/Camera`
references the class [A: file search].

Constructor 0x1405FD440 [I, whole function] = `ATresCameraNormal` constructor 0x1405FC450, then:

| Field | Value | Field camera (camera spec) |
|---|---|---|
| `m_LimitPitchMin` (+0x880) | −70 | −70 |
| `m_LimitPitchMax` (+0x884) | **+70** | +50 |
| `m_Rotation.Pitch` (+0x828) | −10 | −10 |
| `m_Distance` (+0x800) | **200** | 400 |
| `m_CameraLocalOffset` (+0x66C) | (0, 50, 0), replaced at set-up by **(50, 100, 0)** | (0, 0, 0) |
| stick mode (+0x844, i32) | **2** | 0 |
| flags (+0x8F0) | bit 0x04 cleared → 0xD8: **no wall-avoiding rotation** | 0xDC |
| `m_fPositionLerpPer` (+0x450) | **60** | 8 |
| priority (+0x3E8, `FUN_1405D7840`) | 120 | 100 |

Not touched, so the native `TresCameraNormal` / `TresCameraBase` defaults hold [I: constructors 0x1405FC450,
0x1405C02C0]: `m_NearDistance` 0 (**no pitch-dependent distance**; the field camera's 200 comes from its
blueprint), `m_LimitYawMax` 0, `m_AnalogStickPlayX/Y` **0.1** (field camera blueprint: 0.3),
`m_RotationSpeedX/Y` 120, `m_fRotationLerpPer` 8, fast rates +0x458 8 / +0x45C 30, distance rate +0x448 4.0,
`m_fPositionLerpLimit` 100, probe radius +0x464 20, `m_TargetInfo` socket `Zup_Cam`.
FOV: the camera component's native default — **90** per the camera spec 1.2 ("native 90"); no FOV call exists in
the aim path [C: `FUN_1405D6F40` has no caller in slot 800 or the class]. The manager blends FOV with the view
blend [U]. [?: confirm 90 with the probe.]

Virtual overrides (vtable compare with `TresCameraNormal` 0x145D5CA28) [I]:

| Slot | Field camera | Aim camera | Effect |
|---|---|---|---|
| 134 Tick | 0x14060CE60 | 0x1405D9F90 | base Tick + a no-op query of the manager |
| 242 `StartTresCamera` | 0x14060BA20 | 0x14060CA80 | A5.3 |
| 244 `Reset` | 0x140609C40 | stub | **R3 does nothing** |
| 259 stick gate + sensitivity | 0x1405CD290 | 0x140606750 | calls the base with type forced to 1 (A5.4) |
| 300 automatic follow | 0x140600630 | stub returning 0 | **no automatic follow** |
| 163 | 0x140605780 | thunk to the same | none |

Everything else — `CalcCamera`, the update `FUN_1406033C0`, slots 290 (distance), 301 (stick), 303 (target
point), collision — is the field camera's code.

#### A5.2 Geometry (camera spec 2.2 / 2.3 with the values above)

```
P        = socket "Zup_Cam" position + m_TargetOffset (0, 0, 50)      // = actor origin + (0, 0, 152.338) for Sora
rawTarget = P + RotationMatrix(rotCur) × (50, 100, 0)                 // 50 forward, 100 right of the view
lookSmooth: lerpStepVec(lookSmooth, rawTarget, 60, dt) + 100 cm clamp // 60/s: reaches the target within one
                                                                      //   1/60 s slice (factor clamp(step×60) = 1)
distance  = 200 × 1.0 + 0                                             // slot 290 with m_NearDistance = 0
camPos    = lookPoint − Forward(rot) × distance                       // = P − 150·F + 100·R when nothing collides
```

So the camera sits 150 cm behind and 100 cm to the right of the point 152.3 cm above Sora's origin, looking
parallel to the aim direction; Sora is left of the screen centre. The reticle ray starts 100 cm to his right.
Collision is the field camera's (look-point sweep radius 21 from Sora's body to the shifted look point, camera
sweep radius 20 from the look point back to the camera, distance recovery 4 /s).

#### A5.3 Start and end

`StartTresCamera` 0x14060CA80 [C]: `FUN_1405D9CC0` (registers the camera with the manager and stores the blend
block at manager+0x2868) → slot 305 → `rotTarget = slot 299()` = base rotation = (`m_Rotation.Pitch` −10,
Sora's yaw, 0) → component rotation = `rotTarget` → `m_LookPosition` = target point (slot 284). **The aim view
always starts behind Sora at pitch −10, whatever the field camera was showing**; Sora is not turned.

Blend block (both at start, `vt+0x790(1, blk)`, and at end, `vt+0x798(1, blk)`) [I for the bytes]:
`FViewTargetTransitionParams` {BlendTime **0.5**, BlendFunction 3 = `VTBlend_EaseOut`, BlendExp **2.0**,
bLockOutgoing 0, bBlendPostProcess 0} + SQEX byte +0x10 = 2 (`EViewTargetBlendInterp` =
`VTBInterp_LookRotationDistance` by the enum, [?: field identity inferred]) + byte +0x11 = 0. Ease-out with
exponent 2: `alpha = 1 − (1 − t/0.5)²` [U]. The camera is object type CAMERA, which the aim slow motion does not
touch: the blend and the stick run in real time.

End: `vt+0x798` = `TresCameraNormal` 0x140605A10 → `FUN_1405CA980`: marks the camera ended (+0x468 |= 0x10),
removes it from the manager with the blend, and (bit 0x04) destroys it. Called from `StartCommand` (fire /
airstep), `Exit`, and the end of mode 2 if still alive.

#### A5.4 Turning the view [I: slot 301 0x140607ED0 passes type 3 and `this+0x844`; slot 258 0x1405CD3F0 C]

```
mode = cam+0x844 = 2;  type = 1 (forced by slot 259 override 0x140606750)
zero if camera input is disabled (cam+0x469 bit 0x02 clear, manager+0x2A34 bit 0x80 clear)
     or the controller ignores look input (ctrl vt+0x8A0)
(rx, ry) = GetInputAnalogStickState(right stick)                 // engine dead zone 0.25 [A], as the field camera
left = false
if |rx| <= 1e-8 && |ry| <= 1e-8: (rx, ry) = GetInputAnalogStickState(left stick);  left = true      // mode 2
if !left: ry = -ry
X = sign(rx) * max((|rx| - 0.1) / 0.9, 0);   Y = sign(ry) * max((|ry| - 0.1) / 0.9, 0)
if config byte0 & 0x10: X = -X;   if config byte0 & 0x20: Y = -Y    // type-1 reverse options (shipped byte0 = 0xC3: off)
if |manager roll offset| > 90: X = -X, Y = -Y
// no sensitivity scale: slot 259 applies it only for type 3 (or when the manager override +0x2A38 >= 0)
stickDelta.yaw = X * 120 * dt;   stickDelta.pitch = Y * 120 * dt     // slot 266, m_RotationSpeedX / Y
rotTarget += stickDelta;   rotTarget.pitch = clampAngle(pitch, -70, +70);   no yaw limit
rotCur follows rotTarget with rotLerpStep(rate 8 /s)                 // camera spec 2.4
```

- The left stick is free for this because the controller dispatches no movement while aiming (A1).
- Left stick up → pitch up (look up); right stick up → pitch down, as with the field camera (same sign caveat as
  camera spec 2.5).
- Full deflection = 120 °/s (field camera 149.1 °/s at the shipped sensitivity); output starts at raw deflection
  0.25 + 0.1 × 0.75 = 0.325.
- `dt` is the camera manager's (object type CAMERA): real time.
- Which menu options are config byte0 bits 0x10 / 0x20 was not resolved [?].

---

### A6. Slow motion: the SQEX object-type time dilation

Engine modification, not `GlobalTimeDilation` and not per-actor `CustomTimeDilation`:

- Every actor has `m_ObjType` (`ESQEX_ObjectType`, Actor+0x3B0) and `m_bDisableObjTypeTimeDilation` (Actor+0x3B4
  bit 0) [A reflection]. Actor slot 204 (0x143EAE9C0) returns `GameState.SQEX_ObjTypeTimeDilation[m_ObjType]`
  (GameState+0x420, 16 floats) or 1.0 when disabled [I].
- `FActorTickFunction::ExecuteTick` 0x143E968D0: `TickActor(DeltaTime × CustomTimeDilation × slot204())` [I].
  `AActor::GetActorTimeDilation` 0x143E98A80 = `slot204() × CustomTimeDilation × WorldSettings (+0x630 × +0x634 ×
  +0x638 × +0x63C)` [I]. Component tick functions use the same factor [C: callers of slot 204 at 0x143FCDC45 …,
  not read].
- The game state keeps, per channel (`ETresObjTypeProcChannel`, 16) and per type (16), an entry
  {current, target, blendLeft} at `GameState + 0x52C + channel × 0xC0 + type × 0xC` [I 0x144119D90, 0x14411A180].
  - `vt+0x8A8 SetTypeRates(channel, mask, rate, blend)`: for every type bit in `mask`: target = max(rate, 0),
    blendLeft = blend.
  - `vt+0x8A0 SetTypeRate(channel, type, rate, blend)`: one type.
  - Tick 0x14411A8D0 (dt > 0, world not paused) [I]: per entry `if cur != target: if blendLeft <= 0: cur = target
    else: cur = clampToward(cur + (target − cur) / blendLeft × dt, target); blendLeft = max(blendLeft − dt, 0)`;
    then per type `SQEX_ObjTypeTimeDilation[type] = min over channels(cur, 1) × max over channels(cur, 1)`,
    snapped to 1 when within 1e-4.
- Wrappers (world from the pawn, `GameState` = World+0x110) [C]:
  `FUN_1405DA460(pawn, ch, mask, rate, blend)` → vt+0x8A8; `FUN_1405DA3E0(pawn, ch, blend)` → vt+0x8A8(ch,
  0x7FFFFFFF, 1.0, blend); `FUN_1405DA4E0(pawn, ch, type, rate, blend)` → vt+0x8A0.

The aim mode uses channel 1 = `CH_PLAYER`. The "id" 0x4D9E of the first pass is the **type mask**:

| bit | type | slowed |
|---|---|---|
| 1 | PLAYER | yes — Sora (pawn `m_ObjType` = 1, 0x141423382 [I]) |
| 2 | ENEMY | yes |
| 3 | GIMMICK | yes |
| 4 | AVATAR | yes |
| 7 | MAP | yes |
| 8 | BG | yes |
| 10 | FRIEND | yes |
| 11 | VEHICLE | yes — also the **player controller** (`m_ObjType` = 11, constructor 0x1413E6997 [I]) |
| 14 | PHYSX | yes |
| 0 NONE, 5 PROJECTILE, 6 PRIZE, 9 CAMERA, 12 EVENT, 13 UI, 15 DEBUG | | no |

- Rate 0.4 (ctrl+0xA40), blend 0 → takes effect at the game state's next tick, no ramp.
- Sora's animation, movement and gravity integrate with 0.4 × dt; so do enemies and their animation. Projectile
  actors (`m_ObjType` 5, base constructor 0x141470399 [I]) and the camera (9, 0x1405C03DC [I]) are not slowed.
  Particle effects follow the type of the actor that owns them [C: not traced per effect class [?]].
- Because the controller is slowed too, slot 800 receives `dt × 0.4` and divides by `GetActorTimeDilation(ctrl)`
  (start delay, aim time) or by ctrl+0xA40 (lock interval) to count real time.
- **At the fire** `StartCommand` sets type PLAYER back to 1.0 only; ENEMY … PHYSX and the controller stay at 0.4
  until the state's End clears flag 0x02 and mode 2 calls `ResetTypeRates` — the whole shotlock attack plays with
  Sora and his projectiles at full speed against a 0.4 world (plus the attack's own 0.1 slow motion
  `FUN_14140E500`, mask 0x419C, S3).
- A cancel resets every type to 1.0 at once.
- Sound: `FUN_141515D60(2)` / `FUN_141515C90(2)` post the sound-system states `STATE_SLOW_START` /
  `STATE_SLOW_END` (`FUN_14580EE30(FName)`) [C]; what the sound data does with them (pitch, filter) is in the
  sound assets [?]. `STATE_SLOW_END` is sent at the fire, at the airstep and at a cancel.
- UI: type UI is not in the mask; the HUD runs in real time.

---

### A7. What Sora does while aiming

- **No state is pushed, no animation is played by the aim mode** [I: slot 800 and `Enter` contain no state push
  and no animation call; the string `ShootFlowIdle` does not exist in the executable; `ShootFlowStart` is used
  only by the ShootFlow-state kind functions 0x1413B6960, 0x1413B8FC0, 0x1413BC250 and the gun routine
  0x1413B6D50; `ShootFlowEnd` by the attack routines]. The clips 1451 / 1452 belong to other shotlocks; for the
  Kingdom Key only `ShootFlowEnd` (1453) is used, by the attack state.
- **No requests**: with ctrl+0xA10 ≠ 0 the input update skips `MoveInput`, `TryRun`, `JumpRequest`, `JumpHeld`,
  attack, guard and the command menu input (A1) [I 0x1413F2495]. The stick is still read into ctrl+0x848..0x85C.
  - Ground, Run: no request → the Run state's own "no request" path: brake and `RunEnd` / `WalkEnd` → Idle
    (movement spec 3.1), at 0.4 speed.
  - Ground, Idle [C 0x1413944D0]: the idle clip is chosen by `pawn.vt+0x1C68(out, weaponDrawn = pawn.vt+0x1330(),
    bBattle, bLockon)` (0x141433F80: `IdleN`; both first flags → `IdleB`; all three → `IdleLockon`; `DiveIdle`
    when diving) with `bLockon = Idle+0x91 = ctrl.IsShootLockMode() | ctrl+0x960`; the clip is re-picked when that
    value changes. So **in battle stance the aim mode shows `IdleLockon` (body/1018, 1.3333 s)**, otherwise
    `IdleN`. No turn: Idle keeps the actor's yaw.
  - Air (Jump state): no `MoveInput`, no `JumpHeld` → the jump is not sustained, no air steering; gravity acts
    normally on a 0.4 clock. **No hover.** Landing goes to Idle as usual. (Entry is refused in Jump sub-step 9,
    the dive start.)
  - Swimming at the surface: the mode opens but no lock can be taken (A3.3); diving refuses it.
- **Facing**: nothing turns the pawn toward the camera or the targets while aiming [C]. The camera is placed
  behind him instead (A5.3). The attack state turns him to the first locked target.
- Lock-on: the normal lock-on is cleared at entry (A3.2).
- Lens effect [A, C]: `pawn.m_LensEffectClass` (pawn+0x1518) = `Effects/PC/shootLock/bp/BP_slc_camatch00`
  (`TresCameraEmitterLensEffect`, particle `Effects/PC/shootLock/par/vp_slc_cam_atch0`, relative X 50).
  `FUN_141415C30(ctrl, respawn)`: removes the instance in ctrl+0xA88 (`FUN_1405EAD60`) and, with `respawn`, adds
  a new one through the camera manager (`vt+0x8C8`, the engine's add-lens-effect [U]). Called with `respawn =
  true` at entry, at a cancel and at the airstep; with `false` at the fire.

---

### A8. The special routine `FUN_141421190` (not Kingdom Key)

Flag 0x80 is set in `Enter` only when the selected kind is 29 or 39 [I 0x141419766..0x141419777]. Kind 29 comes
from weapon kind 15 (row `アルテマ`, Param0 1), kind 39 is Kairi (row `シャイニング`, Param0 1). Kingdom Key is
weapon kind 1 → kinds 1 / 2 → the flag is never set. The routine [C, skimmed] is a single-target lock: one
candidate in the reticle is charged (`ctrl+0xA48 += dt / rate`, HUD gauge `FUN_14158C800`) and at 1.0 becomes
the one lock (`total = 1`, `hud.FUN_1415895F0(1, 1)`); it refuses to run while swimming.

Mode 5 (`ctrl slot 812 0x141419FA0`, callers 0x140E9E3D0, 0x140EBE400, 0x14115BD90) is a second entry that starts
the aim mode without the R1 hold and with kind forced to 1; slot 800 has no branch for mode 5 [C]. Not on Sora's
R1 path; its use was not traced [?].

---

### A9. HUD calls (names of the values only)

| Call | Values |
|---|---|
| `FUN_141587480(hud, aimTime, bFocusEmptyMode, kind)` | aim-time gauge start: seconds, mode, shotlock kind |
| `FUN_141584140(hud, on)` | aim HUD (reticle) on / off |
| `FUN_1415759C0(hud, comp, type, 1, -1)` | add a marker on a component: type 2 shotlock lock, 3 focus-syphon lock, 0 lock-on |
| `FUN_14157A7A0(hud, comp, type, -1, 0)` | remove markers: type 8 all shotlock / airstep markers, 6 airstep start point |
| `FUN_1415895F0(hud, total, max)` | lock counter and its maximum |
| `FUN_14158A860(hud, ownerPawn or null, 0, -1)` | enemy gauge target |
| `FUN_14158B420(hud, 0 or 1, enabled)` | button prompts: 0 = fire available, 1 = airstep available |
| `FUN_14158C380(hud, kind)` | shotlock command name (changes at MAX) |
| `FUN_141577520(hud, n)` | aim HUD close animation: 1 cancel, 2 fired, 3 attack finished, 4 airstep, 5 airstep finished, 6 no-control variant |
| `FUN_141589650(hud, 0, data, flag)` / `FUN_141577D80(hud, bNow, time)` | command-name banner on / off (off after `time` s) |
| `FUN_141577ED0(hud, 0)`, `FUN_14158C800(hud, charge)`, `FUN_1415898A0(hud, comp)` | special-routine gauge (kinds 29 / 39) |
| `FUN_141577490(hud, 0, …)` | timing-gauge rank display off |

Roles are inferred from the call sites [C]; the HUD functions themselves were not read.

---

### Open questions

1. `gi+0x84C` and pawn+0xD80 bit 3 (two early returns of slot 800); `Damage+0x88 == 1` (the one damage kind that
   does not cancel); ctrl+0xBB8 / +0xC08 / +0xC09 / +0xC18 / +0xB78 bit 3 exact meanings.
2. `ULocalPlayer+0x1B0` / `+0x420` semantics (assumed: last view-projection matrix and view rectangle).
3. Aim camera FOV (native component default assumed 90); the SQEX blend-interp byte (2) and what
   "LookRotationDistance" interpolates; cam+0x468 bit 0x40.
4. Config byte0 bits 0x10 / 0x20 (reverse options used by stick type 1) — menu names.
5. Keyboard key that emulates `Gamepad_RightShoulder`.
6. Sound-side effect of `STATE_SLOW_START`; whether particle systems of slowed actors are slowed.
7. FOCUS_BOOST (ability 42): no reader found in this path.
8. Requests behind iface slot 29 and pawn slot 621 (refused while aiming); ctrl slot 753 (`vt+0x1788`) in `Exit`.
9. Mode 5 entry (slot 812): who uses it.
10. Frame-rate: at most one lock per tick — confirm the observed lock rate at 30 fps equals 0.0933 s (it should:
    interval > frame).

### Runtime probe requests

| Object | Offset | Type | Settles |
|---|---|---|---|
| ctrl | +0xA08 u32, +0xA0C f32, +0xA10 i32, +0xA18 i32, +0xA1C i32, +0xA24 f32, +0xA28 i32, +0xA38 f32, +0xA3C f32, +0xA40 f32, +0xA4C i32, +0xB28 i32, +0xB2C i32, +0x964 i32 | | whole aim-mode state per frame: entry threshold, start delay, cadence, MAX, consumed focus |
| ctrl | +0xAF0, +0xB10, +0xB00, +0xB20 | i32 | list sizes (candidates fixed after entry?) |
| ctrl | +0x3B0 | u8 | controller object type (expect 11) |
| pawn | +0x3B0 u8, +0x3B4 u8 | | pawn object type (1) and the disable bit |
| pawn | +0xD80 | u32 | bit 3 (early return) |
| pawn | +0x614 i32, +0x7C0 u8 | | focus before / after, focus-empty flag |
| GameState (World+0x110) | +0x420 f32 × 16; +0x52C + 0xC0 (channel 1): 16 × {f32, f32, f32} | | effective type rates and the CH_PLAYER entries during aim, at fire, at end |
| Sora actor, an enemy, a projectile, the camera actor | `GetActorTimeDilation` or tick dt | f32 | who is slowed |
| `m_CameraShootFlow` (ctrl+0x868) | +0x800, +0x804, +0x428, +0x66C (3), +0x678 (3), +0x828 (3), +0x880, +0x884, +0x898 (3), +0x844, +0x8F0, +0x450, +0x438, +0x43C, +0x430, +0x434, +0x468, +0x3E8 | f32 / i32 / u8 | every camera parameter of A5 |
| aim camera component (cam+0x3E0) | `FieldOfView` | f32 | FOV (90?) |
| camera manager | +0x2868 f32, +0x286C u8, +0x2870 f32, +0x2874 u32, +0x2878 u8, +0x2879 u8 | | blend block as stored |
| ULocalPlayer (ctrl->Player) | +0x1B0 f32 × 16, +0x420 i32 × 4 | | projection matrix and rectangle of the screen test |
| GameInstance+0x870 | +0x2E8 u8 | | config byte0 (bits 0x10 / 0x20) |
| `TresPlayerState_Damage` | +0x88 | i32 | which damage does not cancel |
| `TresPlayerState_Idle` | +0x91 | u8 | IdleLockon selector |
| current state id (iface+0x150 → id) | | i32 | confirm no state change at aim entry |
| pawn velocity / location while aiming in the air | | vec3 | no hover, 0.4 clock |

### What the host must provide

- R1 as a held button (`bHold`) every controller tick, attack and airstep buttons as "just pressed", and the
  controller's dilated `dt` plus its time dilation (or simply real `dt` and rate 1 with the divisions removed).
- A per-object-type time scale: at least "player", "enemy / world", "projectile", "camera / UI" groups with
  set(mask, rate) / set(type, rate) / reset, applied to actor tick dt.
- The list of lock-on target components at entry: handle, alive flag, enabled && shotlock permission, team,
  owner (tag `Gimmick`, is-character), world position each tick.
- Sora's reference points: origin + (0, 0, 100) (range at entry, trace start) and his own lock-on point (range
  while aiming).
- The view-projection matrix and view rectangle of the last rendered frame.
- A line trace (segment test) against static / dynamic world geometry and gimmicks that block projectiles,
  ignoring Sora and the target's actor chain.
- Focus get / set / max, the focus-empty flag, `MyFocusPointConvertRate`.
- A camera stack: push / pop a camera with a 0.5 s ease-out (exp 2) blend; the aim camera is the field-camera
  model with the parameter set of A5 and a stick source "right stick, else left stick".
- Commands to the state machine: `TryShootFlow` (push state 41 with kind and the locked list) and its End callback
  (clear flag 0x02); state id and sub-step queries for the gate.
- HUD sink for the calls of A9; sound-state sink (`STATE_SLOW_START` / `STATE_SLOW_END`); lens-effect add / remove.

---

# Part G — Second pass: Kingdom Key shotlock attacks and focus syphon (supersedes D S3, extends D S1)

_Source of this part: `build/sora_tmp/magic/kc2/spec.md` (section numbers are local to the part)._

Replacement for Part D section S3 of `docs/sora-magic-spec.md` and additions to S1. Units cm, s, deg; axes X forward,
Y right, Z up (left-handed, Unreal). Tags: [I] instruction level (address), [C] decompiled C only, [U] stock UE 4.17 by
function identity, [A] asset (file, property), [D] derived by arithmetic, [?] not determined statically.
Every number is a row of `params.json` (ids `kc2.*`); assets are in `assets.json`. Disassembly listings used are the
`dis_*.txt` files of this folder.

### 0. What changes against the first pass (read this first)

| # | First pass (Part D S3 / S1) | Now established |
|---|---|---|
| 1 | sub-step 3 waits for the lift, then 4 | sub-step 3 **falls through** into the sub-step 4 code in the same tick; the lift is not a gate [I 0x1413B8DED]. With dt >= 0.002 s the lift is one swept teleport on the second tick [I 0x1413C5481..0x1413C54DD]. |
| 2 | ring: "yaw + i*360/N, pitch 60" | the angle is added to the **roll** of the rotator of the horizontal axis `a` (pawn → keyblade tip), then composed with pitch 60: a cone of half angle 60 deg around `a`, not a horizontal ring [I 0x1413C3DA6, 0x1413C1672]. The whole formation also rotates 180 deg/s (kind 1 one way, kind 2 the other). |
| 3 | spawn direction uses the projectile index | the spawner uses the index **inside the tick** (0..6); the formation function re-places every projectile by its list index from the next tick on [I 0x1413C1641, 0x1413C3D8D]. |
| 4 | target = locked[i mod n] | same, and when that entry's component is gone the projectile gets **no** target (it does not fall back to the first component) [I 0x1413C18FC]. Per-target lock counts are not used. |
| 5 | KC shot "speed 1500 → 4500", spin roll 30 for 0.1 s | `m_Accel` is 0 → constant 1500 cm/s (4500 is only the cap); `m_bEnableSpin` is false in the native constructor and in both blueprints → `m_Spin` / `m_SpinMaxTime` are inert [I 0x141470EDE, 0x1414787C6; A]. |
| 6 | Ragnarok projectile = `TresProjectileBase` | its class is the native **`TresProj_Ragnarok`** (helix motion around the homing path), vtable 0x1469BAEE0 [A import table; I]. MaxSpeed is the native 2000. |
| 7 | Ragnarok slots 132 / 133 open | read: identical to kind 1 except the spin sign, weapon glow and effect group. No finishing burst; no notify handler takes part. |
| 8 | focus syphon slot 137 open | read (section 4): no projectile, no damage; focus += trunc(rate * total locks), min 1, 0.6 s after the `TresShot` notify. |
| 9 | int16 focus bonus source open | it is zeroed by the `UTresChrBaseParam` constructor and has no writer; max focus = `m_MaxFocusPoint` (section 5). |
| 10 | game flag 6 open | option set B bit 6 (0x140EAB160) — an option of the game instance, not an ability (section 5). |

### 1. `TresPlayerState_ShootFlow` (state id 41) — common part

Vtable 0x14693CA58; anim interface sub-object at state+0x40 (vtable 0x14693CF98).

#### 1.1 Fields [I unless noted]

| Offset | Type | Meaning | Evidence |
|---|---|---|---|
| +0x68 | ptr | `MyCharPawn` | |
| +0x78 | i32 | `m_nSubStep` | 0x1413B8CA0 |
| +0x84 | u32 | bit 0 `m_bEnableChange`, bit 4 "face request" (set in sub-step 0) | 0x1413B8F7B, 0x1413C5203 |
| +0x88 / +0x8C | i32 | command id (0xFD) / `ETresShootFlowKind` | Begin 0x1413A5DA4.. |
| +0x90 | i32 | global spawn counter | 0x1413C19E2 |
| +0x94 | i32 | N = total locks | 0x1413C3444 |
| +0x98 | i32 | projectiles per tick (7) | 0x1413C347B |
| +0x9C | i32 | projectiles still to spawn | 0x1413C3485 |
| +0xA0 | f32 | ring timer (0.5 → 0) | 0x1413C34B4 |
| +0xA4 | f32 | syphon timer (0.6 → 0) | 0x1413B7A40 |
| +0xA8 / +0xAC | f32 | face timer / face yaw | 0x1413BDB60, 0x1413C536F |
| +0xB0 / +0xB1 | u8 | weapon kind (pawn vt+0x12E0(−1)) / pawn vt+0x12E8(−1) at Begin | 0x1413A5EB8 [C] |
| +0xB8 | FName | projectile name (`KC_SHOT`, `KC_RAGNAROK`) | Begin [C] |
| +0xC0 | i32 | effect group passed to Play (0x631 for kinds 1, 0x20, 0x22; 0 for kind 2) | 0x1413A63BC |
| +0xC4 | i32 | voice index for the anim interface (0x35 for kind 36) | 0x1413A6683 |
| +0xCC | u32 | bit 0 ring active (kinds 1/2) or "shot notify seen" (kind 36), bit 3 facing active, bit 4 weapon glow on, bit 5 "face time 0" | 0x1413C3474, 0x1413BDB60, 0x1413B8C51, 0x1413C520A |
| +0xD0 | ptr | `m_pComponent` = first locked component | 0x1413C343D |
| +0xE4 | vec3 | volley origin | 0x1413C34A0 |
| +0xF8 | vec3 | lift target | 0x1413AD64C |
| +0x128 | TArray | `m_ProjectileList` | 0x1413C19D0 |
| +0x138 | TArray | `m_EffectList` (syphon particles) | 0x1413BFFC0 [C] |
| +0x158 | u8 | controller vt+0x1958() at Begin / set-up | 0x1413C345B |
| +0x170 | delegate | per-kind update function | Begin [C] |

#### 1.2 Begin 0x1413A5CD0 [C; constants and call order I 0x1413A5D41..0x1413A5ED0]
```
pawn.vt+0x1780(1)            // pawn+0x6E4 = 1: battle (weapon-out) animation set afterwards   [I 0x14054EA00]
pawn.vt+0x16E8()             // StopMovement
pawn.vt+0x1738(1)            // battle mode
pawn.vt+0x1340(1, 1, 1.0)    // draw the keyblade
pawn.vt+0x2068(1)            // pawn+0x630 bit 2 = damage invalid (invincible)              [I 0x1405B9120]
pawn.vt+0x19E0(1)            // movement +0xB70 bits 2,3,4 = 1: overlapped-force volumes off [I 0x1405943C0/E0, 0x140594400]
cmd  = pawn.vt+0x1DA8();  kind = pawn.vt+0x1DB8();  pawn.vt+0x1D38(cmd, kind)
N    = ctrl.vt+0x1918()      // total locks (ctrl+0xA18)
if ctrl.lockedTargets (ctrl+0xB10) < 1: Finish (vt+0x270); return
m_pComponent = component of locked[0] (weak pointer, null if dead)
+0x158 = ctrl.vt+0x1958();  perTick = N;  +0xB0 = weapon kind;  faceYaw = actor yaw
by kind:  1 → update = vt+0x408 (0x1413B8C90), name "KC_SHOT",     effectGroup = 0x631
          2 → update = vt+0x410 (0x1413B88F0), name "KC_RAGNAROK"  (effectGroup stays 0 from Reset)
         36 → update = vt+0x448 (0x1413B78B0), voice index = 0x35, no projectile name
```
The movement flags of `vt+0x19E0`: the getters 0x140589250 / 0x140589270 / 0x140589290 return "force applies" only when
neither the run-time bit nor the matching `m_bIgnoreOverlappedForce…` class flag is set; they gate the force-volume
code 0x14059BA50 / 0x14059BD20 [I, C]. A host without wind / current volumes can ignore the call.

#### 1.3 Tick 0x1413B0D10 [I]
```
if pawn valid:
    pawn.vt+0xDC8(dt*12)         // → pawn.vt+0x21B0(&DAT_14871E410, alpha)   [C; not read further]
    pawn.vt+0xDB8(dt*12)         // slerp of the actor rotation to (pitch 0, own yaw, roll 0), alpha clamped 0..1  [C 0x1405C3E40 → 0x1405DDD90]
    update(dt)                   // delegate at +0x170
    PostTick(dt)                 // vt+0x388 = 0x1413C51D0
```
PostTick [I 0x1413C51D0]:
```
alpha = dt*12
if flags84 & 0x10:                       // requested by sub-step 0
    flags84 &= ~0x10
    SetFace(true, (flagsCC & 0x20) ? 0 : dt*12)       // vt+0x390 = 0x1413BDB60: flagsCC bit3 = on, faceTimer = time
    alpha = 1.0                                         // snap on this tick
if flagsCC & 8:
    if m_pComponent: d = comp.vt+0xB88() − pawn.vt+0x6E0();  d.z = 0;  normalise if |d|² > 1e-8
                     faceYaw = Rotation(d).yaw
    pawn.vt+0xDB0(Rotator(0, faceYaw, 0), alpha)        // slerp towards that yaw
    faceTimer −= dt;  if faceTimer <= 0: SetFace(false, 0)
```
So Sora snaps to the first locked target on the first tick and keeps turning to it (alpha 12·dt) while the face timer
runs: the timer is 12·dt and loses dt per tick = **12 ticks** (0.2 s at 60 Hz) [D]. The volley set-up also switches
facing off (1.5). Frame-rate note: both "12·dt" terms are per tick; a fixed 60 Hz step reproduces the game at 60 fps.

#### 1.4 Lift helpers
`vt+0x398(h)` = 0x1413AD550 [I]:
```
hit = FloorSweep(pawn, forward 0, length h + 300)        // 0x14063A650
if hit:  if !(hit.dist < h): return false                // comiss/jb at 0x1413AD5BC
else:    if !pawn.movement.vt+0x9D8(): return false;  hit.dist = 0     // 0x1405ADAA0 → movement ground query [C, body not read]
liftTarget = pawn.location;  liftTarget.z += h − hit.dist
return true
```
`FloorSweep` 0x14063A650 [C; constants I]: sphere radius 50 swept from `location + (0,0,55)` down by `length`
(channels 0, 1 and 0x12 blocking, the pawn ignored); the first blocking hit of another actor whose surface is not
steeper than 45 deg (slope direction derived from the impact normal, pitch > −45) gives `dist = pawn.z − impactPoint.z`;
a start-penetrating hit ends the search with no result. So `dist` is the height of the pawn centre above the floor point.

`vt+0x3A0(dt)` = 0x1413C5400 [I]:
```
dz = liftTarget.z − pawn.z
if dz > 0 and dt*500*dz < dz:  StopMovement;  movement.Velocity.Z = dt*500*dz;  return false
StopMovement;  SetActorLocation(liftTarget, sweep = true);  return true      // 0x143EAC3D0(pawn, &target, 1, 0, 0)
```
`dt*500*dz < dz` only holds for dt < 0.002 s, so in practice the pawn is moved in one swept step [D]. Result: if Sora's
centre is lower than 500 cm above the floor at the start, he is placed 500 cm above it on the second tick
(blocked by ceilings through the sweep); if no floor is found within 800 cm and he is airborne he stays where he is.

#### 1.5 Volley set-up `vt+0x3F0` = 0x1413C3330 [I] (also slot 131 0x1413C3320 → jmp vt+0x3F0)
```
if subStep == 8:  N = table[kind].Param3 (rank other) / Param1 (rank 3) / Param2 (rank 2)     // timing-gauge kinds only, not KC
else:             N = ctrl.vt+0x1918();  if N > 0: m_pComponent = component of locked[0] (may become null)
                  if ctrl.vt+0x1958(): +0x158 = 1
flagsCC |= 1;  perTick = 7;  remaining = N
origin = pawn.vt+0x1410(0)       // equipment vt+0x580 → weapon 0 mesh socket "ATK_01"   [I 0x1417812E0, 0x141777B6E, FName literal 0x146B7BD30]
ringTimer = 0.5
SetFace(false, 0)
```
`ATK_01` of the Kingdom Key skeleton: bone `wep`, relative location (79.0368, 0, 0) = the tip of the blade
[A `Character/wep/w_so010/mdl/w_so010_0_Skeleton` : SkeletalMeshSocket_2]. The origin is sampled once.

#### 1.6 Spawner `vt+0x3F8` = 0x1413C1350 (kind 1) / `vt+0x420` = 0x1413C0D60 (kind 2) [I]
Called at the end of every update tick. The two functions are the same code except that the kind-1 one has an extra
block for kind 0x26 (Aqua's shotlock, effect colours).
```
if remaining <= 0 or pawn invalid or !pawn.vt+0x1330(): return       // equipment vt+0x4A0: bit 1 of equipment+0x2C8  [I 0x141784670]
team = pawn.vt+0xD90();  nT = ctrl.lockedTargets (ctrl+0xB10);  target = m_pComponent
for i in 0 .. perTick−1:                                              // i restarts at 0 every tick
    if remaining <= 0: return
    a = normalize_xy(origin − pawn.vt+0x6E0())       (left un-normalised when |.|² <= 1e-8)
    R = Rotation(a);  R.roll += (360 / N) * i
    dir = Vector(Rotator(Quat(R) * Quat(Rotator(pitch 60, 0, 0))))
    p = pawn.vt+0x1450(name, origin, origin, &dir, team, bFireNow = 0)     // equipment vt+0x5B8 → projectile manager Spawn
    if !p: continue                                                   // counters unchanged
    p.vt+0x1100(cmd, kind)                                            // SetCommand, section 3.4
    if nT > 0:
        e = locked[spawnIndex % nT]                                   // entries of 0x34 bytes at ctrl+0xB08
        target = e.component if alive else null                       // weak pointer at e+0x1C
    if target: p.vt+0x1220(true, target)                              // SetHomingTargetComponent
    p.vt+0x11F8(origin)                                               // SpawnFireEffects (the blueprints have none)
    p.vt+0x1270()                                                     // StartFire
    m_ProjectileList.push(p);  spawnIndex += 1;  remaining −= 1
```
- 7 projectiles per tick: 24 locks are out after 4 ticks (7, 7, 7, 3) [D].
- Target assignment is round robin over the **locked targets** in list order; `entry.lockCount` (+0xC) is not read
  [I: only +0x1C / +0x20 of the entry are loaded, 0x1413C190A..0x1413C191B]. With 2 targets and 5 locks the split is
  3 / 2 whatever the per-target counts were.
- The projectile set of the weapon resolves the names [A `Blueprints/Player/p_ex001/w_so010_ProjSet`]:
  `KC_SHOT` → `TresProjKC_Shot_C`, `KC_RAGNAROK` → `TresProjKC_Ragnarok_C`.

#### 1.7 Formation `vt+0x400` = 0x1413C3A80 (kind 1) / `vt+0x428` = 0x1413C34D0 (kind 2) [I]
Called first thing in sub-step 5, i.e. from the tick after the set-up.
```
if !(flagsCC & 1) or list empty: return
ringTimer −= dt;  if ringTimer <= 0: flagsCC &= ~1; ringTimer = 0
t = ringTimer
a = normalize_xy(origin − pawn.vt+0x6E0())
for idx, p in m_ProjectileList (idx counts dead entries too):
    if p invalid: continue
    old = p.location
    R = Rotation(a);  R.roll += (360 / N) * idx  −  (0.1 − t) * 180        // kind 1   [subss 0x1413C3DAC]
                      R.roll += (360 / N) * idx  +  (0.1 − t) * 180        // kind 2   [addss 0x1413C37FC]
    dir = Vector(Rotator(Quat(R) * Quat(Rotator(pitch 60, 0, 0))))
    pos = origin + dir * (0.5 − t) * 400
    p.SetActorLocation(pos, sweep = false)
    d2  = Rotation(pos − old)
    p.SetActorRotation(Quat(d2))                                            // 0x143EAD110
    p.vt+0x1208(Vector(d2))                                                 // SetDirection: Velocity = dir * InitialSpeed
    p.vt+0x1240(t + 0.066666)                                               // SetMoveStartDelay
```
Closed form of `dir` [U: FRotationMatrix axes; D]: with `right = (−a.y, a.x, 0)`, `up = (0,0,1)` and the angle φ
(the roll, degrees):
`dir = 0.5·a + 0.8660254·( sin φ · right + cos φ · up )`.
So the projectiles sit on a circle of radius `0.866·r` in the plane perpendicular to `a`, pushed `0.5·r` along `a`,
with `r = (0.5 − t)·400` growing linearly from 0 to 200 cm in 0.5 s (circle radius 173.2, forward offset 100) [D],
φ_idx = idx·360/N + phase, phase running from +72 deg to −18 deg for King of Hearts (−180 deg/s) and from −72 to
+18 deg for Ragnarok (+180 deg/s) [D]. Index 0 starts near the top.
The velocity direction each projectile leaves with is the direction of its last formation step.

#### 1.8 End 0x1413ABA70 [C; call arguments I 0x1413ABAFD..0x1413ABBCA] and Reset 0x1413AD3E0 [C]
```
pawn.vt+0x22E8()                     // movement vt+0xBD0  [C]
pawn.vt+0x1628(); pawn.vt+0x1648()   // MaxFlySpeed / MaxAcceleration back to their defaults  [C 0x1405B62D0, 0x1405B6280]
pawn.vt+0x1728(1)                    // gravity on
if OnGroundAndNotFloating (0x1405AD850): pawn.vt+0x17A0(1, true)     // Walking, stop first
else:                                    pawn.vt+0x17A0(3, false)    // Falling
FUN_144042970(pawn.vt+0xB58(), 1, 1)
pawn.vt+0x2068(0)                    // damage valid again
pawn.vt+0x19E0(0)
FUN_1405D4D90(pawn, 0, 1)
if pawn.vt+0xD70() == 1 (Sora):
    pawn.vt+0x1388(0, 0, 0)
    if +0xB0 == 0: (if flagsCC & 0x10: flagsCC &= ~0x10; pawn.vt+0x1498(0, 1.0))      // weapon glow off
    else: pawn.vt+0x1408(); pawn.vt+0x13C8(pawn.vt+0x12C8(−1), +0xB0, +0xB1, 0, 0.0, 0, 0); +0xB0 = 0
ctrl: thunk 0x1414122A0(ctrl, 0, 0, 0, 0);  0x14140E500(ctrl, 0) (release pending off);  0x141415E70(ctrl, 0);  ctrl.vt+0x1948()
m_ProjectileList emptied (the projectiles live on);  every m_EffectList entry vt+0x368() and the list emptied;
the weak effect array at +0x148 likewise
```
Reset: base reset, command 0, name None, flagsCC bits 0, 1, 2, 4, 6 cleared (and 0xFFFFFFA8) then bits 3, 5 (and 0xFFFFFFD7), all counters / timers 0,
vectors zero, lists emptied, +0x158 = 0 [C].

Ending in the air: the state never moves Sora down. After the last animation `vt+0x1388(0,0,0)` and Finish; End turns
gravity on and selects Falling (he is 500 cm up), so the normal fall / landing states take over [I, D].

Interruptions: `pawn+0x630` bit 2 is set for the whole state (Begin → End), which makes the pawn's damage test
`vt+0x2060` report "invalid" (Part C §0 table, slot 1036/1037) → no damage and no hit reaction can interrupt it.
The update functions themselves have no cancel input. `m_bEnableChange` is cleared when `ShootFlowEnd` starts and set
again by its `TresEnableChange` notify (0.300 s in); what the common state code allows from then on is the general
rule of the state machine (states strand) [?]. Begin itself finishes at once when no locked target is left.

`pawn.vt+0x1388(a, b, c)` → equipment vt+0x508 = 0x141790690 [C]: request byte equipment+0x2E0 = a, bit 0 of +0x2E6 = c,
then the weapon state is re-evaluated (max of the request bytes +0x2E0 / +0x2E2 against +0x2E1, applied by equipment
slot 196 0x14177FA40). It is a weapon presentation mode [?]: the port needs it only if the keyblade must change look
while a shotlock runs (probe request below).

### 2. Sequences

Animation play call (all clips): `Play(name, slot 0, blendIn −1 (asset value), mode 2, effectGroup, rate 1.0,
rootScale default)`, then anim-interface `vt+0x1F0(voice)` when the returned length is > 0 [I e.g. 0x1413B8F09..0x1413B8F67].
"animEnd" = anim interface vt+0xC8(0).

#### 2.1 Kind 1 — King of Hearts (`KC_SHOT`), update 0x1413B8C90 [I]
```
sub 0:  pawn.vt+0x1728(0)                // gravity off
        pawn.vt+0x17A0(5, true)          // Flying, movement stopped first
        pawn.vt+0x1388(1, 0, 0)
        Play "M_1473" (group 0x631), voice 0
        flags84 |= 0x10                  // face request
        subStep = 4;  if LiftCheck(500): subStep = 3
sub 3:  if LiftStep(dt): subStep = 4     // then continues with the sub 4 code below in the same tick
sub 4:  if animEnd:
            pawn.vt+0x17A0(5, true)
            SetUpVolley()                // 1.5
            Play "M_1474" (group 0x631), voice 0
            subStep = 5
sub 5:  Formation(dt)                    // 1.7
        if animEnd:
            ctrl 0x14140E500(ctrl, 0)    // release-pending flag off → the controller leaves the aim mode
            Play "ShootFlowEnd" (group 0x631), voice 0
            flags84 &= ~1                // m_bEnableChange = false
            subStep = 2
sub 2:  if animEnd: pawn.vt+0x1388(0, 0, 0); Finish (vt+0x270); return        // no spawner call on this tick
every tick (except that return): Spawner()   // 1.6
```
Timeline, t = 0 at Begin, 60 Hz [A clip lengths; D]:

| t (s) | Event |
|---|---|
| 0 | `M_1473` (2.5 s) starts; gravity off, Flying; invincible; snap to face the first locked target; `se00001_015` (pitch 1.3) and attached loop `se00001_034` (fade-out 0.7) |
| 1 tick | swept lift to 500 cm above the floor (when lower) |
| 0 – 0.2 | keeps turning to the target (12 ticks) |
| 0.889 | charge effect: `TresAttachEffect` → common attach data `ead_shf_tame0`, group 1585 (= 0x631) → `vw_ex027_thu_sgn00` on the weapon, socket `vfx_top` |
| 1.392 | weapon swing SE (alias SWING1, pitch 1.1) |
| 2.5 | `M_1473` ends → set-up (origin = blade tip, N = total locks), first 7 projectiles spawned; `M_1474` (0.7 s): voice (random of `bt1010700sr0`, `…780…`, `…790…`, `…850…`, weights 25 each), `se00001_083` |
| 2.5 – 2.55 | 7 projectiles per tick until N are out |
| 2.5167 – 3.0 | formation: cone / ring opening to r = 200 while rotating −90 deg |
| 2.682 | weapon swing SE (alias SWING2, volume 0.7, pitch 1.1) |
| 3.0 | ring timer reaches 0; last placement, launch delay 0.066666 s |
| 3.0667 | all projectiles start to move together (4 movement ticks later), 1500 cm/s along their last formation step |
| 3.1 | homing starts (homing delay 0.033333 = 2 moving ticks) |
| 3.2 | `M_1474` ends → `ShootFlowEnd` (0.8 s); aim mode clean-up in the controller; `se00001_015` at +0.031 |
| 3.5 | `TresEnableChange` of `ShootFlowEnd` (0.300) |
| 4.0 | `ShootFlowEnd` ends → Finish → End: gravity on, Falling from 500 cm, invincibility off |
| ≤ 7.5 | projectiles expire (`m_MainLife` 5 s counted from StartFire at the spawn) |

The ring timer is decremented by dt per tick from the tick after the set-up, so "3.0" is 30 ticks after the set-up at
60 Hz, ± one tick from float rounding of `0.5 − 30·dt` [D].

#### 2.2 Kind 2 — Ragnarok (`KC_RAGNAROK`), update 0x1413B88F0 [I]
Identical structure and constants; differences only:
- clips `M_1481` (2.3333 s) and `M_1482` (1.0 s), both played with group +0xC0 = 0; `ShootFlowEnd` with group 0;
- sub 0 additionally: if weapon kind (+0xB0) ∈ {1, 0x16, 0x17}: flagsCC |= 0x10; `pawn.vt+0x1498(5, 1.0)` (weapon glow
  on; equipment vt+0x4F8) [I 0x1413B8C39..0x1413B8C5D];
- sub 2 additionally, before the animEnd test: if `m_bEnableChange` and flagsCC & 0x10: flagsCC &= ~0x10;
  `pawn.vt+0x1498(0, 1.0)` [I 0x1413B8926..0x1413B8953];
- set-up `vt+0x418` (0x1413C3320) is a jump to the same 0x1413C3330; spawner 0x1413C0D60; formation 0x1413C34D0 (+ spin).
- N is the total lock count = 24 (the kind is only selected at 24 locks, S2.2) → **24 projectiles, one volley, no
  second wave and no finishing burst**. The only "finisher" is in the data: the `TOSS` damage kind of the row.
- None of the notify handlers takes part: OnTiming 0x1413B57D0 only reacts for kinds 3, 10 (flagsCC |= 1) and 0x2A
  (|= 4); the Shot handler (slot 69, 0x1413B4A70) would spawn one projectile per `TresShot` notify for the "default"
  kinds including 1 and 2, but `M_1473`, `M_1474`, `M_1481`, `M_1482` contain no `TresShot` notify [A]; slot 64
  (0x1413B5430, the handler at interface vt+0x200) sets flagsCC bit 0 and a 0.3 / 0.5 s timer for kinds 4, 0xB, 0x2A
  and its notify does not occur in these clips either [C, A].

| t (s) | Event |
|---|---|
| 0 | `M_1481` starts; as kind 1; weapon glow on; `se00001_015` (pitch 1.3), loop `se00001_034` |
| 0.88 – 2.333 | charge effect `vw_so010_rgn_tam00` on the weapon socket `R_wep_skt` (state notify, group 0; group 2360 → `vw_so150_rgn_tam00` belongs to another keyblade) |
| 1.072 | weapon swing SE (SWING1, pitch 1.1) |
| 2.3333 | set-up, first 7 of 24; `M_1482`: flash `vw_so010_rgn_fls00` on the weapon ("ラグナロク発射"), voice (random of `bt1010700sr0`, `…780…`, `…790…`, `…800…`, `…850…`, weights 20 each) |
| 2.3333 – 2.3833 | 7, 7, 7, 3 projectiles |
| 2.35 – 2.8333 | formation, rotating +90 deg |
| 2.9 | projectiles start (1000 cm/s, +500 cm/s² up to 2000), helix phase 1 begins |
| 2.9333 | homing starts (720 deg/s) |
| 3.3333 | `M_1482` ends → `ShootFlowEnd` |
| 3.6333 | `TresEnableChange` → weapon glow off |
| 4.1333 | Finish → End |
| ≤ 10.33 | projectiles expire (`m_MainLife` 8 s) |

### 3. Projectiles

The generic life cycle is Part B §1 (spawn, StartFire / DoFire, Tick with life counters, level hit, character hit,
Explode, Respawn) and the steering is Part B §2.2; they apply unchanged. What this section adds: the exact data of
the two blueprints, the per-field rules of the movement component at instruction level, and the Ragnarok class.

#### 3.1 Movement component `UTresProjectileMovementComponent` (vtable 0x1469A7C90)
Tick 0x1414854E0 [I]:
```
if vt+0x3F8() or !UpdatedComponent.owner or !vt+0x4E8(): return          // stock "should skip update" tests
prevPos (+0x370) = position;  prevVel (+0x37C) = Velocity
if UpdatedComponent.vt+0x3C0() or !(flags388 & 2): return                  // bit 1 = started (DoFire, comp vt+0x5A8 0x14147DE20)
if m_bWaitMoveStartOnce: clear it; return                                  // one skipped tick
MoveStartDelay(dt)                                                         // vt+0x5D0 = 0x141487FE0
if !(flags388 & 1): return                                                 // bit 0 = active
OnTickMove.Broadcast(dt)
if !m_bDisableMovementProc:
    ScaleAnim(dt)   // vt+0x5F0 0x141478700
    Spin(dt)        // vt+0x610 0x1414787C0
    Homing(dt)      // vt+0x600 0x1414751E0
    vt+0x5F8(dt)    // 0x141478140 (roll follows velocity; off for these blueprints)
UProjectileMovementComponent::TickComponent(dt)                            // 0x144044AC0, calls ComputeVelocity
stuckTimer (+0x3A4): += dt when (flags388 & 0x13) == 0x13 and !(flags388 & 8) and |Velocity| > 0 and
                     |pos − prevPos| <= |Velocity|*0.1*dt; else = 0
```
Field by field:

- **`m_fMoveStartDelay`** [I 0x141487FE0]: `if delay > 0: delay −= dt; if delay <= 0: delay = 0, active = true; else
  active = false`. Nothing happens when the delay is already 0. The setter (projectile vt+0x1240 → component vt+0x5A0 =
  0x141482960) [I]: `active = false; delay = d; if d <= 0: active = true`. While inactive the component does nothing
  at all (no homing, no move, no delay countdown of the homing). The blueprint value 0.533333 is overwritten every
  formation tick by `ringTimer + 0.066666`, ending with 0.066666; 0.066666 (0x3D88882F) is just below 4/60, so at
  60 Hz the 4th movement tick after the last formation tick is the first moving one [D]. Which of the two ticks
  (pawn state, projectile component) runs first inside a frame decides whether that is 3 or 4 frames after the last
  placement [?] (probe).
- **`m_HomingStartDelay`** [C 0x1414751E0 lines 43-61, consistent with Part B]: counted down only on active ticks, clamped
  at 0; homing runs on the tick where it reaches 0 (0.033333 → the 2nd active tick at 60 Hz).
- **Homing gate**: `m_bEnableHoming` (bit 0 of +0x2F0), target flag (+0x34C bit 0) and delay 0. Target point =
  `m_HomingTargetComponent.vt+0xB88()`; if the component object is gone, or both its vt+0xA80() and vt+0xAB0() return
  false, the target flag is cleared for good and the bolt flies straight [C].
- **`m_HomingTurnMax`, `m_HomingTurnAccel`, `m_HomingMaxTurnMax`** [I 0x1414859E0 @0x141485B20..0x141485B69] (curve mode off:
  bit 1 of +0x320 clear, no curve in these blueprints):
  ```
  if TurnMax > 0:
      TurnMax += dt * TurnAccel
      if MaxTurnMax > 0:  if TurnMax > MaxTurnMax: TurnMax = MaxTurnMax
      else if TurnMax >= 21600: TurnMax = 0
  ```
  It runs before the steering of the same tick and only once homing is past its delay. KC shot: 180 → 360 deg/s at
  +360 deg/s² = 0.5 s after homing starts. Ragnarok: constant 720 (accel 0, no cap).
- **`m_HomingMinDist`** [I 0x141486CB0 @0x141486D01..0x141486D3D]: `if MinDist² > 0 and |target − pos|² <= MinDist²:
  return false` → the caller stores the result into `m_bEnableHoming`: homing is switched off permanently inside
  100 cm, the bolt continues straight.
- **Steering** [I 0x141486CB0, 0x141481160; = Part B §2.2 step 3]: `cosMax = −1` when `m_HomingMaxAngle <= 0` (both
  blueprints); `Δ = min(TurnMax·π/180·dt, π)` when TurnMax > 0, else Δ = π; if Δ > 0: `v̂ = normalize(v)`,
  `t̂ = normalize(target − pos)`, `c = v̂·t̂`; `c <= cosMax` → unchanged; `c >= cos Δ` → `v = t̂·|v|`; otherwise v is rotated
  towards t̂ by `2·atan(sin θ · tan(Δ/2))` (θ = angle between them; unnormalised axis, Part B) and renormalised to the
  old speed. Type NORMAL (3-D) because the gravity scale is 0.
- **`m_Spin`, `m_SpinMaxTime`** [I 0x1414787C0]: only when `m_bEnableSpin` (bit 5 of +0x388):
  after `m_SpinStartDelay`, while `SpinMaxTime == 0 or spinTime < SpinMaxTime`: `Velocity = Vector(Rotation(Velocity) +
  m_Spin) * |Velocity|`, `spinTime += dt` — i.e. the rotator is added once **per tick** (not scaled by dt).
  The constructor clears the bit [I 0x141470EDE `and ecx,0FFFFFFDEh`] and neither blueprint sets `m_bEnableSpin` [A] →
  **no spin for KC_SHOT / KC_RAGNAROK**; `m_Spin` (0, 0, 30) and `m_SpinMaxTime` 0.1 are dead data.
- **`m_Accel`, `InitialSpeed`, `MaxSpeed`**: SetDirection (projectile vt+0x1208 = 0x14147B190) `Velocity = dir *
  InitialSpeed` [C]. ComputeVelocity 0x1414729E0 [I 0x141472A62 for the Accel test, C for the rest]: gravity term only
  when `ProjectileGravityScale != 0`; `if m_Accel > 0: v += normalize(v) * m_Accel * dt` (when |v| = 0 the component's
  forward axis is used); then stock `LimitVelocity` 0x14403B060 (clamp to `MaxSpeed` when > 0) [U]. The displacement of
  the tick is the stock `v·dt + (v' − v)·dt/2` (Part B §1 step 4).
  KC shot: Accel 0 (native default) → 1500 cm/s constant, the 4500 cap never binds. Ragnarok: 1000 + 500·t, capped
  at the native MaxSpeed 2000 after 2.0 s of flight [D].

#### 3.2 `TresProjKC_Shot` [A `Blueprints/Projectile/ShootFlow/TresProjKC_Shot`; native defaults I/C from the constructors 0x141470210, 0x141470D50]
Class `TresProjectileBase` (no code of its own).

| Property | Value | Source |
|---|---|---|
| `m_MainLife` | 5 s | blueprint |
| `m_MaxLifeDistance`, `m_ExplodeLife`, `m_AutoStuckShutdownTime` | 0 | native |
| `m_bNoDestroyOnHit`, `m_bExplodeOnTimeOut`, `m_bReSpawnProjectileOnTimeOut`, residues | false / none | native |
| `m_bPermissionLockOn` | false | blueprint |
| root `TresRoot` | sphere r 10, profile `TresProjectile` (native ctor) | blueprint / native |
| attack shape | sphere r 30, profile `TresProjectile`, `m_MapHitType` NOHIT, row `SF_KCSHOT`, sweep on | blueprint |
| particle | `Effects/wep/w_so010/par/vw_sor_rbs_blt4` (auto-activate) | blueprint |
| pawn hit effect / SE | `vw_sor_rbs_hit0` / `se02001_020` | blueprint |
| map hit effect / SE | `vw_sor_rbs_wht0` / `se02001_021` | blueprint |
| `InitialSpeed` / `MaxSpeed` / `m_Accel` | 1500 / 4500 / 0 | blueprint / blueprint / native |
| `ProjectileGravityScale` | 0 | native |
| `m_fMoveStartDelay` | 0.533333 (overwritten by the state) | blueprint |
| `m_bEnableHoming` | true | blueprint |
| `m_HomingStartDelay` / `m_HomingMinDist` | 0.033333 s / 100 cm | blueprint |
| `m_HomingTurnMax` / `Accel` / `MaxTurnMax` | 180 deg/s / 360 deg/s² / 360 deg/s | blueprint |
| `m_HomingMaxAngle`, `m_HomingType` | 0 (no limit), NORMAL | native |
| `m_Spin` / `m_SpinMaxTime` / `m_bEnableSpin` | (0,0,30) / 0.1 / **false** | blueprint / blueprint / native |
| `m_bEnableMapHitNormalCheck` | false | native |

Behaviour summary: no pierce (dies on the first character hit: Explode, hit effect + SE from the attack collision,
actor destroyed at once since `m_ExplodeLife` = 0); no successor, no residue; the root sphere (r 10) collides with
level geometry and every blocking hit stops and explodes it with the map-hit effect (no normal check, Part B §1
step 5); time-out after 5 s: plain shutdown, no effect [Part B §1 step 3]. During the formation the state places the
actors with `sweep = false`, so walls do not matter until they fly.

#### 3.3 `TresProjKC_Ragnarok` [A `Blueprints/Projectile/ShootFlow/TresProjKC_Ragnarok`]
Native parent **`ATresProj_Ragnarok`** [A import "Class TresProj_Ragnarok", template `Default__TresProj_Ragnarok`]:
constructor 0x1414700D0, vtable 0x1469BAEE0 (616 slots), size 0x838; against `TresProjectileBase` it overrides slot 96
BeginPlay 0x1414721B0, slot 134 Tick 0x1414845E0 and adds slots 613 / 614 / 615 [I vtable comparison `vtcmp.js`].

| Property | Value | Source |
|---|---|---|
| `m_MainLife` | 8 s | blueprint |
| root | sphere r 10 (`SQEX_bEnableSlopeSlide` false) | blueprint |
| attack shape | sphere r 50, NOHIT, row `SF_KCRAGNAROK` | blueprint |
| particle / pawn hit / map hit | `vw_so010_rgn_bst00` / `vw_so010_rgn_hit00` / `vw_so010_rgn_wht00` (no SE assets) | blueprint |
| `InitialSpeed` / `MaxSpeed` / `m_Accel` | 1000 / 2000 / 500 | blueprint / native / blueprint |
| `m_fMoveStartDelay`, homing delay, min dist | 0.533333, 0.033333, 100 | blueprint |
| `m_HomingTurnMax` / `Accel` / `MaxTurnMax` | 720 / 0 / 0 | blueprint / native / native |
| `m_Spin`, `m_bEnableSpin` | (0,0,30), false | blueprint / native |
| `m_RollingSpeed` / `Add` / `Max` | 180 deg/s / 1800 deg/s² / 1200 deg/s | native ctor 0x1414700D0 [C] |
| `m_RollingRadius` / `Add` / `Max` | 100 cm / −200 cm/s / 15 cm | native ctor [C] |

Helix [I 0x1414845E0, 0x141477ED0; C 0x141472750, 0x1414727B0]. Fields: phase +0x808, angle +0x80C, radius +0x810,
last position +0x814 (BeginPlay 0x1414721B0 zeroes them).
```
Tick(dt):                                             // actor tick, before the base Tick 0x141484B10
  phase 0: if movement.active (bit 0 of +0x388, comp vt+0x550 0x14147B6F0): phase = 1; radius = 0; lastPos = location
  phase 1: radius += dt*200;  if radius > m_RollingRadius: radius = m_RollingRadius; phase = 2
           RollSpeed(dt);  Apply(dt)
  phase 2: Radius(dt);  RollSpeed(dt);  Apply(dt)
  base Tick

Radius(dt)    (slot 613): if RadiusAdd != 0: radius += RadiusAdd*dt;
                            if (RadiusAdd < 0 and radius <= RadiusMax) or (RadiusAdd > 0 and radius >= RadiusMax):
                                radius = RadiusMax; RadiusAdd = 0
RollSpeed(dt) (slot 614): same shape on m_RollingSpeed with SpeedAdd / SpeedMax
Apply(dt)     (slot 615): loc = location;  lastPos = loc
                          rot = actor rotation with roll := angle
                          oldOff = rot.RotateVector((0, 0, radius))
                          angle += dt * m_RollingSpeed;  rot.roll = angle
                          newOff = rot.RotateVector((0, 0, radius))
                          SetActorLocation(loc − oldOff + newOff, sweep = false)
```
So from the tick the bolt starts moving it circles around its own flight path: the radius opens 0 → 100 cm at 200 cm/s
(0.5 s), then closes 100 → 15 cm at 200 cm/s (0.425 s) and stays 15; the angular speed rises 180 → 1200 deg/s at
1800 deg/s² (0.567 s, starting in phase 1) [D]. The offset is applied to the real actor position (the attack sphere
moves with it and the homing is computed from the displaced position); the roll axis is the actor's forward
(rotation follows velocity). The actor rotation's pitch / yaw are those of the previous movement tick.

#### 3.4 Calls the state makes on a projectile [I]
| Call | Slot → function | Effect |
|---|---|---|
| pawn vt+0x1450(name, firePos, pos, &dir, team, 0) | equipment vt+0x5B8 0x14178C1E0 → class by name (equipment vt+0x5B0) → manager vt+0x788 (Spawn, Part B §1) | spawns at `pos` facing `dir`, team set, not fired |
| vt+0x1100(cmd, kind) | 544 → 0x1414823A0 | projectile +0x6A4 = cmd, +0x6A8 = kind; every attack-collision object: `+0x9D0 = cmd` when cmd <= 0xFD, `+0x9D1 = kind` when kind <= 0x2C (0x140645DB0) = `FTresDamageInfo.m_CommandKind` / `m_ShootFlowKind` (Part C §4.1) |
| vt+0x1220(true, comp) | 580 → 0x1414826A0 → component vt+0x590 0x1414826C0 | target flag on, `m_HomingTargetActor` = comp owner, `m_HomingTargetComponent` = comp, part 0, offset 0 |
| vt+0x11F8(pos) | 575 → 0x141482ED0 | fire effects at pos (none in these blueprints) |
| vt+0x1270() | 590 → 0x141474970 | StartFire → DoFire 0x141475120: movement active + started, attack collision on (no delay flags set) |
| vt+0x1208(dir) | 577 → 0x14147B190 | `Velocity = dir * InitialSpeed` |
| vt+0x1240(d) | 584 → 0x141482940 → component vt+0x5A0 0x141482960 | move-start delay, section 3.1 |
| vt+0x12B0(1, &vec3) | 598 → 0x14147FAD0 | kind 0x26 only (not Sora) |

#### 3.5 Damage
Rows of `DataTable/Player/Attack/p_ex001_AttackData` [A]; there are no `*_FIN` or other KC rows (the `SF_*` rows of the
table are: KCSHOT, KCRAGNAROK and the other keyblades' rows).

| Field | `SF_KCSHOT` | `SF_KCRAGNAROK` |
|---|---|---|
| `m_Power` | 0.8 | 0.9 |
| `m_DamageKind` | SIDESPIN | TOSS |
| `m_DamageAttribute` | NOTYPE (7) | NOTYPE (7) |
| `m_bIsMagicAttack` / `m_bIsRapidFire` / `m_bIsKillerAttack` | true / true / true | true / true / true |
| `m_ArmorAttackPower` | 1 | 1 |
| `m_RevengePoint` | 0.1 | 0.1 |
| `m_PhysForceLv` | TPDF_LV_2 | TPDF_LV_2 |
| `m_DamageExecRate` | 100 | 100 |
| `m_ReactionPowerValue`, `m_FormPoint`, bad status, guard flags | 0 / none / false | same |

Inputs to the Part C §4.2 formula 0x140632240 for one projectile hit [D from Part C]:
`power` = row power; `info.magic` = true → attacker stat = MagicPower (level row + keyblade + equipment);
`info.rapidFire` = true → the victim's rapid-fire reduction `bp[0xDC]` applies; attribute 7 → no `*_UP` ability term,
resistance index 7; combo values (info+0x52..+0x55) = 0 and full-MP flag false because the state never calls the
projectile's combo / full-MP setters (vt+0x1130 / 0x1140 / 0x1160) [I: not among the calls of 0x1413C1350] → no
MAGIC_COMBO_UP, no FULLMP_BURST term; not a counter. So
`dmg = ceil(Difficulty · ceil(rapid · ceil(resist · ceil((max(max(Magic − Def, 1), lo…) − 1e-6) · (power − 1e-6)))))`
with the clamps of Part C. Each projectile is one hit (attack interval 0, destroyed on hit).
The hit record carries `m_CommandKind = 0xFD`, `m_ShootFlowKind = 1 / 2`; who reads `m_ShootFlowKind` on the victim
side was not traced [?] (it is not an input of 0x140632240).

### 4. Focus Syphon (kind 36, `TRES_SF_KIND_FOCUSASPIR`)

Aim side (S2.3 mode 4; rules re-read at instruction level in `CanAddLock` 0x1413F0320): while the focus-empty flag is
set a lock can be added only if `count(FOCUS_ASPIR 46) != 0` (otherwise: only if focus != 0); MAX is forced at 18 total
locks in mode 4 [I 0x1413F03B1..0x1413F0412]. Table row 36: `Fp` 0, `Param0` 18 [A]. No focus is spent at release.

Attack side, update 0x1413B78B0 [I]:
```
sub 0:  if OnGroundAndNotFloating (0x1405AD850): clip = "MagicCast"            // body/1341, 1.1333 s
        else: pawn.vt+0x1728(0); pawn.vt+0x17A0(5, true); clip = "MagicAirCast" // body/1351, 1.0333 s
        Play clip (group 0); voice index = +0xC4 = 0x35
        subStep = 5
sub 5:  if flagsCC & 1:                       // set by the Shot handler (slot 69) for kind 0x24 on the TresShot notify
            StartEffects()                    // vt+0x450 = 0x1413BFFC0
            timer = 0.6;  subStep = 6;  return
sub 6:  MoveEffects(dt)                       // vt+0x458 = 0x1413BE340
        timer −= dt
        if timer <= 0:
            total = Σ locked[i].lockCount                         // +0xC of each 0x34-byte entry, all locked targets
            amount = (pawn is a player pawn) ? pawn.vt+0x2778() * total : 0
            n = max(1, trunc(amount))
            pawn.vt+0x1878(0, 0, n, 0x16, 0)                      // Recover(hp 0, mp 0, fp n, effect kind 0x16, 0)
            EndEffects()                      // vt+0x460 = 0x1413BD980: every effect vt+0xAE8(), list emptied
            subStep = 2
sub 2, 6: if animEnd: Finish (vt+0x270)
```
- `pawn.vt+0x2778` = 0x141433C90 [I]: `c = count(46)`; 0 → 0.0; 1 → `MyFocusAspirRateLv1` (pawn+0xD14); 2 → `Lv2`
  (+0xD18); >= 3 → `Lv3` (+0xD1C). Values for Sora 1.4 / 1.9 / 2.4 [A `p_ex001_PawnBase` class defaults; the native
  constructor values 1.0 / 1.2 / 1.5 at 0x141423D66.. are overridden]. `FOCUS_ASPIR` has `m_MaxEquip` 3, 3 AP [A].
  **Focus returned = trunc(rate · total locks)**, at least 1: full 18 locks → 25 / 34 / 43 points with 1 / 2 / 3
  copies [D]. This getter is the only reader of the three rates [I displacement scan 0x1413E0000–0x141470000; the other hits,
  0x14144C7A0 and 0x14145DD70, are other classes of the 0x14144C000+ block].
- Recover: player override 0x141445F40 [C]: nothing when the current or queued state id is 0x3B (59, dead); the fp part is dropped when
  the controller byte +0x95B0 bit 4 ("focus usable") is clear; the last argument 0 means no recovery voice; base
  0x1405B57D0: `SetFocus(GetFocus() + n)` and, kind != 0, effect event `(pawn+0x440)` group 4, id 0x16.
  The setter (section 5) clears the focus-empty flag only if this reaches the maximum.
- **No damage**: nothing is spawned but particles; no attack collision, no projectile, no hit call [I: the callees
  of 0x1413B78B0, 0x1413BFFC0, 0x1413BE340, 0x1413BD980].
- StartEffects 0x1413BFFC0 [C; arguments I 0x1413C017D..0x1413C01E9]: for every locked entry: if the component's owner is
  a character pawn: `FUN_1405B8BE0(owner+0x440, 0xC26)` (effect id 3110 on the enemy through its effect-attach
  object); `pos` = the entry's component position (0x1413FE8F0); spawn the particle system
  `playerPawn.m_pFocusAspirEffect` (pawn+0x1520 = `Effects/PC/focusAspir/par/vp_fca_eng00` [A]) at `pos`, rotation 0,
  (0x14411B3D0(pawn, template, &pos, &rot, 0x150, 1, 0)); keep it in `m_EffectList`.
- MoveEffects 0x1413BE340 [I], per effect, with `timer` = value before this tick's decrement:
  ```
  p = effect.location;  ref = pawn.vt+0x6E0()
  d = (ref − p) * (0.6 − timer)
  if timer > 0.3: d.z += (timer − 0.3) * dt * 8000
  rot = Rotation(d);  rot.yaw += timer * (−150)
  effect.SetWorldLocation(p + rot.RotateVector((|d|, 0, 0)))          // 0x144042B80
  ```
  (orbs that rise first, then curl into Sora; purely visual).
- Timeline (t = 0 at Begin) [A, D]: `TresPlayVoice` (group `sora_magic`, index 0x35) at 0.020 (ground) / 0.053 (air);
  `TresShot` at 0.3333 → effects start on the update of that frame or the next (0.333 – 0.35); focus granted 0.6 s
  later (about 0.95);
  `TresEnableChange` at 0.8667 (ground) / 0.6667 (air); clip ends 1.1333 / 1.0333 → Finish. The clip's own
  `TresAttachEffect` notifies only carry group 3400 effects (another keyblade) and show nothing with group 0 [A].
  In the air variant `TresEnableChange` comes before the grant: whether a state change in that window can skip the
  grant depends on the common cancel rule [?].
- The state is invincible like the other kinds (Begin); End restores gravity / movement mode.

### 5. Additions to S1 (focus gauge)

- **Setter** pawn vt+0x1868 = 0x1405B94F0 [I]:
  ```
  old = focus;  max = GetMax();  focus = v < 0 ? 0 : min(v, max);  (pawn+0x448).vt+0x230(old)
  if old > 0 and v <= 0:      if pawn.vt+0x1800(): SetFocusEmpty(true)        // vt+0x1160(1); v = the requested value
  else if IsFocusEmpty() and focus >= GetMax(): SetFocusEmpty(false)          // vt+0x1170, vt+0x1160(0)
  ```
  `ZeroFocus` 0x1405B95A0 [I]: focus = 0; if vt+0x1800(): flag on; HUD notify. So the flag (pawn+0x7C0 bit 0) is set
  when the gauge is driven from > 0 to <= 0 and **clears only when the gauge is full again**; partial refills
  (prizes, the syphon) leave it set. While it is set Sora's shotlock button opens the syphon aim mode (S2.1).
- **Maximum**: `GetMax` 0x1405AB1E0 → 0x1405838E0 [I]: `movsx word [bp+0x70] + dword [bp+0x58]`. `bp+0x58` is copied from
  the base-param row (`row+0x14`, `m_MaxFocusPoint` = 100) in 0x140575C80 / 0x140576130 [C]. The int16 at `bp+0x70` is
  zeroed by the `UTresChrBaseParam` constructor (qword store at 0x140571A2C) and no other store to it exists: a scan
  for 16-bit stores to `[reg+0x70]` over 0x140540000–0x1405C0000, 0x1413E0000–0x141470000 and 0x140E80000–0x140EC0000
  finds only two unrelated classes [I]; its HP / MP siblings `bp+0x6C` / `bp+0x6E` are written from the save data
  (0x140E8CE20) in 0x140574450 and 0x140575C80, the focus one is not. **Max focus is 100, constant.**
- **Prizes** `ApplyPrize(type, value)` pawn vt+0x1898 = 0x1405AB950 [C]: only for team 0; type 7 → `Recover(0, 0, 3, 2, 0)`,
  type 8 → `Recover(0, 0, 5, 2, 0)` (small / large focus prize: +3 / +5, effect kind 2). The other types: 0 / 1 HP +1 /
  +5; 2 / 3 MP +3 / +5 (or −2 / −6 s of MP-charge time while charging); 4 / 5 / 6 munny 1 / 5 / 10.
- **FP_CONVERTER (83)** [C 0x141472DC0, the prize spawn function]: with `partyCount(83) > 0` (player pawn vt+0x28E0 =
  0x1414241F0: the largest count over party slots 0..6) a prize of type 7 is spawned as type 8: small focus prizes
  become large ones. Same function: HP_CONVERTER (79) 0 → 1, MP_CONVERTER (80) 2 → 3, ability 176 (MILLIONAIRE) turns
  HP / MP / focus prizes into munny, MUNNY_CONVERTER (81) 4 → 5 → 6. No other site uses id 83.
- **FOCUS_BOOST (42)** [A `TresAbilityData` row A00042: SUPPORT, 3 AP, max 1, comment "フォーカスゲージの減少速度が0.8倍になる"
  = "the focus gauge drains at 0.8x"]: **no code reads it**. Checked: every call of pawn vt+0x14E0 / vt+0x1500 with an
  immediate id in 0x140400000–0x141800000 (238 sites, `ability_calls.txt`), the direct callers of the base-param
  functions 0x140588690 / 0x140580810 / 0x140E8AAB0 and of the party functions 0x140EA88E0 / 0x140EABAB0 /
  0x1414241F0, every `mov edx/r8d, 2Ah` followed by a call; no package under `Content/Blueprints` contains the name.
  The aim-time countdown 0x1413F9930 is `aimTime −= dt / actorTimeDilation` with no ability factor [I], and the Sora
  controller does not override it (slot 825). Unless a run-time test shows otherwise the ability has no effect [?].
- **`MyFocusPointNeedAF`** (pawn+0xD10, 10): written by the constructor 0x141423D5C only; no instruction reads
  `[reg+0xD10]` as this field in the player, state, HUD or base-pawn ranges and no package under
  `Content/Blueprints/Player` names it → unused; the airstep costs no focus (S4) [I, A].
- **Timed regeneration** (controller slot 827 0x1413F9040 → 0x1413F8EF0 [C]): +7 focus every 1.0 s of game time when
  not aiming, pawn vt+0xA48() false, pawn vt+0x2A88() (→ vt+0x1550) true, option set B bit 6 set
  (`0x1405AD350(pawn, 6)` → `0x140EAB160(gameInstance, 6)`, the same set whose bit 0 triples attack and magic) and the
  pawn not restricted (vt+0x1ED8). It is a game option (cheat-style code), off in a normal game [?: its menu name].

### Open questions
1. Tick order inside a frame between the player state and the projectile movement components (decides 3 vs 4 frames
   between the last formation placement and the launch, and whether a projectile spawned in a tick is placed by the
   formation before it is first drawn).
2. Meaning of the equipment request `vt+0x1388(1,0,0)` (weapon presentation mode) and of `pawn.vt+0xDC8`.
3. What the common state code permits once `m_bEnableChange` is set (ShootFlowEnd 0.300 s; MagicAirCast 0.6667 s before
   the syphon grant).
4. Reader of `m_ShootFlowKind` / `m_CommandKind` on the victim side (enemy reactions to shotlock hits).
5. FOCUS_BOOST: confirm at run time that the drain is unchanged.
6. Effect id 0xC26 played on a syphoned enemy and effect kind 0x16 / 2 of Recover: which particle assets they select
   (effect tables of the pawn's effect-attach object).
7. Name of game option B6.

### Runtime probe requests
| Object | Offset | Type | Settles |
|---|---|---|---|
| ShootFlow state | +0x78 i32, +0x90 i32, +0x94 i32, +0x9C i32, +0xA0 f32 | | volley progress per frame (spawn cadence 7 / tick, ring timer) |
| ShootFlow state | +0xE4 vec3, +0xF8 vec3 | | origin (blade tip) and lift target |
| pawn root | location z, per frame of the first 3 ticks | f32 | instant lift (section 1.4) |
| each KC_SHOT projectile | location vec3; movement +0x2EC f32, +0x388 u32, +0x308 f32, +0x214 vec3 | | launch frame, turn-rate ramp 180 → 360, constant speed 1500 |
| each KC_RAGNAROK projectile | +0x808 i32, +0x80C f32, +0x810 f32, +0x820 f32 | | helix phases |
| equipment component | +0x2E0, +0x2E1, +0x2E2 u8 | | weapon mode of vt+0x1388 |
| pawn | +0x614 i32, +0x7C0 u8 before / after a syphon and after prizes | | grant = trunc(rate * locks); flag clears only at max |
| pawn → m_pBaseParam | +0x70 i16 | | stays 0 |
| pawn with FOCUS_BOOST equipped / not | controller +0xA3C f32 per frame while aiming | | drain rate (question 5) |

### What the host must provide
- For the lift: a downward sphere sweep (r 50, from 55 cm above the pawn centre, 800 cm long) returning the impact
  point, and a swept move of the pawn capsule to a target position.
- Weapon socket `ATK_01` world position (blade tip) at the set-up tick.
- The locked list in lock order with a per-entry "component alive" test and aim point; the projectile needs
  "aim point" and "still targetable" per tick.
- Projectile hosting: spawn by blueprint name, teleport + rotation set without collision during the formation,
  sphere sweeps of the root (r 10) against level geometry while flying, attack sphere (r 30 / 50, swept) against
  enemy bodies only, one hit then destroy, hit delivery with the attack row and `m_ShootFlowKind`.
- Effects / sounds by name: charge and flash effects on weapon sockets (`vfx_top`, `R_wep_skt`), projectile trails,
  hit and map-hit effects, the SE and voice assets listed in `assets.json`; for the syphon a particle per target whose
  world position the simulation sets every tick, an "effect on enemy" hook and the recover effect on Sora.
- Gravity on / off, movement mode Flying / Falling / Walking, invincibility flag, and (optional) force-volume ignore.

---

# Part H — Second pass: airstep in full (supersedes D S4)

_Source of this part: `build/sora_tmp/magic/airstep2/spec.md` (section numbers are local to the part)._

Strand `airstep2`, second pass, 2026-10-11. Replaces section S4 of Part D of `docs/sora-magic-spec.md`.
Tags: [I] instruction level (address given) · [C] decompiled C only · [U] stock UE 4.17 · [A] asset ·
[D] derived · [?] not determined statically. Units cm, s, deg. Offsets are from the object named.
Dumps used: `c_slots.txt`, `c_ctrl.txt`, `c_pawn.txt`, `c_cam.txt`, `*.dis` in this folder and
`../airstep/141348BA0.dis`. Flowmotion states that the airstep hands over to (Superslide, wall kick, pole,
dangle) are specified in `docs/sora-actions-spec.md` sections 8, 10.3, 10.4, 11.1 and are only referenced
here. The follow-up attack is the combat spec's `PlayAttack` (`docs/sora-combat-spec.md` 4.3).

### 4.0 The model in one page

1. While the shotlock aim mode is open (S2), the controller evaluates every tick which **move-flow target**
   is nearest to the screen centre (`AirstepTargetUpdate`, 0x14141FDC0). The result is the **target list**
   `ctrl+0xB18` (count `+0xB20`): one enemy, or a chain of level points ending in the chosen point.
2. The airstep key with count ≥ 1 pushes state 42. No focus is spent, no ability is needed beyond the
   player-state unlock bit.
3. The state flies the pawn in **Flying** mode with a requested velocity straight at the (re-read every
   tick) target point, invincible, gravity off; intermediate hops fly with world collision **off**, the last
   hop with collision on.
4. Arrival (distance < `m_CloseRange + 100`): at an enemy → hit effect, `AthleticSlideEnd`, optional
   follow-up attack `1181`; at a level point → snap to the point, then pole / ledge-hang / END / wall kick /
   next hop.
5. Jump aborts the flight, the guard button queues a Superslide after an enemy arrival, attack queues the
   flow attack.

Original names recovered from debug strings: `ChangeProcess` (slot 115), `Process_AthleticSlide` (116),
`Process_AthleticEnemy` (119), `GetMoveCheck` (122), `m_pAthleticInfoCache` (the target list pointer),
`m_bInputAttack` (state+0x128) [I strings 0x1468FA310, 0x1468FAB00, 0x1468FB000, 0x1468FA6A0].

### 4.1 Unlock, cost, request

- **Unlock** [I 0x14141FE3B]: `AirstepTargetUpdate` returns false at once unless the player-state object
  returned by controller vt+0x1420 (0x140F50C90) has byte `+0x95B0` bit 5 set. Setter:
  `BP_SetAbilityAthleticFlow(b)` (exec 0x142876840) → controller vt+0x1458 (0x1413EA880) → 0x140E97C00
  `byte = (byte & 0xDF) | (b << 5)` [I]. (Same byte: bit 2 MP, bit 4 shotlock, bit 7 flowmotion.) Callers [A]: the level blueprints of Olympus
  (`Levels/he/he_01`, `he_02`, `he_03` and their `_gameflow` sub-levels) — the bit is story-granted; the
  argument values and the start value are not read [? probe below]. No `ETresAbilityKind` is tested.
- **Cost**: none. `StartCommand(37)` (0x14140F1F0) and the kick notify (0x14140CCB0) both take the
  `kind − 36 < 2` branch that zeroes the consumption fields instead of charging focus [C].
  `MyFocusPointNeedAF` (pawn+0xD10 = 10, ctor 0x141423210 @0x141423D5C [I]) has **no reader** in the
  player-state, controller and pawn code ranges (displacement scan 0xD10) [I scan] — unused.
  `MyAthleticSlideKickAnim` (pawn+0xD30 = 0.13333) likewise has no reader on this path (its getter
  0x141108E70 is pawn vt+0x2910; no call site in the state / controller ranges) [I scan].
- **Request** [C, S2.3]: aim mode 1, airstep key (FKey static 0x149C70748) pressed, `ctrl+0xB20 >= 1` →
  mode 3, flags |= 2, shotlock locks cleared, request interface slot 97 `TryAthleticFlow` (0x1414372F0) =
  `if CanAthleticFlow(): PushState(42)`.
  `CanAthleticFlow` (slot 96, 0x141426280) [C]: pawn vt+0x29C0(); vt+0x2770() == 0; !vt+0x2540(); pawn type
  (vt+0xD70) == 1; vt+0x2788() == 0; current state ∉ {6 DANGLE, 40 SHOOTERMODE}; `ctrl+0xB20 > 0`.
  Then `StartCommand(37)`: pawn vt+0x1D98(0xFD, 37); object-type time dilation of type 1 back to 1.0;
  airstep camera on in mode 0 (4.9); `if ctrl+0xA40 != 0.1: SetDilationMask(1, 0x4D9E, 0.1, 0)` (4.8).

### 4.2 Targets

#### 4.2.1 What can be a target
A `TresLockonTargetComponent` (or subclass) qualifies when [C 0x1414165E0 candidate pass, 0x14141FDC0]:
`m_bIsGimmickActive` (comp+0x868 bit 0) and vt+0xAC0() (= active && `m_bPermissionMoveFlow`, 0x140F51A30)
and `Team(comp) != Team(pawn)` (0x140F75F30 vs pawn vt+0xD90). The kind is `m_MoveFlowType`
(`ESQEX_FreeFlowKind`, vt+0xAD0): 0 NONE, 1 ENEMY, 2 ETURN, 3 ERIDE, 4 GROUND, 5 WALL, 6 POLE, 7 RAIL,
8 ROPE, 9 BALL [A reflection enum].

| Class | Constructor | Defaults [C] |
|---|---|---|
| `TresLockonTargetComponent` (enemies, gimmicks) | 0x140F8C720 | lock / shotlock / move-flow permission on, kind **ENEMY**, `m_CloseRange` 100, `m_ShootLockOnRange` 0, `m_AutoLockOnRange` 0 |
| `TresShootFlowComponent` (root `TresRoot0` of the level actor `TresShootFlowActor`, ctor 0x140F8BBE0; no tick) | 0x140F477F0 | lock-on **off**, shotlock **off**, move-flow on, kind **WALL** |
| `TresPoleComponent` (super `TresFlowComponentBase`) | 0x140F8C870 | kind **POLE**, `m_AttachOffset` (0,0,200), `m_AttachRadius` 0, `m_BaseOffset1` (0,0,0), `m_BaseOffset2` (0,0,300), `m_CheckRadius` 50 |

A level "airstep point" is therefore nothing but a scene component with a position, a kind (WALL unless
the level changes it), a team and `m_CloseRange`; it registers through the same world list as enemy lock-on
components (manager vt+0x9D0). Per-instance values of placed points are level data [? not surveyed].

#### 4.2.2 Candidate list (built once when the aim mode opens) [C 0x1414165E0, lines 633–664 of the C]
For every component of the world list passing 4.2.1 an entry (0x34 bytes) is appended to `ctrl+0xAF8`:
`+0 dist = −1`, `+4 kind`, `+8 state = 1`, `+0x14 = m_ShootLockOnRange` (vt+0xAF8 — the first pass called
this "move-flow type"; it is the range), `+0x1C weak component`. The list is not refreshed while aiming.

#### 4.2.3 Per-tick update `AirstepTargetUpdate(dt) -> bool` (0x14141FDC0) [C; constants I]
```
if !unlock(4.1) or pawn type != 1: return false
eye   = pawn.RefPoint()                      // pawn vt+0x2110 = actor location + (0,0,100)  [C 0x141448550]
limit = ctrl+0x984                           // MyAthleticLockonLimit 15000 (reset 0x1414162A0 @0x141416423 [I]; 0x141410E50 copies pawn vt+0x29A8)
s     = min(W * 0.00078125, H * 0.0013888889)                              [I 0x141420011, 0x141420019]
pick2 = s*s * 6400                           // 80 px at 1280x720          [I 0x14142002E]
near2 = s*s * 25600                          // 160 px: (s*12800 + s*12800) * s   [I 0x14142003B, 0x141420047]
prev  = last entry of the old target list;  target list.clear()
best = none; tmp = []
for e in candidates (ctrl+0xAF8, count +0xB00):
    c = e.component; skip unless alive and 4.2.1 still holds
    p = c.world location (ComponentToWorld translation, 0x1413FE8F0)
    d2 = |eye - p|^2
    r  = c.m_ShootLockOnRange
    range2 = (r > 0 and r != ctrl+0x988) ? r*r : limit*limit       // ctrl+0x988 = MyAthleticLockonRange 3750
    if d2 > range2: continue                                       // [I 0x14142030A..0x141420327]
    (sx, sy, w) = Project(p); if w <= 0: continue                  // FUN_140ECF670, camera manager view
    q = (sx - cx)^2 + (sy - cy)^2                                  // cx = x0 + (x1 - x0)/2 (integer), same for y
    if q > near2: continue
    if !LineOfSight(from = pawn, to = c): continue                 // 4.2.4
    e.dist = sqrt(d2)
    if q < pick2: best = e; pick2 = q                              // strictly smaller: nearest to the centre wins
    tmp.push(e)
if best and best.kind != 0 and best.component alive:
    HUD: if prev.component != best.component: remove marker(prev, 8); add marker(best, 4)     // FUN_14157A7A0 / FUN_1415759C0
    if best.kind < 4:  targets = [best]                            // ENEMY / ETURN / ERIDE: never chained  [I 0x141420829 cmp ..,4]
    else:
        sort tmp by dist ascending                                 // 0x14062E940
        for e in tmp:
            if e.component == best.component: targets.push(e); break
            if e.kind > 3 and LineOfSight(from = e.component, to = best.component): targets.push(e)   // [I 0x1414209D6]
    if !ctrl+0xA68: ctrl+0xA68 = 1; ctrl+0xA78 = 0 (segment), ctrl+0xA7C = 0
    PathMarkerUpdate(dt)                                           // 4.2.5
    return true
HUD remove marker(prev, 8); ctrl+0xA68 = 0; destroy path marker; return false
```
So: only a point within 80 px of the centre can be chosen; points within 160 px that are nearer than the
chosen one and see it become intermediate hops, nearest first. `MyAthleticLockonRange` (3750) is **not**
the selection range: it is the sentinel that makes a component's own range ignored, and the radius of the
out-of-aim hint (4.2.6). The return value drives the HUD prompt `FUN_14158B420(hud, 1, canAirstep)`.

#### 4.2.4 Line of sight (0x1413EE400) [C; 35.0 I 0x1413EE6E8]
`FUN_144526380` (SQEX_PhysXCollision.cpp sweep-multi) with shape descriptor {type 0, 35.0}, identity rotation
of the source root, from `pawn.RefPoint()` (or `from.vt+0xB88()` for a component) to `to.vt+0xB88()` (the
component's target point: world location; for a pole the attach point 0x140F95450), trace channel 2 (Pawn),
query tag "Athletic", response table: everything ignored except channels 0 (WorldStatic), 1 (WorldDynamic)
and 0x12 (game channel 5) = block; ignored actors: the source actor, the target's owner and its attach
parents. Result false when a **blocking** hit (flags bit 0) belongs to an actor other than source and target
owner; a hit with flags bit 1 (start penetrating) ends the scan as visible. The companion function
0x141331780 subtracts its 60.0 parameter from the hit location's Z, so the float is a **sphere radius**
[D]; whether type 0 means sphere in this SQEX wrapper is [? — a host should sweep a 35 cm sphere].

#### 4.2.5 Path marker (0x1413E9DB0) [C; 1.0 I 0x1413EA08B]
One actor of class `pawn.m_AthleticRoot` (`AthleticFlowRootEffect`, particle `vp_ath_tgt0` [A]) is spawned
and moved along the chain: segment `i` runs from `pawn vt+0x6E0` (i = 0) or target[i−1] to target[i];
position = lerp(from, to, t), rotation = direction; `t += (i + 1) * rate * dt` with `rate = ctrl+0xA80`
(10.0, reset @0x141416568 [I]), multiplied by `ctrl+0xA40` when pawn vt+0x1170() (focus-empty flag);
`t > 1` → `t = 0, i += 1`; `i >= count` → restart at 0 with a fresh actor. Destroyed when no target.

#### 4.2.6 Hint outside the aim mode (0x14141F6F0) [C, partly read]
In states 1, 2, 3, 5, 0x66 and aim mode 0, every 2.0 s (first after 0.5 s) [I 0x14141F8B0, 0x14141FDAD]
the nearest qualifying component within `max(m_ShootLockOnRange, 3750)` of the pawn that projects in front
of the camera and passes 4.2.4 gets HUD marker type 6 (kept in `ctrl+0xA60`,
`m_pAthleticFlowStartPointDispComponent`); components with `m_bMoveFlowDestination` (vt+0xAE0) are handled
by a second branch that needs no visibility test [C, details not transcribed]. Cosmetic only.

### 4.3 State object

Class `TresPlayerState_AthleticFlow : TresCharState_SuperSlide`, size 0x220, ctor 0x1413308C0, vtable
0x146901158. Reset 0x14133CFB0 (slot 95), Begin 0x141332F90, Tick 0x141341270, End 0x141334B60.

| Slot (vt offset) | Address | Role |
|---|---|---|
| 113 (+0x388) | 0x1413435F0 | `OnAttackInput`: flags &= ~0x18; `m_bInputAttack = 1`; if flags & 1: HUD prompt(3), flags &= ~1. Called by `TryAttack` 0x141437330 and `CanAttack` 0x141426330, case state 42 [C] |
| 114 (+0x390) | 0x1413436C0 | `OnFlowInput`: same with `m_bInputFlow` (+0x135). Called by `TryFlow` (iface slot 72, 0x1414417B0, case 42) = guard button [C] |
| 115 (+0x398) | 0x1413311D0 | `Select(mode)` (`ChangeProcess`) |
| 116 (+0x3A0) | 0x141348BA0 | `Travel` (`Process_AthleticSlide`), delegate id 0 |
| 117 (+0x3A8) | 0x141347510 | `KickProc`, id 1 |
| 118 (+0x3B0) | 0x141347750 | `PoleProc`, id 2 |
| 119 (+0x3B8) | 0x141346600 | `EnemyProc` (`Process_AthleticEnemy`), id 4 |
| 120 (+0x3C0) | 0x14134C1C0 | spawn `m_pAthleticHitEffect` at `pawn.RefPoint()` with the actor rotation (`FUN_14411B3D0`) |
| 121 (+0x3C8) | 0x1413342C0 | `SetEnemyLock(b)` |
| 122 (+0x3D0) | 0x141336DB0 | `StuckCheck(dt)` (`GetMoveCheck`) |
| 123 (+0x3D8) | 0x141336520 | `TargetPoint(out)` |
| 124 (+0x3E0) | 0x141331780 | `FindLanding(from, to, ignoreA, ignoreB, out, radius)` |
| 125 (+0x3E8) | 0x141336EE0 | landing search behind an enemy (no caller found in the functions read [?]) |
| 126 (+0x3F0) | 0x1413321B0 | `PoleCheck` |
| 127 (+0x3F8) | 0x141331C40 | `EndCheck` |
| 128 (+0x400) | 0x141331ED0 | `KickCheck` |

Interfaces: state+0x40 (vtable 0x146901560) overrides only notify slot +0x228 (Shot) = 0x1413442A0;
state+0x48 (vtable 0x1469019A8) slot 8 = 0x141342EA0 `OnReachEnemy(dir, actor, component)`, slot 11 =
0x141345A40 `OnEnemyTurnAck(actor)`. The delegate at state+0x1E0 is bound by `Select` to thunks
0x140430C70 / C7C / C88 / C94 (jump through vt+0x3A0 / 0x3A8 / 0x3B0 / 0x3B8) [I].

Fields [C; written values I where a constant]:

| Offset | Meaning |
|---|---|
| +0x78 | sub-step |
| +0x8C | actor yaw at launch · +0x90 `T` time budget of the hop · +0x98 pose variant 1 / 2 / 3 · +0x9C / +0xA0 counters of upward / downward launches · +0xA8 / +0xAC kick / pole counters |
| +0xB0 | byte, set 1 by `Select(1/2/4)`, 0 at launch · +0xB1 byte **abort request** (jump pressed, or time out on the last hop); cleared at the end of every Tick |
| +0xB8 | pointer to the controller's target list (`ctrl+0xB18`) · +0xC0 current entry · +0xC8 next entry or null · +0xD0 delegate id · +0xD4 index (−1 at Begin) · +0xD8 count · +0xDC kind of the current entry |
| +0xEC / +0xF8 | actor location / RefPoint at Begin · +0x104 landing position · +0x110 RefPoint saved before an attach |
| +0x11C | byte: attached, landing fix pending · +0x11D byte (cleared only) |
| +0x120 | `m_pTargetComponent` |
| +0x128 | `m_bInputAttack` · +0x134 byte (cleared) · +0x135 `m_bInputFlow` |
| +0x12C / +0x130 | hit slow rate / time: Reset 0.5 / 0.1 [I 0x14133CFB0], Begin copies pawn `MyAthleticAttackSlowRate` 0.25 / `MyAthleticAttackSlowTime` 0.05 |
| +0x138 | flags: 0x01 lock-on HUD active · 0x02 enemy target (kind 1..3) · 0x04 debug text · 0x10 lock taken (slow motion armed) · 0x20 first arrival not yet used (set by Reset) · 0x40 current entry is the last · 0x80 hit slow motion running |
| +0x13C | stuck timer · +0x140 target point · +0x158 `m_pHitActor` · +0x160 `m_pHitComponent` · +0x168 hit direction (unit) · +0x174 RefPoint before the snap · +0x180 `m_pPoleComponent` |
| +0x188 pole rotation · +0x194 pole offset · +0x1A0 bit 0 swing (horizontal bar) · +0x1A4 attach radius · +0x1A8 rotation at the pole · +0x1B4 relative grab location · +0x1C0 loop count · +0x1C4 approach timer · +0x1C8 loop time left |
| +0x1D0 | `m_pTargetEnemy` · +0x1D8 `m_pEnemyTurn` |

### 4.4 Begin, Tick, Select, End

Pawn calls used (addresses for `TresPlayerPawnSora`, vtable 0x14696B930) [C unless tagged]:
`StopMovement` vt+0x16E8 (0x140530DD0); `SetMovementMode(m, 1)` vt+0x17A0; `Draw` vt+0x1340;
gravity vt+0x1728; **invincible** vt+0x2068(b) = pawn+0x630 bit 2 (0x1405B9120 [I]; read by vt+0x2060 =
"damage invalid", magic spec C 5); vt+0x19E0(b) = movement component +0xB70 bits 2, 3, 4 = b
(0x1405B92B0 → 0x140594400 / 3E0 / 3C0; meaning [?]); vt+0x1388(a, b, c) = equipment vt+0x508
(0x141790690; meaning [?]); **body collision** vt+0x1010(b, 0): every body component vt+0x6C8(b);
**world collision** vt+0xFF0(b) (0x1405C9B00): b = 1 restores the root's default responses, b = 0 sets
the root's responses to channels 0, 1, 5, 6, 7, 0x12, 0x16, 0x19, 0x1B, 0x1E to **ignore**; vt+0x1620(v)
`MaxFlySpeed = max(v, 0)`; vt+0x1640(a) `MaxAcceleration = max(a, 0)`; vt+0x1630 get `MaxFlySpeed`;
vt+0x1628 `MaxFlySpeed = default` (vt+0x1638); vt+0x1668(v, 0) = movement vt+0x500 (0x140592BC0): stores
`RequestedVelocity = v` and the "has requested velocity" flag (the engine's `RequestDirectMove` shape [U
shape, C body]); vt+0xDB0(rot, a) slerp the actor rotation toward `rot` by `clamp(a, 0, 1)`; vt+0xDB8(a) the
same toward (0, current yaw, 0); vt+0xDC8(a) = vt+0x21B0(zero, a) (mesh-side rotation back to zero [C]).

```
Reset:  flags = (flags & ~0xFF) | 0x20; slowRate = 0.5; slowTime = 0.1; loopCount(+0x1C0) = 2; everything else 0/null
Begin:  StopMovement; SetMovementMode(5 Flying, 1); Draw(1, 1, 1.0); vt+0x1388(1, 0, 0);
        gravity off; invincible on; vt+0x19E0(1)
        ctrl = controller; if none: return
        startLoc(+0xEC) = actor location; startRef(+0xF8) = RefPoint; landing(+0x104) = startLoc; +0x110 = startRef
        FUN_1405B8B30(pawn+0x460)                                   // paired with FUN_1405B6100 in End  [?]
        list = &ctrl+0xB18; count = ctrl+0xB20; index = -1
        if Select(0): if kind in 1..3: flags |= 2
                      slowRate = pawn.MyAthleticAttackSlowRate; slowTime = pawn.MyAthleticAttackSlowTime
Tick(dt): if list and list.count > 0:                              // the controller empties the list at its clean-up
              pawn vt+0xDC8(dt * 12); pawn vt+0xDB8(dt * 12)       // [I 0x1413412DE]
              delegate(dt); abort(+0xB1) = 0
          else Finish()                                            // vt+0x270
```
`Select(mode)` (0x1413311D0) [C]:
```
m_pTargetEnemy = null; if mode != 4: m_pEnemyTurn = null
mode 1 / 2 / 4:  +0xB0 = 1; bind KickProc / PoleProc / EnemyProc; delegateId = mode; sub = 0; run the delegate once now
mode 0:
  if list empty:                                                    // nothing left
      body collision on; world collision on; StopMovement; Play("SlideEnd", blendIn 0, mode 0, group 0)
      if !(flags & 0x40): LandingFix()                              // see Travel
      bind Travel; delegateId = 0; sub = 2; run once; return false
  +0x11C = 0; index += 1; entry = list[index]; kind(+0xDC) = entry.kind; m_pTargetComponent = entry.component
  if index + 1 < count: next = list[index + 1]; body collision OFF; world collision OFF
  else: flags |= 0x40; next = null; body collision on; world collision on; AirstepCamera(on, mode 0)
  bind Travel; delegateId = 0; sub = 0; run the delegate once now (dt = world delta); return true
```
`Select(0)` does not test `index < count`; its callers do (4.5). One path reaches it past the end (last
target is a pole whose `TryPoleTurn` failed after the first arrival): the entry read is then stale memory
[C] — port as "abort" [D].

`End` (0x141334B60) [C]:
```
if m_pEnemyTurn: enemy.Listener(+0x448).vt+0x60(pawn)              // release the turn
StopAnim(0, 0)
if pawn attached (vt+0x968): c = socket "Center" location; StopMovement; Detach; keep rotation; SetActorLocation(c)
SetMovementMode(3 Falling, 1); vt+0x1388(0, 0, 0); gravity on; MaxFlySpeed = default;
invincible off; vt+0x19E0(0); body collision on; world collision on; vt+0xCA8()
if +0x11C:  if target and owner alive: pawn vt+0x2750(+0x110, target point, pawn, owner, &landing, 60, 0)
            SetActorLocation(landing); +0x11C = 0
elif +0x11D: LandingFix(); +0x11D = 0
FUN_1405B6100(pawn+0x460)
AirstepCamera(off); controller vt+0x1948() (0x14140E720: HUD target display off; aim flags &= ~2 when mode is 2 or 3);
SetEnemyLock(false)
```
Exit velocity: nothing is written in End. Every path into `Finish()` called `StopMovement` before its last
clip, so the pawn leaves with zero component velocity in Falling mode (plus the root motion of a clip
played with mode 2) [D]; what the state machine pushes next is its default rule (states notes).

### 4.5 Travel (delegate 0, 0x141348BA0)

```
sub 0 (launch; runs inside Select):                                              [I 0x141349A60..0x141349C2C]
    SetObjTypeDilation(1, 1, 1.0, 0)            // the player object type leaves the 0.1 slow motion, 4.8
    m_bInputAttack = 0
    +0x8C = actor yaw (vt+0xDA0);  T = pawn.MyAthleticSlideTime (vt+0x1B98 → 1.0)
    target = TargetPoint();  variant = 1
    pitch = Rotation(target - RefPoint).Pitch
    if pitch > 45: variant = 3; up += 1        elif pitch < -30: variant = 2; down += 1
    if variant == 3 and (up & 1):     Play("AthleticSlide",  blendIn 0, mode 0, group 0xC80)
    elif variant == 2 and (down & 1): Play("AthleticSlide2", blendIn 0, mode 0, group 0xC80)
    else Play(pawn vt+0x1C90(variant), blendIn 0, mode 1 loop, group 0xC80)   // 1 "SuperSlideLoop", 2 "SonicSlideLoop", 3 "SuperFlightLoop"
    +0xB0 = 0; sub = 3
sub 3 (next tick):                                                               [I 0x141348D01..0x141348E28]
    vmax = pawn.MyAthleticSlideSpeed (vt+0x1BA0 → 6000); target = TargetPoint()
    d = |target - RefPoint|
    speed = max(2*d / T, min(d * 60, vmax))
    pawn vt+0xDB0((0, yaw of (target - RefPoint), 0), dt * 60)       // snaps at 60 fps
    MaxFlySpeed = speed; MaxAcceleration = speed * 60; stuckTimer = 0; sub = 1
sub 1 (every tick):
    if T <= 0: StopMovement; if !(flags & 0x40): Select(0); return        // hop timed out: go on to the next entry
               else abort = 1
    m_pTargetComponent = entry.component (weak)
    if abort or component dead or StuckCheck(dt): goto ABORT
    target = TargetPoint();  T -= dt
    if IsAnimEnd and current clip name == "AthleticSlide":  Play(pawn vt+0x1C90(variant), blendIn -1, mode 1, group 0xC80)
    v = target - RefPoint;  d = |v|
    R = component.m_CloseRange (vt+0xB18) + 100                                  [I 0x1413490A8]
    if d < R:
        if !(flags & 2):                                                          // level point
            +0x174 = RefPoint;  SetActorLocation(target, sweep = true)            [I 0x14134931C]
            body collision on; world collision on
            if PoleCheck(): return                                                // "POLE"
            if EndCheck():  body collision on; world collision on; StopMovement;
                            Play("SlideEnd", blendIn 0, mode 0, group 0); sub = 2; return      // "END"
            if KickCheck(): return                                                // "KICK"
            StopMovement; Select(0); return                                       // "NEXT"
        elif component and owner pawn alive:                                      // enemy
            OnReachEnemy(dir = normalize(startRef - target), owner, component); return         // "ENEMY"
    dir = normalize(v)
    V = max(d * 6, MaxFlySpeed);  if d < 600: V = 3000                            [I 0x14134964E..0x141349670]
    if V * dt * (1/60) > d: return                                                // "Adjust!!": no request this tick [I 0x14134967C..0x14134968E]
    pawn vt+0xDB0((0, yaw of v, 0), dt * 12)                                      [I 0x141349796]
    pawn vt+0x1668(dir * V, 0)                                                    // requested velocity
    return
ABORT:
    body collision on; world collision on; StopMovement; Play("SlideEnd", blendIn 0, mode 0, group 0)
    if !(flags & 0x40): LandingFix()
    sub = 2
sub 2: if IsAnimEnd: Finish()
sub 6: if IsAnimEnd: Select(0)            (other values: IsAnimEnd → Finish())
```
`LandingFix()` [C]: if the target component and its owner are alive: `ok = FindLanding(target.vt+0xB88(),
RefPoint, pawn, owner, &landing, 60)`; if !ok: `FindLanding(startRef, RefPoint, pawn, null, &landing, 60)`;
then `SetActorLocation(landing, no sweep)`; +0x11D = 0. `FindLanding` (0x141331780): sweep as in 4.2.4 with
60.0 from `from` to `to`; the first hit whose actor is neither ignored one gives `out = hit.Location` with
`out.z = out.z − 60 + 1` and returns false; no hit → `out = to`, returns true. It only runs when the flight
is cut while world collision is off (an intermediate hop).

Notes [D]: the motion is the engine's flying move with a requested velocity re-aimed every tick, so the path
is a straight chase of the (possibly moving) target point, not an arc. `MaxAcceleration = 60·speed` lets the
velocity reach the request within one 60 Hz step. With T = 1 and vmax = 6000: speed = 6000 up to 3000 cm,
`2·d` beyond; in flight `V = max(6·d, speed)`, and **3000 cm/s inside the last 600 cm**. The `1/60` factor
in the overshoot guard makes it fire only when `d < V·dt/60` (0.83 cm at 60 fps) — unreachable because
arrival triggers at `d < R ≥ 100` first; keep it literal.

`TargetPoint(out)` (0x141336520) [C; 0.5 / 60 I 0x141336868, 0x1413368C7]: `p = component.vt+0xB88()`;
if this is the last entry: `out = p`. Otherwise with the next entry's point `n`:
`out = WarpSweep(from = (p + n) / 2, to = p)` = pawn vt+0x2750 (0x141431630): a sweep (tag "Warp", radius
60, same channels) from the midpoint toward the point; a hit that is not the pawn gives
`hit.Location` with `z −= 60`, then `z += 1` if it is below the start height, `z −= 1` if above; no hit →
`p`. Intermediate points are thus approached on the side that faces the next point.

`StuckCheck(dt)` (0x141336DB0) [C; 0.5 / 0.2 I 0x141336E40, 0x141336E5D]:
`if |pawn.GetVelocity()| * 0.5 <= |movement+0x9A0 vector|: timer = 0; false` else `timer += dt; return
timer > 0.2`. (`movement+0x9A0` is the component's measured move speed, pawn vt+0xE28; debug text
"VelSpeed / MoveSpeed" [C]. Exact definition of that vector: movement spec / probe [?].)

### 4.6 Arrival at a level point

`PoleCheck` (0x1413321B0) [C; constants I 0x14133220B..0x141332365]: only for kind 6. `range = max(
m_CloseRange + 100 − 75, 125)`; `pawn vt+0x2330(p, p, range, out)` (0x1405A7500: nearest
`TresPoleComponent` of the world's pole list whose vt+0xBD0 accepts the point) must return the target's own
owner. Then `m_pHitActor / m_pHitComponent / m_pPoleComponent` are set, hit direction =
normalize(p − +0x174), and:
- last entry and `TryPoleTurn(out, 0)` (request interface slot 78, 0x14143F0C0) succeeds → `Finish()`:
  **hand-off to `TresPlayerState_PoleTurn` (24)**, actions spec 10.4;
- else if the first-arrival flag 0x20 is still set → `Select(2)` (in-state pole swing, below);
- else `StopMovement; Select(0)`.

`EndCheck` (0x141331C40) [C; 300 / 45 / 90 I]: only for the last entry (flag 0x40) — and then it always
returns true. It runs the ledge probe `FUN_140638740(pawn, out, dz + 300, 300, horizontalDist − 45, 90.0,
debug)` with (dz, horizontalDist) measured from RefPoint to the target point; on success `StopMovement` and
request-interface slot 65 (0x141444810) = **push DANGLE (6)** with the ledge data; if that returns a state:
`Finish()` (hang on the ledge, actions spec 11.1). Otherwise the caller plays `SlideEnd` (END).

`KickCheck` (0x141331ED0) [C]: only while flag 0x20 is set (the first arrival of the state) and therefore
only for a non-last entry. Sets `m_pHitActor / m_pHitComponent`, hit direction = normalize(p − RefPoint),
`m_pPoleComponent = null`, `Select(1)`.

`KickProc` (0x141347510) [C; 0.2 I 0x1413476E0]:
```
sub 0: kicks += 1; StopMovement
       if flags & 0x20: flags &= ~0x20; AirstepCamera(on, mode 1)
       Play("WallKickF", blendIn 0, mode 2, group 0xC80); SetAnimTime(0.2)
       SetActorRotation(Rotation(hit direction)); sub = 2
sub 2: if IsAnimEnd: Select(0)
```
This is a private use of the clip; `TresPlayerState_WallKick` (23) is not entered. `WallKickF` is 0.5 s with
a 180° turn and 80 cm of root motion (actions spec 10.3), started at 0.2 → 0.3 s.

`PoleProc` (0x141347750) [C; 0.5, −100, −0.2 / 1.0 I]: `sub 0`: if the pole is gone → `Select(0)`. Else read
the pole rotation (vt+0xBC8), attach offset (vt+0xBC0), swing bit (vt+0xBA0, 0x140F965E0), `m_AttachRadius`;
poles += 1; first-arrival flag → camera mode 1; loopCount = 1; body collision off; landing = actor location;
+0x110 = RefPoint; +0x11C = 1; orient to the pole (rotation math as in the pole state, actions spec 10.4
[C, not transcribed]); mesh offset ∓radius; attach the pawn to the pole component; approach timer 0.5;
grab = pole.vt+0xBC0(actor location) (+ rotate(0, 0, −100) for a turn); `StopMovement`; Flying; play
`PoleTurn` / `PoleSwing` (blendIn −1, mode 2, group 0xC80); sub = 5.
`sub 5`: until the clip ends `timer −= dt; a = clamp(2·timer, 0, 1)`; relative location = lerp(grab,
current, a); at the end snap to grab, play `PoleTurnLoop` / `PoleSwingLoop` (blendIn −1, mode 3, group
0xC80), `loopTime = clipLength · 1`; sub = 1.
`sub 1`: `loopTime −= dt` until ≤ 0; stop the clip, detach (location = socket "Center"), vt+0xCA8(1); if not
the last entry: `Select(0)`. Last entry: body collision on, upright rotation with the stored yaw, landing =
pawn vt+0x2750(landing, centre, pawn, null, &landing, 60), face the target horizontally,
`SetActorLocation(landing)`, `StopMovement`, `Play("SlideEnd", blendIn 0, mode 2, group 0, rootScale
(−0.2, 1.0))`, sub = 7 → shared tail of 4.7.

### 4.7 Arrival at an enemy

`OnReachEnemy(dir, actor, component)` (0x141342EA0) [C]: accepted when delegateId == 0, sub == 1, the
component's owner is `actor`, flags & 2. If request-interface slot 110 (0x1414476C0: boarding a
`SMALLAIRPLANE` vehicle) takes the actor → state vt+0x268. Otherwise: `m_pHitActor = actor`,
`m_pHitComponent = component`, hit direction = dir, pole = null, **spawn the hit effect** (slot 120), for
kind 2 (ETURN) notify the enemy `enemy.Listener(+0x448).vt+0x58(pawn)` (it answers through
`OnEnemyTurnAck` → `m_pEnemyTurn`), then `Select(4)`. There is **no extra stop distance and no damage**:
the pawn stops where it is when `d < m_CloseRange + 100` (200 cm from the component with the default 100).

`EnemyProc` (0x141346600) [C; constants I 0x141346B98, 0x141346CB5, 0x1413470EF]:
```
sub 0: StopMovement; sub = 6; +0x134 = 0; m_bInputAttack = 0; SetEnemyLock(true)
sub 6: if m_pEnemyTurn not valid:                                   // ENEMY (and ETURN without an answer)
           kicks += 1; sub = 3; Play("AthleticSlideEnd", blendIn 0, mode 2, group 0xC80)
       elif component alive:                                         // ETURN: swing around the enemy
           poles += 1; if flags & 0x10: SetObjTypeDilation(1, 1, slowRate, slowTime); flags |= 0x80
           loopCount = 2; body collision off; landing = actor location; +0x110 = RefPoint; +0x11C = 1
           attach the pawn to the component; timer = 0.5; StopMovement; Flying
           Play("PoleTurn", blendIn 0, mode 2, group 0xC80); grab = rotate(pawn rotation)(0, 0, -100); sub = 5
       else Select(0)
sub 3: if m_bEnableInput:    if flags & 0x10: SetObjTypeDilation(1, 1, slowRate, slowTime); flags |= 0x80
                             sub = 4 (falls through)
sub 4: if m_bEnableChange:   if flags & 0x80: SetObjTypeDilation(1, 1, 1.0, 0); flags &= ~0x80
                             if flags & 1: SetEnemyLock(false)
                             sub = 7
sub 5: until IsAnimEnd: timer -= dt; a = clamp(2*timer, 0, 1); relative location = lerp(grab, current, a)
       then: slow motion off (as sub 4); SetEnemyLock(false) if flags & 1; snap to grab; StopMovement
             Play("PoleTurnLoop", blendIn 0, mode 3, group 0xC80)
             loopTime = clipLength * 2 + 0.2; if m_bInputAttack or m_bInputFlow: loopTime = 0; sub = 2
sub 2: if loopTime > 0: loopTime = (m_bInputAttack or m_bInputFlow ? 0 : loopTime) - dt; return
       if AnimTime < 0.2: return
       StopAnim; detach (location = socket "Center"); vt+0xDC8(); body collision on
       if +0x11C: landing = pawn vt+0x2750(landing, centre, pawn, null, &landing, 60); face the target (yaw only);
                  SetActorLocation(landing); +0x11C = 0
       if m_pEnemyTurn: enemy.Listener.vt+0x60(pawn); m_pEnemyTurn = null
       StopMovement; Play("SlideEnd", blendIn 0, mode 2, group 0, rootScale (-0.2, 1.0)); sub = 7
sub 7 (shared tail, also PoleProc):
       if IsAnimEnd: Finish()
       elif m_bInputFlow:   if no state queued: request iface slot 73 (1, null) = push SUPER_SLIDE (21) kind 1;  Finish()
       elif m_bInputAttack: a = PushState(ATTACK 12); if a: controller vt+0x16D8(); a.target(+0x118) = target component;
                                a.forcedKind(+0x1E8) = 0x38 if kind == 2, 0x36 if kind == 1      [I 0x141347455, 0x141347465]
                                (pawn unique id 0x0B: tutorial event "Tutorial_AthleticFlow")
                            Finish()
       elif kind == 1 and movement+0x9A8 (vertical measured speed, pawn vt+0xE48) == 0: Finish()
```
`SetEnemyLock(b)` (0x1413342C0) [C]: nothing unless flags & 2. `b` and not yet locked: if the component
allows lock-on (vt+0xA80: active, `m_bPermissionLockOn`, owner vt+0x698) → HUD prompt(0), flags |= 0x11,
controller vt+0x18F0(owner, 0, −1) (HUD target display); else no lock. `!b`: if flags & 1: HUD prompt(1);
flags &= ~1. HUD prompt = `FUN_14140DEF0(ctrl, n)` → `FUN_14158B120` (command-menu state; UI strand).

Timeline at a plain enemy [A + D]: `AthleticSlideEnd` (1163, 0.6333 s, root motion: 150 cm hop back):
`TresEnableInput` 0.100 → player object-type dilation 0.25 (if the enemy accepted the lock);
`TresEnableChange` 0.2667 → dilation 1.0, lock prompt off, tail. In the tail the first tick with zero
measured vertical speed ends the state; attack pressed at any time since arrival (the latch is cleared in
sub 0) starts **ATTACK with forced kind 0x36** = clip `1181` (`PlayAttack` 0x1413378B0 case 0x36: reach 850,
+0x18C = 0.3, +0x14C = 4.5 [C]; notifies `TresAtk FF_SONIC_1` 0.400–0.567, `FF_SONIC_2` 0.767–1.067,
`TresEnableInput` 0.733 [A]); after an enemy turn kind 0x38 = clip `1183` (reach 1000; `FF_SONIC_TURN`
0.033–0.167 and 0.200–0.367, `FF_SONIC_TURN_END` 0.400–0.933, `TresEnableChange` 1.067). From there the
normal combo rules apply (combat spec). Attack rows [A `p_ex001_AttackData`]: FF_SONIC_1 power 1.0 SMALL
form point 8; FF_SONIC_2 1.5 TOSS; FF_SONIC_TURN 0.5 (scale 0.5) SMALL; FF_SONIC_TURN_END 1.0 VERTICALROLL.

The **kick projectile**: notify slot +0x228 (`TresShot`) = 0x1413442A0 [C]: only when `m_bInputAttack`:
controller `FUN_14140CCB0(ctrl, 37)` (no focus), and if the weapon is out (pawn vt+0x1330) spawn projectile
`ATHLETIC_KICK` (pawn vt+0x1450(name, RefPoint, RefPoint, actor rotation, team, 1)), then
`SetObjTypeDilation(1, 1, 1.0, 0)`. `ATHLETIC_KICK` = `Blueprints/Projectile/TresProj_AthleticKick` [A]:
life 0.5 s, no movement, not destroyed on hit, attack sphere radius 100 growing 800/s to 350, row
`FF_ATHLETIC_KICK` (power 1.0, BLOW, armor 1, form point 0). The only clip of this state with a `TresShot`
is `WallKickF` (t = 0.1990) and `KickProc` starts it at 0.2000, i.e. just after the notify: whether the
notify still fires is [? probe: projectile spawn on a wall kick with attack held]. `AthleticSlideEnd` has
no `TresShot`.

ERIDE (kind 3) is treated like ENEMY by this state (flag 2, no turn); the riding itself is state 25
`ENEMY_FLOW` (not read).

### 4.8 Slow motion

`AGameStateBase` (world+0x110) keeps a table of time-dilation targets per group and object type (SQEX
"ObjTypeTimeDilation"): vt+0x8A0 (0x14411A180) `Set(group, type, rate, t)` = `FUN_1405DA4E0(actor, group,
type, rate, t)`; vt+0x8A8 (0x144119D90) `SetMask(group, mask, rate, t)` = `FUN_1405DA460`;
`FUN_1405DA3E0(actor, group, t)` = `SetMask(group, 0x7FFFFFFF, 1.0, t)` [C]. `rate` is clamped ≥ 0; the
meaning of `t` (stored next to the rate; presumably the blend time) is [?].

| When | Call | Effect |
|---|---|---|
| aim mode opens | SetMask(1, 0x4D9E, 0.4, 0) | types 1, 2, 3, 4, 7, 8, 10, 11, 14 at 0.4 |
| `StartCommand(37)` | Set(1, 1, 1.0, 0); SetMask(1, 0x4D9E, 0.1, 0) | everything in the mask at **0.1** |
| Travel sub 0 (inside Begin) | Set(1, 1, 1.0, 0) | type 1 (the player) back to 1.0; **the other types stay at 0.1 for the whole airstep** |
| enemy arrival, lock taken | Set(1, 1, 0.25, 0.05) at `TresEnableInput`, Set(1, 1, 1.0, 0) at `TresEnableChange` | hit pause on the player type |
| End → controller clean-up (aim `Exit`, 0x1413FAD90) | SetMask(1, 0x7FFFFFFF, 1.0, 0) | all back to 1.0 |

Which actors carry which object type is engine-side data [? probe]; type 1 containing the player pawn is
[D] from these calls.

### 4.9 Camera (difference to `docs/sora-camera-spec.md`)

`AirstepCamera(on, mode)` = `FUN_141411FD0(ctrl, on, mode)` [C; constants I 0x141411FFA..0x141412210]:
- on, no camera yet: spawn `ATresCameraAthleticFlow` (super `TresCameraTarget`), keep it in `ctrl+0xA90`,
  bind (vt+0x788(pawn, ctrl)), `cam+0x808 = ctrl+0xAA0`, push on the camera stack with a **0 s** blend
  (vt+0x790(1, {0.0, type 3, exp 2.0, 2})).
- mode 0 (flight; from `StartCommand` and from `Select` on the last entry): `cam+0x448 = ctrl+0xA98`
  (distance rate), `cam+0x804 = ctrl+0xA9C` (distance cap), `cam+0x450 = 10.0` (look-point lerp per
  second), `cam+0x44C = 300.0` (look-point lag limit).
- mode 1 (first wall kick / pole): `cam+0x448 = 1.0`, `cam+0x804 = ctrl+0xA9C`, `cam+0x450 = 1.0`,
  `cam+0x44C = 300.0`.
- off (End): remove with a **0.3 s** blend (vt+0x798(0, {0.3, 3, 2.0, 2})), `ctrl+0xA90 = null`.

Controller defaults (aim reset 0x1414162A0 @0x141416572.. [I]): `ctrl+0xA98 = 10.0`, `+0xA9C = 2000.0`,
`+0xAA0 = 1`. These are the fields of `DebugSetAthleticDelayCamera(float, float, int)` (exec 0x142879C10 →
controller vt+0x1290, a stub in the shipped Sora controller). `DebugSetWarpDelayCamera` belongs to state 125
WARP, not to the airstep.

Camera class (ctor 0x1405C01D0 [I]): `+0x800 = 400` (rest distance), `+0x804 = 2000`, FOV 100, look offset
(0, 0, 48) (`FUN_140627E30`), `+0x448 = 10`, `+0x808 = 0`, `+0x3E8 = 120`. On activation it copies the
current camera-manager rotation (vt+0x7F8, 0x1405D2D00). Tick (vt+0x598, 0x1405C8F40) [C], `mode =
cam+0x808` (1 from the controller):
```
target = TargetLookPoint(rot)                              // vt+0x8E0: pawn-based, camera spec
look   = LerpVecClamped(look, target, per = cam+0x450, dt, limit = cam+0x44C)       // FUN_1405D0B30, camera spec 2
mode 1 (0x1405C8360):
    dist = clamp( LerpStep(|look - camPos|, 400, rate = cam+0x448, dt),  |target - look| + 200,  cam+0x804 )   // FUN_1405D08F0
    camPos = look - Forward(rot) * dist;  camera collision FUN_14061CB40(…, 1, 1, 1, 1);  rotation unchanged
mode 0 (0x1405C7F00):
    dist = |look - camPos|;  if dist > 400: dist = clamp(LerpStep(dist, 400, rate, dt), 1.0, cam+0x804)
    rot = Rotation(look - camPos) with pitch clamped to [-80, 80];  camPos = look - Forward(rot) * dist    (no collision call)
```
So during an airstep the camera keeps its orientation, lets the look point trail the pawn by up to 300 cm
and closes its distance toward 400 cm at 10 /s (1 /s after a wall kick or pole), never nearer than lag +
200 and never farther than 2000. Stick input is not read by this class [C].

### 4.10 Interruptions

| Event | Result | Evidence |
|---|---|---|
| damage | none: pawn+0x630 bit 2 (invincible) from Begin to End | [I 0x1405B9120; C Begin / End] |
| jump pressed | `CanJump` (0x14142A3D0 case 42) true; `JumpRequest` (0x14143D8F0 case 42) sets `abort`; **only Travel reads it** (same frame: controller runs before the state): ABORT branch — `SlideEnd` in place, then Finish. No jump is started. | [C] |
| guard button | `OnFlowInput`; consumed only in the shared tail (Superslide) and to cut the enemy-turn loop | [C] |
| attack | `OnAttackInput`; cleared at every launch and at enemy arrival; used by the tail, the turn loop and the Shot notify | [C] |
| target destroyed in flight | weak pointer null → ABORT | [C] |
| blocked (last hop) | `StuckCheck`: 0.2 s below half speed → ABORT | [I] |
| hop time out (T = 1.0 s) | intermediate hop: next entry at once; last hop: ABORT | [C] |
| controller list emptied (aim clean-up) | Tick → Finish | [C] |
| stick, camera stick, other buttons | not read by the state | [C] |

### Open questions
1. Start value of the unlock bit (`+0x95B0` bit 5); the Olympus level scripts call the setter (arguments not read).
2. Sweep shape of `FUN_144526380` descriptor type 0 (sphere assumed, radii 35 / 60) and the exact object
   channels of the level's flow geometry (channel 0x12).
3. Object-type table of the time dilation: which types the mask 0x4D9E covers; meaning of the 4th argument.
4. pawn vt+0x19E0 (movement+0xB70 bits 2–4), vt+0x1388 (equipment vt+0x508), vt+0xCA8,
   `FUN_1405B8B30 / FUN_1405B6100(pawn+0x460)`: visible effect unknown; all are symmetric Begin / End calls.
5. `movement+0x9A0` (measured move speed used by `StuckCheck` and the tail's vertical test).
6. Whether `TresShot` (0.1990) fires when `WallKickF` is started at 0.2000.
7. ETURN / ERIDE enemies (listener vt+0x58 / +0x60, state 25): enemy-side behaviour; pole orientation math in
   `PoleProc` sub 0 (quaternion block not transcribed; same shape as the pole state).
8. `SuperFlightLoop` is not in `p_ex001_AnimSet` (428 names): an even-numbered upward launch then plays
   nothing new [A absence; D].
9. Slot 125 (0x141336EE0) has no caller in the functions read.
10. Per-instance data of placed `TresShootFlowActor`s (kind, `m_CloseRange`, `m_ShootLockOnRange`) per level.

### Runtime probe requests
| Object | Offset | Type | Settles |
|---|---|---|---|
| controller vt+0x1420 object | +0x95B0 | u8 | unlock bits at game start / after the tutorial (Q1) |
| controller | +0x984, +0x988, +0xA80, +0xA98, +0xA9C, +0xAA0 | f32 ×5, i32 | 15000 / 3750 / 10 / 10 / 2000 / 1 in the level under test |
| controller | +0xB00, +0xB20, list +0xB18 entries (+0 f32, +4 u8, +0x14 f32) | | candidate / target counts, kinds and ranges while aiming |
| state 42 | +0x78 i32, +0xD0 i32, +0xD4 i32, +0xDC u8, +0x90 f32, +0x98 i32, +0x138 u32, +0x13C f32, +0x140 vec3 | | sub-step trace of a flight, flags, target point |
| pawn movement component | Velocity, MaxFlySpeed, MaxAcceleration, +0x9A0 vec3, +0xB70 u8 | | speed law, stuck test input (Q5), Q4 |
| pawn | +0x630 u32 | | invincible bit during the state |
| game state (world+0x110) | +0x530 + (group·16 + type)·12: f32 rate, f32 t | | dilation table during aim / airstep / hit (Q3) |
| airstep camera (`ctrl+0xA90`) | +0x428 f32, +0x448, +0x44C, +0x450, +0x800, +0x804 f32, +0x808 i32 | | camera distance law |
| level `TresShootFlowComponent` instances | +0x87C u8 kind, +0x880 bit 0, +0x88C f32, +0x89C f32 | | Q10 |
| function-entry event 0x1413442A0 | with state+0x128 | | Q6 |

### What the host must provide
**Per airstep target** (enemy or level point), refreshed every tick:

| Field | Type | Used for |
|---|---|---|
| handle + alive flag | id, bool | weak reference; death aborts the flight |
| active, move-flow permission | bool, bool | candidate filter (4.2.1) |
| team | int | must differ from the player's |
| kind | 0..9 (`ESQEX_FreeFlowKind`) | 1–3 enemy path; ≥ 4 chain path; 6 pole |
| world position | vec3 | selection distance and screen test |
| target point | vec3 | flight target (= position; for a pole: position + rotated `m_AttachOffset · scale`) |
| own range | f32 (`m_ShootLockOnRange`; 0 = default 15000) | selection range |
| close range | f32 (`m_CloseRange`, default 100) | arrival radius = this + 100 |
| lock-on permission | bool | lock prompt and the 0.25 hit pause (enemies) |
| owner actor handle | id | sweep ignore, hit actor |
| pole only | attach offset, attach radius, horizontal flag, base offsets, check radius, rotation | 4.6 and actions spec 10.4 |
| wall points | nothing extra: the kick direction is the pawn → point direction, no surface normal is read | 4.6 |

**Services**: view-projection + viewport rectangle (screen test, 1280×720 reference); a sphere sweep
(radius 35 / 60) against static + dynamic world geometry with an ignore list, returning the hit centre;
`SetActorLocation` with and without sweep; a flying character move driven by a requested velocity with
settable max speed / acceleration and a "measured speed" read-back; switches for world collision and body
collision of the pawn; gravity and invincibility switches; object-type time dilation (player type vs the
rest); the ledge probe and pole probe of the actions spec; the lock-on HUD target and the command prompt
states 0 / 1 / 3; an effect spawner (`vp_ffa_arv0` at the pawn's reference point, the path marker
`vp_ath_tgt0`); projectile spawn by name (`ATHLETIC_KICK`); buttons: airstep, attack, jump, guard.

---

# Part I — Second pass: spell behaviour details (closes open items of B)

_Source of this part: `build/sora_tmp/magic/spell2/spec.md` (section numbers are local to the part)._

This text REPLACES the open items of Part B (`build/sora_tmp/magic/proj/spec.md` section 5, items 1, 2, 4, 5, 7 and the
`[?]` notes of its sections 3.2-3.6) and answers the grand-magic question left open by Part A (5.2) and Part C (section 7,
open items 4 and 10). Everything not repeated here stays as Part A / B / C state it.
Units cm, s, deg. Tags: [I] instruction level (address given), [C] decompiled C only, [U] stock UE 4.17 by function
identity, [A] asset (file, property), [D] derived, [?] not determined statically.
Every number is a row of `params.json` (ids `spell2.*`); assets are in `assets.json`.

### 0. Shared rules that settle several spells

#### 0.1 One Shot notify = one actor [A, C]
Every cast clip of the Kingdom Key has exactly one `TresShot` notify (`1341, 1348, 1351, 1358, 1342, 1352, 1321, 1322,
1331, 1332` `.anim.json`) [A]. `OnShot` 0x14137BB30 contains one call of the spawner (`vt+0x788`) and no loop [C]; the
spawner is called with `bFireNow = 0` and the actor is fired by the last setter `vt+0x1270` (= slot 590 StartFire) [C].
Correction to Part A 1.4 step 8: the sixth spawner argument is the caster's **team id** (`pawn vt+0xD90`), not a yaw
(the same getter is the team comparison in the hit code 0x140618940 and in `DoCure`) [C].

| clip (AnimSet name) | asset | Shot | EnableChange | length | used by |
|---|---|---|---|---|---|
| MagicCast / MagicCastM | 1341 / 1348 | 0.3333 | 0.8667 | 1.1333 | Thunder, Water, Aero, Cure (ground, standing / moving) |
| MagicAirCast / MagicAirCastM | 1351 / 1358 | 0.3333 | 0.6667 / 0.8667 | 1.0333 / 1.1333 | same, air |
| MagicChant / MagicAirChant | 1342 / 1352 | 0.5667 / 0.5 | 1.4667 / 1.1 | 1.5667 / 1.2 | finish-end and grand casts of the cast-type spells |
| MagicFire / MagicAirFire | 1321 / 1331 | 0.3 | 0.7333 | 1.2 | Fire, Blizzard |
| MagicShoot / MagicAirShoot | 1322 / 1332 | 0.7 / 0.6667 | 1.4333 / 1.1667 | 1.5333 / 1.2667 | finish-end and grand casts of the shot-type spells |

#### 0.2 Hit list: once per enemy, never re-armed for magic [I]
The attack-collision component keeps one record per shape group (map at comp+0x908, entry 0x68 bytes, record = entry+8):
flags (+0), hit-list reset interval (+4), reset timer (+8), callback interval (+0xC) and counter (+0x10), hit list
(+0x18), pending list (+0x38), clash list (+0x48). Activation 0x140625500 writes `interval = m_AttackInterval` of the
`m_CollAutoActivate` entry (0 without an entry) and clears all lists (0x140626910). The per-tick function 0x14062CFC0:

```
if rec.interval > 0:                       # comiss [rbx+4], 0   (0x14062D035)
    if hitList.empty and !(flags & 2): rec.timer = 0
    else: rec.timer += dt; if rec.timer >= rec.interval: hitList.clear(); rec.timer = 0; ...
```

So the list is only ever cleared when `m_AttackInterval > 0`. Every magic blueprint has 0 (or no entry) [A] →
**an enemy is hit at most once per actor** (successors inherit the list where `m_bReSpawnTakeOverAtkCollHitList` is set).
A victim is appended to the list (0x1406180B0, call at 0x140619EA5) when the pre-hit classification 0x14061FE20 returns
1; it returns 0 only for hit body parts with reaction type GHOST / GHOST_NOREACT (`ETresBodyCollReactionType` 8, 9), which
are therefore re-tested every tick without damage [C].

#### 0.3 Attack row selection [I]
`SetFinish(bFinish, bSituation)` = projectile slot 548 0x1414827B0:

```
proj.flags6B8.bit1 = bFinish | bSituation;  proj.flags6B8.bit2 = bSituation;  proj.flags6C0.bit13 = bFinish
if bSituation:   proj.overrideId(+0x6B0) = m_OverrideSituationAttackDataID (+0x5E8)      # setter 0x141482990 stores even None
elif bFinish:    proj.overrideId         = m_OverrideFinishAttackDataID    (+0x5E0)
```

Activation 0x141486890 uses `overrideId`, or the shape's `m_DefaultAttackDataIDName` when it is None [C]. `OnShot` passes
`bFinish = FireMagic flag 0x80` (the *last* finisher, "Finish End") and `bSituation = flag 0x400` (Grand) (0x14137BB30:
`vt+0x1120((flags>>7)&1, (flags>>10)&1)`). A successor / residue receives the command id, combo values and the full-MP
flag always, and `SetFinish(parent.bit1, parent.bit2)` when the parent has `m_bReSpawnTakeOverAtkCollFinishFlag`
(0x141477100, C lines 432-473) [C]. The full table is section 6.2.

#### 0.4 Reflection / clash never applies to Sora's magic [I]
0x140645900 copies the row's `m_ReflectLevel` to prim+0xA2B and sets prim+0x986 bit7 when the level is `< 1`
(0x140645B48..0x140645B6A). The attack-versus-attack loop of 0x140618940 (results 3 REFLECT / 4 GUARD_REFLECT) requires
bit7 clear on both primitives (0x140618E96, 0x140619233, 0x140619240). All 60 base `MAG_*` rows have level 0 [A] → no
magic shape of Sora clashes with, blocks or reflects an enemy attack shape, and none can be reflected by a clash.

### 1. Thunder

**Settled: one column per cast, for every tier. No chain, no second strike.** [A, C]
- one Shot notify per clip (0.1), one actor per notify;
- `TresProj_Mag{Thunder,Thundara,Thundaga,Thundagan}Nml` and `TresProj_MagThunderBase` have no blueprint graph (exports:
  class, CDO, 4 components, SCS only), no `m_ReSpawnProjectileClass`, no `m_ResidueList` [A];
- `ATresProj_MagicThunder` overrides no projectile slot; its notify handler 0x14147D0F0 = base handler + a "tail" particle
  from the column to the victim's `Center` socket, drawn only when `m_HitTailEffect` (+0x888) is set — it is null in
  every cooked thunder blueprint [A] → inactive.
"Thunder hits several enemies" is purely the growing cylinder: every enemy that overlaps it during its life is hit
once (0.2; `m_bNoDestroyOnHit`), with no count limit.

```
on Shot (t_shot from 0.1; MP deducted here):
    T = homingComponent.location  else targetPawn.location  else actor.location + forward(actorYaw) * 500     # 0x14137C216 [I]
    if a target actor exists: T.z = max(T.z, targetActorRoot.z)
    column = spawn(class(cmd), at T, team = caster team);  column.SetFinish(flag 0x80, flag 0x400);  StartFire
column, each tick while age <= life:        # age = time since the Shot
    r = 100 * min(scaleMax, scale0 + inc * age);   cylinder axis +Z, centre T + (0,0,cz), radius r, height h
    for enemy overlapping (swept shape) and not in hitList: hit(enemy, row); hitList.add(enemy)
```

| | Thunder (37) | Thundara (38 / 116) | Thundaga (39 / 117) | grand Thundagan (118) |
|---|---|---|---|---|
| columns per cast | 1 | 1 | 1 | 1 |
| radius: start → max (growth), time to max [D] | 100 → 400 (+1200/s), 0.25 s | 400 → 800 (+2400/s), 0.1667 s | 400 → 1200 (+3600/s), 0.2222 s | 400 → 1400 (+1400/s), 0.7143 s |
| height / centre above T | 600 / +150 | 800 / +150 | 1000 / +100 | 150 / 0 |
| extra shape | – | – | – | capsule r 100 → 1400 (+700/s), half length 500 → 1400, centre +500 |
| life | 0.6 | 0.4 | 0.5 | 2.0 |
| targets | all inside, once each | same | same | same |
| row normal / finish-end / grand | MAG_THUNDER / _FIN / – | MAG_THUNDARA / _FIN / _S | MAG_THUNDAGA / _FIN / _S | – / – / MAG_THUNDAGAN |
| DISCHARGE (100 %) | 5 s | 7 s | 10 s | 10 s |

The target point is the caster's current target (Part A 1.3: lock-on, else auto-target, refreshed every tick until the
shot unless AUTO_LOCK_MAGIC); there is no target search in the projectile.
[?] Whether the DISCHARGE status object (`TresGameModeLoadAsset.m_BadStatDischargeClass`) itself shocks neighbouring
enemies is enemy-status code, not read.

### 2. Aero

#### 2.1 No re-hit; the "swirl counter" limits how many hits are swirls [I]
There is no mechanism that re-arms hits (0.2), and none exists on the victim side either: the SWIRL_UP reaction (2.2)
never touches the attacker's lists. The 2 s column therefore hits each enemy **once** — when the growing cylinder first
overlaps it. The counter of `ATresProj_MagicAero` (notify-interface slot 15 = 0x14147D060):

```
OnAttackHit(hit, {component, primitive, result}):          # called once per reported hit, after the damage call
    base handler 0x14147D600
    count (+0x890) += 1                                    # 0x14147D083
    if m_SwirlLimit (+0x888) > 0 and count >= m_SwirlLimit:
        primitive.info.m_DamageKind (prim+0x9D2) = m_AfterDamageKind (+0x88C)     # 0x14147D0D1
        m_SwirlLimit = -1                                  # 0x14147D0D7
```

`m_AfterDamageKind` = 2 `TRES_DMG_KIND_BLOW` (native constructor 0x14146FA60, never overridden) [I, A]. So the first
`m_SwirlLimit` enemies hit by one cast are swirled up; every later enemy of the same cast is knocked away (BLOW) with the
same row otherwise. Slot 570 0x141483D50 only copies the counter to a spawned successor of the Aero class (slot 613
`SetSwirlCount` 0x141482570) — the residue has no attack shape, so this has no gameplay effect [I].

| | Aero (45) | Aerora (46 / 122) | Aeroga (47 / 123) | grand Aerogan (124) |
|---|---|---|---|---|
| hits per enemy | 1 | 1 | 1 | 1 |
| swirl limit (enemies swirled; the rest BLOW) | 3 | 8 | 14 | none (−1) |
| radius 100 → max (growth) | 400 (+1200/s) | 800 (+1200/s) | 1200 (+2400/s) | 1400 (+2400/s) |
| height / centre above T | 400 / +100 | 600 / +100 | 800 / 0 | 1200 / +200 |
| life | 2 | 2 | 2 | 2 |
| row normal / finish-end / grand | MAG_AERO / _FIN / – | MAG_AERORA / _FIN / _S | MAG_AEROGA / _FIN / _S | – / – / MAG_AEROGAN |
| `m_DamageEffectTime` (hold) normal / finish / grand | 2 / 2.5 / – | 2.5 / 3 / 3 | 3 / 3.5 / 3.5 | – / – / 4 |
| `m_PhysForceLv` | 1 | 2 | 3 | 4 |

Target position T and timing exactly as Thunder (direct-position unit).

#### 2.2 What the victim does: SWIRL_UP as the game implements it for a generic AI character [C, constants I]
Passed to the victim: the `FTresDamageInfo` of the row (`m_DamageKind` 29, `m_DamageEffectTime`, attribute AERO,
`m_CommandKind` = the cast command) and the damage causer = the Aero actor. `UTresAICharState_Damage` (vtable
0x145C92608) setup 0x140457380 selects the per-kind routine by `m_DamageKind` (`case 0x1d` → slot 129 = 0x140450F20) and
stores a weak pointer to the causer (state+0x1A0; a weapon is replaced by its owner). Parameters come from the enemy's
`TresEnemyCommonParam` (state `vt+0x468`), asset `Blueprints/Enemy/EnemyCommonParam` [A].

```
centre C: causer root location if the causer is still valid when the reaction starts, else the victim's own location   # fixed afterwards
start (sub-steps 0, 1):
    vz0 = m_SwirlUpPower (1400) + rand[0,1) * m_SwirlUpPowerRand (100)          # getters 0x14043D310 / 0x14043D340
    movement mode Falling; velocity.z = vz0; GravityScale = 3.0                 # 0x140451021
    animation "m_250" (then loop "m_250_1"); body pitch call vt+0x21A8(-90)     # [C] meaning of the call not read
    effect m_pro_SwirlUpEffData (vm_air_react0) at (C.xy, own z) if the command is of category MAGIC and the attribute is AERO
    orbit = true
every tick while orbit (rise and hold):
    d    = normalize2D(P - C)                                 # P = victim location
    step = RotateAngleAxis(d, 105 deg, +Z)                    # 0x1404568B0; 90 = tangent, the extra 15 turn inward
    G    = P + step * 10;  if dist2D(G, C) < 100: G = C + normalize2D(G - C) * 100
    velocity.xy = normalize2D(G - P) * 2500                   # pawn vt+0x16C0;  0x140452121
    Turn(yaw of that direction, dt * 30)
rise → hold: when velocity.z <= 0:                            # apex of the vz0 / (3 g) arc
    movement mode Flying, gravity disabled
    T_hold = info.m_DamageEffectTime if > 0 else m_SwirlUpFlightDuration (1.5)  # 0x140451560
hold: T_hold -= dt; at <= 0: orbit = false
release (orbit false): same steering direction, but
    speed.xy = |velocity.xy| * 0.95^(dt*60)                   # data 0x1486D8184
    when |velocity.xy| < 500 (data 0x1486D8180):
        stop the effect; movement mode Falling; gravity enabled; GravityScale = m_SwirlUpFallGravityScale (1.0)
        animation "m_250_2" → loop "m_250_3" → on landing: stop, "m_250_4" → "m_250_5" → reaction ends
```

With UE gravity −980·3 the rise takes `vz0 / 2940` = 0.476..0.510 s and reaches 333..383 cm [D]. During the hold the
victim circles the column centre at 2500 cm/s on a spiral that converges to radius 100 (≈ 4 turns per second at 100 cm)
[D]. Per-enemy overrides of `m_SwirlUp*` exist in the `e_ex*_EnemyCommonParam` assets (not listed).
Whether an enemy enters this reaction at all (armour, reaction power 1 / 5, bosses) is decided by the enemy's reaction
tables — host matter. The limit-exceeded hits use kind BLOW with the same row (`m_DamagePowerScale` 1).

#### 2.3 Residue `TresProj_MagAeroCylinder` — a 6 s jump pad for the player only [I, C]
Spawned once per Aero actor (residue check type HIT_ALWAYS: at the first reported hit or at time-out) at the Aero
position. Life 6 s (ctor 0x14146FAF0). Its `TresChrLauncherComponent` (cylinder radius 100, height 50, centre 20 below the
actor; `m_JumpMode` TCLT_AERO, `m_LaunchHeight` 0) collects overlapping character pawns; its tick 0x140F60730:

```
pawn = first valid entry of m_PawnList;  if component active (bit0 of +0x868):
    ok = pawn.vt+0x10A8(height = m_LaunchHeight, bFlag = (mode == NORMAL or AERO), jumpType = (mode == AERO ? 2 : 0))   # 0x140F60875..0x140F608A2
    if ok: broadcast OnChrPawnLaunched(pawn, height, component)       # blueprint: deactivate the launcher, BP_ReqLifeOver → actor ends
```

- `ATresCharPawnBase::vt+0x10A8` = 0x1405ADFB0 returns false; enemies inherit it → **no effect on enemies** [I].
- Player override 0x141436C40 → request-interface slot 217 `TryLaunch` 0x14143E670: gate slot 216 0x14142B070 (can act;
  with bFlag = 1 any current state except the listed cinematic / link / formchange-finish states, see NOTES; nothing
  blocking queued); then `RequestJump(yaw, type 2, bFall 0, bAir, b6 = 0)` (slot 170, actions strand): **a super jump**
  (power `MyJumpPowerSuper` 2500 with ability SUPERJUMP, else the fallback of that function: double-jump or normal power),
  yaw = actor yaw (or the state's move yaw), Jump state: momentum direction zero, `+0x110` = 0.92, flag bit0 set;
  `m_LaunchHeight` 0 → no height override (the flag 0x80000 / `+0x118` path is for `> 0`) [C].
So: for 6 s after an Aero, Sora touching the 100 cm disc at the column position is thrown up with super-jump power,
once (the pad then ends).

### 3. Water

#### 3.1 The barrier follows Sora [I]
`ShotWater` 0x14137FFA0: spawn position argument (50, 0, 100) (0x141380064..0x141380080), direction
`Rot(pitch 60, 0, 0).Vector()` (0x14138008D), then `AttachToComponent(proj, pawn.RootComponent, rules, None)`
(call 0x143E8FC60). The rules object 0x149D88D70 lies in zero-initialised storage and has no writer in the image → all
three rules = KeepRelative [D]: the spawn transform becomes the **relative** transform. The barrier actor therefore sits
at Sora's root + (50 forward, 0, 100 up) in Sora's frame, pitched 60° up, and moves and turns with him for its whole
life. Its hit sphere is centred 50 above the actor in world axes (`m_bAbsoluteOffset1`), radius `10 + 280·t`.
At time-out (0.6 s; grand 0.5 s) the bolt is spawned at the barrier's world position, heading along the barrier's
forward axis (60° above Sora's facing at that moment), and homes as Part B 3.4 describes.

#### 3.2 What the barrier does to incoming attacks [I]
- The barrier shape is an **attack** shape with row MAG_WATER_BARRIER: power 1, reaction power 0, kind SMALL,
  `m_ReflectLevel` 0. Because of 0.4 it never clashes: it does not block, cancel or reflect enemy attack shapes or
  projectiles. The actor's collision profile is the query-only `TresProjectile`; enemy projectiles do not collide with it.
- The protection is the caster flag: `OnShot` sets FireMagic flag 0x80000 and `pawn vt+0x2068(1)` (0x14137BD30) = pawn+0x630
  bit 2 `m_bIsInvincible` (setter 0x1405B9120); `End` 0x14136CA50 clears it when `flags & 0x81480`. The bit is read only
  through `vt+0x2060` (player 0x141435A90 → 0x1405AD120, `test [rcx+630h],4`); on the hit path its consumer is
  `ShouldTakeDamage` 0x1405BA540 (the scripted direct-damage helpers 0x1405A24A0 / 0x1405A3970 test it as well):

```
ShouldTakeDamage(damage, event):  return Super(...) and (!IsDamageInvalid() or info.m_DamageKind == RECOVER(13))    # 0x14064E770
```

  `TakeDamage` 0x1405BB850 then returns 0: no HP loss, no reaction, no status. The attacker still registers a hit (the
  victim was added to its hit list in 0x140618940 before the damage call): a melee swing is consumed, an enemy
  projectile explodes on Sora as usual.
- Duration: from the Shot notify (0.3333 s; Chant 0.5667 / 0.5) to the end of the FireMagic state — animation end
  (1.1333 s; air 1.0333; Chant 1.5667 / 1.2) or an earlier leave through a chain / cancel after EnableChange. It is tied
  to the state, not to the 0.6 s barrier actor.

| | Water (41) | Watara (42 / 119) | Wataga (43 / 120) | grand Watagan (121) |
|---|---|---|---|---|
| barrier life / final radius [D] | 0.6 / 178 | 0.6 / 178 | 0.6 / 178 | 0.5 / 150 |
| barrier row (all casts) | MAG_WATER_BARRIER | same | same | same |
| bolt row normal / finish-end / grand | MAG_WATER / _FIN / – | MAG_WATARA / _FIN / _S | MAG_WATAGA / _FIN / _S | – / – / MAG_WATAGAN |
| splash row | none (effect only) | as bolt | as bolt | MAG_WATAGAN |
| barrier hit list passed to the bolt | no | no | no | no |

Host routing: while the flag is set every incoming hit on Sora = "hit registered, zero damage, no reaction"; healing
passes. Nothing is reflected.

### 4. Blizzard — the ice rail

#### 4.1 Creation [C; constants I]
`OnPutEffect(P)` 0x1414757F0 runs every 300 cm of travel (`m_fPutEffectDuration`; the first one 35 cm after the fire
position) and once more at shutdown. `DoFire` 0x141474C80 first tests a line from behind the spawn point (spawn −
velocity direction × horizontal distance caster→spawn) to the spawn point; a blocking hit sets the "closed" flag
(+0x8B0 bit0) → no rail for this bolt.

```
OnPutEffect(P):
    if m_GenerateRailLife <= 0: prev = none; return
    if !closed and sphereSweep(r 25, lastPut → P) hits a blocking actor of the "solid" kind: P = hit point; closeAfter = true
    floor = first acceptable hit of lineTrace(P + (0,0,100) → P − (0,0,m_GenerateRailHeight = 200))     # channel mask 0xF
            # skipped: start-penetrating hits, actors of category 2, components that do not block channel 0xF;
            # rejected when a water surface lies at or above the hit (0x1445CEA40)
    if no floor: goto trail
    if prev exists:
        obstacle test: sphere sweeps r 60 between (prev + 50 up) and (floor + 50 up); a blocking hit moves the floor
            point to the ground under the obstacle
        if dist(prev, floor) <= 10: no node                                   # dist² > 100
        if capsuleSweep(r 20, half height 40, prev + 70 up → floor + 70 up) hits: no node, chain broken (prev = none)
    else:
        if sphereSweep(r 60, floor + 100 up → floor) starts inside geometry: no node
    if node allowed and !closed:
        node = SpawnRailSlideActor(prev, floor, rotation 0, life = m_GenerateRailLife)       # slot 593, 0x1414769A4
        node.Tags += <rail tag>;  if the floor component is movable: attach node to it
    trail: extend the spline mesh actor (visual)
    if closeAfter: closed = true;  if closed: CloseTrail() (slot 613 0x141472810)
    lastPut = P;  prev = node (or none)
```

`SpawnRailSlideActor` 0x141479A60: spawns the **native** `ATresRailSlideActor` (no blueprint) at the floor point;
`SplineActorTangent` (+0x3F8) = unit vector from the previous node to this one (the bolt's velocity direction for the
first node); `SetLifeSpan(3 s)`; `prev.vt+0x7C0(node)` connects the two (a spline component between consecutive
nodes); registers with the projectile manager [C].

#### 4.2 Geometry
- Nodes lie **on the floor** (the trace hit point; height 0), up to 200 cm below the bolt's path and at most 100 above
  it; spacing = 300 cm of bolt travel (shorter for the first and the last put), never closer than 10 cm.
- Consecutive nodes are joined by one spline segment with unit-length tangents → a polyline for all practical purposes
  [D]. No width: the rail is a curve; the player's detection radius is the "width" (4.3). `m_bIsNarrow` false.
- Each node lives 3 s from its own creation: the rail appears and disappears progressively behind the bolt (a 2000 cm/s
  bolt lays a node every 0.15 s → about 20 nodes / 6000 cm exist at once) [D].
- Node defaults (ctor 0x140F8B5B0) [I]: rail type 0, start direction 0 (either way), `m_DetectRangePlus` 0, reverse /
  attack / jump / air-slide / flow actions all enabled, `m_bDisableActTransferOnRelativeJump` true, speed override off
  (`m_bSetSlideSpeed` false; the stored 2000 cm/s and the acceleration rates 3 / 0.5 are not applied), no nav data.

#### 4.3 How Sora gets onto it [C; constants I]
No `docs/sora-actions-spec.md` rail-slide section exists yet; the entry path is:

```
each tick of Idle, Run (when its rail cool-down Run+0xC4 <= 0) (range 50) and Jump (air range 150):
    # the Damage and Guard ticks contain the same probe, but the gate below refuses in those states
    A = root location;  B = A + (0,0,30);  hint = stick direction if moving else actor forward
    info = pawn.vt+0xEA0(A, B, range, hint, filter = 1)          # 0x1405A7740, rail probe
    if info: requestInterface.TryRailSlide(info, keepQueue = 1, 0)   # slot 75, 0x14143F4E0
probe: for every registered, active rail actor:
    type 1, 2, 7: skip.   type 0 with the spell rail tag: only while GetLifeSpan() >= 0.1 s          # 0x140F967C0
    r = range + actor.m_DetectRangePlus;  cull box = max(r, 200)
    for each active spline segment of the actor: d² = segment.ClosestDistanceSquared(A, B)
    candidate if d² <= r²; the nearest wins; direction along the tangent, reversed when dot(tangent, hint) < 0
        (start direction 1 / 2 force forward / reverse)
TryRailSlide: gate slot 74 0x14142C800: can act (vt+0x29C0), game flag bit7 of GameInstance+0x95B0, not swimming,
    current state EMPTY / IDLE / RUN / JUMP (or COVER on the ground), pawn vt+0x2788() == 0;
    start actor type != 1;  → slot 76 0x141445570: PushUnique(RAIL_SLIDE 22) (138 for rail types 3..6) with the move info
```

Sora's root is the small capsule at the feet (half height 13.6), so a floor rail under him is within the 50 cm range.
The ride itself (`TresPlayerState_RailSlide`, speed, jump off, end of rail when a node expires) is not specified by any
strand yet [?].

#### 4.4 Host interface
Provide: line trace down (P+100 → P−200) with hit point and "is water at / above"; sphere sweeps r 25 / r 60 and a
capsule sweep r 20 × hh 40 against level geometry. Receive: a temporary rail = ordered list of floor points, each with an
expiry time (creation + 3 s), plus "closed" (no further points). A host without rail riding can ignore the nodes; the
visible ice trail is the separate spline-mesh actor (cosmetic).

### 5. Cure

```
request: target = party member chosen in the menu (Sora himself for a self cast); refused while MP charge (Part A 2.2)
state FireMagic (clip MagicCast family, Shot at 0.3333 s):
    Begin: if HasAbility(LEAF_VEIL 43): flag 0x1000, pawn invincible for the whole state (cleared in End); slot 91 also refuses damage reactions
    before the Shot: a damage reaction replaces the state → End: no actor was spawned, no MP was spent, no heal    [C: MP use and spawn are both inside OnShot]
    Shot: spawn Cure actor at the muzzle; SetHomingTargetComponent / SetHomingTarget → m_pTargetActor (0x141482650 = component owner, 0x141482590);
          StartFire; Cure Converter; UseMagicPoint(all)
Cure actor Tick 0x1414840A0 (first actor Tick after StartFire, flag fired && !exploded):
    if GetTarget(): DoCure(target) (slot 613 0x1414742A0); exploded = true; Shutdown
    (no target: nothing; the actor times out after m_MainLife = 1 s)
```

- The heal is done by the actor, not by the state: once the Shot notify has fired, damage to Sora no longer prevents it.
  Delay Shot → heal: at most one frame (whether a spawned actor ticks in its spawn frame is engine tick order) [?].
- Without Leaf Bracer the cast is interruptible like any other from the first frame up to the Shot; after the Shot only
  the remaining animation is lost. With Leaf Bracer no damage can be taken from Begin to End (3.2 mechanism).
- Self cast: `DoCure(target = Sora)` — Sora gets `m_AppendCure + max(MagicPower, 1)`; every other pawn of Sora's team
  (player or NPC class) whose root is within `m_DetectRange` (3-D) of **the target** gets `m_BonusAppendCure +
  max(MagicPower, 1)`; a pawn in state DIE (59) gets half, rounded up (Part C section 5, [I] there). The facing step of
  the state skips a target equal to the caster (Part A 8).
- Target = another member: that member gets the full amount, Sora gets the bonus amount only if he is within range of
  the member.

| | Cure (49) | Cura (50) | Curaga (51) | grand Curagan (125) |
|---|---|---|---|---|
| target heal (+ magic stat) | 80 | 120 | 160 | 9999 |
| partner heal (+ magic stat) | 40 | 60 | 80 | 9999 |
| partner range around the target | 350 | 550 | 800 | 5000 |
| MP | all (→ MP charge) | all | all | none (grand) |

### 6. Fire, and the attack rows of every spell

#### 6.1 Explosion ("Burn") conditions [I]
`Respawn(class, hit)` slot 535 0x141477100:

```
if flags6C0 & 6: return                                  # 0x141477185 (bit1 set by vt+0x11C0, bit2 = already respawned)
if !m_bReSpawnProjectileOnTimeOut: return                # 0x141477192, master enable (+0x61C bit0)
if class == null or not a projectile class: return       # 0x14147719F
switch m_RespawnCheckType (+0x620):                      # 0x1414771DC
    0 HIT_ALWAYS: ok      1 HIT_ANY: hit != null      2 HIT_PAWN: hit != null and !IsLevelGeometry(hit)     # 0x140643CC0
    3 HIT_MAP:   hit != null and IsLevelGeometry(hit)                 4 NO_HIT: hit == null
```

- **Fire (tier 1)**: `TresProj_MagFireNml` has neither the enable bit nor a class [A] → never an explosion, on any
  surface or time-out. Single bolt, single target, row MAG_FIRE.
- Fira / Firaga / grand: enable + HIT_PAWN + class `Fi{ra,ga,gan}NmlBurn` → explosion only when the bolt's hit is a
  non-level actor (an enemy, a destructible pawn); not on walls / floors, not on time-out, not on a stuck shutdown.
  Sphere r 150 / 250 / 400, life 0.25 s, rotation cleared, inherits the hit list (the directly hit enemy is not hit
  again) and the finish / situation flags, same row family as the bolt.
- "MP was full" flag: FireMagic flag 0x20000 → `proj vt+0x1160` 0x141482870 (proj+0x76C bit0) → activation bit →
  prim+0xA19 bit4 = `m_bIsEnableFullMpBurst`; always handed on to successors. Its only consumer is FULLMP_BURST in the
  damage function (Part C 4.2).

#### 6.2 Attack rows by command [A + 0.3]
"finish-end" = the cast carries FireMagic flag 0x80 (with default abilities the 4th cast of a combo; ENDLESS_MAGIC: never).
A finisher that is not the last one (flag 0x40 only) uses the normal row. Grand casts always use the situation row
(the -gan classes have neither override → their default row). Values: power / form points (`m_FormPoint`, section 7).

| command | actor(s) | normal | finish-end | grand (S_ command) |
|---|---|---|---|---|
| Fire 29 | bolt | MAG_FIRE 1.0 / 15 | MAG_FIRE_FIN 1.2 / 18 | – |
| Fira 30 · S_FIRA 110 | bolt + burn | MAG_FIRA 1.5 / 15 | MAG_FIRA_FIN 1.8 / 18 | MAG_FIRA_S 3.0 / 0 |
| Firaga 31 · S_FIRAGA 111 | bolt + burn | MAG_FIGA 2.2 / 15 | MAG_FIGA_FIN 2.6 / 18 | MAG_FIGA_S 4.4 / 0 |
| S_FIRAGAN 112 | bolt + burn | – | – | MAG_FIGAN 6.6 / 0 |
| Blizzard 33 | bolt (burn has no shape) | MAG_BLIZZARD 0.8 / 12 | MAG_BLIZZARD_FIN 0.95 / 14 | – |
| Blizzara 34 · 113 | bolt | MAG_BLIZZARA 1.2 / 12 | MAG_BLIZZARA_FIN 1.4 / 14 | MAG_BLIZZARA_S 2.4 / 0 |
| | burn | MAG_BLIZZARA_BURN 1.2 / 12 | MAG_BLIZZARA_BURN_F 1.4 / 14 | MAG_BLIZZARA_BURN_S 2.4 / 0 |
| Blizzaga 35 · 114 | bolt | MAG_BLIZZAGA 1.8 / 12 | MAG_BLIZZAGA_FIN 2.15 / 14 | MAG_BLIZZAGA_S 3.6 / 0 |
| | burn | MAG_BLIZZAGA_BURN 1.8 / 12 | MAG_BLIZZAGA_BURN_F 2.15 / 14 | MAG_BLIZZAGA_BURN_S 3.6 / 0 |
| S_BLIZZAGAN 115 | bolt / burn | – | – | MAG_BLIZZAGAN 5.4 / 0 · MAG_BLIZZAGAN_BURN 5.4 / 0 |
| Thunder 37 | column | MAG_THUNDER 1.2 / 6 | MAG_THUNDER_FIN 1.4 / 7 | – |
| Thundara 38 · 116 | column | MAG_THUNDARA 1.6 / 6 | MAG_THUNDARA_FIN 1.9 / 7 | MAG_THUNDARA_S 3.2 / 0 |
| Thundaga 39 · 117 | column | MAG_THUNDAGA 2.4 / 6 | MAG_THUNDAGA_FIN 2.85 / 7 | MAG_THUNDAGA_S 4.8 / 0 |
| S_THUNDAGAN 118 | column (2 shapes) | – | – | MAG_THUNDAGAN 7.2 / 0 |
| Water 41 | barrier | MAG_WATER_BARRIER 1.0 / 8 | same | – |
| | bolt (splash has no shape) | MAG_WATER 0.9 / 12 | MAG_WATER_FIN 1.05 / 14 | – |
| Watara 42 · 119 | barrier | MAG_WATER_BARRIER | same | same |
| | bolt + splash | MAG_WATARA 1.4 / 12 | MAG_WATARA_FIN 1.65 / 14 | MAG_WATARA_S 2.8 / 0 |
| Wataga 43 · 120 | bolt + splash (+ barrier) | MAG_WATAGA 2.0 / 12 | MAG_WATAGA_FIN 2.4 / 14 | MAG_WATAGA_S 4.0 / 0 |
| S_WATAGAN 121 | bolt + splash (+ barrier) | – | – | MAG_WATAGAN 6.0 / 0 |
| Aero 45 | column | MAG_AERO 0.6 / 6 | MAG_AERO_FIN 0.7 / 7 | – |
| Aerora 46 · 122 | column | MAG_AERORA 1.0 / 6 | MAG_AERORA_FIN 1.2 / 7 | MAG_AERORA_S 2.0 / 0 |
| Aeroga 47 · 123 | column | MAG_AEROGA 1.4 / 6 | MAG_AEROGA_FIN 1.65 / 7 | MAG_AEROGA_S 2.8 / 0 |
| S_AEROGAN 124 | column | – | – | MAG_AEROGAN 4.2 / 0 |
| Cure 49..51, 125 | heal actor | no attack row (section 5) | – | – |

Row kinds: normal SMALL (Aero SWIRL_UP); finish PARABOLA 75° (Thunder BEAT, Aero SWIRL_UP); grand `_S` rows as finish; the
-gan rows PARABOLA with angle 0 (grand Thunder TOSS, grand Aero SWIRL_UP) and `m_bIsIvalidGuard` (ignore guard);
reaction power 1 normal / 5 finish and grand; armour damage 2 / 3 / 10 (barrier: reaction 0, armour 1).

### 7. Grand Magic — how casts produce the situation command

Grand magic is a by-product of the **situation ("style") gauge** that also offers the Keyblade formchanges. Nothing is
decided at the cast or at the Shot: the statistics call 0x140E66FE0 (a counter) and `pawn vt+0x28C0` (friend
combo-finish roll) take no part [C].

#### 7.1 Points [I]
Every hit that deals damage ≥ 1 calls the player's notify 0x141442DE0 / 0x141443DE0 (notify-interface slots 31 / 32):
`controller.vt+0x1A80(points = info.m_FormPoint, kind = 1, cmd = info.m_CommandKind)` = 0x1413E9000:

```
AddStylePoints(points, kind, cmd):
    if current state == 138: return
    cmd = cmd or 27
    rate = critical mode ? CriticalModeStylePointAddRate * CriticalRateLv[formType]      # 1.0 * (0.5, 0.8, 0.55)
                         : MyStylePointAddRate * MyStylePointAddRateLv[formType]         # 1.0 * (1.0, 0.8, 0.55); formType 0 = base form
    mult = 1.0 (critical mode with CRITICAL_CHARGE 105 during MP charge: CriticalChargeStylePointAddRate 1.3)
    points *= rate
    if cmd is a grand-magic command (32, 36, 40, 44, 48, 110..124, 126, 231; 125 unless ability 103): points = 0
    if HasAbility(FORM_UP 71): mult += 0.2
    if pending (+0xB44) <= 0: chain (+0xB48) = 1.0 (2.0 for form command 0x9A)
    else:                     chain *= 0.5                                               # 2nd add before the next gauge tick counts half, 3rd a quarter ...
    add = points * mult * chain
    if add > 0: pending += add;  decStartTimer (+0xB4C) = MyStylePointDecStartTime (20 s);  decTimer (+0xB50) = 0
                e = TresCmdData[cmd].Attr;  if e < 8: elementPoints[e] += add;  lastCmd[e] = max(lastCmd[e], cmd)      # arrays at *(+0xB98), *(+0xBA8)
gauge tick 0x14141C4F0 (controller, each frame; skipped — pending discarded — while swimming and in some modes):
    gauge (+0xB40) = min(gauge + pending, MyStylePointMax = 100);  pending = 0
    decStartTimer -= dt;  when it has run out: every MyStylePointDecInterval (1 s): gauge -= MyStylePointDec (5)
    if gauge <= 0: gauge = 0; elementPoints[*] = 0; lastCmd[*] = 0
```

Points per hit, base form, before the chain factor = the row's form points (table 6.2): Fire 15 (finish 18), Blizzard /
Water 12 (14; barrier 8), Thunder / Aero 6 (7) per enemy hit, physical attacks their own rows. Element index = `Attr` of
the command row: 1 fire, 2 blizzard, 3 thunder, 4 water, 5 aero (7 cure: Cure deals no damage, so it adds nothing;
Cure Converter adds through the same function with cmd 49..51 → index 7).
Note the chain factor: a Thundaga that hits five enemies in one frame adds 6·(1 + ½ + ¼ + ⅛ + 1/16) = 11.6, not 30 [D].

#### 7.2 The offer [C; constants I]
0x1413F0ED0 (controller tick): when `gauge >= MyStylePointMax` (0x1413F14AF) and Sora is not swimming:
`rolled = vt+0x1A30(alt = pawn.IsMpCharge(), forced = 0)` = 0x1413ED500; if nothing was rolled (and no other offer is
pending) the Keyblade formchange command is offered instead; then **gauge = 0 and both arrays are cleared**
(0x1413F17EA..0x1413F1852).

```
Roll(alt, forced):
    M = alt ? 20 : 10;  if HasAbility(GRAND_MAGIC 180): M = int(M * 1.3)          # 13 / 26
    if !forced and Count(MAGIC_ROULETTE 186) > 0: Roll(alt, forced = 1)           # pre-roll: every element counts one extra -GAN ability; only its first result is
                                                                                  # offered, remembered in +0xB7C and excluded from the rolls below
    offers = []
    for e in (aero, water, thunder, blizzard, fire):                              # this order
        p = int(elementPoints[e]);  g = Count(-GAN ability of e: 185, 184, 183, 182, 181) + (forced ? 1 : 0)
        if g > 0: p += 15 * g;  lastCmd[e] = the S_x-GAN id
        if rand() % 1000 < p * M and characterAllowed(e):                         # by pawn vt+0xD70: aero {1,3}, water {1,3,7}, thunder any, blizzard / fire != 2; id 1 passes all
            cmd = lastCmd[e] == tier-1 id ? S_x-RA : lastCmd[e] == tier-2 id ? S_x-GA : S_x-GAN
            insert cmd at a random position of offers
    Curagan (125): needs HasAbility(CURAGAN 177) and !vt+0x27F0(); v = 25 (50 if vt+0x2538()) + 10 per KO party member; if rand() % 1000 < v * M: insert
    offer the first min(len, 3 − (pre-roll made ? 1 : 0)) entries:  AddSituationCommand(cmd, time = −1, 0)      # vt+0x1578
    result = any offered (the pre-roll's own offer does not count)
    # (CC, 2026-10-15, read in the disassembly; the decompiled tail is garbled) 0x1413EDF4E..0x1413EDFF9:
    if !result: pawn+0x15B4 (vt+0xD80) == 8: offer 0x7E S_DARKFIRAGA; == 0x11: 0xE7 S_CROSSSHOOT; result = true (Sora: 0)
    if int(points[0]) > 0 (physical): if (4 − min(n, 3)) * 250 > rand() % 1000: result = false   # n = successes of the pass, Curagan included
    if Count(CRITICAL_CONVERTER 106) > 0: result = false                                        # 0x1413EDFED
    return result                       # false: AwardAtMax also offers the formchange
```
(CC) Also read: the pre-roll skips an element entirely when its top command was the pre-roll's offer; "lastCmd[e] = S_x-GAN"
is a store (after the pre-roll the main pass offers the top tier); the first success is appended without a rand, later ones go
to `rand() % n`; Curagan in the forced pre-roll only at HP <= 50 % (in a pinch, vt+0x2538 = IsPinch, the list is cleared to
Curagan alone, no roll); vt+0x2540 on the party members = "is dead"; vt+0x27F0 = pawn+0xE60 bit 20 (a restriction bit).
Port: `crates/kh-core/src/magic/grand.rs` `roll_full`.

Reading: with M = 10 the chance per element is `elementPoints %` — an element that supplied all 100 points is certain,
one that supplied 40 has 40 % (52 % with Grand Magic; doubled while in MP charge). Several elements can be offered at
once (max 3). The tier offered is **one above the highest tier of that element cast since the gauge was last empty**:

| element | highest cast tier 1 | tier 2 | tier 3 (or -GAN ability equipped) |
|---|---|---|---|
| Fire | 110 S_FIRA | 111 S_FIRAGA | 112 S_FIRAGAN |
| Blizzard | 113 | 114 | 115 |
| Thunder | 116 | 117 | 118 |
| Water | 119 | 120 | 121 |
| Aero | 122 | 123 | 124 |

#### 7.3 Lifetime and use [I]
`AddSituationCommand` 0x14140DA40: duration = `TresCmdData[cmd].Param0` (20 s for rows 110..126) × (1 + 0.2 ×
Count(MAGIC_TIME 72)), capped at 99.9 s; the HUD owns the countdown (0x1415755A0). Using it (Part A 2.1: TryMagic with
bGrand, flag 0x400: Shoot / Chant clip, invincible until the Shot, no MP) clears the gauge, the decay timers and both
arrays again (0x14141B470). A grand cast itself adds no points (7.1) and its rows have form points 0.

### Open questions
1. Engine tick order: does the Cure actor heal in the frame of the Shot notify or one frame later.
2. Attachment rules of the Water barrier are deduced (zero static, KeepRelative); not single-stepped.
3. `ATresRailSlideActor` riding (`TresPlayerState_RailSlide`), the spline closest-distance function (`vt+0xA88`) and what
   happens when the node under Sora expires.
4. Exact acceptance filters of the Blizzard floor trace (actor category byte +0x3B0, component object types) — the
   pseudo-code of 4.1 gives the structure, the filters are engine collision classes.
5. Enemy side: which enemies accept SWIRL_UP, per-enemy `m_SwirlUp*` overrides, the DISCHARGE status object.
6. `pawn vt+0x21A8(-90)` in the swirl start, `vt+0x27F0` / `vt+0x2538` in the Curagan branch, the gauge-tick block
   conditions (`ctrl vt+0x1B78 / 0x1BD0 / 0x1BF0 / 0x1B08`, GameInstance+0x95B0 bit3).
7. Launch gate 0x14142B070: the list of refusing states is in NOTES (decompiled switch), not individually named.

### Runtime probe requests
| object | offset | type | settles |
|---|---|---|---|
| player controller | +0xB40, +0xB44, +0xB48, +0xB4C | f32 ×4 | gauge, pending, chain factor, decay timer per hit (7.1) |
| player controller | *(+0xB98)[0..7], *(+0xBA8)[0..7] | f32 ×8, i32 ×8 | element points and last command when the gauge fills |
| live `TresProj_MagAero*Nml_C` | +0x888, +0x890; MyAtkColl prim[0]+0x9D2 | i32, i32, u8 | swirl limit, counter, kind switch to BLOW |
| live `TresProj_MagWaterNml_C` | root world location, AttachParent | vec3, ptr | barrier follows Sora (3.1) |
| Sora pawn | +0x630 | u32 | bit2 during a Water cast (Shot → End) |
| live `TresRailSlideActor` spawned by Blizzard | location, +0x3F8, life span | vec3, vec3, f32 | node on the floor, tangent, 3 s life |
| swirled enemy: its Damage state | +0x78, +0x1C0, +0x1CC, +0x1EC, +0x1F0 | i32, vec3, f32, f32, u32 | sub-step, centre, vz0, hold timer, flags |
| Sora HP vs. frame of the Shot notify of a Cure | – | int | heal delay |

### What the host must provide
- Per spell actor: swept overlap of a scaled cylinder / sphere / capsule against enemies with a per-actor "already hit"
  set; "is this hit level geometry" (Fire explosion rule).
- Targets: a target point at the Shot (lock-on / auto-target, else 500 cm ahead).
- Damage application from a row with: damage kind (SMALL, PARABOLA, BEAT, TOSS, SWIRL_UP, BLOW), effect time,
  attribute, status, reaction power; and the **SWIRL_UP reaction of 2.2** for its own enemies (centre = column position).
- Sora invincibility flag honoured by its damage routing (3.2), hits still consumed.
- Floor trace and three sweeps for the Blizzard rail (4.4); optional rail objects with expiry.
- Jump request "super jump at actor yaw" for the Aero pad (2.3), triggered by a 100 cm disc that lives 6 s.
- Party pawn list by team with positions, heal(amount) (Cure).
- A "damage dealt" callback per hit with (row form points, command id) to feed 7.1; random numbers `rand() % 1000`;
  a situation-command slot with a countdown (7.3).

---

# Part J — Second pass: command menu, shortcuts, situation slots, pouch (extends E)

_Source of this part: `build/sora_tmp/magic/menu2/spec.md` (section numbers are local to the part)._

Second-pass addition to Part E of `docs/sora-magic-spec.md`. This part owns the LOGIC; widgets, frame labels and
layout belong to the UI strand (`docs/sora-ui-spec.md` did not exist when this was written; the movie is
`UI/Scaleform/01_hud/hud_command/HudCommand`, decoded in `build/sora_tmp/ui/movies/01_hud/hud_command/HudCommand/`).

Tags: [I] instruction level (address given), [C] decompiled C only, [A] asset, [D] derived, [U] stock UE,
[?] not determined statically. Units: s, int. All addresses are in the game exe.

Vocabulary: "command kind" = `ETresCommandKind` = row id of `Load/Common/TresCmdData` (254 rows, row 0x48 bytes:
Type +0x18, Cate +0x19, Mp +0x1F, Fp +0x20, Param0..3 +0x24..+0x30, LocKey +0x38). Type 3 = COMMAND, 4 = REACTION
(situation commands). Cate 8 ATTACK, 11 MAGIC, 12 ITEM, 13 ACCESS, 14 STYLE (formchange), 15 FINISH, 16 EXTRA,
17 ATTRACTION, 18 FRIEND, 19 DMAGIC (Link), 25 OTHER.

---

### 0. One-page model

- The menu is the native object `TresUIP_HudCommand` (the "HUD"). It reads the UI pad (not the controller's input
  path), keeps the cursor, the sub-list, the three shortcut pages and the situation-command list, and calls the
  player controller: `CanExecuteCommand(kind)` (controller slot 682, vt+0x1550, 0x141401F80) then
  `ExecuteCommand(kind, UICommandInfo*)` (slot 680, vt+0x1540, 0x14140EAA0). The command runs **on the frame the
  HUD sees the press**; nothing is buffered and a refused command is dropped with an error sound.
- Main list: 4 entries, index 0 Attack (27), 1 Magic, 2 Items, 3 Link [A `MainCommandData` row NORMAL; I indices].
  Magic sub-list = the 6 "magic slots" of the save data (one per element, each holding the highest learned tier,
  in the order the elements were learned). Items sub-list = the distinct usable items of the pouch with counts.
  Link sub-list = 5 link slots.
- Shortcuts: 3 pages x 4 slots, stored in the save data as 12 ints (all 0 in a new game; only the Customize menu
  writes them). Hold L1: face buttons use the slot (Circle 0, Triangle 1, Square 2, Cross 3), D-pad down / up
  changes the page. A shortcut that needs a target always targets Sora.
- Situation commands: a list in the HUD (not in the controller): at most 6 counted entries, each with its own
  countdown; Triangle uses the selected one, L2 selects the next. Lifetime of a grand-magic entry = `Param0`
  (20 s) x (1 + 0.2 x copies of ability MAGIC_TIME), capped at 99.9 s. Added by controller slot 687
  `AddSituationCommand` (0x14140DA40); grand magic comes from the roll function 0x1413ED500 (slot 838).
- Time: only with the config option "Command Selection: Slow" (off by default) and only while the sub-list
  (state 2) is open: object types PLAYER, ENEMY, GIMMICK, AVATAR, MAP, BG, CAMERA, FRIEND, VEHICLE, PHYSX run at
  0.3x; the stick, jump and attack inputs are blanked. The main list and the shortcut display never slow the game.
- Pouch: 8 entries `{i32 itemId; u8 flags}`; Sora owns 3 slots at the start, +1 per victory bonus
  ITEM_SLOT_UP1 (3 in the base game -> 6, 2 more in Re Mind -> 8). Items in the pouch are already out of the
  stock; auto-reload slots are refilled from the stock when the game leaves a pause and when the pawn is set up.

---

### 1. Objects, fields, input

#### 1.1 UI pad [I]
Embedded in `TresUIManager` at +0x408 (every part's +0x78 points at it, 0x141591B10). Per key 0xC bytes at
`pad+8+key*0xC`: `state` i32 (1 = pressed this frame, 2 = released, 3 = held), +8 repeat tick (u8), +9 consumed
(u8). Readers: 0x141583B80 `Pressed(key)` (state == 1), 0x141583A60 `Held(key)` (state == 3), 0x141583C30
`Repeat(key)` (state == 3 and repeat tick), 0x141583C00 (state == 2). Filled by 0x14158E390 from `FKey`s:

| id | key | id | key |
|---|---|---|---|
| 0 | FaceButton_Top (Triangle / Y) | 10 | DPad_Up |
| 1 | FaceButton_Bottom (Cross / A) | 11 | DPad_Down |
| 2 | FaceButton_Right (Circle / B) | 12 | DPad_Right |
| 3 | FaceButton_Left (Square / X) | 13 | DPad_Left |
| 4 | LeftShoulder (L1) | 14–17 | LeftStick Up / Down / Right / Left |
| 5 | RightShoulder (R1) | 18–21 | RightStick Up / Down / Right / Left |
| 6 | LeftTrigger (L2) | 22, 23 | Special_Right, Special_Left |
| 7 | RightTrigger (R2) | 24–26 | Escape, Insert, Delete |
| 8, 9 | L3, R3 | 27 | DECIDE = FKey 0x1487D7760 (the attack key: A by default, see `input/NOTES.md`) |
| | | 28 | CANCEL = FKey 0x1487D7778 (the jump key: B by default) |

The menu uses `Pressed` everywhere (no auto-repeat on the cursor) except L1, which is `Pressed || Held`.
Keys 0–3 are the raw face buttons: they do **not** follow the decide / cancel swap.

#### 1.2 HUD object `TresUIP_HudCommand` (fields used by the logic) [I unless noted]

| Offset | Type | Meaning |
|---|---|---|
| +0x68 | f32 | dt of this HUD tick |
| +0x1D0 | 4 x 0x20 | `m_mainCmdInfoAry[i]`: +0 `TArray<Cmd>` (ptr, num +8), +0x10 i32 **sub-cursor** (index among valid entries), +0x14 i32 number of valid entries, +0x18 u8 enabled (shown bright), +0x1B dirty, +0x1C u8 "has command" (index 0: attack allowed) |
| Cmd | 0xC | +0 u8 kind, +1 u8 valid (learned / owned), +2 u8 usable now, +3 u8 needs a target, +4 u8 MP short, +8 i32 count (items; -1 otherwise) |
| +0x250 | 3 x 0x34 | `m_shortcutCmdInfoAry[page]`: 4 Cmd + dirty byte +0x31 |
| +0x2F0 | TArray {u8 charId; i32 idx} | `m_friendInfo` (target list); +0x300 i32 target cursor; +0x304 i32 number of targets |
| +0x648 | ptr | `m_pCommandInfoCockpit` (`TresUICommandInfoBase`: +0x40 m_Actor, +0x48 m_Component, +0x50 u8 char id) |
| +0x650 | ptr | `m_HudCommandDataAsset` (only for guest characters / tutorial) |
| +0x6EC | i32 | sub-menu level: 1 list, 2 target list |
| +0x6F0 | i32 | **main cursor** 0..3 |
| +0x6F4 | i32 | selected situation entry (-1 = none or the reaction / finish slot) |
| +0x6F8 | i32 | counted situation entries |
| +0x6FC | u8 | "player chose the situation entry" (L2 used) |
| +0x708 | i32 | 0 normal, 1 / 2 special game modes (0x141631D90) |
| +0x710, +0x714[6] | i32 | shortcut mode index (0 = Sora) and current page per mode |
| +0x739 | u8 | everything restricted; +0x73A..+0x73D attack / magic / items / link restricted; +0x73E situation commands restricted; +0x740 shortcuts restricted |
| +0x750 | u8 | 1 = three main entries only (no Link); set by 0x141626290 when the player type is not 1 |
| +0x760 | TArray (0x58) | situation list; +0x768 num |
| +0x770, +0x7C8 | 0x58 each | reaction slot A (talk / open / ...) and finish slot B |
| +0x820 | TArray (0x58) | pending "action commands" pushed by `SetActionCommand` (0x14140C040) |
| +0x9B8, +0x9B9 | u8 | command being executed (freezes the situation timers) |
| +0x9BB..+0x9BF | u8 | cached abilities: 0x46 AUTO_FINISH, 0x1B FRIEND_AID, 0x19 MP_SAFETY, 0xAC OVER_TIME, 0x2E FOCUS_ASPIR (0x141616B10) |
| +0x9C0, +0x9C1, +0x9C2 | u8 | config: classic design, **slow selection**, **cursor lock** (save+0x2E9 bits 5, 6, 7; 0x141616C45..C75) |
| +0x9C4 | i32 | situation priority: 1 = older first, 0 = newer first (= !(save+0x2EA bit 0), 0x141616C7B) |
| +0x9C8 | u8 | control type B (save+0x2EA bit 1): D-pad selects situation commands |
| +0x9CA | u8 | (type B) cursor is on the situation list |
| +0x9E0 | i32 | MP (pawn vt+0x17D8); +0x9E4 bit 0 = MP >= max MP, bit 1 = MP charge active (0x141632500) |
| +0x9E8 | u8 | slow currently applied |
| +0x9EA | u8 | under water (magic kinds remapped, Water removed) |
| +0x9F8 + i*0x18 | | per main entry override command (attraction / ride modes; not Sora on foot) |
| +0xAA0 | state machine | current state +0xAC8; table of 13 x 0x48 built by 0x14161E540 |

#### 1.3 HUD states [I table at 0x14161E540]

| id | role | enter | update | exit |
|---|---|---|---|---|
| 0 | boot (0.01 s) | 0x14161A650 | 0x141622010 | |
| 1 | **MAIN** (main list, situation commands, shortcuts entry) | 0x14161A680 | 0x1416220D0 | |
| 2 | **SUB** (sub-list and target list) | 0x14161A8B0 | 0x1416236C0 | 0x14161C190 |
| 3 | **SHORTCUT** (L1 held) | 0x14161A830 | 0x141623360 | 0x14161C090 |
| 4 | command running (returns to 1 or, with +0xA98 = 2, to 2) | 0x14161A8A0 | 0x141623620 | 0x14161C120 |
| 5..12 | finish / attraction / ride / event variants | | | |

HUD tick: 0x14162FF80 (vtable slot 90): copies dt to +0x68, refreshes MP flags (0x141632500), the target list
(0x1416195A0), auto-finish (0x141618830), runs the state update (0x140507D50 on +0xAA0), then the per-frame
refreshes: mode (0x141631D90), **usable flags of every sub-entry** (0x141632660), situation timers
(0x141630AF0), sounds / animation requests.

The HUD only handles input when (0x141619870) the controller flag `ctrl+0x878 bit 1` is set, and (0x141620BC0)
the player's current state is not STYLECHANGE (31) nor DIE (59) and the queued state is not DIE.

---

### 2. Menu tree at runtime

#### 2.1 Main list [A, I]
`UI/Data/MainCommandData` row `NORMAL`: text keys `kh3_system.command.attack`, `.magic`, `.item`, `.link` in this
order ("Attack", "Magic", "Items", "Link"); 79 rows in total, the others are per-mode button legends
(shotlock, rides, formchange logos). Index 3 (Link) exists only when HUD+0x750 == 0; the cursor range is
`0 .. 3 - (HUD+0x750 != 0)` [I 0x1416220D0 @ `3 - (byte[+0x750] != 0)`].

| Index | Entry | Decide (key 27) | Bright when |
|---|---|---|---|
| 0 | Attack | `CanExecute(27)` -> `ExecuteCommand(27, null)` (ExecCommand case 0x1B -> interface vt+0x168 TryAttack) [C] | save+0x95B1 bit 2 and not restricted |
| 1 | Magic | opens the sub-list at the remembered sub-cursor | >= 1 valid magic slot and save+0x95B0 bit 2 |
| 2 | Items | opens the sub-list | >= 1 distinct item in the pouch (HUD+0x22B) |
| 3 | Link | opens the sub-list | >= 1 valid link slot and save+0x95B0 bit 2 |

Opening needs: `m_mainCmdInfoAry[i].count != 0`, entry bright, and not restricted (`+0x739`, `+0x73A+i`);
otherwise error sound (HUD SE 4). With "Slow" on, opening is refused while the current state is 31..39
(STYLECHANGE, WEAPONCHANGE, STYLEFINISH*) [I 0x141621410: `state - 0x1F < 9`].

#### 2.2 Magic sub-list [I 0x141616E70, 0x141625A80, 0x14159EE70; A]
Source: save data block `S = save+0x13CE0`, `magicSlot[6]` = 6 ints at `S+0x30` (reader 0x140E8BA80). Slot `i`
is listed when `magicSlot[i] != 0` and `pawn.IsCommandLearned(kind)` (pawn slot 800, vt+0x1900, 0x141402D00):
the element level (pawn vt+0x1958 with ids 0x18 Fire, 0x19 Blizzard, 0x1A Thunder, 0x1B Water, 0x1C Aero,
0x1D Cure) must be > tier index (Fire 29 needs level > 0, Fira 30 > 1, Firaga 31 > 2; same for 33–35, 37–39,
41–43, 45–47, 49–51).

Who fills the slots [C 0x14159EE70, called by the bonus handlers 0x140E9F040 / 0x140EC0220]: when a spell kind
is learned, the three-tier group containing it is looked up in `UI/Data/CustomizeDataAsset.MagicCommands`
(groups in asset order: Fire/Fira/Firaga, Blizzard.., Thunder.., Water.., Aero.., Cure/Cura/Curaga). If a slot
already holds a lower tier of the same group it is **replaced in place** (unless the slot's bit in `S+0x5C` is
set); otherwise the kind goes into the first slot that is 0. So:

- one entry per element, always the highest learned tier; lower tiers cannot be selected;
- order = the order in which the elements were first learned.

Story data [A `TresVBonusData`, Sora columns `MELEM_*`]: vbonus_082 (prologue Darkside) Fire then Water;
014 Cure; 016 Blizzard; 021 Thunder (Toy Box); 024 Aero (Corona); second tiers 034 Fire, 037 Water, 040 Cure,
043 Thunder, 047 Blizzard, 057 Aero; third tiers 054 Fire, 061 Thunder, 064 Water, 067 Blizzard, 068 Aero,
070 Cure. Result at the end of the game: **Firaga, Waterga, Curaga, Blizzaga, Thundaga, Aeroga** in this order
when Toy Box is cleared before Corona [D]; Thundaga and Aeroga swap if the player visits Corona first (player
choice). The grand-magic kinds (-gan, 32 / 36 / 40 / 44 / 48) never appear in the list.

Under water (HUD+0x9EA, 0x141625A80): Water slots (41–43) are dropped and kinds are remapped by 0x14161DAB0:
29–32 -> 53 WATERFIRE, 33–36 -> 54, 37–40 -> 55, 45–48 -> 56.

Per entry, every frame (0x141632660):
`usable = Available(kind) && CanUse(kind) && magicUnlocked && !restricted`, with

- `Available` (0x141620DE0) for Cate MAGIC and not a -gan kind: false when MP < 0 or **MP charge is active**
  (HUD+0x9E4 bit 1); Cure kinds (49–51, 125) additionally false when game flag 5 is set.
- `CanUse` (0x141617830, cached form): false when magic is restricted (HUD+0x73B), or when the ability
  MP_SAFETY (25) is equipped and the entry is "MP short"; Cure kinds only test the restriction.
- `MP short` (controller slot 686, 0x141403FC0) [C]: `Mp != 0xFF` and `cost(kind) > 0` and not (MP_LEAVE (76)
  and MP >= 1) and `MP < cost`. Shown with the "short" colour; without MP_SAFETY the cast is still allowed (it
  empties the gauge and starts the MP charge, part C).
- `magicUnlocked` = save+0x95B0 bit 2.

#### 2.3 Items sub-list [I 0x1416172B0, 0x14161DB30]
Built from the pouch of the active character (section 7): for slots 0..7 in order, each slot that holds an item
and is either unused (0x140E8F790) or auto-reload (0x140E8E4F0) contributes its command kind (item ->
kind by 0x141127250, inverse of `ItemOfCommand`); duplicates are merged (first occurrence keeps the position).
At most 8 distinct entries (the HUD array holds 9). Per entry: `count` = number of usable pouch slots holding the item (0x1405AA930 ->
0x140E8CCF0), `valid` = count >= 1, or the item sits in an auto-reload slot (0x141620AF0) — such an entry stays
listed with count 0 and is greyed. `needs target` flag for Potion 58, Hi-Potion 59, Ether 61, Hi-Ether 62,
Elixir 64, Panacea 68 (party items 60, 63, 65 and the Refocusers 66, 67 have none) [I list in 0x14161EFD0].

Usable = `valid && count != 0 && !itemRestricted (HUD+0x73C)`; in special modes (HUD+0x708 > 0) also game flag 6
clear and a non-empty usable pouch. The live check at confirm is `CanUseItem` (interface slot 227, part E I2).
The list is rebuilt whenever a pouch count changes (0x141619940 compares every frame).

#### 2.4 Link sub-list
`linkSlot[5]` = 5 ints at `S+0x48` (0x140E8B9F0), valid when learned (pawn vt+0x18D8 item ids 0x906..0x90A for
kinds 70–74). `Available`: MP must be full (HUD+0x9E4 bit 0) and game flag 0xB clear. Out of scope otherwise.

#### 2.5 Restrictions (greying of whole entries) [C 0x141449CD0, getters I]
Every pawn tick the player pawn pushes five booleans to the HUD (cockpit wrappers 0x1415848E0, 0x141584970,
0x141584910, 0x141584940, 0x141584890 -> 0x141625370 codes 2..6 set / 10..14 clear):

| HUD flag | Pawn getter | True when |
|---|---|---|
| +0x73A attack | vt+0x1EE8 0x1414365D0 | pawn+0x1130 bit 3, or pawn+0xE60 & 0x8001, or vt+0x1FE8(0) |
| +0x73B magic | vt+0x1EF8 0x141436820 | pawn+0x1130 bit 4, or pawn+0xE60 & 0x8002, or vt+0x1FE8(1) |
| +0x73C items | vt+0x1F08 0x141436730 | pawn+0x1130 bit 5, or pawn+0xE60 bit 2, or vt+0x1FE8(2), or (game flag 6 and vt+0x1740()) |
| +0x73D link | vt+0x1F28 0x1414367C0 | pawn+0x1130 bit 6, or pawn+0xE60 & 0x8010, or vt+0x1FE8(3), or link cooldown pawn+0x15C0 >= 0 (or game flag 0xB) |
| +0x73E situation | vt+0x1F18 0x1414368D0 | pawn+0x1130 bit 7, or pawn+0xE60 bit 3, or vt+0x1FE8(7) |

All five are forced true while the current or queued state is SWIM (102) and (not vt+0x1C30() or vt+0x1FB8()).
When all of attack / magic / items / link / situation are set, +0x739 (everything) is set as well.
(So the restriction bits of pawn+0xE60 are: 0 attack, 1 magic, 2 items, 3 situation commands, 4 link, 15 all,
16 "command selection slow".)

#### 2.6 Navigation

MAIN (0x1416220D0) [C, key ids I]:

| Input | Action |
|---|---|
| D-pad up / down (10 / 11) | cursor -1 / +1; past either end it wraps to the other end (type A). SE 0 |
| Decide (27) | cursor 0: attack command; cursor 1..3: open the sub-list (2.1) -> state 2, SE 6 |
| D-pad right (12) | cursor 1..3: same as Decide. Cursor 0: next Keyblade (interface vt+0x750) |
| D-pad left (13) | cursor 0: previous Keyblade (interface vt+0x758) |
| Triangle (0) | use the selected situation command (section 5.5) |
| L2 (6) | type A: select the next situation command (5.4) |
| L1 (4) pressed or held | if `CanShortcut` (3.4): state 3 and the face buttons are tested on the same frame |

Type B (HUD+0x9C8): D-pad up from the top entry moves the cursor onto the situation list (HUD+0x9CA = 1), up /
down then step through it (0x14162BAF0 with direction +1 / -1, no wrap) and down past its end returns to the
main list.

SUB (0x1416236C0), level 1 = list:

| Input | Action |
|---|---|
| D-pad up / down | sub-cursor -1 / +1 with wrap (0x141627CF0); only when the list has >= 2 entries. SE 0 |
| Decide (27) | see 2.7 |
| D-pad right (12) | only for an entry that needs a target, with >= 2 targets and FRIEND_AID: open the target list |
| Cancel (28) | close: sub-cursor = 0, **main cursor = 0**, state 1, SE 3 |
| D-pad left (13) | close: sub-cursor = 0, main cursor unchanged, state 1, SE 3 |
| Triangle (0), L2 (6) | situation command use / cycle, as in MAIN |

Level 2 = target list: up / down move HUD+0x300 with wrap; Cancel or D-pad left return to level 1; Decide
executes with the chosen target.

The sub-list also closes by itself (-> state 1, cursors reset) when the main entry becomes restricted, when a
finish / attraction command takes over, or when the party changes while the target list is open.

#### 2.7 Confirm in the sub-list [C 0x1416236C0]
```
entry = list[subCursor]                       // kind, needsTarget
if level == 1:
    if !Available(kind, entry): SE 4 (error); return
    if entry.needsTarget && targets >= 2 && HasAbility(FRIEND_AID 27):
        level = 2; show target list; SE 6; return
if !CanUse_cached(kind) && !ctrl.CanExecuteCommand(kind): SE 4; return
info = null
if entry.needsTarget:
    idx  = (level == 2) ? targetCursor : 0
    info = m_pCommandInfoCockpit; info.m_Actor = targetPawn[idx]; info.charId = friendInfo[idx].charId
if ctrl.ExecuteCommand(kind, info):           // section 4
    items: mark the item list dirty
    link:  close the menu, go to state 4
    else if !cursorLock: close the menu (sub-cursor = 0, main cursor = 0) -> state 1
    else: stay in state 2 (the list stays open on the same entry)
```
Cursor memory: the sub-cursor of each main entry is remembered only while that list was not closed; every close
path (0x141617CF0) writes sub-cursor = 0 and main cursor = 0, so with the default "Cursor Position: Reset" the
cursor never returns to the last spell. With "Lock" (HUD+0x9C2) the list simply stays open after a magic / item
command [I `byte[+0x9C2]` tests at the end of 0x1416236C0].

#### 2.8 Target list
Built every frame by 0x1416195A0: target 0 = the player (pawn vt+0xF10(0)), targets 1..4 = party members
(vt+0xF10(1..4)) that exist and are not excluded (their vt+0x1978() == 0); entries {charId, index}; count in
HUD+0x304. The list is offered only when `count >= 2` **and** the player has the ability FRIEND_AID (27, part of
Sora's initial equipped abilities in `TresChrInitData.m_PlayerSora`; absent for the tutorial Sora).
**With no party members the count is 1: no target list, the target is Sora** (`info.m_Actor` = own pawn,
index 0) [C 0x141629060 with `single = 1`].

---

### 3. Shortcuts

#### 3.1 Storage [I]
Save data block `S = save+0x13CE0` (`save` = the object returned by 0x140EA8620):

| Offset | Content | Access |
|---|---|---|
| S+0x00 | `i32 shortcut[3][4]` — command kind per page and slot | get 0x140E8CC40 (`page < 3 && slot < 4`), set 0x140E97F10 |
| S+0x30 | `i32 magicSlot[6]` | 0x140E8BA80, set 0x140E970B0 |
| S+0x48 | `i32 linkSlot[5]` | 0x140E8B9F0, set 0x140E97000 |
| S+0x5C | bit set per magic slot (blocks the automatic tier replacement) | 0x140E8EA00 |

The only caller of the setter is the camp-menu Customize screen (0x1415EED00) [I xref]. The save constructor
(0x140E84470 @0x140E84801) and the new-game initialisers (0x140E8CF90, 0x140E92D60) zero the whole block:
**a new game has 12 empty shortcut slots**; whatever a level-99 save holds is the player's choice (any learned
magic kind 29–51 at its exact tier, link 70–74, or item kind 58–68 from
`CustomizeDataAsset.ItemCommands`). Guest characters and the tutorial use fixed pages from a
`TresUIHudCommandDataAsset` (`ShortcutCommands.Page1..3`) instead [A, I 0x14161EFD0].

The current page of Sora's mode is kept in `TresUIManager+0x6B54` (i32, clamped to 0..2) so it survives the HUD.

#### 3.2 Page model [I 0x14162CAE0]
3 pages, index 0..2. In state 3: D-pad down (11) -> page+1, D-pad up (10) -> page-1, wrapping 2 -> 0 and
0 -> 2. Four slots per page.

#### 3.3 Slot state (recomputed by 0x14161EFD0)
For each slot kind k != 0: `valid` = (item kind: usable pouch count >= 1; else `pawn.IsCommandLearned(k)`);
`needsTarget` for 49, 50, 51, 58, 59, 61, 62, 64, 68; `usable` = `Available(k) && CanUse_live(k)` where the
live form calls the pawn interface: magic -> slot 51 (vt+0x198, 0x14142F330), link -> the same slot, items ->
`CanUseItem` (vt+0x718), attack -> vt+0x160; `count` = pouch count for items. Under water the displayed kind is
remapped as in 2.2.

#### 3.4 Press path [C 0x141623360, 0x14161E050, 0x14161D450]
`CanShortcut` = pawn interface slot 30 (vt+0xF0, 0x14142BDF0): save+0x95B1 bit 1, controller vt+0x1A70() != 0x8B,
and the current state is none of 28 DM_SIMBA, 40 SHOOTERMODE, 117 DM_WANDANYAN, 126 DM_RALPH, 127 DM_ARIEL,
128 DM_STITCH (74 OPEN_TREASURE_BOX and 102 SWIM have their own tests). Also blocked by HUD+0x740 / +0x9ED /
+0x9F1.

```
state 1: (Pressed(L1) || Held(L1)) && CanShortcut  -> state 3, then the same frame:
state 3 each frame:
    if !Held(L1) || !CanShortcut || blocked: -> state 1 (after the slot test below with slot = -1)
    slot = Pressed(Circle) ? 0 : Pressed(Triangle) ? 1 : Pressed(Square) ? 2 : Pressed(Cross) ? 3 : -1
    if slot < 0: if Pressed(DPad_Up) page-- ; else if Pressed(DPad_Down) page++ ; return
    cmd = pages[page][slot]
    if cmd.kind == 0: return
    if !(Available(cmd) && CanUse_live(cmd)): SE 4; return
    info = cmd.needsTarget ? target 0 (Sora) : null          // shortcuts never open the target list
    if !ctrl.CanExecuteCommand(kind): if !Available(...) SE 4; return
    ctrl.ExecuteCommand(kind, info)
    item kind: mark the page dirty (counts)
```
(Slot order on the pad: Circle, Triangle, Square, Cross = slots 0..3 [I the four `Pressed` tests].)
Link kinds leave the shortcut state into state 4. Holding a face button does nothing further (pressed edge only).

---

### 4. The confirm path and the pending command

`ExecuteCommand(kind, info)` = controller slot 680, 0x14140EAA0 [C]:
```
ctrl+0x87C         = kind                       // the pending command
ctrl.m_pUICommandInfo = info
if !ExecCommand():  return false                // slot 678, vt+0x1530, 0x14141B470; +0x87C is left as is
for each actor tagged "Enemy" in the level: enemy.iface(+0x448).vt+0x1E8(kind)   // AI notification
gameInstance: 0x140E9E2B0(kind, 1), 0x140EB2700(kind)                            // usage record
ctrl+0x87C = 0
return true
```
Writers of `ctrl+0x87C` / `m_pUICommandInfo`: only this function (the HUD is the only caller for menu, shortcut
and Triangle; other systems call it with their own kinds). `CanExecuteCommand(kind)` (slot 682, 0x141401F80)
[C]: first the gate slot 681 (0x1413EC9F0: not in aim mode `ctrl+0xA10 == 0`; the **command lock timer**
`ctrl+0x8C8` must be <= 0 (slot 677 0x14128C050; the timer is set by slot 676 vt+0x1520(t), 0x141415A50, e.g.
after a grand magic); pawn vt+0x2788() == 0, vt+0x29C0(); current state not one of 31 STYLECHANGE, 33..39
STYLEFINISH*, 59 DIE, 74 OPEN_TREASURE_BOX, 104 TALK, 105 SAVE_MENU, 106 SHOP_MENU, 108 CHECK, 109 RESCUE,
112 FR_MICKEYCHAIN, 150 ONE_ACTION [I switch on `state - 0x1F`]; rows of Cate EXTRA (16) skip all that and
need state 33 or 38), then per kind: 27 ->
interface vt+0x160; magic 29..31, 33..35, 37..39, 45..47, 49..51, 53..56, 125 -> interface vt+0x1A0 (slot 52
`CanMagic`); Water 41..43, the -gan kinds and grand magic 110..124 -> not vt+0x1C30() and `CanMagic`; link
70..75 -> vt+0x330; any Cate ITEM kind -> `CanUseItem(kind, 1)`.

Timing: the HUD state update runs inside the UI tick; the press edge lives one frame in the pad; both calls
happen in that update, and `ExecCommand` calls `TryMagic` / `TryUseItem` / `TryAttack` directly, which push the
state. **The command starts on the press frame** (order of the UI tick relative to the controller's own input
update within the frame: [?]). There is no repeat: holding Decide does nothing after the first frame; a second
press while the pawn refuses (`CanExecute` false) gives SE 4 and is lost. With cursor lock the list stays open,
so repeated presses re-issue the same spell; chaining is then decided by `CanMagic` (part A 5).

---

### 5. Situation commands (the Triangle slot)

#### 5.1 Container
The list lives in the HUD: `TArray` at HUD+0x760 of 0x58-byte entries, plus two single slots: A (HUD+0x770,
reaction commands: talk, open, check, ...; classes 2, 0xB, 0xC) and B (HUD+0x7C8, finish / extra commands,
classes 4 and 6). Entry [I offsets from 0x1416158F0 / 0x141630AF0]:

| Off | Type | Meaning |
|---|---|---|
| +0x00 | i32 | 1 = command kind entry, 2 = ex-kind entry |
| +0x04 | i32 | class (0x141619E60): 1 magic, 3 formchange, 4 finish, 5 attraction, 7 team attack, 8 shotlock, 10 link, 2 / 0xB / 0xC reaction ... |
| +0x08 | i32 | command kind |
| +0x10 | u8 | ex kind |
| +0x14 | f32 | total time (-1 = untimed) |
| +0x18 | f32 | remaining time |
| +0x1C | f32 | start delay, initial 0.5 |
| +0x20 | f32 | appear time, initial 0.2 (0 for classes 2, 0xB and ex-kind entries) |
| +0x24 | f32 | grace time |
| +0x34 | i32 | serial (HUD+0x9CC, incremented per add) |
| +0x38 | u8 | selectable |
| +0x3A | u8 | usable now (`Available(kind)`, refreshed every frame) |
| +0x50 | ptr | UICommandInfo passed back at execution |

#### 5.2 Adding — controller slot 687 `AddSituationCommand(kind, time = -1, info)` 0x14140DA40 [C; constants I]
```
t = 0
if row.Type == 4 && row.Param0 > 0: t = Param0                       // seconds  [I +0x18 == 4, +0x24]
switch kind:
  30,31,32, 34,35,36, 38,39,40, 42,43,44, 46,47,48, 110..126, 231:   n = Ability(MAGIC_TIME 72)
  186..191 (attraction):                                             n = Ability(ATTRACTION_TIME 47)
  128..154, 221..224 (formchange): if ctrl+0xB60 != 0 return;        n = Ability(FORM_TIME 49)
  (the three groups above)        if n > 0: t *= 1 + 0.2 * n
  193..210, 237..240, 242 (team attacks): if time != -1 { t = time; time = -1 }
        partner ability COMBI_TIME 197: t *= 1 + 0.5 * n
  171..174: remove the mutually exclusive ones first
if time != -1: t = time
t = min(t, 99.9)
cockpit.AddCommand(kind, t, info)        // 0x1415755A0 -> HUD 0x1416158F0(hud, 1, kind, 0, t, info)
```
`Param0` of the Type 4 rows [A]: 20 for the grand-magic rows 110–126, 30 for the formchange rows
(`change_dw`, `change_hm`, ...), 0 for reaction rows. `Param3` is **not** the situation lifetime: it is read by
controller slot 872 (0x141400E60, [I 0x141400F1E `cvtdq2ps [row+0x30]`]) as the time of a "set action command"
(0x14140C040: finish / link rows 69–74 Param3 20, `change_dw` 20, `change_hm` 30) and by the formchange start
(0x14141A9D0: form duration = Param3, 30 when 0). `Param1` of the formchange rows (1) was not seen read in this
strand [?].

Removal: controller slot 688 `RemoveSituationCommand(kind)` 0x14140C790 -> 0x14157A590 -> HUD 0x14161B910;
kind 0xFE removes all.

#### 5.3 HUD insert 0x1416158F0 [C; constants I]
```
if an entry with this kind exists:                        // refresh, never a duplicate
    if OVER_TIME (ability 0xAC) && old.total > 0 && old.total > t: remaining = old.total
    else: total = remaining = t (<= 0 -> -1);  appear = 0.2;  return
class = Classify(kind)
reaction classes 2 / 0xB / 0xC -> slot A (replaces);  finish classes 4 / 6 -> slot B
else (counted entry):
    if count (HUD+0x6F8) >= 6:                            // "5 < count"
        remove the first entry (list order, among the first 6) whose total >= 0.1   // the oldest timed one
    (class 9: refused when count > 6)
    count++
    entry = {type 1, class, kind, total = t, remaining = t, startDelay = 0.5,
             appear = 0.2 (0 for class 2 / 0xB), serial = HUD+0x9CC++, selectable = 1,
             usable = Available(kind), info}
    append at the end of the list
    if the player has not chosen an entry (HUD+0x6FC == 0): selected = Select()          // 5.4
```
Capacity: **6 counted entries**; the 7th pushes out the oldest timed entry.

#### 5.4 Selection, L2 [C]
`Select()` (0x141627B70): slot A if occupied (and its appear time is over) -> index -1; else slot B -> -1; else
among the list entries that are selectable and whose appear time is over: priority mode 1 "Prioritize Older"
(default) = smallest serial, mode 0 "Prioritize Newer" = largest serial. A remembered player choice
(HUD+0x954) wins while it is valid.

L2 (type A) -> 0x14162BAF0(hud, +1, wrap = true): index + 1, skipping non-selectable entries; past the last
entry -> the reaction / finish slot when present (-1), else the first valid entry. Sets HUD+0x6FC (player
choice) and remembers {kind, index}; SE 0 when the index changed. Refused while situation commands are
restricted (HUD+0x73E).

#### 5.5 Use (Triangle) [C 0x1416220D0 / 0x1416236C0 -> 0x141618A50, 0x14161CF40]
```
if HUD+0x739 || +0x73E || +0x73F:                  // restricted: only a reaction command of Cate ACCESS (13)
    if !(slot kind is ACCESS && selected < 0): SE 4; return   // in slot A / B may still be used
(class, kind, exKind, info) = selected entry      // -1 -> slot A, else slot B; else list[selected]
if kind == 0: return
if !ctrl.CanExecuteCommand(kind): return           // silent
if !ctrl.ExecuteCommand(kind, info): return        // the entry stays
remove the entry (0x14161AB00, "used")  -> count--, selection re-evaluated (5.4)
classes 3, 5, 7, 0xB (formchange, attraction, team) -> state 4 (menu hidden until the action ends)
```
For grand magic (class 1; kinds 110–125): `ExecCommand` calls `TryMagic(..., grand = 1)` (part E I1), no MP
cost; on success the controller clears its grand-magic bookkeeping (ctrl+0xB40, +0xB4C, arrays +0xB98 / +0xBA8).

#### 5.6 Timers [I 0x141630AF0]
Per HUD tick, unless HUD+0x751 or a command is executing (HUD+0x9B8 / +0x9B9):
```
dtP = dt * GameState.ObjTypeTimeDilation[PLAYER]          // `mov dl,1` @0x141630B59, GameState vt+0x8B0
for each list entry:
    if appear > 0: appear -= dt                            // undilated
    if startDelay >= 0: startDelay -= dt; continue         // 0.5 s before the countdown starts
    if grace > 0: grace -= dtP
    if remaining >= 0:
        remaining -= dtP
        if remaining < 0: remaining = 0
            if grace <= 0: remove entry; pawn.iface(+0x448).vt+0x1D0(kind)     // expiry notification
```
So a grand-magic command lives 0.5 s + 20 s (x ability) of player time, and the countdown slows with the player
(0.4 while aiming a shotlock, 0.3 in the slow sub-menu).

HUD values (0x141630ED0): number shown = `min(999, int(remaining * 10))` (tenths of a second); gauge fraction =
`remaining / total`; a different style below 100 (10.0 s).

#### 5.7 Grand magic entry (production rule: strand `spell2`; not yet written when this was read)
Adding function: controller slot 838 (vt+0x1A30) 0x1413ED500 `RollGrandMagic(bFinisher, bForced)`, called from
controller slot 830 (0x1413F0ED0 @0x1413F14FE) and recursively with `bForced = 1` for MAGIC_ROULETTE (ability
186). Shape [C; constants I where given]: `factor = 10` (20 when `bFinisher`), x 1.3 with ability GRAND_MAGIC
(180) [I 0x1413ED61A]; for each element e with points `P[e]` (f32 array at `[ctrl+0xB98]`: +4 Fire, +8 Blizzard,
+0xC Thunder, +0x10 Water, +0x14 Aero) plus 15 per copy of the element's "-gan" ability (181..185):
`rand() % 1000 < P[e] * factor` -> candidate kind by the last cast tier of that element (`[ctrl+0xBA8]`: Fire 29
-> 110 Fira, 30 -> 111 Firaga, else 112 Firagan; ... Aero 122 / 123 / 124); Curagan 125 by HP / party rules.
Up to `3 - (ctrl+0xB7C != 0)` candidates are added with `AddSituationCommand(kind, -1.0, null)`, i.e. the
lifetime of 5.2.

---

### 6. Game time while a menu is open

#### 6.1 Rule [I]
`SetSlow(on)` = 0x141615760: acts only when `on == 0` or HUD+0x9C1 (config "Command Selection: Slow"), and only
on a change of HUD+0x9E8. It is called with 1 from the **enter of state 2** (0x14161A8B0) and with 0 from the
exit of state 2 (0x14161C190) and from the HUD shutdown. States 1 (main list) and 3 (shortcuts) never call it.

`on` -> controller 0x14140E610(ctrl, on, rate):
```
if on:  GameState.SetObjTypeTimeDilation(channel 0 CH_COMMON, mask 0x4F9E, rate, blend 0)   // 0x1405DA460 -> GameState vt+0x8A8 0x144119D90
        ctrl.vt+0x1968()                              // slot 813 0x1413FAD90: leave shotlock aim
else:   GameState.SetObjTypeTimeDilation(0, 0x7FFFFFFF, 1.0, 0)                              // 0x1405DA3E0
ctrl+0x878 bit 6 = on
pawn.vt+0x2768(on)                                    // 0x141112970: pawn+0xE60 bit 16 = on
```
- `rate` = `TresUISlowModeDataAsset.SlowRate` = **0.3** (class default set in the constructor 0x141553100
  @0x141553147 `0x3E99999A`; the cooked asset `UI/Data/SlowModeDataAsset` does not override it) [I, A].
  The same asset gives the depth-of-field block applied with it (0x1415AAD60): focal distance 300, focal region
  300, near transition 1000, far transition 4000, transition time 0.05 s [A].
- The "id" 0x4F9E is a **bit mask of `ESQEX_ObjectType`**: bits 1 PLAYER, 2 ENEMY, 3 GIMMICK, 4 AVATAR, 7 MAP,
  8 BG, 9 CAMERA, 10 FRIEND, 11 VEHICLE, 14 PHYSX. Not slowed: 0 NONE, 5 PROJECTILE (projectiles take the
  owner's type unless `m_bDisableOwnerObjTypeCalcTimeDilation`), 6 PRIZE, 12 EVENT, 13 UI, 15 DEBUG.
  (The shotlock mask 0x4D9E on channel 1 CH_PLAYER is the same without CAMERA.) The global time dilation is not
  touched: UI ticks and real-time timers keep running.
- While the flag is set the controller's input update (0x1413F1AB0) zeroes the stick vector and magnitude
  (ctrl+0x848.., +0x854), clears the jump press / hold flags and skips the attack / guard dispatch
  (`ctrl+0x878 & 0x40`) [C]: the config text "you won't be able to attack or evade while selecting" [A].

#### 6.2 Config options [I; names A `kh3_camp.config.*`]
Stored in the save data at +0x2E9 / +0x2EA; defaults from the save constructor 0x140E84470 @0x140E84545: word
+0x2E8 = 0x1FC3 (so +0x2E9 = 0x1F: bits 5..7 clear), +0x2EA = (old & 0xE4) | 4 (bits 0, 1 clear).

| Option (text key) | Bit | Default | Effect |
|---|---|---|---|
| Command Menus (`config_0110`): By World / Classic | +0x2E9 bit 5 | 0 = By World | design only |
| Command Selection (`config_0120`): Active / Slow | +0x2E9 bit 6 | 0 = **Active** | 6.1 |
| Cursor Position (`config_0130`): Reset / Lock | +0x2E9 bit 7 | 0 = **Reset** | 2.7 |
| Situation Commands (`config_0140`): Prioritize Older / Newer | +0x2EA bit 0 | 0 = **Older** | 5.4 |
| Control Type (`config_0150`): Type A / Type B | +0x2EA bit 1 | 0 = **Type A** (L2) | 2.6 |

(Bit-to-option assignment [D]: the HUD reads the five bits in this order and each behaviour matches the help
text; the camp-menu writer was not read.) These are save-data options, not `UTresGameSettings`.

---

### 7. Item pouch

#### 7.1 Layout [I]
Per-character block `B = save + 0x9640 + charIndex * 0x9C0` (0x140E8AE20 / 0x140E8BF90; the pawn reaches it as
`[[pawn+0x4D0]+0x130]`). Equipment slot arrays of `{i32 id; u8 flags}` (8 bytes): weapons B+0x80 (3), armour
B+0x98 (8), accessories B+0xD8 (8), **items B+0x118 (8)**. Slot counts: B+3 armour, B+4 accessories,
**B+5 items** (initial values); bonus counters `B+0x960+kind` (u8, kinds 1..31): B+0x970 armour (kind 0x10),
B+0x971 accessories (0x11), **B+0x972 items (kind 0x12 = ITEM_SLOT_UP1)**.

Flags: bit 0 slot exists, bit 1 fixed (content cannot be changed; set for preset loadouts), bit 2 auto-reload,
bit 3 used (the item is spent but the slot remembers it).

#### 7.2 Slot count [I]
`slots = min(8, B[5] + B[0x972])`: 0x140E8DAA0 sets bit 0 for `i < B[5]`, 0x140E98BE0(B, 0x12) then sets bit 0
for `i < B[5] + B[0x972]`. A victory bonus goes through 0x140574450 -> 0x140E86BC0 -> 0x140E86B30
(`B[0x960+kind]++`, saturating at 255) -> 0x140E98BE0. No other code that sets bit 0 of the item slots was found
(displacement scan of +0x11C over the save code: 0x140E8DAA0 and, by pointer walk, 0x140E98BE0), and no equipment
field feeds B[5] or B[0x972]: **equipment does not add item slots** [C].

- Initial: `TresChrInitEquip.m_InitItemSlot` = **3** for Sora (struct default written by the `TresChrInitData`
  constructor 0x140571C00 @0x140571C41; `Load/Common/TresChrInitData.m_PlayerSora` does not override it;
  `m_Items` is empty, so the pouch starts empty) [I, A]. (Donald 2, Flynn / Mike 4, Marshmallow 0.)
- Bonuses [A `TresVBonusData`, Sora columns]: vbonus_038 (Monstropolis), 062 (Caribbean), 069 (Keyblade
  Graveyard), dlc_001, dlc_015.
- **End of the base game: 6 slots; with Re Mind: 8** [D] (level has no influence).

#### 7.3 Counting and use [I]
- `Count(item)` = slots with bit 0, id == item, bit 3 clear (0x140E8CCF0; pawn wrapper 0x1405AA910).
- Use (0x140E98B90, called from `UseItem` 0x1405BCBE0 at the `TresTiming` notify, part E I4): first usable slot
  holding the item: if neither bit 1 nor bit 2 is set the id is zeroed (the slot becomes empty), otherwise bit 3
  is set.
- `FUN_140E86A80(save+0x12C2670, id, +1)` (0x140E9FC50) is a **usage counter** (int per item: battle items
  index < 16, category 2 index + 0x10, category 6 index + 0x18; never the stock) [I].

#### 7.4 Stock [I]
Inventory `save+0x86B4`; entry of an item (0x140E99330) = `{u8 count; u8 flags (bit 1 = obtained)}`. Stock
count: pawn vt+0x18D8 (0x1405A9D40 -> 0x140E8AB50); consume: pawn vt+0x18D0 (0x1405BB6E0 -> 0x140E8A2E0:
`count = max(0, count - n)`). Maximum 99 per battle item (`TresItemData.m_MaxNum`) [A]. Putting an item into a
pouch slot (camp menu 0x1415ECC00 -> 0x140E94F90) and every refill take it out of the stock: **an item in the
pouch is no longer in the stock**.

#### 7.5 Auto-reload and refill [C; flag tests I]
- "Auto-Reload" is bit 2, toggled per item kind (camp menu 0x1415E8430 sets it on every slot holding that
  item; 0x140E94F60). A newly equipped item inherits the setting of other slots with the same item.
- Refill of one character = 0x1405B5B70: for slots 0..7: `if (flags & 0xD) == 0xD` (exists, auto-reload, used;
  0x140E8F320) `&& id != 0 && Stock(id) > 0`: clear bit 3 (0x140E913F0) and `Stock(id) -= 1`.
  `RefillAll` = pawn vtable slot 1293 (vt+0x2868, 0x141446500): the player, then the 6 party members.
- Triggers: (1) the game-instance tick 0x140EAE5A0: while the world is paused (`UWorld::IsPaused`-like
  0x1441B2920) a flag (instance+0x2D0 bit 1) is set; on the first unpaused frame with dt > 0 and
  `(instance+0x8D4 & 0x14) == 0` it calls `RefillAll` and clears the flag — i.e. **when a pause / camp menu is
  closed**; (2) the pawn set-up 0x1405B4A30 (spawn / map change) does the same loop for its own character.
  There is no refill at the moment a battle ends [C]; the meaning of the two blocking bits of instance+0x8D4 is
  [?] (probably battle / event).
- Without auto-reload a used slot is simply empty afterwards.

#### 7.6 Outside battle
There is no separate path: the same command (menu or shortcut) -> `CanUseItem` -> state USE_ITEM (part E). The
items restriction is only pawn+0xE60 bit 2 / pawn+0x1130 bit 5 / the map restriction category 2 (2.5).

---

### 8. Interface for the port

The core (engine-free) owns the command gates, the pouch, the situation list and the slot data; the host / UI
layer owns the pad, the cursor state machine and drawing. Function-level contract:

Core -> UI (queries, valid every frame):
```
main_entries() -> [MainEntry; 4]          { kind_or_group, visible, enabled, restricted }
magic_list()   -> [SubEntry; <=6]         { kind, usable, mp_short }             // 2.2, slot order
item_list()    -> [SubEntry; <=8]         { kind, item_id, count, usable, needs_target }   // 2.3
link_list()    -> [SubEntry; <=5]
targets()      -> [Target; 1..5]          { handle, char_id }                    // index 0 = Sora
has_friend_aid() -> bool
shortcuts(page) -> [ShortcutSlot; 4]      { kind, valid, usable, needs_target, mp_short, count }
situation_slots() -> { list: [SitEntry; <=6] {kind, class, total, remaining, usable, selectable, serial},
                       reaction: Option<SitEntry>, finish: Option<SitEntry>, selected: i32 }
restrictions() -> { all, attack, magic, items, link, situation, shortcut }
can_execute(kind) -> bool                 // CanExecuteCommand
can_shortcut()   -> bool
mp_state()       -> { mp, max_mp, charging }
```
UI -> core (events):
```
execute(kind, target: Option<handle>) -> bool          // ExecuteCommand; same frame as the press
situation_use() -> bool                                // Triangle on the selected entry
situation_cycle(dir: +1/-1, wrap: bool)
set_command_slow(on: bool)                             // only when the option is on and the sub-list is open
shortcut_page(delta)                                   // persists the page
```
Core internal (called by combat / magic code):
```
add_situation_command(kind, time: f32 /* -1 = table */, info)      // 5.2, 5.3
remove_situation_command(kind | ALL)
tick_situation(dt, player_time_dilation)                           // 5.6; emits expired(kind)
pouch() -> [PouchSlot; 8] { item_id, exists, fixed, auto_reload, used }
pouch_count(item_id) -> i32;  pouch_use(item_id) -> bool;  pouch_refill_all();  add_item_slot()
stock(item_id) -> u8;  stock_add(item_id, n) (cap 99)
learn_magic(kind)                                                  // 2.2 slot rule
```
Host must provide: pad edges (Decide, Cancel, D-pad x4, L1 held + edge, L2, the four raw face buttons), the
five config bits (6.2), an object-type time-dilation hook `(mask, rate)` (or at least "slow player + enemies +
camera to 0.3"), the party list (empty in the first host: target = Sora), sound events for HUD SE ids 0 (cursor),
3 (cancel), 4 (error), 6 (open / confirm) [ids only; the cue names belong to the audio strand], and the text
table for the keys in `assets.json`.

---

### Open questions
1. Order of the UI tick and the controller input update inside one frame (decides whether a press that the
   HUD turns into a command can also reach the controller's own attack / jump dispatch on that frame).
2. `HUD+0x9B8 / +0x9B9`: exact set / clear points — they freeze every situation timer while a command "is
   executing"; how long that lasts for a magic cast is not settled.
3. Blocking bits `gameInstance+0x8D4 & 0x14` of the unpause refill, and whether the camp menu can be opened in
   battle at all (if not, "refill when the menu closes" is effectively "after battle").
4. `HUD+0x68`: whether the HUD tick dt is real or game delta (the global dilation is 1 in both menu cases, so
   it only matters with engine slomo).
5. The camp-menu function 0x1415E1FA0 also writes magic slots (three call sites of 0x140E970B0): a reorder /
   tier screen not analysed.
6. `Param1` (= 1) and the unused `Param2` of the formchange rows.
7. The class-9 situation entries (limit 7) and classes 5 / 7 details (attraction, team attacks) — outside Sora
   solo.

### Runtime probe requests
| Object | Offset | Type | Settles |
|---|---|---|---|
| HudCommand (`TresUIManager` part) | +0x6F0, +0x6F4, +0x6F8, +0xAC8 | i32 x4 | cursor, selected situation entry, count, state id |
| HudCommand | +0x760 (ptr), +0x768 (num); entry +0x08, +0x14, +0x18, +0x1C, +0x34 | i32, f32 x3, i32 | list order, lifetimes (20 s grand magic), start delay, serials |
| HudCommand | +0x9B8, +0x9B9, +0x9E8 | u8 x3 | timer freeze windows, slow flag |
| save (0x140EA8620 result) | +0x13CE0 .. +0x13D3B | i32 x 23 | shortcut pages, magic slots, link slots of a real save |
| save | +0x2E9, +0x2EA | u8 x2 | option bits |
| character block (save+0x9640) | +5, +0x972, +0x118..+0x157 | u8, u8, 8 x {i32,u8} | slot count parts, pouch flags before / after use and after closing the menu |
| save+0x86B4 entry of 0x101 | +0 | u8 | stock change at refill |
| GameState | +0x420 (16 x f32) | f32 | object-type dilation while the slow sub-menu is open (expect 0.3 on indices 1,2,3,4,7,8,9,10,11,14) |
| controller | +0x87C, +0x878 | i32, u32 | pending command, flag bits 1 and 6 |

---

# Part K — Implementation order and test list

_Source of this part: `build/sora_tmp/magic/tests/spec.md` (section numbers are local to the part)._

Scripted tests against a training dummy, with the numbers the specification predicts. They are predictions
from static reading: a test that fails against the running game is a finding about the specification, not
about the port. The stat numbers are those of `sheets/port/sora-loadout-99.json` (stats agent): level-99 row Strength 62 /
Magic 64 / Defense 59, Kingdom Key +4 / +3, max focus 100, max MP 169 (Balance).

### T0. Reference loadout and dummy

| Quantity | Value | From |
|---|---|---|
| level | 99 | — |
| MagicPower `MAG` | 64 (level row) + 3 (Kingdom Key `m_MagicPlus`, `TresEquipItemData` I03001) = **67** | Part C 4.2 [A]; no accessories, no meal, no boost items |
| max MP `M` | **100** (bonuses cleared, to keep the arithmetic simple) — second run with the loadout value 169 (Balance, all story and Re Mind bonuses) | Part C 1; `sheets/port/sora-loadout-99.json` |
| max focus | 100 | Part D S1 |
| abilities | none equipped except where a test says so | — |
| difficulty | Standard (multiplier 1) | Part C 4.2 |
| dummy | defence 0, no damage min / max, all attribute resistances 0, rapid-fire resistance 0, not a boss, infinite HP, permissions lock + shotlock + move-flow, 800 cm in front, same height | host test object |

Damage per hit on this dummy: `dmg = ceil((MAG − 1e-6) · (power − 1e-6))` with `ceil(x) = −(cvt(−0.5 − 2x) >> 1)`
(Part C 4.2). `power` is the float32 of the attack row.

### T1. Order of implementation
1. MP and focus resources with their setters (clamp, charge start, HUD notifications) — tests T2.
2. Command execution and gates (`CanUseCommand`, `CanMagic`, `TryMagic`) — tests T3.
3. Cast state with clips and notifies, hover, facing — tests T4.
4. Generic projectile (motion, homing, root sweep, attack shapes, respawn chain) and the damage formula —
   tests T5 per spell.
5. Combo / finisher rules and the `_FIN` rows — tests T6.
6. Cure and items — tests T7, T8.
7. Aim mode, lock acquisition, focus drain; `KC_SHOT`, then `KC_RAGNAROK` — tests T9.
8. Airstep — test T10.
9. Command menu model, shortcuts, situation slot (grand magic) — tests T11.

### T2. MP rules
| # | Script | Expected |
|---|---|---|
| 2.1 | M = 100; cast Fire 10 times (wait for each to end) | MP 90, 80, … 0 after the shot of each cast; no charge after the 10th (landing exactly on 0) |
| 2.2 | continue 2.1: cast Fire once more | accepted (gate is "not in charge"); the bolt is spawned; at the shot MP stays 0 and the MP charge starts |
| 2.3 | during the charge: request Fire | refused (`CanUseCommand`) |
| 2.4 | measure the charge | 50.0 s game time, then MP = M |
| 2.5 | same with 1 × MP Haste / 1 × Hastera / 1 × Hastega / all three | 45.45 s / 41.67 s / 38.46 s / 31.25 s (`50 / (1 + 0.1a + 0.2b + 0.3c)`) |
| 2.6 | M = 100, MP 95: cast Firaga 5 times | 75, 55, 35, 15, then 0 + charge on the 5th shot (cost 20 > 15); the 5th bolt is still spawned |
| 2.7 | MP 100, ability MP_SAFETY, cast Firaga until refused | 5 casts (100 → 0), the 6th refused before the state starts; no charge (the 5th landed exactly on 0) — with MP 95: 4 casts, the 5th refused at MP 15 |
| 2.8 | 1 × MP_SAVE (ability 75), Fire / Firaga | cost `10 − floor(0.2·10 + 0.9) = 8`, `20 − floor(4.9) = 16` |
| 2.9 | MAGIC_COMBO_SAVE (56): Fire, Fire, Fire in one chain | cost 10 − combo count at each shot, minimum 1 (the count is pawn+0xE18 at the shot: 0, 1, 2 → 10, 9, 8) [expected from Part A 2.3 / C 2: verify the count used] |
| 2.10 | charging, MP prize small / large | timer −2 s / −6 s; not charging: MP +3 / +5 |
| 2.11 | Critical difficulty | M = 50 at base; charge 70 s |

### T3. Gates and queueing
| # | Script | Expected |
|---|---|---|
| 3.1 | request Fire while idle / running / in a jump (rise and fall) | accepted at once; the previous state is replaced |
| 3.2 | request Fire during a ground combo attack before its chain point | accepted, queued behind the attack; starts at the attack's enable-change |
| 3.3 | request Fire during a Fire cast before the shot (t < 0.3 s) | refused (BulletCount = 0) |
| 3.4 | request Fire at t = 0.35 s of a Fire cast | accepted; the second cast starts on the next tick (Fire-family follow-up), not at 0.733 s |
| 3.5 | request Blizzard at t = 0.35 s of a Fire cast | accepted; starts at `TresEnableChange` (0.733 s) |
| 3.6 | request jump during a cast | never accepted |
| 3.7 | dodge at t = 0.5 s / t = 0.75 s of a standing Fire | refused / accepted |

### T4. Cast timing and movement (standing Fire, target at 800 cm)
| t (s) | Expected |
|---|---|
| 0 | state begins; velocity zeroed; clip `MagicFire`; facing snaps toward the target (alpha 1·dt·60) |
| 0.0696 | voice `sora_magic` index 20 |
| 0.300 | Shot: bolt spawned at the Keyblade tip (`ATK_01`), MP −10 |
| 0.733 | `m_bEnableChange` |
| 1.200 | clip ends → state finishes |

Air cast (requested at the apex of a jump): vertical velocity 0 and gravity off from the first tick to
0.300 s (`MagicAirFire`), then falling with gravity scale 0.1 until 0.733 s, then normal gravity. Moving
ground cast (running, stick held): clip `MagicFireMF`, Shot at 0.3667 s, XY speed = max walk speed along the
request direction for the whole state. Thunder standing: `MagicCast`, Shot 0.3333 s, enable-change 0.8667 s,
end 1.1333 s.

### T5. Spells: damage and behaviour (MAG 67, dummy of T0)
| Spell | Row (normal) | power | Damage per hit | Behaviour to assert |
|---|---|---|---|---|
| Fire | MAG_FIRE | 1.0 | 67 | bolt 2000 cm/s, homing 180°/s, reaches 800 cm in 0.40 s (+ the 0.3 s shot); no explosion actor (tier 1) |
| Fira | MAG_FIRA | 1.5 | 101 | explosion sphere r 150 for 0.25 s on a character hit |
| Firaga | MAG_FIGA | 2.2 | 148 | 0.067 s hold at spawn; explosion r 250; homing 360°/s |
| Blizzard | MAG_BLIZZARD | 0.8 | 54 | FREEZE applied (100 %, 5 s); ice rail nodes every 300 cm over ground |
| Blizzara | MAG_BLIZZARA | 1.2 | 81 | FREEZE 7 s; burn r 600 with the `_BURN` row (FREEZE 50 %) |
| Blizzaga | MAG_BLIZZAGA | 1.8 | 121 | FREEZE 10 s; burn r 800 |
| Thunder | MAG_THUNDER | 1.2 | 81 | one column per cast at the target (500 cm ahead with no target), r 100 → 400, life 0.6 s; every enemy inside is hit once; DISCHARGE 100 % 5 s |
| Thundara | MAG_THUNDARA | 1.6 | 108 | r 400 → 800, life 0.4 s; DISCHARGE 7 s |
| Thundaga | MAG_THUNDAGA | 2.4 | 161 | r 400 → 1200, life 0.5 s; DISCHARGE 10 s |
| Water | MAG_WATER (bolt) / MAG_WATER_BARRIER 1.0 | 0.9 | 61 (barrier contact 67) | barrier sphere radius 10 + 280·t for 0.6 s around Sora; then bolt 1400 cm/s; Sora invincible from the shot to the end of the state |
| Watara | MAG_WATARA | 1.4 | 94 | splash to r 600 |
| Wataga | MAG_WATAGA | 2.0 | 134 | splash to r 800 |
| Aero | MAG_AERO | 0.6 | 41 (once per enemy) | static cylinder 2 s, r to 400; the first 3 enemies touched get SWIRL_UP (hold 2 s), later ones BLOW; no re-hit |
| Aerora | MAG_AERORA | 1.0 | 67 (once per enemy) | r to 800; SWIRL_UP for the first 8 enemies (2.5 s) |
| Aeroga | MAG_AEROGA | 1.4 | 94 (once per enemy) | r to 1200; SWIRL_UP for the first 14 enemies (3 s) |

Modifiers to test on one spell each: 1 × Fire Boost (FIRE_UP 33): Fire `67 + ceil(0.2·67) = 81`; Full MP Blast
(77) with MP full at the first shot: `67 + ceil(0.5·67) = 101`; Beginner difficulty ×1.25: `ceil(1.25·67) = 84`;
a dummy with defence 20: Fire `ceil((47 − 1e-6)·(1 − 1e-6)) = 47`; a dummy with Fire resistance 50:
`ceil(0.5·67) = 34`. With three dummies inside a Thunder column or an Aero cylinder each takes the listed damage exactly once. The
Aero residue is a jump pad for Sora only (6 s, super jump). Rail geometry and the victim-side swirl: part I.

### T6. Magic combo and finisher (no Combo Plus: MaxCombo 4, NumFinish 1)
| # | Script | Expected |
|---|---|---|
| 6.1 | Fire ×4 chained (press after each shot) | casts 1–3: `MagicFire`, 67 each; cast 4: `MagicShoot` (Shot 0.7 s), row MAG_FIRE_FIN power 1.2 → **81**, damage kind PARABOLA; Sora invincible from the first tick of cast 4 to its shot; a 5th press is refused until the combo resets |
| 6.2 | Thunder as 4th cast | `MagicChant` (Shot 0.5667 s), MAG_THUNDER_FIN 1.4 → 94 |
| 6.3 | Fire, attack, attack, Fire | the magic casts count in the shared combo index: the last one (index 3) is the finisher |
| 6.4 | 1 × MAGIC_COMBO_UP (57), Fire ×3 | hit n (combo value n = 0, 1, 2 from state+0xC0): `67 + ceil(0.1·n·67)` → 67, 74, 81 [verify which counter feeds info+0x54] |
| 6.5 | ENDLESS_MAGIC (82), Fire ×8 | no finisher, no refusal |
| 6.6 | finisher, then wait | command cool-down `MyCommandCoolDownTime` before the next cast is accepted |

### T6b. Grand magic production (part I)
| # | Script | Expected |
|---|---|---|
| 6b.1 | hit one dummy with Fire repeatedly, no other attacks | each hit adds the row's form points (Fire 15, finisher 18; Blizzard / Water 12 / 14; Thunder / Aero 6 / 7) to the formchange gauge; extra adds before the next gauge tick are halved each time |
| 6b.2 | gauge reaches 100 | roll per element: chance = (points of that element + 15 per owned -GAN ability) × M / 1000 with M = 10 (20 during MP charge), × 1.3 with GRAND_MAGIC; the tier offered is one above the highest tier cast; at most 3 offers; gauge and tallies reset |
| 6b.3 | no hits for 20 s | gauge decays 5 per second |
| 6b.4 | use the offered command | no MP cost; row `*_S` (Fira → MAG_FIRA_S power 3.0 → 201 with MAG 67); Sora invincible until the shot |

### T7. Cure (MAG 67)
| # | Script | Expected |
|---|---|---|
| 7.1 | HP 1, MP 100, Cure / Cura / Curaga on self | HP + (80 / 120 / 160) + 67 = **+147 / +187 / +227** (clamped to max HP); MP → 0 and MP charge starts at the shot (0.3333 s, `MagicCast`) |
| 7.2 | Cure with MP 1 | accepted; same heal; charge starts |
| 7.3 | Cure during MP charge | refused |
| 7.4 | Cure as a combo cast | closes the combo (next cast refused until reset), no finisher clip |
| 7.5 | LEAF_VEIL (43), hit Sora during the cast | no damage, no reaction from Begin to End; without it: a hit before the Shot notify (0.3333 s) cancels the cast — no actor, no MP spent, no heal; a hit after the Shot does not stop the heal (done by the Cure actor on its first tick) |
| 7.6 | an ally at 300 cm / 400 cm, Cure | ally +40 + 67 = 107 at 300 cm (range 350); nothing at 400 cm |

### T8. Items (pouch filled by the test)
| # | Script | Expected |
|---|---|---|
| 8.1 | HP 1: Potion / Hi-Potion | state 0.8333 s (`ItemUse`); slot consumed at 0.5 s; HP +80 / +160 at 1.0 s |
| 8.2 | 1 × ITEM_UP: Potion | +120 |
| 8.3 | MP 0 not charging: Ether | MP +100 (clamped) at 1.0 s |
| 8.4 | charging with 40 s left: Ether / Hi-Ether | 15 s left / charge ends, MP full |
| 8.5 | focus 0: Refocuser / Hi-Refocuser | focus 40 / 100; focus-empty flag cleared |
| 8.6 | Elixir at HP 1, MP charging, focus 0 | all full, charge ended |
| 8.7 | Potion in the air | `ItemAirUse`, gravity off until 0.5 s |
| 8.8 | Potion with an empty pouch | refused (`CanUseItem`) |
| 8.9 | Potion requested during an attack before its window | refused; inside the window (Attack flag bit 7): accepted, the queued attack is dropped |

### T9. Shotlock (details: parts F and G)
| # | Script | Expected |
|---|---|---|
| 9.1 | tap the shotlock key (release before 0.2667 s) | lock-on tap; no aim mode |
| 9.2 | focus 100; hold; dummy centred | aim mode opens after 0.2667 s; object-type slow motion 0.4 (Sora, enemies, map; not projectiles, camera, UI); camera 150 cm behind and 100 cm right of Sora, blend 0.5 s; first lock ≥ 0.1 s real time later; then one lock per 2.24 / 24 = 0.0933 s real time, at most one per tick; 24 locks → MAX, kind becomes Ragnarok; consumed focus is frozen while MAX holds |
| 9.3 | hold 1.25 s of real time after the mode opened, fire with n locks | focus = `int((5.0 − 1.25) / 0.05)` = **75**; n ≈ `floor((1.25 − 0.1) / 0.0933) + 1` = 13 locks |
| 9.4 | fire 9.3 (King of Hearts) | timeline from Begin: clip `M_1473`; on the second tick Sora is lifted 500 cm (one swept move); set-up at 2.5 s: 13 projectiles, 7 on the first tick and 6 on the next, on a cone of half-angle 60° around the axis Sora → blade tip, distance 0 → 200 cm in 0.5 s, rotating 180°/s; launch ≈ 3.07 s; each flies at a constant 1500 cm/s, homing 180 → 360°/s over 0.5 s; targets `locked[i mod lockedTargets]`; each hit **54** (row SF_KCSHOT, power 0.8, MAG 67); end clip at 3.2 s; state ends at 4.0 s in Falling; Sora invincible throughout |
| 9.5 | hold with the dummy outside the 150 px circle (scaled by min(W/1280, H/720)) | no lock; the attack key does nothing; aim time runs down; after 5.0 s no lock can be added but the mode stays open |
| 9.6 | hold, no locks, release the button | mode closes, focus unchanged (a cancel costs no focus) |
| 9.7 | dummy behind level geometry inside the circle | no lock (line trace on the projectile channel, blocked by world and gimmicks) |
| 9.8 | 24 locks, fire (Ragnarok) | clips `M_1481` / `M_1482`; set-up at 2.333 s, launch ≈ 2.9 s, end clip 3.333 s, finish 4.133 s; 24 `TresProj_Ragnarok` in one volley (helix radius 0 → 100 → 15 cm), speed 1000 + 500·t capped at 2000, homing 720°/s; each hit **61** (row SF_KCRAGNAROK, power 0.9); focus as in 9.3 for the time spent |
| 9.9 | focus 0 with 1 × FOCUS_ASPIR, hold, 18 locks, fire | syphon mode: lock timer per target = distance · 0.0002 s; MAX at 18; clip `MagicCast`; 0.6 s after its shot notify focus += max(1, trunc(1.4 · 18)) = **25** (2 copies 34, ≥ 3 copies 43); no damage; the focus-empty flag stays until focus is full |
| 9.10 | take a hit while aiming | the mode closes only through the DAMAGE-state gate (hold forced false), no focus cost |

### T10. Airstep (details: part H)
| # | Script | Expected |
|---|---|---|
| 10.0 | airstep not unlocked (player-state bit 5 of +0x95B0 clear) | no airstep target is ever offered |
| 10.1 | aim so the dummy is within 80 px of the centre (2000 cm away), press the airstep key | state 42; no focus cost; the masked object types stay at 0.1 for the whole airstep, Sora returns to 1.0 at launch; launch speed 6000 cm/s; in flight max(6·dist, MaxFlySpeed), 3000 cm/s inside the last 600 cm; arrival at dist < close range (100) + 100 |
| 10.2 | target at 6000 cm | launch speed `2·dist / 1.0` = 12000 cm/s |
| 10.3 | arrival at the dummy | hit effect `vp_ffa_arv0`, clip `AthleticSlideEnd` (no damage), hit pause 0.25 between 0.100 s and 0.2667 s of the clip; attack pressed → ATTACK with forced kind 0x36 (rows FF_SONIC_1 / FF_SONIC_2); guard → queued Superslide |
| 10.4 | press jump during the flight | flight aborted with `SlideEnd`; no jump starts |
| 10.5 | hit Sora during the airstep | no damage (invincible for the whole state) |
| 10.6 | airstep to a level point (kind wall) as the last hop | snap with sweep; ledge probe → DANGLE, otherwise `SlideEnd` |

### T11. Menu, shortcuts, situation slot, pouch (details: part J)
| # | Script | Expected |
|---|---|---|
| 11.1 | open the Magic list with all spells learned | six entries, one per element with the highest tier (Firaga, Waterga, Curaga, Blizzaga, Thundaga, Aeroga in learn order) and their MP cost |
| 11.2 | during MP charge | Magic entries greyed; confirm is refused with the error SE; Link needs full MP |
| 11.3 | confirm a spell | `ExecuteCommand` runs on the press frame (pressed edge only, no buffer); after closing, sub-cursor and main cursor are back at 0 (default cursor option) |
| 11.4 | L1 + Circle / Triangle / Square / Cross with a shortcut page filled by the test | slots 0..3 execute the same command ids as the menu path; an empty slot does nothing; L1 + D-pad down / up changes the page with wrap; a shortcut needing a target targets Sora |
| 11.5 | sub-list open, config "Command Selection: Slow" off / on | game speed unchanged / object-type slow motion 0.3 only while the sub-list is open; the main list and the shortcut display never slow |
| 11.6 | add 7 timed situation commands | capacity 6: the 7th evicts the oldest timed entry; re-adding a kind only refreshes its timer |
| 11.7 | grand-magic situation command | lifetime 20 s (× (1 + 0.2 n) with MAGIC_TIME, cap 99.9 s), countdown starts after 0.5 s; Triangle uses and removes it; production rule: part I |
| 11.8 | pouch at end of game | 3 + 1 per ITEM_SLOT_UP1 usable slots: 6 after the base game, 8 with Re Mind; a used auto-reload slot is refilled from stock when the game leaves a pause, not at battle end |
