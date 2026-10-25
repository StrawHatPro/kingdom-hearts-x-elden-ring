# Sora: action and traversal states beyond Idle / Run / Jump — specification

Extends `docs/sora-movement-spec.md` (revision 2), `docs/sora-movement-spec-turning.md` and
`docs/sora-animation-spec.md` Part I; the conventions are theirs (units cm, s, degrees; frame order
controller → movement (jump check → pose tick with notifies → move) → state `MainProc`; "slot n" =
vtable index; state writes to the movement component act in the next frame's movement tick).
Research only. Parameters: `sheets/port/sora-actions.json`; states: `sheets/port/sora-states.json`;
transition map: `sheets/port/sora-transitions.json` (all three written by
`tools/sora_actions_sheet.js`). Assets referenced: `build/sora_tmp/actions/asset_requests.json`.

**Second pass.** Part II (after section 17) re-read these states in the disassembly and governs wherever it
differs; sections below carry a banner naming what was corrected. Start with II.0.

Tags: **[I]** instruction level (address given) · **[C]** decompiled C only · **[A]** asset ·
**[D]** derived · **[?]** not determined (section 16).

**Evidence level of this document.** One pass by one reader. Read at instruction level: every small
pawn virtual quoted in section 2.4 (getters, setters, availability bits, cooldown countdown), the
notify constructors and the Guard timing handler, the constructor defaults of all tunables, the
ability-level getters. **Everything else in the state bodies and gates is [C]**: the control flow was
read in the decompiled C (`re/decomp`), whose known weaknesses are lost float arguments and wrong
argument order; constants quoted from it were not each re-read in the disassembly. Sections are
headed with their tag. The states of sections 3–9 were read in full ([C]); those of sections 10–13
at architecture level (entry conditions, animations, tunables, main sub-steps) — the sheet column
`scope` says which (`specified` / `specified-partial` / `stub`). An implementation of a
`specified-partial` state needs a second, instruction-level pass of its Tick first.

---

## 0. Summary

> **Part II governs.** Items 2, 4, 6 and 7 are corrected in II.0.1 (guard jump-out, guard window, weapon timers, wall-run enable, climbing dead in retail).

1. The game has **126 player state classes**; 113 are registered for Sora's pawn (14 by the
   `TresPlayerPawnBase` constructor, 99 by `TresPlayerPawnSora`). Traversal and defence are 22 of
   them (section 1).
2. All requests come from one controller function per frame, in a fixed order: **guard button**
   (flow trigger → air slide → guard *or* dodge roll), **jump button** (risk dodge → context attack →
   jump; held: glide), then the stick (section 2). There is no input buffer: the queue holds at most the
   next state, and each state decides at its `TresEnableChange` / `TresEnableInput` notify whether the
   queued state may start.
3. Dodge Roll, Aerial Dodge, Air Slide, Roll Land and the Superslide end are **root-motion clips**;
   their distance and speed profile are the clip's root track (600 / 700 / 1130 × level scale / 300 /
   426 cm). Glide, Superslide, slope slide and swaying are velocity-driven.
4. Invincibility is the body-collision reaction type `GHOST_INVINCIBLE` (11): Dodge Roll from Begin to
   its `TresEnableChange` (0.466 s), Air Slide / Aerial Dodge / Aerial Recovery for the whole state.
   Guard is reaction type 3 plus guard collision parts, from Begin to the `TresTiming(0)` notify
   (0.6 s).
5. Abilities gate the requests, not the states: `REFLECT_GUARD` 3 (guard), `DODGE` 22, `AIRSLIDE` 23,
   `AIRDODGE` 24, `GLIDE` 20, `HIGHJUMP` 16, `DOUBLEFLIGHT` 17, `SUPERJUMP` 18, `SUPERSLIDE` 19,
   `AIR_RECOVERY` 1, `RISKDODGE` 8, `POLE_SPIN` 95, `POLE_SWING` 96. Levels exist only for Air Slide
   (root-motion scale 0.725 / 1.075 / 1.425 by equipped count) and Superslide (time, all 1.0 s). The
   high-jump power is one value (1950): the two lower blueprint values have no reader.
6. The "battle stance" is three independent things: the battle flag (pawn+0x630 bit 1, set from
   outside), weapon out (equipment component), and the **last animation pose** (pawn+0x6E4, written by
   the `TresLastAnimPose` notify of the clip that played last). For the base Keyblade the `B` animation
   set follows the last pose (section 9).
7. Wall run, climbing and dive fall are switched off in the pawn's defaults (`m_bEnableWallRun`,
   `m_bEnableClimbing`, `m_bEnableDiveFall` = false natively, no blueprint override): the level turns
   them on. Flowmotion needs an unlock bit in the player data (+0x95B0 bit 7).

---

## 1. State inventory [I for registration and ids, C for the rest]

> **Part II governs.** Scopes and sub-step counts: sheet `sora-states` (second pass). Idle+0x9C and the id-44 statement stand; registration list unchanged.

State objects are created on demand by `PushState(id)` 0x141513260 → class lookup 0x1415178C0: a map
(queue+0x208) keyed by state id, filled by queue vt+0x3B8(class) in the pawn constructors with each
class's `MyStateID` (+0x58, written by the class constructor chain); id 0x43 (CINEMATIC) uses
`m_CinematicStateClass` (queue+0x260); a `MyStateAsset` (`TresStateSet`, queue+0x200) is consulted
first — Sora's blueprints set none [A].

Registered by `TresPlayerPawnBase` 0x141423210: Idle, Run, Jump, Attack, DodgeRoll, FireMagic, Damage,
AirSlide, Guard, Reflect, UseItem, Die, GameOver, Bind. By `TresPlayerPawnSora` 0x1413E7580: all others
of the sheet whose `registered_by` says so (Dangle, Climbing, WallRun, Hopping, RiskDodge, SuperSlide,
RailSlide(+MI), WallKick, PoleTurn, EnemyFlow, SlopeSlide, Swaying, StyleChange, WeaponChange,
ShooterMode, ShootFlow, the StyleFinish and Combo families, attraction / link / Disney-magic states,
Glide, Floating, AthleticFlow, OpenTreasureBox, Swim, SaveMenu, ShopMenu, Talk, Check, Rescue,
bad-status states, …). Not registered for Sora: TURN 44 (an AI / cinematic state class at pawn level),
STAGGER, the `AI_*` ids, Roxas / Kairi / Riku / Aqua attack classes.

Traversal and defence states (addresses: Reset / Begin / Tick / End = vtable slots 95–98):

| Id | State | Vtable | Reset | Begin | Tick | End | Scope |
|---|---|---|---|---|---|---|---|
| 1 | Idle | 0x1469301E0 | 0x14138F6F0 | 0x141386050 | 0x1413944D0 | 0x14138D7F0 | stage 1 + section 9 |
| 2 | Run | 0x14693B450 | 0x1413AD350 | 0x1413A58A0 | 0x1413AED70 | 0x1413AB9A0 | stage 1 + section 9 |
| 3 | Jump | 0x14692CE20 | 0x14138F740 | 0x1413868C0 | 0x141395450 | 0x14138D850 | stage 1 + section 5 |
| 4 | Glide | 0x14692E478 | 0x14138F4E0 | 0x141384D20 | 0x141391D00 | 0x14138C840 | specified (7) |
| 5 | Floating | 0x14691E048 | 0x14136ED60 | 0x14136ABE0 | 0x14137A680 | 0x14136CF30 | partial (12) |
| 6 | Dangle | 0x146918D38 | 0x14150DEA0 | 0x1413685A0 → 0x141508DE0 | 0x14150F220 | 0x14136B710 → 0x14150BB80 | partial (11.1) |
| 7 | Climbing | 0x14690AB80 | 0x141352DA0 | 0x14134F120 | 0x1413533B0 | 0x141350570 | partial (11.2) |
| 8 | WallRun | 0x146945A10 | 0x1413D0F80 | 0x1413CCD10 | 0x1413D87E0 | 0x1413CF500 | partial (10.2) |
| 9 | Hopping | 0x14692FE48 | 0x14138F650 | 0x141385D80 | 0x141393270 | 0x14138D300 | partial (11.4) |
| 10 | SlopeSlide | 0x146945360 | 0x1413D0BA0 | 0x1413CB400 | 0x1413D1EA0 | 0x1413CE320 | partial (11.3) |
| 11 | Swaying | 0x14694C308 | 0x1413D0EA0 | 0x1413CC7B0 | 0x1413D8110 | 0x1413CF3B0 | partial (11.5) |
| 15 | Guard | 0x14692E800 | 0x14138F510 | 0x1413851D0 | 0x141392080 | 0x14138CA10 | specified (4) |
| 16 | DodgeRoll | 0x14691B7E0 | 0x14136E4A0 | 0x141368880 | 0x141373EE0 | 0x14136B8B0 | specified (3) |
| 17 | RiskDodge | 0x14693A918 | 0x1413AD2A0 | 0x1413A5730 | 0x1413AED20 | 0x1413AB810 | partial (4.6) |
| 20 | AirSlide | 0x1468FFA78 | 0x14133CF60 | 0x141332930 | 0x141340A10 | 0x1413347F0 | specified (6) |
| 21 | SuperSlide | 0x14694BC58 | 0x1413D0E40 | 0x1413CC460 | 0x1413D7C40 | 0x1413CF350 | specified (8) |
| 22 | RailSlide | 0x146936AF8 | 0x1413AD0B0 | 0x1413A4650 → 0x141509CA0 | 0x141510EE0 | 0x1413AAF40 | partial (10.6) |
| 23 | WallKick | 0x14694D490 | 0x14150E2F0 | 0x14150A0B0 | 0x141511E90 | 0x14150C3E0 | partial (10.3) |
| 24 | PoleTurn | 0x146935FE8 | 0x14150DFE0 | 0x1413A41F0 → 0x141509AA0 | 0x1415102E0 | 0x1413AA820 → 0x14150BF30 | partial (10.4) |
| 42 | AthleticFlow | 0x146901158 | 0x14133CFB0 | 0x141332F90 | 0x141341270 | 0x141334B60 | partial (10.5) |
| 66 | Reflect | 0x1469383D8 | 0x1413AD220 | 0x1413A4FF0 | 0x1413AE9A0 | 0x1413AB6F0 | partial (4.6) |
| 102 | Swim | 0x14694C9B0 | 0x1413D0EF0 | 0x1413CC890 | 0x1413D8630 | 0x1413CF3C0 | partial (12) |

Counts by scope (sheet `sora-states`, 126 rows): implemented in stage 1: 3; specified: 5;
specified-partial: 14; stub: 104 (combat, magic, items, hit reactions, forms, attractions, links,
minigames, interaction stubs of section 13).

Common state idioms [C]: every Tick starts with `pawn vt+0xDC8(dt × 12)` / `vt+0xDB8` (restore pitch and
roll of the actor toward zero; 12 → 9 in Idle); `ANIM[0x18]` = PlayAnim(name, slot 0, blendIn, mode,
effectGroup, playRate 1.0, rootScale), `ANIM[0xC8]` = IsAnimEnd, `ANIM[0xE8]` = clip time,
`ANIM[0x120]` = set blend-space X; mode 1 loop, 2 one-shot with root motion, 3 loop with root motion.
Notifies reach the pawn's `TresAnimInterface` and are forwarded to the current state's interface
(state+0x40): `TresEnableChange` → +0x238 (default 0x1411FBA40: `m_bEnableChange` = 1 [I]),
`TresEnableInput` → +0x240 (0x1415124D0: `m_bEnableInput` = 1 [I]), `TresTiming(param)` → +0x348,
`TresLandAnimPose(pose)` → +0x2C8 and `TresLastAnimPose(pose)` → +0x2C0 (state handlers are empty: the
pawn stores them, pawn+0x6E8 / +0x6E4).

---

## 2. Input dispatch and the request interface

> **Part II governs.** II.7 gives the complete key layout; II.1–II.3 correct gate details (dodge from Jump refused, second-jump flag 8 must be SET, bit-0 setters, velocity setters vt+0x9E8 = XYZ / vt+0x9F8 = XY, ability mask at player data +0x95B0).

### 2.1 Controller order [C] (`TresPlayerControllerSora` input update 0x1413F1AB0)

Keys are `FKey` globals; pressed = `WasInputKeyJustPressed` (one frame), held = `IsInputKeyDown`.

| Key global | Meaning | Default pad (movement spec 2.4) |
|---|---|---|
| 0x1487D7760 | attack / decide | FaceButton_Bottom |
| 0x1487D7778 | jump / cancel | FaceButton_Right |
| 0x149C70748 | **guard / dodge** | FaceButton_Left [? binding table not re-read] |
| 0x149C70760, 0x149C70790, 0x149C707C0, 0x149C70778, 0x149C707A8, 0x149C706B8 | command / situation, lock-on, shotlock and swim keys (other agents) | |

```
if controller vt+0x888(): all inputs zeroed this frame (input lock, section 13)
stick as in movement spec 2.3 (magnitude < 0.1 = inactive)
... lock-on / camera / command-menu virtuals (other agents) ...
if the command menu is open: jump press and jump hold are dropped (guard press only if the menu allows)
GUARD pressed:
    if TryFlow(1, 0)            (iface slot 72 0x1414417B0)  -> return     // section 10.1
    if TryAirSlide(yaw, mag, 0) (slot 69 0x141436D20)        -> return     // section 6
    if stick inactive: TryGuard()            (slot 55 0x14143C6D0) -> return on success   // section 4
    else:              TryDodge(yaw, mag, 0) (slot 58 0x14143AAA0) -> return on success   // section 3
else if GUARD held: GuardHeld() (slot 56 0x14143CA20)                      // shield form only
JUMP pressed:
    if TryRiskDodge(yaw, mag, ..) (slot 61 0x14143FF10) -> return
    if context attack (slot 45)                          -> return
    JumpRequest(0) (slot 42 0x14143D8F0)
else if JUMP held: if TryGlide(yaw, mag) (slot 67 0x14143C5B0) -> return;  JumpHeld() (slot 43)
if stick active: MoveInput(yaw, mag) (slot 35 0x141440600);  TryRun(yaw, mag) (slot 40)
swim input (slot 223) when any of stick / jump press / swim keys
```

- With the stick active the guard button means **dodge only**: a refused dodge does not fall back to
  guard. With the stick inactive it means guard only.
- A successful request returns from the function: the stick is not forwarded on that frame.
- No buffering anywhere on this path. What looks like buffering in play is the state queue: a request
  made during a cancellable state pushes the next state at once, and the current state chains to it
  when its notify allows (`m_bEnableChange`, `m_bEnableInput`, or its own rule).
- `MoveInput` (slot 35) forwards the stick to the current state when that state takes it: Jump, Glide
  (through `TryGlide`), AirSlide (+0x88 flag, +0x8C yaw, +0x90 magnitude), Dangle (+0xB4, +0xB8 angle),
  WallRun, SlopeSlide, Swaying, Guard, DodgeRoll, SuperSlide, RailSlide, WallKick, PoleTurn and others
  (per-state writes of slot 35 were not read except where a section says so).

### 2.2 Gates: common part [C, I for the virtuals]

Every gate starts with `CanAct` = pawn vt+0x29C0 (0x141427690 [I]): `!(pawn+0x868 & 2)`, pawn byte
+0x85D ∉ {0x0C, 0x11} (stun / freeze), current and next state ∉ {CATCH 61, BIND 62}; and ends with
`!(pawn+0xE60 bit 17)`. Restriction bits of pawn+0xE60 [I]: bit 8 (0x100) dodge / guard / air slide
(vt+0x1F68, or component query vt+0x1FE8(8)), bit 9 jump / glide (vt+0x1F78), bit 11 run (vt+0x1F98),
bit 15 weapon (vt+0x1ED8), bit 16 (+0xE62 bit 0) everything, bit 17 command lock. All are 0 in plain
field play (movement spec 7); writers are script (`BP_SetRestrict…`), `TresRestrictActionVolume`
(overlap, pawn tick part 0x141449CD0) and the HUD.

"Next queued" (0x1405AB6E0) must be empty or an attack-type state for dodge / guard / air slide (a
queued attack is dropped by the request); jump needs an empty queue in most states.

"Weapon kind" = equipment component vt+0x440(−1): a byte of the current weapon style [I: reads the
style slot]. Kind 1 is the plain Keyblade (the gates require `REFLECT_GUARD` for its guard and `DODGE`
for its roll — the abilities named Block and Dodge Roll); kinds 2–24 are formchange styles with their
own branches (`MoleDodge`, `BackSlide`, `SlideShot*`, `GuardChange`, shield guard …), which this
document names and skips. [? the value for each Keyblade without formchange: probe P3]

### 2.3 Availability bits and cooldowns [I]

pawn+0xE48 (constructor: bits 0–4 set, 0x141423AC0):

| Bit | Meaning | Get / set | Cleared by | Set again by |
|---|---|---|---|---|
| 0 | second jump available | vt+0x1E60 / +0x1E58 | `AirSlide::End` when the next state is not Jump | landing (Jump sub-steps 4 and 10), `Idle::Begin`, WallRun / RailSlide / SlopeSlide / Swim Begin |
| 1 | air slide available | vt+0x1E70 / +0x1E68 | `AirSlide::Begin` | cooldown expiry; every Idle tick in sub-step 0 |
| 2 | air guard available | vt+0x1E80 / +0x1E78 | `Guard::Begin` when airborne | cooldown expiry; Idle tick |
| 3 | flow jump allowed (super jump out of slide / kick / pole) | vt+0x1E90 / +0x1E88 | | landing, Idle tick, WallRun / Rail / Slope / Swim Begin |
| 4 | Superslide out of a flow jump allowed | vt+0x1EA0 / +0x1E98 | `SuperSlide::Begin` | landing, Idle tick, WallKick Begin |

Timers, counted down in the pawn tick part 0x141449CD0 (0x14144A016–0x14144A0A9):

```
if AirSlideBlocked():           // vt+0x1E20 = (pawn+0xE38 > 0) || !bit1
    pawn+0xE38 -= dt;  if pawn+0xE38 < 0: pawn+0xE38 = 0; bit1 = 1
if AirGuardBlocked():           // vt+0x1E30 = (pawn+0xE3C > 0) || !bit2
    pawn+0xE3C -= dt;  if pawn+0xE3C < 0: pawn+0xE3C = 0; bit2 = 1
pawn+0xE34 (glide gate, vt+0x1E10), +0xE40, +0xE44: plain countdowns
```

`AirSlide::End` writes pawn+0xE38 = `MyAirSlideCoolDownTime` 0.6 s when it ends airborne; the Guard
`TresEnableChange` handler writes pawn+0xE3C = `MyAirGuardCoolDownTime` 0.3333 s when airborne and
not hit; `Glide::Begin` writes both. So an air slide can be repeated in one airtime, 0.6 s after the
previous one ended; when a timer was never set the bit returns on the next tick.

### 2.4 Pawn virtuals used below [I] (one-instruction functions unless noted)

| Slot (offset) | Does |
|---|---|
| 741 (+0x1728) | gravity on / off: unless pawn+0x634 bit 1, movement component vt+0x928(b) |
| 756 (+0x17A0) | `SetMovementMode(mode, b)` on the component (1 Walking, 3 Falling, 4 Swimming, 5 Flying) |
| 726 / 728 (+0x16B0 / +0x16C0) | component vt+0x9E8 / +0x9F8 (velocity / launch-velocity write; which is which: [? P6]) |
| 731 (+0x16D8) | component vt+0xA40 (skip-CalcVelocity flag) |
| 616 (+0x1340) | Draw(bOut, b, rate): `bOut && !(restrict bit 15)` → 0x1405BA440: equipment vt+0x4A8(…), then pawn+0x10E0 = 10.0 |
| 619 (+0x1358) | Sheathe(b, rate): only if weapon out and kind ∉ {1, 0x16, 0x17}: Draw(0, …) |
| 614 (+0x1330) | weapon out (equipment+0x2C8 bit 1) |
| 744 (+0x1740) | battle flag: pawn+0x630 bit 1 |
| 745 (+0x1748) | battle animation set wanted (section 9) |
| 751 / 752 (+0x1778 / +0x1780) | get / set last animation pose pawn+0x6E4 |
| 753 (+0x1788) | land pose pawn+0x6E8 |
| 668 (+0x14E0) | `HasAbility(kind)`; +0x1508 = equipped count (levels) |
| 882 (+0x1B90) | air-slide root scale by `AIRSLIDE` count: ≤ 1 → pawn+0xBD0, 2 → +0xBD4, else +0xBD8 (0x141432EB0) |
| +0x1AA0 | high-jump power: pawn+0xB7C (0x141434320) |
| +0x1AA8 | Superslide time by `SUPERSLIDE` count (pawn+0xBDC / +0xBE0 / +0xBE4; 0x1414354B0) |
| +0x1AC8 / +0x1AD0 / +0x1AD8 | glide gravity scale / speed / turn rate (pawn+0xB90 / +0xB94 / +0xB98) |
| +0x1C30 | in water: component vt+0x420() && pawn+0xD70 bit 0 |
| +0x1BD8 | floating: pawn+0xD80 bit 0 |

`FUN_1405B8B00(pawn, type)` sets the body-collision reaction type (pawn+0x619 and
`MyBodyColl`), `FUN_1405B60D0(pawn)` restores `m_DefaultBodyCollReactionType`. Types
(`ETresBodyCollReactionType`): 0 default, 1 no reaction, 3 **guard**, 6 invincible, 8 ghost, 11
**ghost + invincible**.

### 2.5 Abilities [C; kinds from `ETresAbilityKind`]

| Kind | Tested in | Gates |
|---|---|---|
| 1 AIR_RECOVERY | jump gate 0x14142A3D0, `JumpRequest` 0x14143D8F0 (state DAMAGE) | aerial recovery (6.6) |
| 2 BLOW_COUNTER | `AirSlide::Begin` (recovery variant) | counter command after recovery (combat) |
| 3 REFLECT_GUARD | guard gate 0x1414294C0 | guard with weapon kind 1 |
| 8 RISKDODGE | 0x14142CD20 | risk dodge (4.6) |
| 16 HIGHJUMP, 17 DOUBLEFLIGHT, 18 SUPERJUMP | jump type resolver 0x141434010; 17 and 18 also in the jump gate | section 5 |
| 19 SUPERSLIDE | flow gate 0x14142E320 | section 8 |
| 20 GLIDE | 0x141429390 | section 7 |
| 22 DODGE | 0x141427CC0 | section 3 |
| 23 AIRSLIDE, 24 AIRDODGE | 0x141425780, 0x141436D20 | section 6 |
| 74 GUARD_REGENE | 0x14143CAF0 | heal on a guarded hit |
| 95 POLE_SPIN, 96 POLE_SWING | 0x14143F0C0 | section 10.4 |

`WALL_KICK` 97 exists in the enum; the wall-kick gate read here (0x14142F990) was not seen testing it
[?]. A port stubs `has_ability(kind)` and `ability_count(kind)`; the ability set lives in the player
state data reached by pawn vt+0x8C8 (flowmotion unlock: byte +0x95B0 bit 7; bits 0 / 1 mask the
counts of kinds 0x13 / 0x17 [I 0x1414242F0]).

---

## 3. Dodge Roll (id 16) [C]

> **Part II governs.** II.1 (Dodge Roll) is the instruction-level specification; corrections: no roll out of Jump, per-form ability rule, flag handling.

Fields: +0x88 flags (bit 0 clip time ≥ 0.4, bit 1 clip time ≥ 0.1, bit 2 snap to +0x8C, bit 3
invincible, bit 4 `MoleDodge` form), +0x8C yaw, +0x90 effect group, +0x94.

**Request** `TryDodge(yaw, mag, bKeep)` 0x14143AAA0 → gate 0x141427CC0:

```
CanAct && !restrict(bit 8) && !(pawn+0xE62 & 1); controller mode != 0x8B
weapon kind 1: HasAbility(DODGE 22)                       // other kinds: form rules
next queued: none or an attack state
current state:
  IDLE, RUN                : yes
  JUMP                     : OnGround && Jump+0xF8 == 0   // i.e. during the landing
  attack states            : OnGround && Attack+0x234 bit 7
  GUARD                    : OnGround && guarded hit (+0xDC) && m_bEnableInput
  FIREMAGIC                : OnGround && m_bEnableChange && !(+0xD8 & 0x50) && !(pawn+0xE48 bit 5)
  DAMAGE (+0x90 == 0)      : OnGround; types 1, 2, 9: m_bEnableChange; type 19: sub-step 4
  AIR_SLIDE                : !(+0xA8 & 0x80) && m_bEnableChange
  USE_ITEM, treasure, stun : m_bEnableChange (+ OnGround)
  DODGEROLL                : no (the branch is for another character)
then: a queued attack is removed; unless (bKeep and the state is Idle / Run / item / treasure): ClearQueue
PushState(DODGEROLL) (0x141444A50); if mag > 0.5: flags |= 4; +0x8C = stick yaw     // [I] comiss mag,0.5; jbe
```

**Begin** 0x141368880: `StopMovement`; battle flag set → Draw(1, 1, 1.0), else Sheathe(1, 1.0) (no-op
for kind 1); if flag 4: `Turn((0, +0x8C, 0), 1.0)` (full snap); reaction type 11 (`GHOST_INVINCIBLE`);
flags |= 8.

**Tick** 0x141373EE0:

```
pawn vt+0xDC8(dt * 12); vt+0xDB8
sub 0: Play(weapon out ? "DodgeRollB" (fallback "DodgeRoll") : "DodgeRoll", blendIn -1, mode 2, group +0x90, rate 1); sub = 1
sub 1: if !IsAnimEnd:
          t = clip time; if t >= 0.1: flags |= 2; if t >= 0.4: flags |= 1
          if m_bEnableChange:
              if flags & 8: restore reaction type; flags &= ~8
              if a state is queued: (battle: last pose = BATTLE) DoChain; return
              if !OnGround: DoChain                    // Idle -> Fall -> Jump sub-step 3
       else: DoChain
flags &= ~4
```

**End** 0x14136B8B0: restore the reaction type.

Clip `DodgeRoll` = `DodgeRollB` = `rt/body/0051` [A]: 0.7 s, 22 frames, `bEnableRootMotion`,
`m_InterpolationTime` 0.07; root track forward 600 cm, no yaw, per frame: 0, 45.6, 109.8, 173.5, 234.3,
289.1, 337.0, 378.3, 416.1, 449.8, 479.1, 504.5, 526.6, 545.7, 561.9, 575.2, 586.0, 593.8, 598.5, 600,
600, 600 (peak ≈ 1900 cm/s in frames 1–4). Notifies: voice at 0, SE `se00001_022` 0.0243, effect
`vs_spd_lin1` 0.1435 (0.0333 s), `TresIK` 0.4339–0.7, **`TresEnableChange` 0.4659**,
`TresLastAnimPose` (NORMAL) 0.69. No velocity is written: the displacement is the root motion added to
the walking move (movement spec 3.6); the roll follows the ground and can leave an edge (it keeps the
root-motion velocity in the falling delta).

Numbers: invincible 0–0.466 s (28 ticks at 60 Hz, counting from the state's Begin; the first clip
update shows t = 0); cancellable into a queued jump / guard / attack / second roll request from
0.466 s; otherwise ends at 0.7 s → Idle. A roll cannot be requested out of a roll (gate), so chaining
goes roll → (0.466 s) Idle / Run → roll on the next press.

---

## 4. Guard (id 15) [C; timing handler I]

> **Part II governs.** II.1: no jump out of a plain guard; out of a guarded hit the jump starts at once from `TresEnableInput` (coordinator decision 2026-10-11, probe G-J1); player guard uses the collision parts only; Reflect is the weapon-recoil state; Risk Dodge has four types.

Fields: +0xDC guarded-hit flag, +0xDE held, +0xE0 timer, +0xE8 flags (bit 0 requested by the player,
bit 1 success, bit 2 guard collision on, bit 3 push-back active, bit 4 airborne, bit 6 in water),
+0xF4 turn rate, +0xF8 turn timer, +0xFC push-back speed, +0x100 decay, +0x10C hit strength, +0x110
attacker, +0x128 look-at point, +0x134 push direction.

### 4.1 Request

`TryGuard` 0x14143C6D0 → gate 0x1414294C0: `CanAct`, !restrict (bits 8, 15), air guard not blocked
(2.3), weapon kind 1 needs `REFLECT_GUARD` (3); next queued none / attack / guard; current state:
IDLE, RUN, GLIDE, FLOATING yes; JUMP: Jump+0xF8 == 0; attack: +0x234 bit 7; GUARD: guarded hit &&
`m_bEnableInput`; DODGEROLL, AIR_SLIDE: `m_bEnableChange`; FIREMAGIC, DAMAGE, item, stun: their
change flags. Then `SetState(GUARD)` (queue cleared), flags = (flags & ~0x40) | 1 (| 0x40 from Swim).

### 4.2 Begin 0x1413851D0

Last pose = BATTLE; Draw(1, 1, 1.0); `StopMovement`; not on ground → flag 0x10, air-guard bit cleared,
**gravity off** (vt+0x1728(0)); reaction type 3 (guard) — and, for a player-requested guard, the
default reaction is restored at once and slot 116 0x1413A0650 switches the body-collision parts:
`Body` off, `GuardBody` and `Guard` on (flag 4). Look-at point = position + forward × 100, or the
attacker's position (guarded hit), or the lock-on target's position. Hit strength +0x10C = 2 for hit
kinds 2–5, 7–10, else 1. The per-kind tick is bound as a delegate; kind 1 → slot 117 0x14139C310.

### 4.3 Tick 0x141392080 and slot 117

```
pawn vt+0xDC8(dt * 12); vt+0xDB8
slot 117:
  sub 0, player request: Play(airborne ? "AirGuard" : "Guard", -1, mode 2, group 0 (0x209 in water)); +0xF4 = 1.0; sub = 1
  sub 0, hit entry:      Play("GuardHit" / "AirGuardHit"); sub = 2
                         success: +0xF4 = 1.0   else: +0xF4 = +0xF8 = 0.05
                         push direction = hit direction (+0x14C) or -forward; slot 115 (push-back); sub = 3
  sub 1: when +0xDC becomes set (a hit was guarded): Play hit clip; look-at = attacker; +0xF8 = 0.2; push-back; sub = 3
  any:   if IsAnimEnd: DoChain
         else by next queued id: JUMP: DoChain only if m_bEnableChange;  DODGEROLL / AIR_SLIDE / SWIM: DoChain at once;  else wait
if +0xF8 > 0: Turn(yaw to the look-at point, dt * +0xF4 * 60); +0xF8 -= dt        // 0.2 s from Reset
if flag 8: speed *= pow(decay 0.95, dt * 60); component vt+0x9F8(speed * push direction)
```

Push-back (slot 115 0x14138E820): speed = max(current, by strength 1 / 2 / 3: 300 / 1000 / 3000 cm/s
— or the `TresDamageCommonParam` values) × the attack's scale; decay from the attack or 0.95.

### 4.4 The guard window [I for the handler]

`TresTiming` handler 0x14139AD70: `test r8d,r8d; jne return` — acts only for `m_Param == 0`: restore the
reaction type and, if flag 4, slot 116(0) (guard parts off). The `TresEnableChange` handler
0x14139A4E0 does the same and, airborne, turns gravity back on, resets `GravityScale`, and starts the
air-guard cooldown (0.3333 s) if no hit was guarded.

| Clip [A] | Length | `TresTiming(1)` | `TresTiming(0)` = guard off | `TresEnableInput` | `TresEnableChange` |
|---|---|---|---|---|---|
| `Guard` 1281 | 0.8667 | 0 | **0.6** | | 0.6333 |
| `AirGuard` 1283 (+ `TresLandAnimPose` LIGHT at 0) | 0.8667 | 0 | **0.6** | | 0.6333 |
| `GuardHit` 1282 | 0.9667 | | | 0.2667 | 0.7 |
| `AirGuardHit` 1284 (blend 0.13) | 0.9667 | | | 0.2667 | 0.6667 |

So: guarding from Begin to 0.6 s; no root motion; the button is not held (holding only sets +0xDE,
which the plain-Keyblade tick does not read — `MyGuardHoldTime` 3 s belongs to the shield form); the
state ends at the clip end (0.867 s) or at 0.633 s into a queued jump. Air guard hangs in the air
(gravity off, velocity zero) until 0.633 s. Effects: `vs_com_grd01` at 0.034 (guard flash),
`vs_com_grd00` + `vs_com_grd_smk00` on a hit; SE alias `WeaponSwing::SWING2` at 0.033.

### 4.5 Guarded hit (called by the damage system) [C]

`OnGuardedHit(damageInfo, …, attacker, hitDirection)` = iface slot 142 0x14143CAF0 (gate slot 141):
`SetState(GUARD)` again with +0xDC = 1, attacker, hit strength byte, hit direction, `m_bEnableInput`
cleared, bit 1 = success. **Success** = the current state is a player-requested Guard that has not yet
guarded a hit; it calls the controller's counter-window function 0x1414145C0(1.0, attacker, success,
flag) — the reprisal openings (`GUARD_COUNTER` etc.) are decided there and in the attack states (combat
agent). `GUARD_REGENE` (74): pawn vt+0x1878(0, 3, 0, 0, 0). After a guarded hit, from `TresEnableInput`
(0.267 s): dodge roll, air slide, guard again and jump are accepted; the hit clip is cancellable at
0.7 s.

### 4.6 Guard break → Reflect (id 66) and Risk Dodge (id 17) — entry only

A guarded hit whose damage-info strength byte (+0x63) is > 2, for the plain Keyblade, enters
`TresPlayerState_Reflect` instead (push-back speed 400, flags 9; bit 2 when the controller's test of
the attacker vt+0x1CB0 passes). In that state a jump press with `RISKDODGE` (8) and bit 2 → `TryRiskDodge`
0x14143FF10 → `SetState(RISKDODGE)`; its Begin 0x1413A5730: `StopMovement`, MaxWalkSpeed 3000,
MaxAcceleration 180000, Sheathe, reaction type 11, camera request toward the attacker. Both belong to
the hit-reaction / combat agents; a traversal core needs only the entry (`on_guarded_hit` from the
host's damage system).

---

## 5. Jump: types, second jump, remaining sub-steps, exits, landing variants [C]

> **Part II governs.** II.3: second-jump flag polarity and consumption, no reader of the −800 value, type-2 clips, fall-slide sub-steps 5 / 6, landing `B` clips by weapon out.

### 5.1 Types

`RequestJump(yaw, type, bFall, bAir, b6)` 0x141444FA0 resolves type and power through pawn slot 1347
0x141434010:

```
if !b6:
    type 2 without SUPERJUMP  -> (DOUBLEFLIGHT ? 3 : 0)
    type 3 without DOUBLEFLIGHT -> 0;   type 1 without HIGHJUMP -> 0
    type 0 with HIGHJUMP -> 1                               // every ground jump becomes a high jump
power: 0 MyJumpPowerNormal 1480 | 1 pawn+0xB7C = MyJumpPowerHighLv3 1950 [I] | 2 MyJumpPowerSuper 2500 | 3 MyJumpPowerDouble 2300
```

| Type | Name set (selector slot 913) | Start clip / `TresJumpStart` | Requested by |
|---|---|---|---|
| 0 normal | `JumpNormal…` 0031 / 0032 | 0.1 s / 0.0666 | Idle, Run, landing |
| 1 high | `JumpHighStart` 0039, `JumpHighRiseLoop` 0040 | 0.5 s / **0.0647** | same, with `HIGHJUMP` |
| 2 super | `JumpSuperStart` 1161, `JumpSuperRiseLoop` 1162 | 1.8333 s / 0.0667 | Superslide, wall run, wall kick, rail (flow jumps) |
| 3 double flight | `JumpDoubleFlightStart` 0041, `…RiseLoop` 0042 | 0.4333 s / **0.0335** | air jump |
| 4 super flight | (`SuperFlightLoop`: not in Sora's set) | | a form |

The launch mechanism is that of the normal jump (notify → latch → `DoJump` next movement tick, movement
spec 4.1); the fall loop is always `JumpNormalDownLoop`. Tick differences: the variable-height cut
(`MySmallJumpGravityScale` on a released button) is skipped for type 3 (`type != 3` tests in sub-steps
1 and 2); type 2 has its own sub-step-1 branch (waits for the rise, then falls back to type 0 when the
actor reaches the height stored at +0xF4, clearing +0xF8 — the "flow jump" flag that the Superslide
gate tests) [details C, not re-derived]. Horizontal control is that of the normal jump (680, 3672
cm/s²) except in a flow jump with a launch direction (+0x100) and momentum factor +0x110 (0.92).

High jump height [D]: 1950² / (2 × 4116) = 462 cm (button held); double flight: 2300² / 8232 = 643 cm
above the point of the second jump, if the jump-phase gravity applies as for type 0 (same `DoJump`
path; R-probe P5).

### 5.2 Second jump (Doubleflight)

Gate (jump gate 0x14142A3D0, state JUMP): nothing queued; sub-step 11 → ground jump; otherwise pawn+0xE48
bit 0 && `DOUBLEFLIGHT` && !(Jump+0x114 & 8). `JumpRequest`: yaw = Jump+0x8C (the stick yaw of the
current jump, not the facing), type 3, bAir, Jump+0x98 = 1; the queue is cleared and a new Jump is
pushed → Begin at sub-step 0 plays `JumpDoubleFlightStart`; the launch replaces `Velocity.Z` with 2300
at the notify (0.0335 s → second movement tick after the press at 60 Hz). The same request serves
GUARD / DODGEROLL / AIR_SLIDE / SHOOTERMODE / DAMAGE when airborne. One air jump per airtime: [?
where bit 0 is cleared after the second jump was not found in the functions read — P4; the only clear
seen is `AirSlide::End`].

### 5.3 Sub-steps 4–11 beyond stage 1

- **4 (falling)**: also runs slot 113 and, when in water, the dive check (slot 115) → `Diving` clip,
  sub-step 9. On landing: bit 0 set; `LandB` instead of `Land` when flag 0x2000; **`RollLand`** when the
  stick request is set and fall height ≥ 800 (Sheathe first).
- **5 / 6**: fall variants entered from other states (same code as 4 without the dive check); +0xF8
  handling as above.
- **7** `DiveFallStart` (0048, 0.7 s) → **8** `DiveFallLoop` (blend space ±10; mode 3): MaxWalkSpeed
  `MyDiveFallMoveSpeed` 2000, MaxAcceleration × 5.4, GravityScale `MyDiveFallGravityScale` 0.25, steer
  blend ±10 × magnitude when the yaw delta exceeds ±15°, lock-on within 3500; landing → `RollLand`.
  Entered from sub-step 4 only when `m_bEnableDiveFall` (off by default), fall time > 0.5 s, ground
  farther than `MyDiveFallStartHeight` 1600 and the down loop is playing.
- **9** `Diving` (0081): at the clip end or water contact → `PushState(SWIM)` at sub-step 3.
- **10**: additionally tries the slope slide (iface slot 188) before the landing snap; sets bits 3, 4, 0.
- **11**: with `m_bEnableChange` also checks cover.

### 5.4 Exits started by the Jump tick

| Where | Test | To |
|---|---|---|
| head, every tick | in water (component vt+0x5A0) and not (flag 2 && sub < 3) and not sub 9 | SWIM |
| head, every tick (not stunned) | rail probe pawn vt+0xEA0(from actor, to actor + (0,0,30), range `MyRailSlideCheckAirRange` 150, direction = stick or facing) → `TryRailSlide` slot 75 | RAIL_SLIDE |
| tail, falling | pawn+0xD80 bit 0 | FLOATING |
| tail, falling, after Jump+0xA8 (0.2 s from Begin) ran out | dangle gate 0x141427830 (nothing queued; sub 1–2 with Velocity.Z ≤ 500, or Velocity.Z ≤ 0) && ledge probe 0x140638740(pawn, out, 250, 50, 0, 100) | DANGLE (slot 65) |
| tail, same place, when the dangle test fails | `TryHopping(yaw, 0, 0)` slot 184 | HOPPING |
| sub 4 | dive-fall conditions (5.3) | sub 7 |

Wall run, wall kick and pole are not started by the Jump tick: see 10.1.

### 5.5 Landing variants and the land pose [I for the notify default, A, C]

- `RollLand` 0108: 0.7667 s, root motion 300 cm forward (frames: 0, 29.2, 63.1, 98.8, 133.8, 167.6,
  198.6, 225.3, 247.5, 263.7, 275.7, 284.5, 291.2, 295.6, 298.3, 299.6, 300 …), `TresEnableChange`
  0.4325; requires the stick request on the landing tick and fall height ≥ 800.
- **Land pose** (pawn+0x6E8, `ETresLandAnimPose`): written by the `TresLandAnimPose` notify of the clip
  that is playing (default value 2 = LIGHT, constructor 0x14055A970 [I]), cleared by every
  play-by-name. When a state ends in the air *without playing another clip* and Sora reaches the
  ground in Idle, `Idle::Begin` plays the landing chosen by the pose: 1 `Land`, 2 `LightLand`,
  3 **`LightLandR`** (0109), 4 **`LightLandL`** (0110) (`B` variants by the battle animation flag), sets
  sub-step 1 and returns to the idle clip when it ends. Clips carrying the notify [A]:
  LIGHT: `AirDodge`, `AirSlide`, `AirGuard`, `AirGuardHit`, `AirRecovery` (+ many attack clips);
  LIGHT_R: `ClimbingR` 0105, `ClimbingH` 0107, `ClimbingMR` 0117, `HopR` 0113;
  LIGHT_L: `ClimbingL` 0106, `ClimbingML` 0118, `HopL` 0114, air attacks 1328–1330, 1348; LAND: 0903.
  `LightLandR/L`: 0.6333 s, no root motion, `TresEnableChange` 0.0989 / 0.1005. So `LightLandR/L` are
  the foot-specific touchdowns after a vault or a hop (and after an air combo), not selected by the
  Jump state.
- Jump flag 0x40 (set by the Air Slide stick exit): in sub-step 3 the current clip keeps playing until
  it ends, then the down loop.

---

## 6. Air Slide, Aerial Dodge, Aerial Recovery (id 20) [C; levels and cooldown I]

> **Part II governs.** II.2: Draw / Sheathe rule, cooldown conditions, recovery timing (0.2 s after the hit), water variants.

One state, three variants selected at the request. Fields: +0x88 stick flag, +0x8C yaw, +0x90
magnitude, +0x98 target pawn, +0xA4, +0xA8 flags (bit 0 invincible, bit 1 **Aerial Dodge**, bit 2 slide
shot (form), bit 3 in water, bit 4 back slide (form), bit 5 **Aerial Recovery**, bit 7 abort), +0xAC /
+0xB0 reaction-command ids.

### 6.1 Request

`TryAirSlide(yaw, mag, bKeep)` 0x141436D20 → gate 0x141425780:

```
CanAct && !restrict(bit 8) && !(pawn+0xE62 & 1) && !AirSlideBlocked() (2.3); next queued none / attack
current state: JUMP (Jump+0xF8 == 0) | GLIDE, FLOATING | IDLE, RUN (fail later: on ground)
               | attack (+0x234 bit 7, bit 1) | GUARD (guarded hit, m_bEnableInput, bit 1) | DODGEROLL (m_bEnableChange)
               | DAMAGE (types 1, 2, 9: m_bEnableChange; 19: sub 4) | AIR_SLIDE (recovery variant && m_bEnableChange) | RAIL (state rule) | SWIM
weapon kind 1: stick magnitude >= 0.1 (controller value)             // neutral stick -> the press becomes a guard
not in battle: HasAbility(AIRSLIDE 23) && !OnGround
in battle:     (HasAbility(AIRSLIDE) || HasAbility(AIRDODGE 24)) && !OnGround
```

then (unless bKeep) ClearQueue, `PushState(AIR_SLIDE)`, flags bit 1 = **battle flag && `AIRDODGE`**,
bit 3 = in water; with mag ≥ 0.1: +0x8C = stick yaw, +0x88 = 1. So: outside battle → Air Slide (needs
`AIRSLIDE`); in battle with `AIRDODGE` equipped → Aerial Dodge (even if `AIRSLIDE` is equipped too); in
battle with only `AIRSLIDE` → Air Slide.

### 6.2 Begin 0x141332930

In water → flag 8. Plain slide: not in battle → Sheathe, else Draw. Air-slide bit (pawn+0xE48 bit 1)
cleared. If +0x88: `Turn((0, +0x8C, 0), 1.0)` — the slide goes exactly along the stick yaw. Reaction
type 11 (`GHOST_INVINCIBLE`); flag 1; component vt+0xBD0; pawn+0x660 = 0.

### 6.3 Tick 0x141340A10

```
pawn vt+0xDC8(dt * 12); vt+0xDB8
sub 0: StopMovement; gravity off (vt+0x1728(0))
       recovery (flag 0x20): Play("AirRecovery", blendIn 0, mode 2)
       else Aerial Dodge (flag 2, or in water): Play("AirDodge", -1, mode 2, rootScale (1, 1))
       else: Play(weapon out ? "AirSlideB" : "AirSlide", -1, mode 2, rootScale (pawn vt+0x1B90(), 1.0))
       sub = 1
sub 1: if flags & 0x80: DoChain
       if !IsAnimEnd:
           if m_bEnableChange:
               if a state is queued: DoChain
               elif !recovery && !OnGround && +0x88 && +0x90 > 0.5: Fall(+0x8C) with Jump flag 0x40; DoChain
       else: if !OnGround: Fall(actor yaw); DoChain
every tick: pole probe (10.4): pawn vt+0x2330(point ahead, 100) -> TryPoleTurn; if none: +0x88 = 0
```

The stick flag +0x88 is rewritten by `MoveInput` each frame the stick is active, so "stick held past
`TresEnableChange`" ends the slide early into a fall that keeps the clip playing.

### 6.4 End 0x1413347F0

If not recovery and (airborne or floating): pawn+0xE38 = pawn+0xC6C `MyAirSlideCoolDownTime` 0.6 [I 0x1413348ED]. Movement mode
Falling, gravity on, reaction type restored. If the next state is not JUMP: second-jump bit cleared.

### 6.5 Clips and numbers [A; scale I]

| | `AirSlide` / `AirSlideB` 0055 | `AirDodge` 0054 |
|---|---|---|
| Length | 0.7333 s (23 frames) | 0.7 s (22 frames) |
| Root motion (forward) | **1130 cm** × scale | **700 cm** |
| Per frame | 0, 200, 344.4, 476.7, 580, 661.2, 737.0, 805.5, 865.0, 913.8, 950, 977.7, 1002.3, 1024.3, 1043.9, 1061.7, 1078, 1092.1, 1103.5, 1113.2, 1122, 1128.2, 1130 | 0, 100, 197.5, 280.6, 351, 408.4, 462.2, 509.3, 546.3, 577.1, 604.7, 624.3, 637.6, 649.7, 660.5, 670.0, 678.2, 685.1, 690.4, 694.3, 697.3, 700 |
| `TresEnableChange` | 0.5 | 0.4992 |
| `TresLandAnimPose` | 0 (LIGHT) | 0 (LIGHT) |
| SE / effects | `se00001_007` 0, `se00001_036` 0.307; `vs_spd_lin0`, `vs_air_slide0` | `se00001_006`; `vs_spd_lin1`, `vs_adg_rng0` (`vp_div_flo4` in water) |

**Air Slide levels**: the root scale X is `MyAirSlideSpeedScale` 0.725 / `Lv2` 1.075 / `Lv3` 1.425 for
1 / 2 / 3+ equipped `AIRSLIDE` abilities → 819 / 1215 / 1610 cm in the same 0.7333 s (first frame
145 / 215 / 285 cm → 4350 / 6450 / 8550 cm/s). Gravity is off for the whole state, so the path is level;
the root motion is applied in the falling delta (movement spec 4.4 `RM`) and is blocked by walls like
any falling move. Both variants are invincible for the whole state (code: reaction type set in Begin for
every variant, restored only in End).

### 6.6 Aerial Recovery

In state DAMAGE with the "recoverable" byte Damage+0x8E set, damage types 2, 7, 8, 10, and
`AIR_RECOVERY` (1): a **jump press** (`JumpRequest` 0x14143D8F0) does `SetState(AIR_SLIDE)` with flag
0x20 (| 8 in water) and the attacker as target (the jump gate lets type 2 through also on
`m_bEnableChange` without the ability → then an ordinary air jump / fall). Begin (recovery branch): yaw
= current, or toward the target for the 0x100 variant; `Turn(1.0)`; with `BLOW_COUNTER` (2) a reaction
command id (2 / 0x27) is armed through slot 113. Tick sub 0 plays `AirRecovery` 0291 (0.5 s, blend-in 0,
no root motion, `TresEnableChange` 0.2667, SE `se00001_012`, effect `vs_grd_sor_arv00`): Sora stops
dead in the air (velocity zero, gravity off), invincible, and from 0.267 s chains into a queued state
(air slide again, jump, attack) or at 0.5 s falls. End re-draws the weapon unless an attack or damage
follows; no cooldown is written.

---

## 7. Glide (id 4) [C; tunables I / A]

> **Part II governs.** II.2: nothing but releasing the button ends a glide; End zeroes the velocity; gate sub-steps 4 / 6 / 8.

**Request** `TryGlide(yaw, mag)` 0x14143C5B0, called every frame the jump button is down and was not
pressed this frame; gate 0x141429390: `CanAct`, !(pawn+0xE62 & 1), !restrict(bit 9), pawn+0xE34 ≤ 0,
`GLIDE` (20), nothing queued; state JUMP with Jump+0xF8 == 0 and (sub-step 4 or 6, or pawn+0x1460 bit
0), or state GLIDE. From JUMP: `SetState(GLIDE)`. In both cases Glide+0x88 = 1 ("held this frame"),
+0x90 = mag, and +0x8C = stick yaw only when mag > 0.5.

So the glide starts on the first frame the button is held while the Jump is in its falling sub-step
(after the apex; a button held since the jump press starts gliding at the apex frame).

**Begin** 0x141384D20: Sheathe(1, 1.0); `StopMovement` (**velocity zero, all axes**); movement mode
Falling; gravity on; `GravityScale = MyGlideGravityScale` **0.001**; `MaxWalkSpeed = MyGlideSpeed`
**1500**; sub-step 1; +0x8C = actor yaw; play `GlideLoop` (blend space, −1 → 0.33 s, mode 3) with X = 0
unless it is already playing; pawn+0xE38 = pawn+0xC6C (0.6) and pawn+0xE3C = pawn+0xC70 (0.3333) [I 0x141385176..0x141385196].

**Tick** 0x141391D00:

```
if in water: PushState(SWIM); DoChain
pawn vt+0xDC8(dt * 12); vt+0xDB8
sub 1: if +0x88:
           delta = wrap180(+0x8C - actorYaw);  blend X = delta        // GlideLoop axis -180..180
           Turn((0, +0x8C, 0), MyGlideTurnRate 0.03 * dt * 60)
           SetMaxAcceleration(MyGlideSpeed * 60)
           SetMovementInputXY(forward of the ACTOR, 1.0)              // travel follows the facing, not the stick
       else: DoChain                                                  // button released
+0x88 = 0
```

**End** 0x14138C840: movement mode Falling, `ResetMaxWalkSpeed`, `ResetGravityScale`, gravity on.
Releasing the button chains to the default state: Idle begins (returns at once: not on ground), its
tick calls `Fall` → Jump sub-step 3 in the same `MainProc` call.

Model [D]: `PhysFalling` with gravity −980 × 0.001 = −0.98 cm/s² from `Velocity.Z = 0` — after 10 s the
sink rate is 9.8 cm/s and 49 cm of height are lost: the glide is practically level. Horizontal: air
control 0.95 × 90000 cm/s² toward the facing, cap 1500 cm/s (reached in the first tick; the entry
speed is discarded). Steering: 3 % of the remaining yaw error per 60 Hz tick (time constant 0.55 s);
with the stick neutral the target yaw stays, Sora flies straight. Blend space `GlideLoop` [A]:
samples −180 / −90 `GlideLoopL` 0046, 0 `GlideLoopF` 0043, 90 / 180 `GlideLoopR` 0045, sample
interpolation 0.3 s. `GlideIdle` 0047 and `GlideLoopS` 0044 are not used by this state (`GlideIdle` is
the Floating state's).

- **Superglide** does not exist as a separate ability or parameter set in this build
  (`ETresAbilityKind` has only `GLIDE`; the pawn has one `MyGlide*` triple).
- Out of glide: guard (gate: GLIDE allowed → air guard), air slide (gate: GLIDE counts as airborne),
  attack (combat). Landing while gliding and wall contact: the Tick has no ground test [? Q2].

---

## 8. Superslide (id 21) [C; tunables I / A]

> **Part II governs.** II.2: the stick steers; the lock-on aim is for pole launches only; `PhysFlying` specified.

A flowmotion follow-up, not a ground action. **Request**: guard button → `TryFlow(1, 0)` 0x1414417B0 →
gate 0x14142E320: flowmotion unlocked (player data +0x95B0 bit 7), pawn type 1, `CanAct`, not in water;
state JUMP **in a flow jump** (Jump+0xF8 != 0) with pawn+0xE48 bit 4, or WALLRUN / WALL_KICK /
POLE_TURN, or RAIL_SLIDE (state rule) — each with `SUPERSLIDE` (19). From JUMP / RAIL: ClearQueue,
`PushState(SUPER_SLIDE)` (slot 73 0x141445B90) with kind 1; from WallRun / WallKick / Pole the
request is latched in that state (WallRun+0x98 bit 6, WallKick+0xA5, pole state vt+0x390) and the
state pushes the slide with its launch direction.

**Begin** 0x1413CC460: bit 4 cleared; `StopMovement`; movement mode **Flying** (5); gravity off; Draw;
sub-step 0; time +0x90 = `MySuperSlideTime` (1.0 for every level); speed +0x9C = max(`MySuperSlideSpeed`
4000, min); min +0xA0 = `m_SuperSlideMinSpeed` 1000; brake +0xA4 = `MySuperSlideBreak` 1000; play
`SuperSlideLoop` 1152 (kind 1; kind 2 `SonicSlideLoop`) with blend-in 0, mode 1; start position
stored; slot 113 0x1413CD270 aims at the nearest lock-on target within 2500 cm inside a 50° cone
(cos ≥ 0.643) → yaw; else yaw = facing (or the given yaw, snapped).

**Tick** 0x1413D7C40:

```
sub 0: if time <= 0: StopMovement; Play("SlideEnd", blendIn 0.05, mode 2); sub = 2
       else: Turn((0, yaw, 0), dt * 1.5)
             speed = max(min, speed - dt * brake)
             component vt+0x9E8(max(speed, 0.1) * actor forward)        // every tick
             time -= dt
             if time < total - 0.1: pole probe at actor + rotate(yaw)(120, 0, 90), range 100 -> TryPoleTurn
sub 2: if IsAnimEnd: (airborne: air-slide cooldown 0.6) DoChain
```

**End** 0x1413CF350: movement mode Falling, gravity on. Numbers [D]: 4000 → 3000 cm/s over 1.0 s,
3500 cm, level flight (Flying mode, no gravity); then `SlideEnd` 1157 (0.7667 s, root motion 426 cm
decelerating). Jump out: gate needs pawn+0xE48 bit 3 and `SUPERJUMP` (18) → type 2 (2500) with launch
direction = facing and the momentum parameters pawn+0x644 / +0x648. `SuperSlide` 1151 / `SonicSlide`
1154 (root-motion intro clips, 414 / 300 cm) are not referenced by the functions read.

---

## 9. Battle stance, weapon, idle and run variants [C; virtuals I]

> **Part II governs.** II.3: the 10 s timer is dead, `BattleEnd` at the idle clip end for the plain Keyblade, battle flag mirrored from the engaged-enemy count, restriction test vt+0x1EE8.

### 9.1 The three inputs

1. **Battle flag** pawn+0x630 bit 1: set / cleared only through pawn slot 743 `SetBattleMode(b)`
   0x141446CE0 (→ 0x14054E4F0 → 0x1405B8AC0), called by the battle manager when enemies engage or the
   encounter ends — **host-supplied**. Entering and leaving both write pawn+0x10E0 = 10.0.
2. **Weapon out**: equipment component flag (pawn vt+0x1330). `Draw(1, …)` also writes pawn+0x10E0 =
   10.0 s — the keep-out timer, counted down by pawn slot 1309 0x14144A5D0, read by Run.
3. **Last animation pose** pawn+0x6E4 (0 NORMAL, 1 BATTLE): written by the `TresLastAnimPose` notify of
   the playing clip (constructor default NORMAL [I]) and by states (`Guard::Begin` → BATTLE).

Suffix selection: the selectors of animation spec 2.1 receive `weaponOut` and
`battleAnim = pawn vt+0x1748()` [I 0x141435A10]:

```
battleAnim: (weapon out && kind not in {1, 0x16, 0x17}) -> true
            else restrict bit 15 -> false
            else lastPose == BATTLE
```

`N` = weapon away; `W` = weapon out, not `battleAnim`; `B` = `battleAnim`. For the plain Keyblade
(kind 1) the `B` set therefore follows the pose of the last clip: attacks, guard, `BattleStart`,
`IdleB`, `IdleLockon`, `RunBattleStart(W)` end in BATTLE; `BattleEnd` (at 0.1 s), `RunBattleEnd`,
`RollLand`, `LightLandR/L`, `Land`, `IdleN`, `DodgeRoll` (0.69 s), `WaterFlowEnd` end in NORMAL.
`Sheathe` is a no-op for kind 1: its Keyblade is put away by `Draw(0, …)` calls only (idle break,
restricted zones, Run timeout).

### 9.2 Idle (complete) — Begin 0x141386050, Tick 0x1413944D0

Fields: +0x88 battleAnim at Begin, +0x8C sheathe timer, +0x90 water flow, +0x91 lock-on, +0x92 break
played, +0x93 land pose playing, +0x98 wrap counter, +0x9C the int tested by CanRun / jump gate (set to
1 by the Jump landing; its countdown was not located [? Q5]), +0xA8 last clip time.

Begin, beyond stage 1: floating → FLOATING; `TryHopping`; second-jump bit set; land pose → landing clip,
sub 1 (5.5); if the weapon is not restricted and `battleAnim`: Draw, +0x8C = 3.0; a pending
battle-start flag on the pawn plays `BattleStart` (−1, mode 2), sub 1 — unless the stick is ≥ 0.8 and
`TryRun` succeeds, then the state finishes at once; otherwise the idle name by (weaponOut, battleAnim,
lock-on): `IdleN` / `IdleB` / `IdleLockon` (`DiveIdle` in water).

Tick sub-step 0, every tick: bits 1–4 of pawn+0xE48 set (2.3). Then, weapon not restricted:

```
battle = battle flag (also forced by controller modes 0x85, 0x87, 0x88, 0x8B)
if battleAnim == battle: if battleAnim: +0x8C = 3.0
elif !battle:                      // battle over, still in the battle pose
    +0x8C -= dt
    if +0x8C <= 0: Sheathe(1, 1.0); Play("BattleEnd", -1, mode 2); sub = 1
else:                              // battle began while idle
    Draw(1, 1, 1.0); Play("BattleStart", -1, mode 2); sub = 1
weapon restricted and out: Draw(0, 1, 1.0); Play("BattleEnd"); sub = 1
sub 1: at the clip end: Play(idle name by the current flags, -1, mode 2); sub = 0
```

Outside the battle pose: the idle clip is replayed at each wrap; on the third wrap `IdleBreakW` (weapon
out) or `IdleBreakN`; after a break the weapon is put away (`Draw(0, 1, 1.0)`) and the counter
restarts. In the battle pose: `IdleB` ↔ `IdleLockon` when the controller's lock-on flag changes.
Water flow (component vt+0x9B8): `WaterFlowIdle` (mode 3) / `WaterFlowEnd`; entry only.

Timers: 3.0 s of idle after the battle flag drops → `BattleEnd` (1.1 s); 10.0 s keep-out timer for
Run. Clips [A]: `BattleStart` 1014 0.4333 s; `BattleEnd` 1028 1.1 s; `IdleB` 1015 5.9333 s;
`IdleLockon` 1018 1.3333 s; `IdleBreakW` 1004 9.6333 s.

### 9.3 Run (sub-step 3 and the variants) — Tick 0x1413AED70

In sub-step 0 (running, no turn-around), weapon not restricted, when `battleAnim != battle flag` and
the weapon kind is 1 / 0x16 / 0x17: battle ended → play `RunBattleEnd` 1027 (−1, mode 2; fallback
`RunLoopB`), pose NORMAL, sub-step 3; battle began and weapon not out → `RunBattleStart` 0013 /
`RunBattleStartW` 1013, pose BATTLE, sub-step 3. Sub-step 3 behaves like sub-step 2 (clip end → loop,
input and turning continue as in sub-step 0 per movement spec 3.1 COMMON). On the first tick of a
Run begun with the weapon out, not in battle and the keep-out timer expired: `Draw(0, …)`, pose
NORMAL. Loop / start / end names by suffix: `WalkLoopW/B` 1002 / 1016, `RunLoopW` (blend space
1003 / L / R), `RunLoopB` (1017 / L / R), `RunStartW` (1007 set), `RunStartB` (1021 set),
`WalkEndR/L W/B` 1005 / 1006 / 1019 / 1020, `RunEndR/L W/B` 1011 / 1012 / 1025 / 1026 — same axes and
play arguments as the `N` set. No speed or turning difference between stances was seen in the Run
tick.

### 9.4 Weapon change / style change — entry only

`TresPlayerState_WeaponChange` (32; iface slot 232 0x1414426E0) and `StyleChange` (31; slot 190):
Keyblade switch and formchange transformation, combat agent.

---

## 10. Flowmotion [C, architecture level]

> **Part II governs.** II.4 (wall run, wall kick, pole) and II.5 (rail, athletic flow = airstep travel) are the specifications; `m_bEnableWallRun` is ON for Sora.

Common conditions: pawn type byte (pawn+0x5F0) == 1, the unlock bit (player data +0x95B0 bit 7), not in
water, `CanAct`. Movement mode during all of them: **Flying** (5), gravity off, velocity zero; the
displacement is root motion or direct `SetActorLocation` / velocity writes by the state.

### 10.1 Trigger layer

| Trigger | Caller | Probe | Request (iface slot) |
|---|---|---|---|
| rail | Idle / Jump / SlopeSlide / Swaying ticks | pawn vt+0xEA0(start, end = start + (0,0,30), range 50 ground / 150 air, direction, out, 1) | `TryRailSlide` 75 0x14143F4E0 → gate 74 → push 76 |
| pole | AirSlide tick, SuperSlide tick (after 0.1 s), AthleticFlow | pawn vt+0x2330(point, point, 100, out): pole component near the point | `TryPoleTurn` 78 0x14143F0C0 → gate 77 0x14142C640 (state ∈ {AIR_SLIDE, SUPER_SLIDE, ATHLETICFLOW}, nothing queued) → push 80; `POLE_SPIN` / `POLE_SWING` by pole type |
| wall kick | WallRun (guard-button latch), wall contact in AIR_SLIDE / SUPER_SLIDE / ATHLETICFLOW / DODGEROLL | gate 81 0x14142F990: state mask {8, 16, 20, 21, 42}, an overlap query for wall geometry, wall yaw within ±30° | `TryWallKick` 82 0x141442480 → push 83 0x141445D80 |
| wall run | from a wall hit (callers not traced [? Q6]) | gate 180 0x14142FF80: nothing queued, wall validity 0x1406402E0, wall-run point 0x14063EF00 | `TryWallRun` 181 0x141442550 → push 182 |
| Superslide | guard button (controller) | — | `TryFlow` 72 (section 8) |
| athletic flow | [? caller not traced] | controller target list (controller+0xB18, count +0xB20 > 0) | slot 97 0x1414372F0 → gate 96 0x141426280 (state ∉ {DANGLE, SHOOTERMODE}) → `SetState(42)` |

### 10.2 Wall run (id 8)

Requires `m_bEnableWallRun` (off by default). Fields: +0x98 flags, +0x9C wall point / normal (also fed
to the camera `m_Camera`, an `ATresCameraWall` spawned at Begin — camera spec's `m_WallDistance` user),
+0xA8 direction class (0 up, 1 right, 2 left), +0xB4 switch timer, +0xB8 no-input timer, +0x104 / +0x108
/ +0x10C = `MyWallRunSideSwitchAngle` 80, `MyWallRunDownSwitchAngle` 155, `MyWallRunSideMoveSpeed` 980.
Begin: Sheathe; bits 0, 3, 4 set; skip-CalcVelocity on; `ResetMaxWalkSpeed`. Tick sub-steps: 6 entry clip
(`WallRunF/R/L` 0121–0123, mode 2; root motion e.g. R / L: 1238 cm along, 314 cm in, −/+45° yaw), 1
loop (`WallRunLoopF` blend space ±10 with root scale `MyWallRunSpeedScaleUp` 1.84, or
`WallRunLoopR/L` 0125 / 0126 with `MyWallRunSpeedScaleSide` 1.84: 317 × 1.84 = 584 cm per 0.4667 s =
1251 cm/s), 4 direction change (`WallRunChangeFR/FL/RF/LF` 0136–0139, 0.2333 s; the stick angle
relative to the wall must stay in the new class for 0.1 s), 7 end. Each tick the wall point is
re-probed (0x14063EF00; lost, or pitch / yaw deviation > 35° → end); in the entry clip after 0.3 s a
top-edge probe (0x140634980, 225) ends the run over the edge; no stick for 0.2 s ends it. Exits: jump
(sub-steps 1, 3; needs `SUPERJUMP`; type 2), guard button → wall kick, top edge → dangle / fall, End
gives the falling move the last velocity. Rotation: the actor is aligned to the wall
(`SetActorRotation`).

### 10.3 Wall kick (id 23)

Sub 0 (from a plain wall hit): face away from the wall, Flying, `WallJumpStart` 1168 (0.4667 s, root
200 cm up) → sub 1 → Draw → sub 2. Direct entry (+0xA4): `WallKickF` 1169 (effect group 0x139), sub 2.
Sub 2 waits for input: **jump** (latched +0xA6; clip time > 0.1; bit 3 && `SUPERJUMP`) →
`RequestJump(actor yaw, type 2, bAir)`; guard button (+0xA5) at the clip end → Superslide; stick →
kick direction by the angle to the wall normal (dot ≥ −0.819 i.e. more than 35° off the wall line;
sign of the cross product): `WallKickR` 1170 / `WallKickL` 1171 (0.3333 s, root 286 cm forward, ∓112 cm
side, ∓45° yaw) or `WallKickF` (0.5 s, 80 cm back, 180° turn), sub 3. End: Falling.

### 10.4 Pole (id 24)

The pole is a component (`UTresPoleComponent`) giving a grab point (vt+0xBC8), an axis / attach
transform (vt+0xBC0) and a type bit (vt+0xBA0: 0 vertical pole = spin, 1 horizontal bar = swing). Sub
0: snap rotation, attach to the component, Flying, play `PoleTurn` 1172 or `PoleSwing` 1174
(0.1667 s); the position is interpolated to the grab pose over 0.5 s. Sub 1 → at the clip end the loop
`PoleTurnLoop` 1173 (0.3 s) / `PoleSwingLoop` 1175 (0.4 s) in mode 3; spin budget = `MyPoleTurnRotMax`
4 loops (1.2 s / 1.6 s). Sub 2: jump press → launch jump with direction (iface slot 171; needs bit 3);
guard button → Superslide (kind 1 for a bar, 2 for a pole) with the pole direction; stick > 0.5 on the
guard press latches the direction; budget exhausted → release. Controller camera requests at Begin /
End.

### 10.5 Athletic flow (id 42) — entry only

Derived from `TresCharState_SuperSlide`. Begin 0x141332F90: `StopMovement`, Flying, Draw, gravity off,
takes the target list from the controller (+0xB18 / +0xB20) and moves toward the chosen target
(`MyAthleticSlideTime` 1.0 s, `MyAthleticSlideSpeed` 6000, `MyAthleticSlideKickAnim` 0.1333; clips
`AthleticSlide` 1156, `AthleticSlide2` 1153, `AthleticSlide3` 1150, `AthleticSlideEnd` 1163 — root
−150 cm hop back, `TresEnableInput` 0.1, `TresEnableChange` 0.2667). Jump press sets +0xB1; pole and
wall-kick gates accept this state. What fills the target list and who requests the state: [? Q6].

### 10.6 Rail slide (id 22) — entry only

Player class over `TresCharState_RailSlide` (45 own slots). Begin 0x141509CA0: bits 0, 3, 4 set, Flying.
Clips: `RailSlideStart` 1158 (0.2333 s), `RailSlideLoop` (2-D blend space ±30) /
`RailSlideNarrowLoop` (±30), `RailSlideTurn` 1180 (0.3 s), `RailSlideTransfer` 1164 (1.0 s). Jump: the
state decides (vt+0x390 / +0x3C0 with the type-2 power; `m_RailSlideJumpInertialVelocity` 175 [A]) or
falls back to the Superslide-style flow jump. Guard button: state vt+0x3A0 → Superslide. Enemy flow
(25) and the airstep / shotlock state `ShootFlow` (41) are the combat / magic agents'
(`docs/sora-magic-spec.md` covers airstep).

---

## 11. Terrain-bound states [C, architecture level]

> **Part II governs.** II.6 (dangle, climbing, slope slide, swaying) and II.5 (hopping); the vault is dead in retail.

### 11.1 Dangle (ledge hang, id 6)

Entry: Jump tail (5.4) or from Idle / WallRun through `TryDangle` slot 64 0x14143A0D0 → push 65 with
(actor, component, ledge point, normal). Geometry must be flagged for dangling (`SQEX_bEnableDangle`,
movement spec 7). Begin 0x141508DE0: validate / refine the ledge with pawn vt+0x9D8 (handles movable
bases), `StopMovement`, Sheathe, attach to the component, play `GripStart` 0035 (sub 0), position =
ledge + `m_DangleOffsetPos` (−60, 0, −150). Tick 0x14150F220: `GripStart` end → `GripLoop` 0036 (mode
3), sub 1. From `TresEnableChange` (0.199 s) the stick decides (angle between stick and wall direction,
from `MoveInput`): < 90° → climb: Flying, `GripRise` 0037 (0.5 s, root 100 forward / 200 up), sub 2 →
clip end → DoChain; ≥ 135° (≥ 90° with a pawn flag) → release: cooldown 0.6 s (pawn vt+0x2378), `Fall`.
Jump press (gate: sub 0 with `m_bEnableChange`, or sub 1) → drop (`RequestJump` with bFall). End:
Falling.

### 11.2 Climbing (vault, id 7)

`m_bEnableClimbing` is off by default. `TryClimbing(yaw, bReverseLeg)` slot 175 is called by the Run,
SlopeSlide and Swaying ticks; probe sizes `m_fClimbing*` (radius 35, bottom 44, front 150, far 210,
near 120, final 59.5). Begin 0x14134F120: `StopMovement`, Flying, gravity off, clip by obstacle class
and leading leg through pawn vt+0x1CA0: `ClimbingR/L` 0105 / 0106 (0.3667 s), `ClimbingMR/ML` 0117 /
0118 (0.5 s), `ClimbingH` 0107 (0.5333 s) — all root motion 250 cm forward, blend-in 0, mode 2,
`m_bEnableTraction`. Tick: while traction, the actor is lifted toward the ledge height (rate scale
+0xB8); then Falling + gravity; at the clip end on ground → Walking, land-pose clip (`LightLandR/L`,
5.5), sub 4 with `m_bEnableChange`; a new obstacle found by 0x140634E60 restarts the state (chained
vaults).

### 11.3 Slope slide (id 10)

`TrySlopeSlide` slot 188 0x141441490 (floor probe slot 187 0x14142DE50) from Idle, Run, Jump landing,
Swaying. Begin 0x1413CB400: Sheathe; play `SlopeSlide` 0103 (blend-in 0.1, mode 1); acceleration per
tick = (`MySlopeSlideSpeed` 1600 − current speed) / (`MySlopeSlideAccelTime` 0.5 × 60); movement mode
Walking; bits 0, 3, 4 set. Tick: slides along the slope, facing blended to the slope direction; ends
when the slope flattens (floor angle term < 30) or the speed drops under `MySlopeSlideMinSpeed` 680 /
980: `LightLandR` (blend-in 0.1) or a `RunEnd` clip, velocity × 0.98 per tick; exits to Fall, rail,
climbing, hopping, `TryRun` at the end. Jump: gate OnGround → normal jump with yaw = slide yaw.

### 11.4 Hopping (id 9) — entry only

Hop points (markers shown within `MyHopEffectRange` 2000; `BP_SetDisableHopPointDisp`) and balance
beams: clips `HopR/L` 0113 / 0114 (0.4333 s, land pose R / L), `BalanceR/L` 0111 / 0112, `Balance`
0115. `TryHopping(yaw, …)` slot 184 0x14143D7F0 from `Idle::Begin`, the Run tick and the Jump tail;
pushes 185 / 186. Jump allowed in sub-steps 2 and 3.

### 11.5 Swaying (id 11)

The side-step around a pawn in the way: `TrySway` slot 178 0x141441B20 → gate 177 0x14142E570: pawn
type 1, not in battle, nothing queued, the other pawn (its vt+0x1980) in front within 50° (dot ≥
0.643) → push 179 with the pawn, the side (+0x90) and the travel yaw (+0x98). Begin 0x1413CC7B0: play
`RunSwayR` 0101 or `RunSwayL` 0102 (0.9 s, blend-in 0.1, mode 2). Tick 0x1413D8110: not on ground →
`Fall`; `Turn(yaw, dt × 6)`; velocity = `GetMaxWalkSpeed()` × direction through component vt+0x9F8;
from `TresEnableChange` (0.39 s) or at the clip end → `TryRun(yaw, 1.0)` and chain. The caller of
`TrySway` (pawn-contact event) was not traced [? Q6].

### 11.6 Turn, step-down, force walk

- `TurnN` (id 44): no state class is registered for the player; `TurnN` is played by cinematic / AI
  states only.
- Step-down: nothing beyond movement spec 4.5 (Jump sub-step 3 flag 0x20).
- Force walk (`MyForceWalkSpeed` 140, restrict bit 11) and action restrictions come from
  `TresRestrictActionVolume` overlaps (pawn tick part 0x141449CD0) and script.

---

## 12. Swimming, diving, floating [C, entry level]

> **Part II governs.** II.6 (swim) and II.2 (floating).

Swim (id 102) is reachable in field play wherever the movement component reports water (component
vt+0x5A0; the pawn "in water" flag is component vt+0x420 && pawn+0xD70 bit 0): Idle, Run, Jump, Glide
and SlopeSlide ticks push SWIM and chain. Begin 0x1413CC890: bits 0, 3 set; movement mode **Swimming**
(4); if the entry speed `Velocity.Z ≤ −1000` the dive branch starts (sub-step 4), else surface swim
(sub-step 0); from Jump sub-step 9 (`Diving` 0081) it starts at sub-step 3. Tunables (pawn):
`MySwimSurfaceSpeed` 370, `MySwimSurfaceBuoyancy` 1.5, dive low / high 300 / 500, dive 600, rise 800,
dash 1250 for 3 s; component: `BrakingDecelerationSwimming` 700, `m_SwimSurfaceOffset` 100,
`m_SwimSurfaceToDiveVelZ` −2400. Jump out: `JumpRequest` from SWIM (state vt+0x390 gate; flags 3).
Guard in water → Guard flag 0x40 (`AirGuard` clip, effect group 0x209); air slide in water →
`AirDodge` clip. The host must supply a water volume test and the surface height; the motion model
needs its own pass.

Floating (id 5): pawn+0xD80 bit 0 (set by volumes / script); Begin 0x14136ABE0: Falling with
`GravityScale = MyFloatingGravityScale` 0.01 (0.6 when pawn+0xD80 bit 1); clips `GlideIdle` /
`GlideLoop`. Entry only.

---

## 13. Interaction and lock states — stubs [C, entry points]

| State | Id | Pushed by (iface slot) | Stub behaviour |
|---|---|---|---|
| Talk | 104 | 209 0x141445CE0 | hold Idle pose, input locked until the host releases |
| Check (examine) | 108 | 212 0x141444710 | same |
| SaveMenu / ShopMenu | 105 / 106 | 203 0x1414457A0 / 206 0x141445840 | same |
| Rescue | 109 | 215 0x141445700 | same |
| OpenTreasureBox | 74 | 200 0x141445150 (`TresCharState_OpenTreasureBox` Begin 0x141509800) | play `OpenTreasureS` 1072 (0.9667 s) or `OpenTreasureL` 1073 (2.9667 s; `TresTiming(1)` at 1.732 = lid), then Idle; jump / dodge / guard gates ask the state (vt+0x388) |
| GimmickAttach | 97 | 219–221 | attached to an actor that answers the gates |
| OneAction / WaterPlay | 150 / 153 | script | one clip, then Idle |
| HoldCamera | 144 | selfie camera | — |
| Cover | 142 | gate 74-family, `TresCoverActionVolume` / `…Component` overlap (pawn tick part) | shooter sections only |
| Cinematic | 67 | 26 0x1414393D0 | `m_CinematicStateClass`; the Jump tick also chains to it when queued |

Control lock: the controller's `vt+0x888()` zeroes every input of the frame (2.1); pawn+0x518 bit 0
(vt+0xA48) zeroes the stick; restriction bits (2.2) close single gates. The interactable actors and the
"decide button means talk / open / examine" selection are on the controller side (vt+0x1528 / +0x1B78 /
+0x1BD0 before the attack call) and were not read: the host decides what the decide button does and
calls the stub push. At release the queue's default state Idle begins; the weapon follows 9.2.

---

## 14. What the host must supply

> **Part II governs.** Affordance data: the "Host affordances" parts of II.1–II.6 and II.0.4.

Inputs per frame: stick, the three buttons (guard pressed / held, jump pressed / held, attack pressed),
`has_ability(kind)`, `ability_count(kind)`, battle flag changes (`set_battle_mode`), lock-on flag and
target position (guard look-at, Superslide aim), guarded-hit events (`on_guarded_hit(attacker,
direction, strength)`), flowmotion unlock, level switches (`enable_wall_run`, `enable_climbing`,
`enable_dive_fall`), weapon kind (1 for the plain Keyblade).

Collision queries beyond movement spec 6:

| Query | Shape / result | Used by |
|---|---|---|
| ledge probe | 0x140638740(pawn, out, 250, 50, 0, 100): ledge point, normal, component ahead of and above the pawn; geometry flagged for dangling | Jump tail → Dangle |
| rail probe | segment actor → actor + (0, 0, 30), range 50 / 150, direction: rail spline hit | Idle, Jump, SlopeSlide, Swaying |
| pole probe | point query within 100 cm: pole component (grab point, attach transform, type) | AirSlide, SuperSlide, AthleticFlow |
| wall probes | wall validity + wall-run point (position, normal; re-probed each tick; ±35° tolerance); top-edge probe 225; wall-kick overlap (±30°) | WallRun, WallKick |
| slope probe | floor classification for sliding | Idle, Run, Jump landing |
| obstacle probe | vault geometry (sizes in 11.2) | Climbing |
| water | in-water flag, surface height | Swim, dive, water variants |
| hop points | marker search | Hopping |
| pawn in the way | other pawn within a 50° cone | Swaying |

Traversal affordances as data: **ledge** (point, outward normal, component, movable flag); **rail**
(spline, width class, direction); **pole** (grab point, axis, type spin / swing); **runnable wall**
(surface point + normal query, extents implied by the probes failing); **hop point**; **slide slope**;
**water volume** (surface Z).

Effects, sounds and voices referenced by the clips: `asset_requests.json` (29 effect ids, 28 sound
ids / aliases, 12 voice ids).

---

## 15. Transition map (summary; all rows in `sheets/port/sora-transitions.json`)

> **Part II governs.** Sheet `sora-transitions` (110 rows, second pass) governs.

```
IDLE / RUN --guard btn, stick-->  DODGEROLL --EnableChange 0.466 + queued | end 0.7--> queued | IDLE
IDLE / RUN / JUMP / GLIDE --guard btn, no stick--> GUARD (air: AirGuard) --0.633 + queued jump | end 0.867--> ...
GUARD --guarded hit--> GUARD(hit) | REFLECT --jump + RISKDODGE--> RISKDODGE
JUMP(falling) --jump held + GLIDE--> GLIDE --release--> IDLE -> JUMP(3)
JUMP / GLIDE / … (airborne) --guard btn + stick + AIRSLIDE / AIRDODGE--> AIR_SLIDE --EnableChange 0.5 + queued | stick --> queued | JUMP(3)
JUMP(air) --jump + DOUBLEFLIGHT--> JUMP(type 3);  IDLE / RUN --jump--> JUMP(type 0, or 1 with HIGHJUMP)
DAMAGE --jump + AIR_RECOVERY--> AIR_SLIDE(recovery)
JUMP(flow jump) / WALLRUN / WALL_KICK / POLE_TURN / RAIL_SLIDE --guard btn + SUPERSLIDE--> SUPER_SLIDE --jump + SUPERJUMP--> JUMP(type 2)
AIR_SLIDE / SUPER_SLIDE / ATHLETICFLOW --pole in reach--> POLE_TURN --jump--> JUMP | --guard--> SUPER_SLIDE
JUMP(falling, after 0.2 s) --ledge--> DANGLE --stick in--> climb (GripRise) | --stick out / jump--> JUMP(3)
IDLE / JUMP / SLOPESLIDE / SWAYING --rail--> RAIL_SLIDE;   IDLE / RUN / landing --slope--> SLOPESLIDE
RUN --pawn in the way--> SWAYING --> RUN;   RUN --obstacle (if enabled)--> CLIMBING --> IDLE (LightLandR/L)
any ground / air state --water--> SWIM;   * --event--> CINEMATIC / TALK / … (stubs)
```

---

## 16. Open questions and runtime probe requests

> **Part II governs.** Status of each question: II.0.3; probe requests: `probe_requests.json` key `pass2`.

| # | Question | Probe (object, offset, type / event) |
|---|---|---|
| Q1 | The whole [C] layer: each constant and branch of sections 3–8 should be re-read in the disassembly before implementation | per-frame trace: state id, sub-step (state+0x78), state flags (DodgeRoll+0x88 u32, Guard+0xE8 u32, AirSlide+0xA8 u32), pawn+0x619 u8 (reaction type), Velocity vec3, actor position, clip name and time |
| Q2 | Landing and wall contact while gliding (no ground test in the Glide tick) | trace state id + MovementMode (component+0x314 u8) through a glide onto the ground |
| P3 | Weapon kind byte of each Keyblade without formchange (expected 1) | equipment component (pawn+0x598): i32 +0x378, byte of slot [+0x2E8 + index × 0x30] |
| Q4 / P4 | Where the second-jump bit is cleared after a double flight, and the reader of `MyJump2ndEnableMinimumVelocityZ` (−800) | pawn+0xE48 u32 per frame through jump → air jump → land; write watch on bit 0 |
| P5 | Gravity phases of types 1–3 (assumed those of type 0) | component Velocity.Z + GravityScale (+0x2F8 f32) + flag +0xBFC u8 per frame for a high jump and a double flight |
| Q5 | Countdown of Idle+0x9C (the int that delays run / jump after a landing) | Idle state +0x9C i32 per frame after a landing |
| P6 | Which of component vt+0x9E8 / +0x9F8 sets Velocity and which the launch velocity; how they interact with Flying mode | component Velocity (+0x214 vec3) and +0xC0C vec3 during a Superslide and a guard push-back |
| Q6 | Callers of `TryWallRun`, `TrySway`, the athletic-flow request and the wall-kick-on-contact path (pawn hit callbacks not traced) | function-entry events 0x141442550, 0x141441B20, 0x1414372F0, 0x141442480 with the current state id |
| P7 | Guard: push-back values of `TresDamageCommonParam` (+0xEC / +0xF0 / +0xF4) and the guard arc (none seen in the state; decided by the collision parts) | read the asset; trace Guard+0xFC f32 on guarded hits of three strengths |
| P8 | Air Slide distances per level in the running game | actor position per frame for Air Slide with 1, 2, 3 abilities: expect 819 / 1215 / 1610 cm in 0.733 s |
| P9 | Glide sink and speed | Velocity vec3 per frame over a 5 s glide: expect \|XY\| = 1500, Z ≈ −0.98 × t |
| Q10 | Default pad binding of the guard key (FKey 0x149C70748) | read the key struct at 0x149C70748 at run time |
| Q11 | Dodge Roll off a ledge: root-motion velocity carried into the fall, state chain on `m_bEnableChange` | trace through a roll across an edge |

---

## 17. Files

- `docs/sora-actions-spec.md` (this file)
- `sheets/port/sora-states.json` (126 rows), `sheets/port/sora-transitions.json` (52 rows),
  `sheets/port/sora-actions.json` (88 rows); writer `tools/sora_actions_sheet.js`; declared in
  `tools/lib/sheet_defs.js`
- `build/sora_tmp/actions/asset_requests.json` — animations (123 names with asset, length, root
  motion, notifies, play arguments), effects, sounds, voices
- Scratch (reading aids, reproducible): `build/sora_tmp/actions/` — `NOTES_facts.md` (condensed
  facts), `anim_facts.json` (per-sequence root tracks and notifies with properties), tools
  `cc.js` (compact decompiled C), `tiny.js` (one-line disassembly of small functions / vtable slots),
  `ainfo.js` / `asum.js` (animation facts), `ctorvals.js` (constructor stores), `slots.js`, `ivt.js`
  (state interface vtables), `states_dump.js`, `state_ids.js`, `iface_scan.txt`
- Progress note: `build/sora_tmp/ACTIONS_STATE.md`

<!-- PART II (generated by build/sora_tmp/actions/merge_p2.js from the second-pass strands) -->

# Part II — second pass: instruction-level re-read and full specifications

Part II was produced by seven reading strands (II.1–II.7 below), each working in the disassembly; it
**governs wherever it differs from Part I**. Part I is kept for its structure, the inventory and the
transition overview; its sections carry a banner naming the Part II section that supersedes them. Each
strand section ends with "Corrections to Part I", "Host affordances", "Tests" and "Still needs the
running game". The sheets (`sora-actions` 376 rows, `sora-transitions` 110, `sora-states` 126) already
carry the second-pass rows (a second-pass row replaces a first-pass row of the same id).

Limits of the merge: the coordinator of this document read every strand's correction table, the
disagreement lists and the probe tables, and reconciled them (II.0.1–II.0.3). The strand bodies
themselves (about 420 000 characters) were appended as delivered and were **not** re-verified line by
line against the machine code a second time; where two strands touch the same function the
reconciliation below says which one governs.

## II.0.1 Cross-check: what Part I had wrong (most consequential first)

| Topic | Part I | Governing reading (strand, address) |
|---|---|---|
| Second jump flag | gate needs `!(Jump+0x114 & 8)` | needs flag 8 **set**: `test [Jump+114h],8; jne allow` 0x14142A589. Flag 8 is maintained by Jump slot 113 0x1413837F0: set when `Jump+0x9C == 0 && Velocity.Z ≤ 500` (jump, air — both strands agree) |
| Second jump consumed | "where bit 0 is cleared: not found" | `Jump::Begin` 0x141386DB0: `SetSecondJump(type != 3)`; type 2 also clears it (0x141386E21); also the rail jump 0x14150AEC3. `MyJump2ndEnableMinimumVelocityZ` (−800) has **no reader**; the live threshold is the literal 500 |
| Wall run enable | off by default, "the level turns it on" | `m_bEnableWallRun` is set by the `TresPlayerPawnSora` constructor 0x1413E7ECD; the gates are the script ability mask (player data +0x95B0 word, bit 8 `BP_SetAbilityWallRun`, default on) and the component flag `SQEX_bEnableWallRun`. Bit 7 (`BP_SetAbilityFreeFlow`) gates the flow trigger, wall kick and pole only; airstep uses bit 5 (wall, rail) |
| Weapon put-away | Run hides the weapon when the 10 s timer pawn+0x10E0 expires; Idle `BattleEnd` after 3.0 s | the Run reader sits behind Run+0xBD, never set: **the 10 s timer has no effect**. Plain Keyblade: Idle plays `BattleEnd` at the end of the idle clip that is playing once the battle flag is off; the 3.0 s countdown applies to formchange kinds only (jump) |
| Battle flag source | "battle manager, host-supplied" | mirrored every frame by the controller (slot 630) from the game object: engaged-enemy count > 0, plus a 1.0 s linger, or a forced flag (jump). The host supplies the count |
| Guard: jump out | queued jump chains at `TresEnableChange` 0.633 s | the Guard `TresEnableChange` handler never sets `m_bEnableChange`, so a jump QUEUED behind a Guard would wait for the clip end; but the jump request does not queue behind it. COORDINATOR DECISION 2026-10-11: `JumpRequest` 0x14143D8F0 (cases 15 / 16) calls `ClearQueue` 0x1405A8010 before `RequestJump`; `ClearQueue` empties the WHOLE queue, the running Guard's entry included, so `NeedChange` replaces the Guard at the next `MainProc`: a jump accepted out of a guarded hit (from `TresEnableInput`) **starts at once**. The slot-117 JUMP branch (queued jump waits for `m_bEnableChange`, never set) is then dead for this path. Probe request G-J1. Dodge roll / air slide chain at once after a guarded hit's `TresEnableInput` too (defence) |
| Guard window | reaction type 3 + parts until 0.6 s | player guard: collision parts `Guard` / `GuardBody` only, Begin → 0.6 s; type 3 on `Body` exists only in the hit-entry guard until its `TresEnableChange` (0.7 / 0.6667 s) (defence) |
| Reflect | "guard break" | the weapon-recoil state (`AttackReflect` / `AttackAirReflect`), entered when `m_ReflectLevel` ≥ 3 and from `TakeDamage` results 3 / 4; Risk Dodge is offered by the attacker's `TresAnimNotify_DamageCaution` and has four types (UnderSlide, OverSlide, SlideTurn, Warp) (defence) |
| Dodge from Jump | allowed during the landing | `TryDodge` returns false for state JUMP (table 0x14143AC94) (defence) |
| Glide end | landing / wall contact unknown | **nothing ends a glide but releasing the button** (or water / a queued state): on rising ground the component walks while the state stays GLIDE; End zeroes the velocity (`SetMovementMode(3, bStop)`); gate sub-steps 4, 6 or 8; speed 1425 after the first tick, 1500 from the second (air) |
| Velocity setters | "which is which [?]" | component vt+0x9E8 = Velocity XYZ, vt+0x9F8 = XY only, vt+0xA08 = Z only; each also arms a one-shot "direct velocity" record that overrides the next `CalcVelocity`. No separate launch velocity. `StopMovement` also zeroes Acceleration (air, wall, terrain agree) |
| Flying mode | "Flying, velocity zero" | `PhysFlying` 0x14058E570 specified in II.2; rail slide is Flying in label only — `PhysRailSlide` (component slot 408) advances along the spline without sweep (rail) |
| Superslide steering | aims at a lock-on target | the aim runs only for a pole launch (flag 8); otherwise the stick steers (`MoveInput` → +0x8C, `Turn(dt × 1.5)`). `TryFlow` from WallRun latches and returns false, so the same press continues to air slide / guard / dodge (air) |
| Jump type 2 clips | `JumpSuperRiseLoop` | the selector never returns it; `JumpSuperStart` is followed by the down loop. Type 1: the short-hop cut is live in sub-step 1 (clip time > 0.1) (jump) |
| Landing `B` clips | by battle animation flag / flag 0x2000 | by weapon out at the landing tick (jump) |
| Idle+0x9C | delays run / jump after a landing | never written: both gate tests are constant true. The landing writes Jump+0x9C (jump). This also corrects movement spec 2.5 ("Idle.timer") |
| Climbing | "the level turns it on" | nothing in the code or the cooked data sets `m_bEnableClimbing`: the vault is **dead in retail**; it needs `SQEX_bEnableAutoRun` on floor and obstacle top, heights 44–225 cm (terrain) |
| Swaying | ends at `TresEnableChange` | stick released: ends at `TresEnableChange` (0.39 s); stick held: continues to the clip end (0.9 s), steering. Caller: Run state handler 0x1413B2960 (terrain) |
| Rail | "rail spline hit" by collision probe | a registry of rail nodes (spline node chain), not a collision query; started from Idle, Run and Jump only (gate ids 0–3); the on-rail jump exists only on `m_JumpLimitType = RELATIVE` rails (rail) |
| Hopping | hop points and balance beams | no beam object: the Balance clips are idles on a hop point (rail) |
| Dangle | cooldown at release; Flying | cooldown pawn+0x6F8 refreshed to 0.6 every hanging tick; hang runs in Custom mode 6 (empty Phys); component flag `SQEX_bEnableDangle` = primitive +0x854 bit 2 (terrain) |
| Swim entry | `Velocity.Z ≤ −1000` → dive | −1000 is a clamp; underwater start = immersion ≥ 1.0 and not (near surface and Velocity.Z > −2400); water = `bWaterVolume` physics volume with the surface > 100 cm above the feet (terrain) |
| Athletic Flow | flow dash with `AthleticSlide*` clips | it **is the airstep travel**: a chain of legs to lock-on / `UTresShootFlowComponent` targets, cruise clips `SuperSlideLoop` / `SonicSlideLoop` / `SuperFlightLoop`, invincible through pawn+0x630 bit 2 and reaction type 8 (rail) |
| Aerial Recovery | Damage+0x8E writer unknown | set by the damage sub-ticks 0.2 s after the hit (air; combat pass 2 section E agrees: +0x138 = 0.2, the 0.4667 variant is never armed) |

Strand-versus-strand: no contradiction was found in the correction tables. Overlaps resolved as:
second-jump flag — jump and air agree; setters 0x9E8 / 0x9F8 — air, wall and terrain agree; pole
probe — wall governs for AIR_SLIDE / SUPER_SLIDE (registry walk, point = actor + rot·(120, 0, 90),
range 100), rail governs for state 42 (last target of type POLE only); dangle pushers — terrain's list
(Jump tail, WallRun tick, Hopping helper, AthleticFlow) governs over Part I 5.4.

## II.0.2 Reconciliation with the other specifications

**Combat (`docs/sora-combat-spec.md`, and the unmerged `build/sora_tmp/combat/P2_C/D/E_section.md`).**
The defence strand lists seven points (II.1, end of "Corrections"). Resolved from the code as read
there: (1) dodge / guard gates out of an attack test Attack+0x234 bit 7, not `m_bEnableChange`
(0x141428190, 0x1414296EF) — combat 5 should be corrected; (2) controller+0x8A0 is the 1.0 s counter
(reprisal) window, controller+0x89C the 0.1333 s just-guard timer — combat 3.2 / 3.3 / 13 name them
the other way round; (3) `TresDamageInfo` +0x60..+0x63 (victim result, attacker result,
`m_ReflectReaction`, `m_ReflectLevel`) are missing from combat 8.2; (4) "Ignored" has two forms (ghost
types drop the hit, invincible types register it for the attacker). Agreement with combat P2_E:
reaction types 3–6 make `apply = false`; no mercy invincibility — DAMAGE re-enters DAMAGE; Aerial
Recovery becomes available 0.2 s after the hit (+0x12C > +0x138) in the blow sub-steps, which is the
"+0x8E" of Part I 6.6. Reaction type 11 is not the only invincibility: controller+0x8F4 and the pawn
tests vt+0x2080 / vt+0x2070 are independent mechanisms that none of the states here writes; airstep
uses pawn+0x630 bit 2.

**Magic (`docs/sora-magic-spec.md`, airstep).** The rail strand lists nine points (II.5, "Disagreements
with the magic spec"). Its readings carry instruction addresses and govern for the state's side:
the speed formula `max(2·d/T, min(60·d, 6000))` is evaluated once per leg (0x141348D01–0x141348D9D)
and stored as `MaxFlySpeed`; the per-tick request is `max(6·d, MaxFlySpeed)`, replaced by 3000 when
d < 600 (0x14134964B–0x141349678); the leg ends at d < `m_CloseRange` + 100; the request is
`SetState`, not `PushState`; the unlock is bit 5; the cruise clips are the Superslide loops. Ownership:
target selection in aim mode stays with the magic spec; Begin / Tick / End and the arrival branches are
II.5. Points 4 and 6 (component vt+0xAF8 = `m_ShootLockOnRange`; 3750 is the "unset" sentinel, default
search range 15000) concern the magic agent's own section and should be checked there — both readings
are stated with their addresses in II.5.

**VFX (`docs/sora-vfx-spec.md`).** Consistent: the Keyblade is never detached; "weapon away" is the
same actor made invisible, appearing 0.22 s after the call with the dither ramp (0.06 s) and effect
`VW_SO010_APP0`. `Draw(1)` / `Draw(0)` of this document are those calls; `Sheathe` is a no-op for weapon
form 1 (every Keyblade without formchange, `ETresWeaponForm`). The stance rules that decide *when* are
II.3 (battle flag mirrored from the engaged-enemy count; `BattleStart` on the flag rising in Idle,
`RunBattleStartW` in Run; `BattleEnd` at the idle clip end after the flag drops; put-away after an idle
break; the 10 s timer dead).

## II.0.3 What moved

- **To instruction level**: Dodge Roll, Guard and the guarded-hit path, Reflect, Risk Dodge (II.1);
  Air Slide / Aerial Dodge / Aerial Recovery, Glide, Superslide, Floating, the velocity setters,
  `PhysFlying` (II.2); jump types, second jump, sub-steps 5–9 and tail exits, landing variants, land
  pose, the stance system, weapon form, Idle+0x9C (II.3); Wall Run, Wall Kick, Pole Turn and their
  callers (II.4); Rail Slide, Athletic Flow, Hopping (II.5); Dangle, Climbing, Slope Slide, Swaying,
  Swim (II.6); the input layout (II.7, sheet `sora-input-layout`, 32 rows). Statements still tagged [C]
  inside a strand section are marked there.
- **Scope column** (`sora-states`): specified 15 — Glide, Floating, Dangle, Climbing, WallRun,
  SlopeSlide, Swaying, Guard, DodgeRoll, AirSlide, SuperSlide, RailSlide, WallKick, AthleticFlow,
  Reflect; implemented-in-stage-1 3 (Idle, Run, Jump; their extras are specified in II.3);
  **still specified-partial 4**: Hopping, RiskDodge, PoleTurn, Swim (the strands kept them partial: see
  the "Still needs" tables of II.5, II.1, II.4, II.6 for what is [C]); stub 104.
- **Open questions of Part I section 16**: Q2, P3, Q4 / P4, Q5, P6, Q6 (all four callers), P7
  (push-back 300 / 1000 / 3000 confirmed in the asset; no guard arc in the state), Q10 (II.7) are
  settled statically. Q1 is done for the states above. P5 (gravity phases of types 1–3), P8, P9, Q11
  remain measurements.

**Still needs the running game** (consolidated: `build/sora_tmp/actions/probe_requests.json`, key
`pass2`, 90 field requests; the tables at the end of II.1–II.6). The ones that decide behaviour rather
than confirm it: the body primitive an attack reports when `Guard` and `GuardBody` both overlap, and
pawn+0xD84 (selects the guard push-back formula) (II.1); braking between the per-tick velocity writes
(II.1, II.2); the root-scale element that scales vertical root motion on a wall (II.4 W3); pawn+0x644 /
+0x648 defaults for pole and flow jumps (II.4 W4); the pole grab-pose algebra (II.4 W6); the writer of
Jump+0xBC (fall slide) (II.3); the engine event behind the Sway bump (II.6 S1); swim fluid friction and
surface factors (II.6 W2); rail speed fields on a real rail (II.5).

## II.0.4 Implementation order and test list

Loadout assumed: level 99, every ability equipped (so: high jump on every ground jump, Air Slide level
by equipped count, Aerial Dodge in battle), plain Keyblade (weapon form 1), 60 Hz. Each strand's
"Tests" section gives the frame-by-frame scripts and expected numbers; this list fixes the order and
the affordances the test level must provide. Numbers below are the headline expectations.

**Tier 1 — no level features needed (flat floor, one wall, one drop of ≥ 800 cm, a dummy attacker)**

| # | State | Host must supply | Headline expectations | Tests |
|---|---|---|---|---|
| 1 | Dodge Roll | guard button, stick; body-collision reaction type to the damage system | 600 cm in 0.7 s on the 0051 root track; reaction type 11 from Begin to 0.4659 s; queued state chains from 0.4659 s; no roll out of a roll or out of Jump | II.1 |
| 2 | Guard + guarded hit | collision parts `Body` / `Guard` / `GuardBody` or an equivalent "guarding" answer; `on_guarded_hit(damage info)` | guarding 0–0.6 s; clip 0.8667 s; no jump out of a plain guard; hit clip: input 0.2667 s, then jump / roll / air slide start at once; push-back 300 / 1000 by damage kind, × 0.95 per tick; air guard hangs, cooldown 0.3333 s | II.1 |
| 3 | Air Slide / Aerial Dodge / Aerial Recovery | battle flag (engaged-enemy count), ability counts | Air Slide 819.25 / 1214.75 / 1610.25 cm in 0.7333 s for 1 / 2 / 3 abilities, level path; Aerial Dodge (battle) 700 cm in 0.7 s; invincible whole state; refusal for 0.6 s after End; recovery from 0.2 s into a blow, `AirRecovery` 0.5 s, cancel 0.2667 s | II.2 |
| 4 | Jump types | — | high jump 1950 (462 cm if type-0 gravity: P5), `DoJump` one frame earlier than the normal jump's notify; double flight 2300 when flag 8 (Velocity.Z ≤ 500), consumed at its Begin, restored on landing; super jump 2500 from flow states only | II.3 |
| 5 | Glide | jump held | starts in falling sub-steps 4 / 6 / 8; velocity zeroed; 1425 then 1500 cm/s; sink −0.98 cm/s²; turn 3 % per tick; ends only on release; velocity zero at End | II.2 |
| 6 | RollLand and land poses | — | `RollLand`: stick + fall ≥ 800 → 300 cm root motion, control 0.4325 s; `LightLand` after air slide / air guard / recovery through the land pose; `LightLandR/L` after hop clips | II.3 |
| 7 | Battle stance and Keyblade | engaged-enemy count (+ 1.0 s linger), VFX appear / disappear calls | flag rises in Idle → `BattleStart` 0.4333 s, in Run → `RunBattleStartW`; flag drops → `BattleEnd` at the idle clip end; `B` set follows the last-pose notify; put-away after the idle break | II.3 |

**Tier 2 — flowmotion and terrain features**

| # | State | Affordance the level must place (exact data in the strand's "Host affordances") | Tests |
|---|---|---|---|
| 8 | Superslide | a flow jump source (wall kick, pole or rail) | II.2: 3983 → 3000 cm/s, 3492 cm in 1.0 s, then `SlideEnd` 426 cm; stick steers |
| 9 | Wall Run | wall primitive with `SQEX_bEnableWallRun`; ability-mask bit 8; entry from a falling Jump with stick held, Run (stick > 0.5), SlopeSlide, Swim | II.4 |
| 10 | Wall Kick | wall primitive with `SQEX_bEnableWallKick`; probe: sphere r 5 swept 105 cm along −normal from 75 cm above the origin, normal pitch within ±30°; `WALL_KICK` (97), mask bit 7 | II.4 |
| 11 | Pole | `UTresPoleComponent` in the pole registry (segment, type spin / swing, grab transform); probe = distance from actor + rot·(120, 0, 90) to the segment ≤ 100 | II.4 |
| 12 | Rail Slide | rail node chain (spline nodes with `m_DetectRangePlus`, jump-limit type, rail type) in the rail registry; probe range 50 ground / 150 air | II.5: default rail speed 2000 |
| 13 | Athletic Flow arrival | lock-on targets / `UTresShootFlowComponent` points with `m_bMoveFlowDestination`, `m_ShootLockOnRange`, `m_CloseRange`; a chosen target list from aim mode (magic spec) | II.5: leg speed `max(2d, min(60d, 6000))`, final 3000 cm/s from 600 cm |
| 14 | Dangle | ledge on a primitive with `SQEX_bEnableDangle` (+0x854 bit 2), walkable top, trace channel Pawn | II.6 |
| 15 | Slope Slide | floor with `SQEX_bEnableSlopeSlide` (≤ −25° fall line; forced variant −15°), 120 cm clear | II.6 |
| 16 | Sway | another character pawn within the 50° cone while running, not in battle | II.6: 0.39 s (stick released) / 0.9 s |
| 17 | Hopping | hop-point markers (registry; `m_IdleMotionType`) | II.5 |
| 18 | Reflect, Risk Dodge | damage results 3 / 4 or `m_ReflectLevel` ≥ 3; a risk-dodge offer from the attacker (`DamageCaution`) with its type | II.1 |

**Tier 3**

| # | State | Affordance | Note |
|---|---|---|---|
| 19 | Swim | water volume (`bWaterVolume`) with a surface height query; restriction bit 13 and the dive clips for diving | II.6; motion model partly [C] |
| 20 | Floating | `BP_NotifySetFloatingMode(IsOn, IsSpecial)` | II.2 |
| 21 | Climbing (vault) | host switch `m_bEnableClimbing` + `SQEX_bEnableAutoRun` surfaces, 44–225 cm | dead in retail: implement last, behind the switch |
| 22 | Interaction stubs, Cinematic lock | host pushes | Part I section 13 |

---

## II.1 Dodge Roll, Guard, Reflect, Risk Dodge

(strand `defence`; working notes in `build/sora_tmp/actions/p2_defence/`)

Strand `defence`, pass 2. Everything below was read in the machine code (`build/sora_tmp/mv/dis.js`;
listings kept as `build/sora_tmp/actions/p2_defence/d_<ADDRESS>.txt`) unless tagged otherwise.
Tags as in Part I: **[I]** instruction level (address of the instruction given), **[C]** decompiled-C /
skimmed listing only, **[A]** asset, **[D]** derived, **[?]** not determined.
Conventions: `comiss a,b; ja` = a > b, `jae` = a ≥ b, `jb` = a < b, `jbe` = a ≤ b. Frame order
C (controller) → M (movement: pose tick with notifies, then move) → S (state `MainProc`).
"Queued" = `PushState` behind the running state (the running state decides when to `DoChain`);
"replaced" = `SetState` (queue emptied including the running state: End / Begin at the next S).

Shared helpers [I]:

| Function | Does |
|---|---|
| `SetReaction(pawn, type, name)` 0x1405B8B00 | pawn+0x619 = type (skipped when equal), then `MyBodyColl` (pawn+0x590) 0x140645C30: for every primitive of `m_BodyColls` (+0x418 / count +0x420) and `m_IBodyPrims` (+0x428 / +0x430) whose interface (primitive+0x868, vtable 0x145D78528) `Match(name)` (+0x30) is true: `Set(type)` (+0x40) |
| `RestoreReaction(pawn, name)` 0x1405B60D0 | pawn+0x619 = pawn+0x618 (`m_DefaultBodyCollReactionType`); 0x1406454C0: matching primitives `Reset()` (+0x48) |
| primitive `Match(name)` 0x140643EE0 | name == None: `!MyIgnoreNameNoneGroup` (primitive+0x878 bit 0); else name == `MyBodyName` (primitive+0x870) |
| primitive `Set(type)` 0x140645D90 / `Reset` 0x140645620 | unless `m_bFixedBodyReactionType` (primitive+0x898 bit 0): `m_BodyReactionType` (primitive+0x894) = type / = `m_DefaultBodyReactionType` (primitive+0x8C8) |
| `NextQueued(iface)` 0x1405AB6C0 / id 0x1405AB6E0 | the state queued behind the running one (null / 0 if none) |
| `DropQueued` 0x1405A7FF0, `ClearQueue` 0x1405A8010 | remove the queued state(s); the running state stays |
| `CanAct` pawn vt+0x29C0 0x141427690 | `!(pawn+0x868 & 2)`, 0x1405A9DD0() ∉ {0x0C, 0x11}, next id ∉ {61, 62}, current id ∉ {61, 62} |

Pawn bytes for Sora [I]: character kind pawn+0x5F0 (vt+0xD70) = **1** (ctor 0x1413E761B); pawn+0x15B4
(vt+0xD80) = **0** (ctor 0x1413E7622). Branches for kinds 0x0B / 0x10 / 0x11 and character kinds
2, 3, 6, 7, 10 are other characters and are named, not specified.

---

#### Dodge Roll

State `TresPlayerState_DodgeRoll`, id 16, vtable 0x14691B7E0, interface vtable 0x14691BB68
(`TresEnableChange` default 0x1411FBA40, `TresEnableInput` default 0x1415124D0, `TresTiming`
0x14137CD80).

| Field | Type | Meaning |
|---|---|---|
| +0x88 | u32 | flags: bit 0 clip time ≥ 0.4, bit 1 clip time ≥ 0.1, bit 2 snap to +0x8C at Begin, bit 3 reaction type 11 is set, bit 4 `MoleDodge` (weapon kinds 6 / 0x15 in battle) |
| +0x8C | f32 | yaw to roll toward (written by the push when the stick magnitude > 0.5) |
| +0x90 | i32 | effect group of the clip (0 for the Keyblade) |
| +0x94 | i32 | argument of anim-interface +0x1F0 after the play (0) |

Bits 0 and 1 have no reader inside the state (readers elsewhere: not searched [?]).

**Gate** `CanDodge` (interface slot 57) 0x141427CC0 [I]

```
CanAct                                   0x141427CEA
!pawn.vt+0x1F68()   (pawn+0xE60 bit 8, or vt+0x1FE8(8))            0x141427CFE
!pawn.vt+0x2770()   (word pawn+0xE62 bit 0)                          0x141427D12
controller (pawn vt+0x948) mode vt+0x1A70() != 0x8B                  0x141427D54
weapon kind k = pawn.vt+0x12E0(-1)
  battle flag (vt+0x1740) set:   k ∈ {1,4,8,11,12,18,19}: HasAbility(22 DODGE) 0x141427DA5
                                 k ∈ {6,21,22,23}: free;  other k: false           (table 0x141428424 / 0x141428418)
  battle flag clear:             k ∈ {7,9,10}: false;  every other k: HasAbility(22)   0x141427DD2
next queued id ∈ {0, 12,13,14, 75..88, 90, 156,157,159,160,161}      (table 0x141428448 / 0x14142843C)
current state id (table 0x141428528 / 0x1414284EC):
  1 IDLE, 2 RUN          : pass
  3 JUMP                 : OnGround && (Jump+0xF8 == 0)                                0x141427E34..E8A
  12-14, 75-88, 90, 156, 157, 159-161 (attack states): OnGround && (Attack+0x234 & 0x80) 0x141428190
  15 GUARD               : OnGround && Guard+0xDC != 0 && (Guard+0x84 & 0x20)          0x141427FEA / FF7
  16 DODGEROLL           : pawn.vt+0xD80() == 0x11 && controller+0xCC8 == 0   -> never for Sora
  18 FIREMAGIC           : OnGround && (+0x84 & 1) && !(+0xD8 & 0x50) && !pawn.vt+0x2748()
  19 DAMAGE              : OnGround && Damage+0x90 == 0 && type(+0x88): 1,2,9 -> (+0x84 & 1); 19 -> sub-step == 4; else false
  20 AIR_SLIDE           : !(+0xA8 & 0x80) && (+0x84 & 1)
  74 treasure            : state.vt+0x388() && OnGround
  97                     : (state+0x88 object).vt+8()
  111 USE_ITEM           : (+0x84 & 1) && OnGround
  122 stun               : (+0x84 & 1)
  142 COVER              : OnGround && +0xA0 == 0 && !(+0x84 & 1) && sub-step != 10
  162                    : +0xA1 != 0
  every other id         : false
!pawn.vt+0x2788()   (pawn+0xE60 bit 17)                              0x141428402
```

**Request** `TryDodge(yaw, mag, bKeep)` (slot 58) 0x14143AAA0 [I]

```
if !CanDodge(): return false                                         vt+0x1C8, 0x14143AAD8
switch current state id (table 0x14143AC94 / 0x14143AC80):
  1, 2, 74, 97, 111                : bKeep as passed
  12-15, 18-20, 75-88, 90, 122, 142, 156, 157, 159-161 : bKeep = 0
  16                               : only pawn kind 0x11
  162                              : state helper 0x1413DD170
  EVERY OTHER ID, INCLUDING 3 JUMP : return false                    0x14143AC5F
if next queued id is an attack id (table 0x14143AD40): DropQueued    0x14143ABF9
if !bKeep: ClearQueue                                                0x14143AC06
Push(yaw, mag)                                                       vt+0x1D8 (slot 59)
(pawn kind 0x0B: tutorial "Tutorial_DodgeRoll")
return true
```

`Push` (slot 59) 0x141444A50 [I]: `PushState(16)`; `comiss mag,[0.5]; jbe skip` (0x141444AAE) — when
mag > 0.5: flags |= 4, +0x8C = yaw.

The gate's JUMP branch is dead for this request: `TryDodge` returns false while the Jump state is
current, so **no roll starts during a landing clip**; it starts from Idle / Run after the Jump state
has chained.

**Reset** 0x14136E4A0 [I]: base 0x14150DF80; `flags &= ~0x1B` (0x14136E4AE: bits 0, 1, 3, 4; bit 2
survives but `PushState` resets a fresh instance before the push sets it); qword +0x90 = 0.

**Begin** 0x141368880 [I]

```
pawn.StopMovement()                      vt+0x16E8                    0x1413688E6
if pawn.vt+0x1740() (battle flag):
    if kind ∈ {6, 0x15}: flags |= 0x10; pawn.vt+0x19E0(1)
    Draw(1, 1, 1.0)                      vt+0x1340                    0x141368959
else: Sheathe(1, 1.0)                    vt+0x1358 (no-op for kind 1) 0x14136896E
if flags & 4: Turn(rot(0, +0x8C, 0), 1.0)      vt+0xDB0               0x1413689AB
SetReaction(pawn, 11, None)                                           0x1413689BF
flags |= 8                                                            0x1413689CD
+0x90 = 0x7DA (kind 0x12) | 0x76C (kind 0x15) | 0x834 / 0x898 (controller mode 0xDD / 0xDE) | unchanged (0)
```

**Tick** 0x141373EE0 [I]

```
a = dt * 12 (0x141373F3D); pawn.vt+0xDC8(a); pawn.vt+0xDB8(a)
sub 0:
    name = (flags & 0x10) ? "MoleDodge" : pawn.vt+0x1330() (weapon out) ? "DodgeRollB" : "DodgeRoll"
    len = Play(name, slot 0, blendIn -1.0, mode 2, group +0x90, rate 1.0, rootScale (1,1))   0x14137419E / 0x141374204
    "DodgeRollB" with len <= 0 (comiss len,0; ja ok): Play("DodgeRoll", same arguments)
    if len > 0: anim.vt+0x1F0(+0x94)
    sub = 1; flags &= ~4
sub 1:
    if IsAnimEnd(0): DoChain; flags &= ~4; return                     0x141373F89 / 0x141374067
    t = anim.vt+0xE8(0)
    comiss t,[0.1]; jb skip   -> flags |= 2                           0x141373FB0
    comiss t,[0.4]; jb skip   -> flags |= 1                           0x141373FC2
    if !(state+0x84 & 1): flags &= ~4; return                         0x141373FDA
    if flags & 8: RestoreReaction(pawn, None); flags &= ~8            0x141373FFF
    if NextQueued != null:
        if battle flag: pawn.vt+0x1780(1)          (last pose = BATTLE)
        DoChain; return                            (bit 2 is not cleared on this path)
    if !OnGround(pawn): DoChain                                       0x141374054
    flags &= ~4
```

**End** 0x14136B8B0 [I]: `RestoreReaction(pawn, None)`; if flags & 0x10: pawn.vt+0x19E0(0).
`TresTiming` handler 0x14137CD80 [I]: any parameter: if flags & 8: restore, clear bit 3 (clip 0051
has no `TresTiming`).

Clip `DodgeRoll` = `DodgeRollB` = `rt/body/0051` [A]: 0.7 s, 22 frames at 30 Hz, root motion forward
600 cm (frames: 0, 45.57, 109.75, 173.53, 234.27, 289.1, 337.02, 378.34, 416.05, 449.77, 479.12,
504.49, 526.64, 545.72, 561.87, 575.24, 585.98, 593.83, 598.47, 600, 600, 600), `m_InterpolationTime`
0.07; notifies: voice 0, SE `se00001_022` 0.0243, effect `vs_spd_lin1` 0.1435 (0.0333 s), `TresIK`
0.4339–0.7, `TresEnableChange` 0.4659, `TresLastAnimPose` 0.69.

Invincibility: reaction type 11 on every primitive without `MyIgnoreNameNoneGroup` (the two `Body`
primitives and the camera primitives; not `Guard` / `GuardBody`) from Begin (S of the request frame N)
until the Tick of the first S after the notify: at 60 Hz the notify fires in M(N+28), restored in
S(N+28). Hits resolved between S(N) and S(N+28) are dropped (see "Body-collision reaction types").

Exits: (1) queued state at `m_bEnableChange` (0.4659 s); (2) airborne at / after `m_bEnableChange` →
`DoChain` → Idle → fall; (3) clip end 0.7 s → `DoChain` → Idle. Requests accepted while rolling
(gates, Part I): jump (`m_bEnableChange`, nothing queued), guard and air slide (`m_bEnableChange`),
attack (combat spec 3.2: `m_bEnableChange`); a second roll never.

---

#### Guard

State `TresPlayerState_Guard`, id 15, vtable 0x14692E800, interface vtable 0x14692EBD8
(`TresEnableChange` 0x14139A4E0, `TresTiming` 0x14139AD70, +0x350 0x14139A360 = other characters).
Own slots: 113 0x14139FE30, 114 0x1413513D0 (damage common parameters), 115 0x14138E820 (push-back),
116 0x1413A0650 (collision parts), 117 0x14139C310 (Keyblade tick), 118 / 119 form ticks, 120–122 other
characters.

| Field | Type | Meaning |
|---|---|---|
| +0x90 | delegate | per-kind tick (Keyblade: slot 117) |
| +0xD0 | u8 | cleared every tick |
| +0xDC | u8 | entered by a guarded hit |
| +0xDE | u8 | "button held this frame" (set by slot 56, cleared every tick; no reader in slot 117) |
| +0xE0 | f32 | countdown (no reader in slot 117) |
| +0xE8 | u32 | flags: bit 0 player request, bit 1 success (guarded hit of a player guard), bit 2 guard parts on, bit 3 push-back active, bit 4 airborne, bit 5 (other character), bit 6 from Swim, bit 7 debug, bits 9–11 tick variant, bit 12 push-back scale applied, bit 13 automatic guard |
| +0xF4 | f32 | turn rate (Reset 0.2) |
| +0xF8 | f32 | turn timer (Reset 0.2) |
| +0xFC | f32 | push-back speed |
| +0x100 | f32 | push-back decay per 1/60 s (Reset 0.95) |
| +0x10C | i32 | hit strength 1 / 2 |
| +0x110 | ptr | `m_pAttackPawn` |
| +0x120 | ptr | `m_pTargetComponent` (lock-on target at Begin) |
| +0x128 | vec3 | look-at point |
| +0x134 | vec3 | push-back direction |
| +0x140 / +0x14C | vec3 | hit direction as passed / flattened and normalised |
| +0x158 | 0x68 bytes | copy of the `TresDamageInfo` (+0x168 = `m_DamagePowerScale`, +0x174 = `m_DamageBrakeParam`) |
| +0x1C0 | u8 | `m_DamageKind` of the guarded hit |
| +0x1C4 / +0x1C8 | weak ptr | object notified in Begin (0x140F6F670) and End (0x140F74110) [?] |

**Gate** `CanGuard` (slot 54) 0x1414294C0 [I]

```
!pawn.vt+0x1F68()  (restrict bit 8)                                  0x1414294E4
!pawn.vt+0x1ED8()  (restrict bit 15)                                 0x1414294F8
CanAct                                                               0x14142950C
!pawn.vt+0x1E30()  (air guard blocked: pawn+0xE3C > 0 || !(pawn+0xE48 bit 2))   0x141429520
player data (pawn.vt+0x8C0()) null or its byte +0x84C == 0           0x14142953F
next queued id ∈ {0, 12,13,14, 15, 75..90, 156,157,159,160,161}      (table 0x141429B14)
k = weapon kind; controller present, mode != 0x8B, k == 1: HasAbility(3 REFLECT_GUARD)   0x1414295C2
k ∈ {2,4,5,7,10,12,18}: false                                        (table 0x141429BC0)
current state id (table 0x141429C10 / 0x141429BD4):
  1 IDLE, 2 RUN, 4 GLIDE, 5 FLOATING : pass
  3 JUMP        : Jump+0xF8 == 0
  12-14, 75-90, 156, 157, 159-161, 171 : Attack+0x234 & 0x80
  15 GUARD      : Guard+0xDC != 0 && (Guard+0x84 & 0x20)
  16 DODGEROLL, 20 AIR_SLIDE : state+0x84 & 1
  18 FIREMAGIC  : !(+0xD8 & 0x10) && (+0x84 & 0x20) && !(+0xD8 & 0x40) && (+0x84 & 1)
  19 DAMAGE     : +0x90 == 0 && type ∈ {1, 2, 9} && (+0x84 & 1)      (table 0x141429CBC)
  74            : state.vt+0x388() && OnGround
  102 SWIM      : pawn.vt+0x1C30() && iface.vt+0x700()
  111 USE_ITEM  : (+0x84 & 1) && OnGround            (character kind 1)
  122 stun      : +0x84 & 1
  97, 142, 162  : as in the dodge gate
  other         : false
!pawn.vt+0x2788()                                                    0x141429AF1
```

**Request** `TryGuard` (slot 55) 0x14143C6D0 [I]: gate; current id ∈ {1–5, 12–16, 18–20, 74–90, 97,
111, 122, 142, 156–161, 171} → swim = 0; 102 → swim = 0x40; other → false (table 0x14143C8C8). If
controller+0x878 bit 8: `FUN_1414142D0(ctl, 0)`, clear bit 8. Queued attack → `DropQueued`.
`SetState(15)`; `flags = (flags & ~0x40) | swim | 1` (0x14143C887). `GuardHeld` (slot 56)
0x14143CA20: Guard+0xDE = 1.

**Reset** 0x14138F510 [I]: flags bits 0–13 cleared (three `and`: 0xFFFFE280, 0xFFFFFF7F, 0xFFFFDDFF);
+0xD0, +0xD4..+0xDB, +0xDC..+0xDF = 0; +0x110, +0x118, +0x120 = 0; +0xF4 = 0.2, +0xF8 = 0.2
(0x3E4CCCCD); +0x128, +0x134, +0x140, +0x14C = zero vector; +0xFC = 0; +0x100 = 0.95 (0x3F733333),
+0x104 = 0; +0x108, +0x10C = 0; +0x1C0 = 0; +0xE0, +0xE4, +0xEC, +0xF0 = 0.

**Begin** 0x1413851D0 [I]

```
pawn.vt+0x1780(1)                         last pose = BATTLE          0x141385284
Draw(1, 1, 1.0)                                                       0x1413852A1
pawn.StopMovement()                                                   0x1413852AE
if !OnGround(pawn): flags |= 0x10; pawn.vt+0x1E78(0)   (pawn+0xE48 bit 2 = 0)      0x1413852C7
SetReaction(pawn, 3, "Body")                                          0x1413852F3
+0x128 = rootPosition + forward(pawn.vt+0xE60) * 100                  0x141385309
controller = pawn.vt+0x938(); not a player controller: return
if +0x110 (attacker) valid:   if flags & 2: +0x128 = attacker.vt+0x6E0()          0x1413854B7
else: +0x120 = controller.vt+0x1690(); if valid: +0x128 = +0x120.vt+0xB88()       0x141385521
tick variant (character kind 1, controller mode != 0x8B):
    weapon kind 11, 19 -> slot 118;  8, 9, 16, 17, 20-23 -> slot 119;  24 -> slot 119, flags |= 0xC00   (table 0x141385898)
    every other kind (Keyblade 1):                                    0x1413856EE
        if +0xDC == 0: RestoreReaction(pawn, "Body"); slot116(1)      0x141385711 / 0x141385720
        delegate = slot 117; flags |= 0xC00
+0x10C = (+0x1C0 ∈ {2,3,4,5,7,8,9,10}) ? 2 : 1                        (table 0x1413858DC)
if flags & 0x20: flags |= 0x10
if flags & 0x10: pawn.vt+0x1728(0)        gravity off                 0x1413857AA
weak object +0x1C4 valid: 0x140F6F670(object, pawn)
if +0xDC == 0: FUN_141414B70(controller)  (just-guard timer, below)   0x14138586B
else:          pawn.vt+0x1E28(0.0)        (pawn+0xE3C = 0)            0x14138587B
```

So the reaction type GUARD (3) is put on the `Body` primitives only for a **hit entry** (+0xDC set by
`OnGuardedHit`); a player-requested guard takes it back at once and switches parts instead.

**Slot 116** `SetGuardParts(bOn)` 0x1413A0650 [I]: for every primitive of `MyBodyColl.m_BodyColls`:
name "Body" → primitive.vt+0xA38(!bOn); "GuardBody" → vt+0xA38(bOn); "Guard" → vt+0xA38(bOn);
`flags bit 2 = bOn`.

**Tick** 0x141392080 [I]

```
a = dt * 12 (0x1413920F7); pawn.vt+0xDC8(a); pawn.vt+0xDB8(a)
if +0xE0 > 0: +0xE0 -= dt; if +0xE0 < 0: +0xE0 = 0
delegate(dt)                                                 slot 117
if +0xF8 > 0   (comiss 0,[+0xF8]; jae skip, 0x14139219F):
    d = (+0x128 - rootPosition).xy; if |d|^2 > 1e-8: normalise
    Turn(rot(0, yaw(d), 0), dt * +0xF4 * 60)                 0x1413922EC / 0x141392319
    +0xF8 -= dt; if +0xF8 < 0: +0xF8 = 0
if flags & 8:
    +0xFC = pow(+0x100, dt * 60) * +0xFC                     0x141392374
    pawn.vt+0x16C0(+0xFC * +0x134)                           component vt+0x9F8, 0x1413923C1
flags &= ~0x80; +0xD0 = 0; +0xDE = 0
```

**Slot 117** 0x14139C310 [I]

```
sub 0, +0xDC == 0 (player request):                                   0x14139CA6B
    name = (flags & 0x40) ? "AirGuard" with group 0x209 : (flags & 0x10) ? "AirGuard" : "Guard"   (group 0)
    len = Play(name, 0, -1.0, mode 2, group, 1.0, (1,1)); if len > 0: anim.vt+0x1F0(0)
    +0xF4 = 1.0; sub = 1; return                                      0x14139CB16
sub 0, +0xDC != 0 (hit entry):                                        0x14139C6D8
    name = (flags & 0x50) ? "AirGuardHit" : "GuardHit"; group = (flags & 0x40) ? 0x209 : 0
    (flags & 2 and weapon kind 0x0B: "GuardChange" / "GuardChange2" form request)
    Play(name, 0, -1.0, mode 2, group, 1.0, (1,1)); sub = 2
    if flags & 2: +0xF4 = 1.0                                         0x14139C803
    else:         +0xF4 = 0.05; +0xF8 = 0.05                          0x14139C809 / 0x14139C811
    (kind 0x0B: return)
    if flags & 2: +0x134 = (+0x14C != 0) ? +0x14C : -forward2D(pawn)  0x14139C84A..0x14139CA27
    slot115(); sub = 3; return                                        0x14139CA33
sub 1: if +0xDC != 0 (same instance re-entered; not reached through OnGuardedHit, which replaces the state):
           sub = 2; attacker valid: +0x128 = attacker position, +0xF8 = 0.2; +0x134 as above; slot115(); sub = 3
           (the clip name is built but no Play is called)             0x14139C355..0x14139C63B
sub 2: slot115(); sub = 3
sub >= 1, every tick:                                                 0x14139C65B
    if IsAnimEnd(0): DoChain
    else switch NextQueuedId:
        3 JUMP                              : DoChain only if state+0x84 & 1     0x14139C69D
        16 DODGEROLL, 20 AIR_SLIDE, 102 SWIM: DoChain                            0x14139C689..0x14139C696
        other                               : nothing
```

**`TresTiming(param)`** 0x14139AD70 [I]: `test r8d,r8d; jne return` (0x14139ADB3): only param 0:
`RestoreReaction(pawn, "Body")`; if flags & 4: slot116(0).

**`TresEnableChange`** 0x14139A4E0 [I]: `RestoreReaction(pawn, "Body")`; if flags & 4: slot116(0);
if !pawn.vt+0x1A80() (in water): pawn.vt+0x1728(1) (gravity on), `ResetGravityScale` (vt+0x1658);
pawn.vt+0x16D8(0); if !OnGround and +0xDC == 0: pawn+0xE3C = pawn+0xC70 (`MyAirGuardCoolDownTime`
0.3333, 0x14139A609). **The handler does not set `m_bEnableChange`** (no write to interface+0x44; the
notify 0x14055F920 reaches only this handler through 0x1405B2C40). So Guard+0x84 bit 0 stays 0 for
the whole state: the JUMP branch of slot 117 never passes, and a jump that is queued BEHIND the Guard
would start when the clip ends.

> COORDINATOR DECISION 2026-10-11: `JumpRequest` 0x14143D8F0 (cases 15 / 16) calls `ClearQueue` 0x1405A8010 before `RequestJump`; `ClearQueue` empties the WHOLE queue, the running Guard's entry included, so `NeedChange` replaces the Guard at the next `MainProc`: a jump accepted out of a guarded hit (from `TresEnableInput`) **starts at once**. The slot-117 JUMP branch (queued jump waits for `m_bEnableChange`, never set) is then dead for this path. Probe request G-J1.

**End** 0x14138CA10 [I]

```
in water (pawn.vt+0x1A80) ? SetMovementMode(4, 0) : gravity on (vt+0x1728(1))        0x14138CADA / 0x14138CAE4
ResetGravityScale; pawn.vt+0x16D8(0); pawn.vt+0x1408(); pawn.vt+0x22E8(); ResetMaxWalkSpeed (vt+0x1578)
if flags & 0x10: if !in water: SetMovementMode(3 Falling, 0)
                 if +0xDC == 0: pawn+0xE3C = pawn+0xC70                               0x14138CB6A
controller valid, next = NextQueuedId != 0, not (next == 15 and that Guard has +0xDC != 0),
    controller+0x8A0 > 0 (comiss 0,[+0x8A0]; jae skip, 0x14138CC39), weapon kind ∉ {0x0B, 0x13}:
        FUN_1414145C0(controller, 0.0, null, 0, 0); FUN_1414142D0(controller, 0)    (counter window closed)
RestoreReaction(pawn, "Body")
if flags & 4: "Body" on, "GuardBody" off, "Guard" off; flags &= ~4                   0x14138CD24..0x14138CDAD
weak object +0x1C4 valid: 0x140F74110(object, pawn)
```

Clips [A] (no root motion): `Guard` 1281 0.8667 s — `TresTiming(1)` 0, voice 0, swing SE `SWING2`
(volume 0.7) 0.0333, effect `vs_com_grd01` 0.0339, `TresTiming(0)` **0.6**, `TresEnableChange` 0.6333.
`AirGuard` 1283 0.8667 s — same plus `TresLandAnimPose` 0, effects `vp_div_flo1` / `vp_rvj_ksk0`
(group 521). `GuardHit` 1282 0.9667 s — effects `vs_com_grd00`, `vs_com_grd_smk00` at 0,
`TresEnableInput` 0.2667, `TresEnableChange` 0.7. `AirGuardHit` 1284 0.9667 s (blend 0.13) —
`TresLandAnimPose` 0, `TresEnableInput` 0.2667, `TresEnableChange` 0.6667.

Timeline of a player guard requested at C(N), 60 Hz: S(N) Idle End, Guard Begin (parts switched),
Play; guard parts on until M(N+36) (`TresTiming(0)`); `TresEnableChange` M(N+38) (air: gravity back,
cooldown 0.3333 s); clip end → S(N+52) `DoChain`. Air guard: gravity off and velocity zero from S(N)
to M(N+38). No exit before the clip end except a queued dodge / air slide / swim — whose gates require
a guarded hit — and replacement by `SetState` (attack out of guard, guarded hit, damage).

**Guarded hit** `OnGuardedHit(info, _, causer, hitDir)` (slot 142) 0x14143CAF0 [I]; called by the
victim side of `TakeDamage` 0x1405BB850 for hit result 5 (0x1405BC004).

Gate (slot 141) 0x141429E70: `CanAct`; next queued none or 15; current 12 (ATTACK): Attack+0x23C & 2;
current 15: pass; other states: the automatic-guard test of the damage interface (pawn+0x460,
vt+0x2D0 0x141427240: option flag 2 of the player data through 0x140EAB160, `!pawn.vt+0x2070()`,
`CanGuard`); `!pawn.vt+0x2788()`.

```
attackerPawn = causer if it is a character pawn, else its owner (+0x588 for class 0x142C52900) / instigator (+0xB8)
(controller mode 0x8B: its own branch)
if character kind 1 && weapon kind ∈ {1, 3, 6, 14, 15} && (i8) info+0x63 >= 3                   0x14143CDEE (cmp byte [info+63h],3; jl skip)
   && iface.vt+0x450(autoGuard):                              (counter-allowed gate 0x141429CF0)
       st = SetState(66 REFLECT); st+0xB8 = causer; st+0x98 = 400.0 (0x14143CE75); st+0x88 |= 9
       st+0xA0 = hitDir; st+0xAC = hitDir flattened and normalised
       if controller.vt+0x1CB0(attackerPawn): st+0x88 |= 4     (risk dodge offered, see Risk Dodge)
       return
success = 0; critical = 0; old = none
B = damageInterface.vt+0x2C0()                                (automatic guard option)
current 12 ATTACK:  B = 0; if Attack+0x23C & 2: success = 1; critical = (controller+0x89C > 0)
current 15 GUARD:   B = 0; if (Guard+0xE8 & 1) && Guard+0xDC == 0: success = 1; critical = (controller+0x89C > 0)   0x14143D0D3
                    else: old = (Guard+0xFC, +0x134, +0x128); critical = controller+0x878 bit 8
other:              success = B
if success-or-repeat as above, !pawn.vt+0x1C30() (not in water) and iface.vt+0x450(...):
       FUN_1414145C0(controller, 1.0, attackerPawn, success, critical)                        0x14143D1C9 / 0x14143D255 / 0x14143D3AE
st = SetState(15)
st+0xE8 bit 1 = success; st+0x110 = attackerPawn; st+0xDC = 1; st+0x84 &= ~0x20                0x14143D41B..0x14143D43F
st+0x158 = *info; st+0x1C0 = info+0x0A; st+0xE8 bit 13 = B
if old.speed > 0: st+0xFC, +0x134, +0x128 = old            (push-back carried over consecutive hits)
st+0x140 = hitDir; st+0x14C = hitDir flattened and normalised (zero if |xy|^2 < 1e-8)
if HasAbility(74 GUARD_REGENE): pawn.vt+0x1878(0, 3, 0, 0, 0)                                  0x14143D5EA
```

`info+0x63` is the low byte of the attack row's `m_ReflectLevel` (row +0x54 → primitive+0xA2B,
0x140645B31), `info+0x62` its `m_ReflectReaction`, `info+0x0A` `m_DamageKind`.

After the replacement: Guard Begin puts reaction type 3 on both `Body` primitives (all directions)
until `TresEnableChange` of the hit clip (0.7 s; air 0.6667 s) or End. Requests out of the hit clip
from `TresEnableInput` (M(H+16), first accepted at C(H+17)): dodge roll (on ground), air slide, guard,
jump (starts at once: coordinator decision above), attack (combat spec 3.2 / 3.3, `SetState`).

**Push-back** (slot 115) 0x14138E820 [I]

```
+0x100 = info.m_DamageBrakeParam (+0x174); if not > 0: 0.95             0x14138E8D3 / 0x14138E8E5
p = slot114()   (TresDamageCommonParam default object)
base = +0x10C == 2 ? (p ? p+0xF0 : 1000) : +0x10C == 3 ? (p ? p+0xF4 : 3000) : (p ? p+0xEC : 300)
s = info.m_DamagePowerScale (+0x168)
if pawn+0xD84 > 0 (vt+0x2650):
    if s != 1 && !(flags & 0x1000): if flags & 8: s *= 0.75;  flags |= 0x1000;  base *= s
    +0xFC = max(+0xFC, base)
else:
    +0xFC = max(+0xFC, base); if s != 1: +0xFC *= s
flags |= 8
```

`TresDamageCommonParam`: `m_GuardHitBackPowerLv1 / Lv2 / Lv3` at +0xEC / +0xF0 / +0xF4; native
constructor 0x14064C130: **300 / 1000 / 3000** (0x14064C299, 0x14064C2A3, 0x14064C2AD);
`Blueprints/Player/PlayerDamageCommonParam` (and `Blueprints/Npc/FriendDamageCommonParam`) do not
override them [A]. Begin only produces strengths 1 and 2, so Lv3 has no user in the Keyblade guard.
An unsuccessful hit entry (flags bit 1 clear) leaves the direction zero: the velocity write is then
zero each tick (Sora is held in place).

**Guard direction.** No angle is tested in the state, the gates or `OnGuardedHit`. The front / back
difference comes from the two parts [A] (`p_ex001_PawnBase`, attached to the mesh, whose
`RelativeRotation` yaw is 270): `Guard` — capsule radius 51, half height 91, at mesh (0, 2, 90) = **2 cm
forward** of the actor axis, reaction type GUARD (3); `GuardBody` — capsule radius 50, half height 90,
at (0, 0, 90), reaction type WEAK_GUARD (4). `Guard` is the outer surface where `1 + 2·cos θ > 0`, i.e.
within **120° of the facing** on either side; in the rear 120° wedge `GuardBody` is outside [D]. A
WEAK_GUARD part guards only attacks whose row has `m_bIsEnableWeakGuardAttack`. Which primitive the
attack sweep reports when both overlap was not read [?].

**Counter window and just guard** (controller; consumers in the combat spec) [I]

- `FUN_141414B70(controller)` (Guard Begin, player request): if `AbilityCount(104 CRITICAL_COUNTER)`
  != 0 (pawn vt+0x1500), movement component vt+0x540() false (0x1405AD820), not (character kind 1 and
  mode 0x8B): **controller+0x89C = pawn+0xCD4** (native 0.13333, 0x141423C49); controller+0x878 &=
  ~0x100. The countdown of +0x89C was not located [?].
- `FUN_1414145C0(controller, time, attackerPawn, bSuccess, bCritical)`: returns if controller+0x920
  != 0. time > 0 needs `HasAbility(4 GUARD_COUNTER)`, or for character kind 1 `HasAbility(5
  REVENGEIMPACT)` / `HasAbility(6 REVENGEDIVE)` (form modes 0x86–0xE0 by table), controller+0xA10 ==
  0, and attacker null or 0x1413F0040(controller, attacker); otherwise it closes.
  Open: controller+0x8A4 (command id) = 3, or 5 with ability 6, else 4 with ability 5 (form modes:
  0x0E, 0x0F, 0xD7–0xD9, 0x10–0x13 with time 2.0); **controller+0x8A0 = time (1.0)**; +0x8A8 = weak
  pointer to the attacker; if bCritical and `AbilityCount(104) > 0`: controller+0x878 |= 0x100 and
  `FUN_1414142D0(controller, 1)` (bit 9 = 1, pawn.vt+0x2840(1)); HUD 0x14158C180(hud, 0, id, 0);
  +0x898 = id.
  Close: +0x8A0 = 0, +0x8A4 = 0, +0x898 = 0, weak pointers cleared, +0x878 &= ~0x100, HUD
  0x14158C180(hud, 3, old id, 0).
- Counter-allowed gate (interface slot 138) 0x141429CF0(bAuto): current state ∈ {12–16, 19, 20, 75–85,
  87–90, 156, 157, 159–161} or (bAuto and the automatic-guard test); then pawn.vt+0x12C8(-1) ∈ {1, 2, 4,
  8..19} or controller.vt+0x1A78(mode) == 0.

---

#### Reflect

State `TresPlayerState_Reflect`, id 66, vtable 0x1469383D8, interface vtable 0x1464AA848 (all
default handlers). The weapon-recoil state: clips `AttackReflect` / `AttackAirReflect`.

| Field | Type | Meaning |
|---|---|---|
| +0x88 | u32 | flags: bit 0 push-back, bit 1 started airborne, bit 2 risk dodge offered, bit 3 entered from a guarded hit |
| +0x8C | vec3 | push-back direction |
| +0x98 | f32 | push-back speed |
| +0x9C | f32 | risk-dodge window timer |
| +0xA0 / +0xAC | vec3 | hit direction as passed / flattened and normalised |
| +0xB8 | ptr | `m_pTargetActor` (the causer) |

Entries [I]: (1) `OnGuardedHit` with `m_ReflectLevel` ≥ 3 (above): flags 9 (| 4), speed 400, direction
from the hit. (2) Interface slot 137 0x14143FE00(_, actor), called by the victim side of `TakeDamage`
for hit results 3 and 4 (0x1405BC059 / 0x1405BC079; result 4 first tries slot 139): if slot 136
(= slot 138 with bAuto 0): `SetState(66)`, flags |= 1, +0xB8 = actor, +0x98 = 400.0. Both replace the
running state.

**Reset** 0x1413AD220 [I]: flags &= ~0xF; +0x8C, +0xA0, +0xAC = zero vector; +0x98, +0x9C = 0; +0xB8 = 0.

**Begin** 0x1413A4FF0 [I]

```
controller = state controller (0x14150DAF0); none: return
pawn.vt+0x1780(1); pawn.StopMovement()                               0x1413A5093 / 0x1413A50A1
if !OnGround: Play("AttackAirReflect", 0, -1.0, mode 2, group 0, 1.0, (1,1)); SetMovementMode(3 Falling, 0); flags |= 2   0x1413A510E..0x1413A513E
else:         Play("AttackReflect", same arguments)                  0x1413A5196
if +0xB8 is of class 0x142C52900: +0xB8 = its owner (+0x588)
if flags & 1:
    +0x8C = (+0xAC == 0) ? -forward2D(pawn) : +0xAC                  0x1413A5250..0x1413A5434
    if +0x8C == 0: +0xB8 valid ? +0x8C = normalise2D(pawnPos - actorPos) : (flags &= ~1; +0x98 = 0)
FUN_1414145C0(controller, 0.0, null, 0, 0)                           (guard counter window closed)   0x1413A5692
if flags & 4: FUN_141414170(controller, 0, controller+0x89C > 0); +0x9C = 0.6 (0x3F19999A)           0x1413A56A9 / 0x1413A56AE
```

**Tick** 0x1413AE9A0 [I]

```
pawn.vt+0xDC8(dt * 12)
if (flags & 1) && +0x8C != 0: Turn(rotationOf(-+0x8C), dt * 30)       0x1413AEAA4 / 0x1413AEAB9   (faces against the push)
else: pawn.vt+0xDB8(dt * 12)
if IsAnimEnd(0): DoChain                                             0x1413AEAED / 0x1413AEB02
if flags & 1: +0x98 = pow(0.95, dt * 60) * +0x98; pawn.vt+0x16C0(+0x98 * +0x8C)        0x1413AEB12..0x1413AEB77
if flags & 4:
    +0x9C -= dt
    if controller.vt+0x1CB8() == null || controller.vt+0x1CC0() == 0: +0x9C = 0
    if +0x9C <= 0 (comiss; jbe, 0x1413AEBE1): FUN_141414170(controller, 1, 0); flags &= ~4
else if pawn.vt+0x1A80() (in water): if PushState(102 SWIM): DoChain
```

**End** 0x1413AB6F0 [I]: in water → `SetMovementMode(4, 0)`; else not on ground →
`SetMovementMode(3, 0)`; else if flags & 2 → `SetMovementMode(3, 1)`. If flags & 4: clear;
`FUN_141414170(controller, next id == 17 ? 3 : 1, 0)`.

`FUN_141414170(controller, mode, bCritical)` [I]: HUD 0x14158B260(hud, mode) (0 show the risk-dodge
prompt, 1 hide, 3 used); bCritical with `AbilityCount(104) > 0`: controller+0x878 |= 0x100,
`FUN_1414142D0(controller, 1)`; mode 1: `FUN_1414142D0(controller, 0)` and bit 8 cleared; mode other
than 0: bit 8 cleared.

Clips [A]: `AttackReflect` 1235 0.9667 s, no root motion, `TresPlayWeaponHitSE` HIT7 at 0, `TresIK`
0.0666–0.9667; `AttackAirReflect` 1236 0.6333 s, hit SE at 0, `TresLandAnimPose` 0. Neither has
`TresEnableChange` or `TresEnableInput`.

Exits: clip end (`DoChain` → Idle; airborne → fall), Risk Dodge (`SetState`), damage. No gate of
Part I lists REFLECT as a source state, so nothing else cancels it. No invincibility and no guard:
the reaction type is the default for the whole state. Push-back: speed 400 × 0.95 per 1/60 s along
+0x8C, written every tick through component vt+0x9F8.

---

#### Risk Dodge

State `TresPlayerState_RiskDodge`, id 17, vtable 0x14693A918, interface vtable 0x14693ACE8 (default
handlers; `TresTraction` begin / end = interface +0x258 / +0x260 set / clear state+0x84 bit 2).

| Field | Type | Meaning |
|---|---|---|
| +0x88 | i32 | `ETresRiskDodgeType`: 1 UNDERSLIDE, 2 OVERSLIDE, 3 SLIDETURN, 4 WARP |
| +0x8C | u8 | set in Begin |
| +0x90 | vec3 | start position |
| +0x9C | vec3 | working target point |
| +0xA8 | vec3 | destination / reference point |
| +0xB4 | f32 | distance or radius |
| +0xB8 | f32 | root-motion scale (Reset 1.0) |
| +0xBC / +0xC0 | f32 | orbit angle / its goal (SLIDETURN) |
| +0xC4 | f32 | warp delay |
| +0xC8 | u32 | bit 0 debug, bit 1 slow motion on, bit 2 counter command used |
| +0xD0 | ptr | `m_pAttackerPawn` |
| +0xD8 | u32 | bit 0 counter command offered, bit 1 critical (just guard) |
| +0xDC / +0xE0 | i32 | reaction-command id 7–10 / attack kind 0x28–0x2B (read by the combat `TryAttack`) |
| +0xF0 | delegate | per-type tick |

**Offer.** An enemy attack clip carries `TresAnimNotify_DamageCaution` with `DodgeType`
(`ETresRiskDodgeType`) [A: reflected property]; offers are kept in the controller (array +0xC50, count
+0xC58, 0x14 bytes each: type at +0, attacker weak pointer at +0x0C / +0x10). `controller.vt+0x1CB0(attackerPawn)`
0x1413ED1E0 [I], called by `OnGuardedHit` on the Reflect entry: needs `HasAbility(8 RISKDODGE)`, not
in water; picks the offer whose attacker is the guarded attacker (0x1405D01D0(attacker) true, type !=
0, type 1 only when Sora is on the ground); stores the attacker (controller+0xC40 / +0xC44) and the
type (+0xC48), empties the list, returns true. `vt+0x1CB8` = that attacker, `vt+0x1CC0` = that type.

**Gate** (slot 60) 0x14142CD20 [I]: `CanAct`; `!pawn.vt+0x2770()`; nothing queued; `HasAbility(8)`
(0x14142CD74); `!pawn.vt+0x2788()`; current state is Reflect with flags bit 2.

**Request** `TryRiskDodge(yaw, mag, target, type)` (slot 61) 0x14143FF10 [I] (controller: jump button
pressed, before the jump request; target = `vt+0x1CB8()`, type = `vt+0x1CC0()`): gate; target != null;
type != 0; if controller.vt+0x1680(): controller.vt+0x1688(); `ClearQueue`; `SetState(17)`;
`Setup(type, target, controller+0x878 bit 8)` (slot 114).

**Setup** (slot 114) 0x1413C44C0 [I]: +0x88 = type; +0xD0 = target; sub = 0; with `HasAbility(8)`:
(+0xDC, +0xE0) = type 2 → (8, 0x29), 3 → (9, 0x2A), 4 → (10, 0x2B), other → (7, 0x28); delegate = type
2 → slot 119, 3 → slot 120, 4 → slot 121, other → slot 118; +0x90 = pawn position;
`Turn(rot(0, yaw(target - pawn), 0), 1.0)`; +0xD8 bit 1 = critical.

**Reset** 0x1413AD2A0 [I]: +0xD0 = 0; +0x88 = 0; +0x8C = 0; +0xC4 = 0; +0x90, +0x9C, +0xA8 = zero;
+0xC8 &= ~7; +0xD8 &= ~3; +0xB4 = 0; +0xB8 = 1.0; +0xDC, +0xE0 = 0.

**Begin** 0x1413A5730 [I]

```
pawn.StopMovement()                                                  0x1413A5791
SetMaxWalkSpeed(3000)        vt+0x1570                               0x1413A579B / 0x1413A57A6
SetMaxAcceleration(180000)   vt+0x1640                               0x1413A57B0 / 0x1413A57BB
Sheathe(1, 1.0)                                                      0x1413A57D2
SetReaction(pawn, 11, None)                                          0x1413A57E6
pawn.vt+0x1010(0, None)      components of pawn+0x470: vt+0x6C8(0, None)   [? collision toggle]   0x1413A57FC
+0x8C = 1
controller: FUN_14140DF20(ctl, 0, 0, 0, 0); FUN_14140DF20(ctl, 1, 3, attacker.vt+0x2108(0), 0)    (camera request)
```

**Tick** 0x1413AED20 [I]: the delegate (through 0x141336430).

**End** 0x1413AB810 [I]: `ResetMaxWalkSpeed`; `RestoreReaction(pawn, None)`; pawn.vt+0x1010(1,
None); on ground ? `SetMovementMode(1, 1)` : `SetMovementMode(3, 0)`; gravity on; slot116(0);
slot115(0); `FUN_14140DF20(ctl, 0, 0, 0, 0)`; if type ∉ {1, 3} and pawn.vt+0x1360(): `Draw(1, 1, 1.0)`.

Helpers [I]: **slot 115** `OfferCounter(b)` 0x1413BD170 (no-op if +0xDC == 0): on →
0x141414350(controller, +0xDC, 2.0, 0), +0xD8 |= 1; off (if +0xD8 & 1) → 0x141414350(controller, 0,
0.0, 0), `FUN_1414142D0(controller, 0)`, +0xD8 &= ~1. **Slot 116** `SlowMotion(b)` 0x1413C31E0 (no-op
if +0xDC == 0): on → 0x14140E500(controller, 1), 0x1405DA4E0(pawn, 1, 1, 0.25, 0.05), +0xC8 |= 2; off
(if +0xC8 & 2) → 0x14140E500(controller, 0), 0x1405DA4E0(pawn, 1, 1, 1.0, 0.0). **Slot 117**
`CounterUsed` 0x1413BE760: if +0xD8 & 1: +0xC8 |= 4; 0x141414350(controller, +0xDC, 0.0, 0); +0xD8 &=
~1; slot116(0). **Slot 113** 0x1413ACA10: warp destination by two traces of pawn.vt+0x2750 [C].

**Type 1 UNDERSLIDE** — slot 118 0x1413BB660 [I]

```
sub 0:
    pawn.StopMovement()
    +0xB4 = 0.5 * |XY extent of the attacker's body bounds| (attacker root +0x468 .vt+0x7C8(), 0x143FC6110)
    comiss +0xB4,[300]; jae -> else 300   (0x1413BBC38);  comiss +0xB4,[1000]; jbe -> else 1000   (0x1413BBC54)
    +0xA8 = attackerPos - attackerRotation.rotate((+0xB4, 0, 0))     the point +0xB4 behind the attacker's back
    +0xB8 = |(+0xA8 - +0x90).xy| * 0.00125                           0x1413BBE0A
    Play("UnderSlide", 0, -1.0, mode 2, group 0, 1.0, rootScale (+0xB8, 1.0)); if len > 0: anim.vt+0x1F0(0)
    Turn(rot(0, yaw(+0xA8 - +0x90), 0), 1.0); sub = 1
sub 1:
    if state+0x84 & 1: pawn.vt+0x1010(1, None); sub = 3; return
    +0x9C = attackerPos
    a = normalise2D(attackerPos - pawnPos); b = normalise2D(+0xA8 - pawnPos)
    if a·b < 0  (comiss dot,0; jae skip, 0x1413BB9FE):               Sora has passed the attacker
        if +0xB8 > 1.0: 0x1405A1D30(pawn+0x440, 1.0, …)               [? root scale back to 1]
        slot116(1); slot115(1); sub = 2; 0x14140ECC0(controller)
sub 2:
    if state+0x84 & 1: if +0xC8 & 4: DoChain; else slot116(0), slot115(0), pawn.vt+0x1010(1, None), sub = 3
    if IsAnimEnd(0): DoChain
sub 3: if IsAnimEnd(0): DoChain
```

`UnderSlide` 0060 [A]: 1.0667 s, 33 frames, root motion forward **800 cm** (0, 58.75, 150, 315.27,
409.84, 482.84, 542.19, 587.57, 622.99, 652.44, 679.94, 704.84, 722.83, 732.96, 740.53, 750.83,
762.69, 771.73, 778.8, 785.55, 792.47, 797.84, 799.55, 800 …) and yaw 0 → 180° over frames 14–19;
`TresTraction` 0–0.2333, SE `se00001_011`, effects `vs_ddg_uds_blr00` 0.04, `vs_ddg_uds_dst00` 0.064,
**`TresEnableChange` 0.4667**, `TresIK` 0.6347–1.0667. The scale 0.00125 = 1 / 800: Sora ends exactly
at the point behind the attacker, turned to face it.

**Type 3 SLIDETURN** — slot 120 0x1413BA340 [I]

```
sub 0:
    pawn.StopMovement(); SetMovementMode(5 Flying, 0)                 0x1413BA91C / 0x1413BA930
    d = normalise2D(attackerPos - pawnPos); c = forward.x * d.y - forward.y * d.x
    if c < 0 (comiss c,0; jae other, 0x1413BAB4C): Play("SlideTurnR"); +0xBC = 120; +0xC0 = -24
    else:                                          Play("SlideTurnL"); +0xBC = -120; +0xC0 = 24
    state+0x84 |= 4
    +0xB4 = max(200, 0.5 * |XY extent of the attacker's bounds|)      0x1413BACD4
    +0xA8 = attackerPos + d * +0xB4; sub = 1
sub 1:
    if !(state+0x84 & 4): slot116(1); slot115(1); sub = 2                                   (TresTraction ended)
    else:
        yaw = yawOf(+0xA8 - attackerPos) + +0xBC
        +0x9C = attackerPos + direction(yaw) * +0xB4
        +0xBC moves toward +0xC0 by dt * 540, clamped                 0x1413BA648
        pawn.vt+0x1668(normalise2D(+0x9C - pawnPos) * dt * 240000, 0)        component vt+0x500, 0x1413BA798 / 0x1413BA7C3
    Turn(rot(0, yaw(attackerPos - pawnPos), 0), dt * 12)
sub 2:
    if state+0x84 & 1:
        if +0xC8 & 4: DoChain; return
        slot116(0); slot115(0); SetMovementMode(1, 1); pawn.StopMovement(); pawn.vt+0x1010(1, None); sub = 3
    else: velocity = pawn.vt+0x2B8() * pow(0.9, dt * 60); pawn.vt+0x16C0(velocity)          0x1413BA455
    if IsAnimEnd(0): DoChain
    Turn toward the attacker as above
sub 3: if IsAnimEnd(0): DoChain
```

`SlideTurnR` 0052 / `SlideTurnL` 0053 [A]: 0.6667 s, no root motion, `TresTraction` 0–0.3,
`TresEnableChange` 0.5, SE `se00001_005`, effects `vs_ddg_slt_blr00`, `vs_ddg_fup_dst00`. The cross
product is taken right after Setup snapped the facing to the attacker, so its sign is rounding noise
[D]; c == 0 gives `SlideTurnL`.

**Type 2 OVERSLIDE** — slot 119 0x1413B8280 [I for the flow, C for the steering arithmetic]:
sub 0: `StopMovement`, `SetMovementMode(5, 0)`, gravity off, Play("OverSlide"), state+0x84 |= 4, +0x9C =
pawn position + (0, 0, 150), sub = 1. Sub 1 while traction: +0x9C = attacker position with z + (150 −
bounds z extent − 20); a side test against the attacker's rotation ends traction early
(state+0x84 &= ~4, 0x1413B868E); while clip time × 4.2857 < 1 (0x1413B86D8): pawn.vt+0x1668(direction ×
dt × 600). Traction off: velocity write (vt+0x16C0), slot116(1), slot115(1), 0x1405A1D30(…, 1.0), sub =
2. Sub 2 at `m_bEnableChange`: +0xC8 & 4 → `DoChain`; else slot116(0), slot115(0),
`SetMovementMode(3, 0)`, gravity on, pawn.vt+0x1010(1, None), sub = 3; clip end → `DoChain`.
`OverSlide` 0059 [A]: 0.8667 s, root motion forward 700 cm with a vertical arc (peak 273 cm) and yaw 0 →
−180°, `TresTraction` 0–0.2333, `TresLandAnimPose` 0, effect `vs_ddg_ovs_sho00` 0.36, SE `se00001_010`,
`TresEnableChange` 0.7.

**Type 4 WARP** — slot 121 0x1413BC250 [I for the flow]: sub 0: gravity off, `SetMovementMode(5, 1)`,
Play("ShootFlowStart"), effect anim.vt+0x1A0(pawn.vt+0x2458(), 0x0B, 0), +0xC4 = 0.2, sub = 1. Sub 1:
+0xC4 −= dt; while > 0 wait (0x1413BC2DD); then slot113(&+0x9C), `SetActorLocation(+0x9C)`
(0x143EAC3D0), `Turn(yaw to the attacker, 1.0)`, effect 0x0C, state+0x84 |= 1, slot116(1), slot115(1),
sub = 2. Sub 2: +0xC8 & 4 or clip end → `DoChain`. `ShootFlowStart` 1451 [A]: 0.6667 s, no root motion,
SE `se00001_020` / `se00001_034`.

Invincibility: reaction type 11 from Begin to End for every type (no restore inside the ticks).
Counter: while offered (+0xD8 bit 0) the combat `CanAttack` accepts an attack (combat spec 3.2); it
queues the attack with kind +0xE0, slot 117 marks it, and the state chains at `m_bEnableChange`.

---

#### Body-collision reaction types: what a hit does

Resolver 0x14061FE20(attackComponent, attackPrimitive, victim, bodyPrimitive, bFlag, info, out) [I],
called per hit by the attack application 0x140618940 (0x140619F6D). It writes `info+0x60` (victim
result R) and `info+0x61` (attacker result A). A == 0: the application skips the hit entirely
(0x14061A143: no hit effect, no `TakeDamage`). The type is read from the hit primitive
(interface +0x38 = `m_BodyReactionType`, 0x14061FFC9).

Inputs: W = victim damage interface vt+0x60(attribute) (bit `attribute` of the status bit set +0xE8);
Imm = vt+0x68(attribute) (resistance ≥ 100). Automatic guard: type ∈ {0, 4} (or 2 without W / Imm),
damage kind != 0x0C and victim vt+0x2C8() → type = 3 (0x14061FFFF).

| Type | Result (table 0x1406202F4) |
|---|---|
| 11 GHOST_INVINCIBLE | **R 0, A 0, unconditionally** (0x14061FF5C): the hit does not exist — no damage, no reaction, no hit effect, the attack is not consumed by Sora |
| 8 GHOST, 9 GHOST_NOREACT | W: normal hit (R 2; R 1 for type 9), A 1; else R 0, A 0 |
| 6 INVINCIBLE, 7 INVINCIBLE_NOREACT | W: normal hit; else R 0, A 1, *out = 1 (the attacker registers a hit, the victim receives nothing) |
| 3 GUARD, 5 GUARD_NOREACT | W: normal hit (R 2; R 1 for type 5), A 1. Imm: R 0, A 1. Damage kind 0x11 / 0x12 (0x14064E780): normal hit. Otherwise **guarded**: R 5; A 1 when the row's `m_ReflectLevel` ≤ 0 (attack primitive+0x986 bit 7) or bFlag; else A 3, changed by `m_ReflectReaction` 2 → A 4, 3 → A 1; bad status cleared (0x14064CFD0) |
| 4 WEAK_GUARD | W: normal; Imm: R 0, A 1; kind 0x11 / 0x12 or the row lacks `m_bIsEnableWeakGuardAttack` (info+0x51 bit 3): normal hit R 2 (R 1 with the rapid-fire test), A 1; else guarded as type 3 |
| 10 REFLECT1, 1, 2 | attack clash / no-reaction / strong: not part of these states |

Victim side `TakeDamage` 0x1405BB850 [I]: R ∈ {3, 4, 5, 6} → no HP loss (0x1405BBB5F..0x1405BBB6B);
then, if reactions are allowed (`!pawn.vt+0x2070()`): R 3 → interface +0x448 (Reflect, slot 137); R 4 →
+0x458, on false +0x448; **R 5 → +0x470 `OnGuardedHit(info, _, causer, direction)`**; other → the
normal reaction (slot 88).

So for the port: reaction type 11 (Dodge Roll, Risk Dodge, Air Slide) = the host must not deliver the
hit at all and must let the attack continue; reaction type 3 = the host delivers "guarded" with the
damage info, the causer and the hit direction, applies no damage and no bad status, and gives the
attacker the recoil result when the row's reflect level is positive.

---

### Corrections to Part I

| Section | Part I statement | Correct |
|---|---|---|
| 3 (request) | Dodge from JUMP: "OnGround && Jump+0xF8 == 0, i.e. during the landing" | The gate has that branch, but `TryDodge` 0x14143AAA0 returns false for current id 3 (table 0x14143AC94): no roll out of the Jump state |
| 3 (gate) | "weapon kind 1: HasAbility(DODGE)" | By battle flag: battle kinds 1, 4, 8, 11, 12, 18, 19 need DODGE, 6, 21–23 are free, others refused; outside battle kinds 7, 9, 10 refused, all others need DODGE |
| 3 (Reset) | not stated | Reset clears bits 0, 1, 3, 4 (not bit 2) |
| 3 (Tick) | "flags &= ~4" at the end of every tick | Not on the queued-state `DoChain` path (0x14137404F jumps past it) |
| 4.2 | "reaction type 3 (guard) — and, for a player-requested guard, the default reaction is restored at once" | Confirmed, with the names: `SetReaction(3, "Body")` then `RestoreReaction("Body")`; a player guard therefore never has type 3 on the body — it guards with the parts `Guard` / `GuardBody`; type 3 on `Body` exists only in the hit-entry Guard, until its `TresEnableChange` |
| 4.3 | "sub 1: when +0xDC becomes set: Play hit clip" | That branch builds the name but never calls Play (0x14139C355–0x14139C63B); it is not reached through `OnGuardedHit`, which replaces the state: the hit clip is always played by sub 0 of the new instance |
| 4.3 | "DODGEROLL / AIR_SLIDE / SWIM: DoChain at once" | Confirmed (ids 0x10, 0x14, 0x66) |
| 4.3 / 4.4 | "JUMP: DoChain only if m_bEnableChange"; "ends … at 0.633 s into a queued jump"; "the hit clip is cancellable at 0.7 s" | The Guard `TresEnableChange` handler 0x14139A4E0 never sets `m_bEnableChange`; a queued jump starts at the clip end (0.8667 / 0.9667 s). `TresEnableChange` only ends the guard, restores gravity and starts the air cooldown |
| 4.3 | push-back for every hit entry | The direction is written only when flags bit 1 (success) is set; otherwise the velocity write is zero |
| 4.3 | "+0xF8 = 0.2 … turn 0.2 s from Reset" | Player guard: rate 1.0 from sub 0, timer 0.2 s from Reset. Hit entry with success: rate 1.0, timer 0.2. Without success: rate 0.05, timer 0.05 |
| 4.3 (push-back) | "× the attack's scale" | Two formulas depending on pawn+0xD84 > 0 (see slot 115); `m_GuardHitBackPowerLv1–3` = 300 / 1000 / 3000 in the asset as in the fallback; only strengths 1 and 2 are produced |
| 4.5 | "hit strength byte"; Reflect when "the damage-info strength byte (+0x63) is > 2" | +0x1C0 = `m_DamageKind` (info+0x0A); the Reflect test is `m_ReflectLevel` (info+0x63, signed) ≥ 3 |
| 4.5 | success = "player-requested Guard that has not yet guarded a hit" | Also: an Attack state with +0x23C bit 1, and any other state under the automatic-guard option |
| 4.5 | counter-window function "(1.0, attacker, success, flag)" | 5th argument = just guard: controller+0x89C > 0 (0.1333 s timer armed in Guard Begin with CRITICAL_COUNTER on the ground), or controller+0x878 bit 8 for a repeated hit |
| 4.6 | Reflect = "guard break"; "bit 2 when the controller's test of the attacker vt+0x1CB0 passes" | Reflect is the weapon-recoil state (clips `AttackReflect` / `AttackAirReflect`), also entered from `TakeDamage` results 3 / 4 through slot 137; vt+0x1CB0 matches a risk-dodge offer registered by the attacker's `TresAnimNotify_DamageCaution` |
| 4.6 | Risk Dodge "MaxWalkSpeed 3000, MaxAcceleration 180000, Sheathe, reaction type 11, camera request" | Confirmed; the state has four types (UnderSlide, OverSlide, SlideTurn, Warp) chosen by the attacker's offer |
| 0.4 | "Guard is reaction type 3 plus guard collision parts, from Begin to TresTiming(0)" | Player guard: parts only, Begin → 0.6 s. Hit-entry guard: type 3 on `Body`, Begin → 0.7 s (air 0.6667 s) |
| 2.4 | reaction types "0 default, 1 no reaction, 3 guard, 6 invincible, 8 ghost, 11 ghost + invincible" | Full enum: 0 DEFAULT, 1 NO_REACTION, 2 STRONG, 3 GUARD, 4 WEAK_GUARD, 5 GUARD_NOREACT, 6 INVINCIBLE, 7 INVINCIBLE_NOREACT, 8 GHOST, 9 GHOST_NOREACT, 10 REFLECT1, 11 GHOST_INVINCIBLE |

Disagreements with `docs/sora-combat-spec.md`:

1. Combat 5 ("What ends an attack") says the dodge / guard gates test the Attack state's
   `m_bEnableChange`. They test **Attack+0x234 bit 7** (0x141428190, 0x1414296EF), the bit combat 5
   describes as "arms the swim exit" (set 0.1333 s after the first hit window opens, or at
   `m_bEnableChange`); the dodge gate also requires `OnGround`.
2. Combat 3.2 / 3.3 / 13 call controller+0x8A0 the "just-guard timer". It is the counter (reprisal)
   window written by `FUN_1414145C0` (1.0 s after any successful guard with GUARD_COUNTER /
   REVENGEIMPACT / REVENGEDIVE); the just-guard timer is controller+0x89C (0.1333 s), and its result is
   controller+0x878 bit 8, which combat 3.3 reads correctly.
3. Combat 8.2 lists `TresDamageInfo` offsets up to +0x65 without +0x60 – +0x63: +0x60 victim result,
   +0x61 attacker result, +0x62 `m_ReflectReaction`, +0x63 `m_ReflectLevel` (low byte).
4. Combat 12 `HitResult { Applied, Guarded, Reflected, Ignored }`: "Ignored" has two forms with
   different attacker behaviour (dropped A 0 for ghost types; registered A 1 for invincible types), and
   "Guarded" carries the attacker result (none / recoil 3 / 4).
5. Combat 9 names a second "no reaction" mechanism (controller+0x8F4 > 0) and `TakeDamage` has the pawn
   tests vt+0x2080 (no HP loss) / vt+0x2070 (no reaction); Part I 0.4 presents reaction type 11 as the
   only invincibility. They are independent; none of the four states here writes them.
6. Combat 3.3 "GUARD (just guard): forcedKind 0x24 / 0x23 / 0x21" and the controller command id 5 / 4 / 3
   of this document are the same ability order (REVENGEDIVE, REVENGEIMPACT, default); the attack out of
   guard is allowed by the open window, not by a just guard.
7. Agreement: combat 8.4 (results 3 / 4 / 5 → interface +0x448 / +0x458 / +0x470), combat 3.2
   (RISKDODGE: +0xD8 bit 0 set and +0xC8 bit 2 clear), combat 3.3 (kind = RiskDodge+0xE0 0x28–0x2B),
   combat 5 (slot 129 pushes REFLECT as weapon recoil).

### Host affordances

None of the four states sweeps the world (exception: the warp destination). What the host supplies:

| Consumer | Data |
|---|---|
| all | `OnGround` (movement mode Walking / NavWalking and not floating), in water (component vt+0x5A0) |
| Dodge Roll | nothing else: displacement is the clip's root motion through the walking move; ground and walls as in the movement spec |
| Guard Begin | attacker position (vt+0x6E0) or lock-on target point (vt+0xB88) for the look-at point |
| Sora's hurt shapes | `Body` (damage): capsule radius 50, half height 90, on bone `Center`, relative (0, 15, 0), roll 90, push off. `Body` (push only, damage off): capsule 50 / 90 at mesh (0, 0, 90). `Guard`: capsule 51 / 91 at mesh (0, 2, 90), type 3, off by default. `GuardBody`: capsule 50 / 90 at mesh (0, 0, 90), type 4, off by default. Mesh yaw 270 relative to the root. Camera primitives and `BaymaxRide` (sphere 120) are not hurt shapes |
| incoming hit | per hit: the primitive hit (name → reaction type), `TresDamageInfo` (damage kind +0x0A, attribute +0x0B, power scale +0x10, brake +0x1C, flags +0x50 / +0x51, reflect reaction +0x62, reflect level +0x63), causer, hit direction (world, any length; flattened by Sora) |
| host as attacker | result per hit: dropped / registered-no-effect / guarded (+ recoil kind) / applied |
| Risk Dodge | per enemy attack: offer type 1–4 while its warning window is open; attacker position, rotation, XY and Z extent of its body bounds; for type 4 two traces between Sora and the attacker (pawn vt+0x2750) [C] |
| test level | an attacker object with: a hit volume (sphere, radius of choice) switchable per frame, a row (damage kind, power scale, brake, reflect level, weak-guard flag), an optional offer type, a box of known extents |

### Tests

60 Hz fixed step, every ability at level 99 (DODGE, REFLECT_GUARD, RISKDODGE, GUARD_COUNTER,
REVENGEIMPACT, REVENGEDIVE, CRITICAL_COUNTER, GUARD_REGENE present), Keyblade (weapon kind 1), flat
ground, Sora in Idle facing +X at the origin. N = frame of the press.

1. **Dodge Roll distance.** Press guard with the stick at (1, 0) on frame N. Expect: state 16 from
   S(N); position X after M(N+k) = root track at k / 60 s (2 → 45.57, 8 → 234.27, 20 → 479.12, 38 →
   600.0); final 600.0 cm; pawn+0x619 = 11 from S(N), = default from S(N+28); state Idle from S(N+42) (± 1 frame: end-of-clip rule of `IsAnimEnd`).
2. **Dodge direction threshold.** Stick magnitude 0.5 at yaw 90°: the roll goes along the current
   facing (+X). Magnitude 0.51: Sora snaps to yaw 90 in S(N) and rolls along +Y.
3. **Dodge i-frames.** Attacker volume overlapping Sora, active one frame at a time: hits resolved in
   frames N+1 … N+27 produce no damage, no reaction and are not consumed (A 0); frame N+29 produces
   damage.
4. **Dodge cancel.** Guard press (stick neutral) at C(N+20): refused (`m_bEnableChange` clear). At
   C(N+29): accepted, Guard begins S(N+29). Dodge press at any frame of a roll: refused.
5. **No dodge out of Jump.** Land from a fall and press guard + stick while the Jump state is still
   current: no roll; the same press on the first Idle frame rolls.
6. **Guard timing.** Press guard, stick neutral, frame N: state 15 from S(N); `Guard` and `GuardBody`
   primitives enabled and `Body` disabled from S(N) until M(N+36); velocity zero; state Idle from
   S(N+52). Jump pressed at any frame: refused (gate needs a guarded hit).
7. **Guarded hit, front.** Hit from +X (damage kind 1, power scale 1, reflect level 0) at frame N+10:
   no HP loss; Guard re-entered at the next S (H): clip `GuardHit`, flags bit 1 set, push-back speed
   300 → velocity 285 cm/s along the hit direction in S(H), × 0.95 per tick (expected travel ≈ 90 cm by
   the clip end if the write is a plain velocity); `Body` primitives type 3 until M(H+42); counter
   window controller+0x8A0 = 1.0, command id 5 (REVENGEDIVE present); if the just-guard timer controller+0x89C (0.1333 s from
   Begin) is still above 0 also controller+0x878 bit 8. Damage kind 2: speed 1000 (first write 950, ≈ 300 cm).
8. **Guard from behind.** Same hit from −X with the weak-guard flag clear: normal damage reaction.
   With the flag set: guarded. From ±110° off the facing: guarded; from ±130°: as from behind.
9. **Exits from the hit clip.** Dodge press at C(H+16): refused; at C(H+17): accepted, roll begins
   S(H+17). Jump press at C(H+17): accepted, Jump begins S(H+17) (coordinator decision 2026-10-11; first-pass reading: S(H+58)).
10. **Air guard.** Guard at the apex of a jump: `AirGuard`, Velocity = 0 and gravity off from S(N) to
    M(N+38); pawn+0xE48 bit 2 = 0 from S(N); pawn+0xE3C = 0.3333 at M(N+38); second air guard refused
    until 20 frames later.
11. **Reflect.** Guard, then a hit with reflect level 3 and no offer: state 66 at the next S, clip
    `AttackReflect` 0.9667 s, speed 400 (first write 380 cm/s, ≈ 120 cm total), no request accepted,
    Idle at S(R+58). Reaction type default throughout: a second hit at R+10 damages.
12. **Risk Dodge, type 1.** As 11 with an offer of type 1 from an attacker 400 cm ahead facing Sora,
    bounds half-diagonal ≤ 300: Reflect flags bit 2, window 0.6 s (jump accepted for C(R+1) … C(R+35)).
    Jump at C(R+5): state 17 at S(R+5), reaction type 11 until End, destination = attacker position +
    300 cm behind it (X = 700), root scale 700 / 800 = 0.875, Sora at X = 700 facing −X when the root
    track completes (frame 23 of the clip, 0.7667 s of clip time); slow motion and the counter command
    from the tick in which Sora has passed the attacker (clip time ≈ 0.155 s) until `TresEnableChange`
    (clip time 0.4667 s); Idle at the clip end (1.0667 s of clip time; the slow motion stretches real time).
13. **Risk Dodge refused.** Same without RISKDODGE, or jump at C(R+40): an ordinary refusal (the
    Reflect state accepts no jump).

### Still needs the running game

| Object | Offset | Type | Why |
|---|---|---|---|
| Guard state | +0x84 | u32 | confirm bit 0 never sets during `Guard` / `GuardHit` |
| state queue / state id | — | — | G-J1: jump pressed after `TresEnableInput` of `GuardHit` — frame on which state id 3 begins (port: at once, by `ClearQueue`; first-pass reading: at the clip end 0.9667 s) |
| Guard state | +0xE8, +0xFC, +0x100, +0x10C, +0x134 | u32, f32, f32, i32, vec3 | flags, push-back speed, decay, strength and direction on guarded hits of damage kinds 1 and 2 |
| pawn | +0xD84 | i32 | selects the push-back formula of slot 115; meaning unknown |
| pawn | +0x619 | u8 | reaction type per frame through a roll, a guard, a guarded hit, a risk dodge |
| movement component | +0x214 | vec3 | effect of the vt+0x9F8 write: does braking act between the per-tick writes (Part I P6); guard push-back and Reflect travel |
| body primitives `Body`, `Guard`, `GuardBody` | +0x894 and the collision-enabled state | u8, bool | part switching by slot 116; which primitive an attack reports when `Guard` and `GuardBody` both overlap |
| controller | +0x89C, +0x8A0, +0x8A4, +0x878 | f32, f32, i32, u32 | just-guard timer and its countdown, counter window, command id, bits 8 / 9 |
| controller | +0xC40, +0xC48, +0xC50, +0xC58 | weak ptr, i32, ptr, i32 | risk-dodge offers and the chosen type per enemy attack |
| Reflect state | +0x88, +0x98, +0x9C | u32, f32, f32 | flags, speed, window timer (35 or 36 frames) |
| RiskDodge state | +0x78, +0x88, +0xB4, +0xB8, +0xC8, +0xD8 | i32, i32, f32, f32, u32, u32 | sub-steps, type, distance, root scale, slow-motion and counter bits |
| pawn | function 0x1405DA4E0 arguments | — | meaning of (0.25, 0.05): time dilation and blend |
| pawn+0x470 components | vt+0x6C8(0 / 1) | — | what Risk Dodge switches off (pawn-to-pawn blocking presumed) |
| DodgeRoll state | +0x88 | u32 | readers of bits 0 / 1 (0.4 s / 0.1 s marks) |
| attack row flag | info+0x50 bit 3 `m_bIsIgnoreGuard` | — | where it bypasses the guard (not in the resolver read) |

---

## II.2 Air Slide / Aerial Dodge / Aerial Recovery, Glide, Superslide, Floating; movement-component setters and Flying mode

(strand `air`; working notes in `build/sora_tmp/actions/p2_air/`)

Second pass of sections 6, 7, 8 and the Floating part of 12 of Part I, read in the machine code
(`build/sora_tmp/mv/dis.js`, iced-x86). Every statement below is **[I]** (address given) unless tagged
otherwise: **[A]** asset, **[D]** derived, **[U]** stock engine function identified by shape, **[C]**
decompiled C only (said where), **[?]** not determined. Conventions are those of Part I and of
`docs/sora-movement-spec.md` (frame order C → M → S; a state write acts in the next movement tick;
`comiss a,b; ja` = a > b).

Helper identities used in the pseudo-code (all read):

| Name | Address | Does |
|---|---|---|
| `OnGround(pawn)` | 0x1405AD850 | `!Floating() && component vt+0x538()` (MovementMode ∈ {1, 2} with an updated component) |
| `InWater(c)` | pawn vt+0x1A80 → component vt+0x5A0 | water test of the movement component (argument: depth offset, always 0.0 here) |
| `InWaterFlag()` | pawn vt+0x1C30 | component vt+0x420() && pawn+0xD70 bit 0 |
| `Floating()` | pawn vt+0x1BD8 0x14110E7E0 | pawn+0xD80 bit 0 |
| `NextId()` / `NextState()` | 0x1405AB6E0 / 0x1405AB6C0 | id / object of the first queued state (0 / null if none) |
| `Fall(yaw)` | iface vt+0x560 (slot 172) | `RequestJump(yaw, 0, bFall)` → Jump at sub-step 3; returns the new state |
| `DoChain()` | state vt+0x270 | end this state, begin and tick the next one in this `MainProc` call |
| `Turn(rot, a)` | pawn vt+0xDB0 | slerp of the actor rotation toward `rot` by `clamp01(a)` |
| `RotVec(rot)` | 0x142D93270 [U `FRotator::Vector`] | unit forward vector of a rotator |
| `Play(name, blendIn, mode, group, rate, scale)` | state+0x40 vt+0x18 | returns the clip length; every call below is followed by `if length > 0: ANIM vt+0x1F0(n)` (voice variant, n = 0 unless said) |

---

#### Movement-component primitives used by these states

Component `TresAICharMovementComponent` (vtable 0x145D073F0). Fields: Velocity +0x214, Acceleration
+0x4A4, MovementMode +0x314, GravityScale +0x2F8, MaxWalkSpeed +0x388, MaxAcceleration +0x39C, flags
+0xA3C (bit 1 gravity disabled, bit 2 skip `CalcVelocity`), root-motion velocity +0xA24, **direct
velocity** +0xC0C (vec3) with flags +0xC18 bit 0 (XY pending) / bit 1 (Z pending), measured velocity
+0x9A0.

| Pawn slot (offset) | Component slot | Function | Effect |
|---|---|---|---|
| 726 (+0x16B0) 0x1405B5FF0 | vt+0x9E8 | 0x140592DD0 | **direct velocity XYZ**: `+0xC0C..+0xC14 = v`; `+0xC18 \|= 3`; `Velocity = v` (all three axes, at once) |
| 728 (+0x16C0) 0x1405B6010 | vt+0x9F8 | 0x140592E10 | **direct velocity XY**: `+0xC0C, +0xC10 = v.xy`; `+0xC18 \|= 1`; `Velocity.xy = v.xy` (Z untouched) |
| (730) | vt+0xA08 | 0x140592E40 | direct velocity Z: `+0xC14 = z`; `+0xC18 \|= 2`; `Velocity.z = z` |
| 731 (+0x16D8) 0x1405A7FB0 | vt+0xA40 | 0x14057E740 | `+0xA3C bit 2 = b` (skip the body of `CalcVelocity`) |
| 741 (+0x1728) 0x1405B93F0 | vt+0x928 | 0x140594480 | unless pawn+0x634 bit 1: `+0xA3C bit 1 = !b` (b = gravity on) |
| (+0x22E8) 0x1405B4FF0 | vt+0xBD0 | 0x14057CFD0 → 0x1405C98F0 | zeroes the two pointers component+0xA78 / +0xA80 of the sub-object component+0xA40 (the "driven move" record whose bit 0 makes `StartNewPhysics` call vt+0xCC0 0x14058ECF0 instead of a Phys function, 0x140596ED3) — cancels a pending driven move; no other effect [the driven move itself: not read] |
| 733 (+0x16E8) 0x140530DD0 | vt+0x3F0 + vt+0xA18 | 0x1405268A0, 0x14057CFE0 | `StopMovement`: `Velocity = 0` (three axes), copied to the root component (+0x368), **`Acceleration = 0`** (0x140597BA0), and the direct-velocity record cleared (`+0xC0C.. = 0`, `+0xC18 &= ~3`) |
| 756 (+0x17A0) 0x14054EAF0 | vt+0x3F0, vt+0x590 | | `SetMovementMode(mode, bStop)`: **if bStop: the same velocity / acceleration zeroing as `StopMovement` (without the vt+0xA18 call of the pawn function, but vt+0x3F0 ends in it)**; then component vt+0x590(mode, 0) |
| (+0x1650 / +0x1658) | | 0x1405B9690 / 0x1405B6230 | `GravityScale = x` / `= class default object's` |
| (+0x1570 / +0x1578) | | 0x1405B9900 / 0x1405B6350 | `MaxWalkSpeed = max(x, 0)` / `= pawn vt+0x1588()` (default) |
| (+0xE28) 0x1405AB330 | | | address of component+0x9A0 = **measured velocity**: location delta of the last movement tick / dt (post step 0x140591690, 0x140591D3A) |

Component vt+0x590 = 0x1405262A0 (AI subclass): when the current mode is neither Falling nor Flying and
the new one is, it stamps component+0xCA4 from the world (+0x938); then the base `SetMovementMode`
0x140594A00: no-op when the mode does not change; otherwise `SetJumpDurationFinish(true)` (vt+0x940,
0x140594A9C), mode stored, `OnMovementModeChanged(prev)` (vt+0x598 0x14058C140, which ends in pawn
vt+0xAC0 0x1405B4860 = script event + delegate pawn+0x5D0; no Sora blueprint implements it [A]).

**How the direct velocity is consumed** (`CalcVelocity` 0x1405795F0): the tail at 0x14057A286 runs after
the normal computation *and also when the skip flag is set* (`test [+0xA3C],4; jne 0x14057A286` at
0x140579658): `if +0xC18 & 1: Velocity.xy = +0xC0C.xy; clear bit 0` · `if +0xC18 & 2: Velocity.z =
+0xC14; clear bit 1`. So a direct write is one-shot: the setter puts the value into `Velocity` at once,
and the next `CalcVelocity` (whatever friction, braking or acceleration it computed) is overridden by it
once more. A state that wants a held velocity rewrites it every tick (Superslide, guard push-back,
Floating coast, Swaying). Per mode:

- **Walking**: one `CalcVelocity` per tick → the direct XY survives; a direct Z is wiped by
  `MaintainHorizontalGroundVelocity`.
- **Falling**: `PhysFalling` calls `CalcVelocity` twice when there is input acceleration (movement spec
  4.4: the no-air-control copy first, velocity restored afterwards) — the first call consumes the flags,
  so with stick input the direct value only acts through `Velocity` itself [D]. With no input (all users
  here) the single call applies it; gravity is then added on top by `NewFallVelocity`.
- **Flying**: one `CalcVelocity` → the velocity of the tick is exactly the direct value.

##### `PhysFlying` (mode 5) — 0x14058E570, component vt+0x7C0

`StartNewPhysics` 0x140596E10 dispatches through the table at 0x14059703C: 1 → vt+0x7B0 `PhysWalking`
0x1405900B0, 2 → vt+0x7B8 0x14058F530, 3 → vt+0x6F0 `PhysFalling` 0x14058D0C0, 4 → vt+0x7C8
`PhysSwimming` 0x14058FC40, **5 → vt+0x7C0 0x14058E570**, 6 → vt+0x7D0 (empty), 0 → `Velocity = 0`.

```
PhysFlying(dt, iterations):
  if dt < 1e-6: return                                                    // 0x14058E590
  if (+0x594 & 0x100) && Acceleration == 0: Velocity = 0                   // cheat-flying flag, off
  CalcVelocity(dt, Friction = 0.5 * FluidFriction(+0xB80), bFluid = true, BrakingDecelerationFlying(+0x3BC))   // 0x14058E62E
  bJustTeleported = false                                                  // +0x594 &= ~0x40
  Ext = (UpdatedComponent has no attach parent) ? vec(+0xB90) : 0          // external (force-volume) velocity, 0 in plain play
  Adjusted = (Velocity + Ext) * dt
  if +0x458 & 4: Adjusted += RM(+0xA24) * dt                               // root-motion velocity; the bit is set in the ctor (0x140571866)
  Adjusted += vec(+0xBA8)                                                  // per-tick push displacement (PerformMovement 0x14058CEB7), 0 in plain play
  SafeMoveUpdatedComponent(Adjusted, rotation, sweep) -> Hit               // 0x14058E756
  if Hit.Time < 1:
      [+0xB30 bit 0 variant (surface-normal mode, off for the player: cleared in the ctor 0x140571642, set only by 0x1405944A0): not specified]
      n = normalize(Adjusted)
      if |Hit.ImpactNormal.Z| < 0.2 && n.z > -0.5 && n.z < 0.2 && CanStepUp(Hit):          // 0x14058E9CF..0x14058EA07
          if StepUp((0,0,-1), Adjusted * (1 - Hit.Time), Hit): return
      HandleImpact(Hit, dt, Adjusted)                                      // vt+0x430 -> pawn MoveBlockedBy
      SlideAlongSurface(Adjusted, 1 - Hit.Time, Hit.Normal, Hit, true)     // vt+0x470
  // NO "Velocity = displacement / dt" (stock has it): the velocity is kept through a wall slide
```

No gravity term at all, no landing test, no floor search: a pawn in Flying mode never becomes Walking by
itself. `FluidFriction` (+0xB80) is copied from the physics-volume class default at construction
(0x1405716EC) and from a water volume on entry (0x140591240) — stock default 0.3 [U];
`BrakingDecelerationFlying` (+0x3BC) has no Sora override [A] (stock 0 [U]). Neither matters while a
state rewrites the direct velocity each tick, and with `Velocity = 0` and no input they do nothing.

##### Root-motion scale (the `rootScale` argument of `Play`)

The `FVector2D*` argument goes pawn 0x1405A0E50 → mesh 0x1405C4EA0 (loads `scale.x`, `scale.y`,
0x1405C4F63) → anim instance vt+0x398 0x14056AD00 → vt+0x680 0x14056E920 → vt+0x670 0x14056F470 →
montage-instance play 0x143F4D3F0, which stores **instance+0x174 = scale.x, +0x178 = scale.y**
(0x143F4D44E / 0x143F4D45C). The montage advance 0x143F4EA20 extracts the root-motion transform of the
tick (0x143F544C0) and multiplies its translation, 0x143F4EB47–0x143F4EB6E:

```
T.translation *= (scale.x, scale.x, scale.y, 0)          // X and Y by scale.x, Z by scale.y
```

before handing it to the movement component (0x1405738A0). So `rootScale = (horizontal, vertical)`; the
default constant 0x14871E368 is (1, 1). The rotation part is not scaled. Root motion reaches the move as
`RM = world translation of the tick / dt` added to the falling delta (`Adjusted = ((OldV + V)/2 + RM)·t`,
movement spec 4.4) or to the flying delta (above); it is never written to `Velocity`. With gravity
disabled (`GetGravityZ` = 0, movement spec 4.3) and `Velocity = 0`, a clip whose root track has no Z
(all clips below [A]) moves the pawn on a level line.

---

#### Air Slide / Aerial Dodge / Aerial Recovery (`TresPlayerState_AirSlide`, id 20)

Vtable 0x1468FFA78; anim interface (state+0x40) 0x1468FFE10; second interface (state+0x48) 0x146900258.

| Field | Type | Meaning |
|---|---|---|
| +0x88 | u8 | stick flag (set by the request and by `MoveInput`, cleared at the end of every tick that finds no pole) |
| +0x8C | f32 | yaw: stick yaw, or the recovery facing |
| +0x90 | f32 | stick magnitude (written by `MoveInput` only) |
| +0x98 | ptr | target pawn (recovery: the attacker) |
| +0xA4 | i32 | voice variant passed to ANIM vt+0x1F0 for the Air Slide clip |
| +0xA8 | u32 | flags: 1 invincible (reaction type set), 2 **Aerial Dodge**, 4 slide shot (form), 8 in water, 0x10 back slide (form), 0x20 **Aerial Recovery**, 0x40 reaction command armed, 0x80 abort, 0x100 recovery variant of another mode |
| +0xAC / +0xB0 | i32 | reaction-command id / second id |

**Reset** 0x14133CF60: base reset; `+0x88 = 0`; `flags &= ~0x1FF`; `+0x8C, +0x90, +0x98, +0xA0, +0xA4,
+0xAC, +0xB0 = 0`.

##### Request

Controller, guard button pressed, after `TryFlow` returned false: `TryAirSlide(yaw, mag, bKeep = 0)`
iface slot 69 0x141436D20 → gate slot 68 0x141425780.

```
gate 0x141425780:
  CanAct (vt+0x29C0) && !restrict bit 8 (vt+0x1F68) && !(pawn+0xE62 & 1) (vt+0x2770)
  && !AirSlideBlocked (vt+0x1E20 0x14110E640: pawn+0xE38 > 0 || !(pawn+0xE48 bit 1)) && controller != null
  next = NextId(); next in {0, 12, 13, 14, 75..90, 156, 157, 159, 160, 161}            // table 0x141426060 / 0x141426054
  bGround = OnGround()
  switch current id (table 0x14142614C / 0x141426104):
    1 IDLE, 2 RUN, 40, 73 : pass
    3 JUMP                : Jump+0xF8 == 0                                             // 0x141425C75
    4 GLIDE, 5 FLOATING   : bGround = false; pass                                      // 0x141425926
    12-14, 75-90, 156..   : bit 1 (vt+0x1E70) && Attack+0x234 & 0x80                   // 0x141425D3E
    15 GUARD              : bit 1 && Guard+0xDC != 0 && m_bEnableInput                 // 0x14142598C
    16 DODGEROLL          : m_bEnableChange
    18 FIREMAGIC          : bit 1 && m_bEnableChange && !(+0xD8 & 0x50) && !(pawn+0xE48 bit 5)
    19 DAMAGE             : Damage+0x90 == 0; type(+0x88) in {1, 2, 9}: m_bEnableChange; 19: sub == 4   // table 0x1414261F0
    20 AIR_SLIDE          : (flags & 0x20) && m_bEnableChange   [else a branch for pawn kind 0x11]      // 0x141425A38
    22, 138 RAIL          : return state vt+0x3A0()            // returns directly: no ability / ground test
    102 SWIM              : InWaterFlag
    74, 97, 111, 122, 142, 162: state rules (not Sora field play)
    others                : fail
  !(restrict bit 17) (vt+0x2788)
  kind = weapon kind (vt+0x12E0(-1));  m = controller stick magnitude (controller+0x828 vt+0x10)
  if m < 0.1:  only kinds 2, 5 (sub-kind 3) and 4, 7, 10, 12, 18 pass   -> kind 1 FAILS            // 0x141425F00 comiss m,0.1; jae
  else if controller mode == 0x8B: pass
  else if !battle (vt+0x1740): kind 1: HasAbility(AIRSLIDE 0x17) && !bGround                       // 0x141426030 -> 0x141425FED
  else (battle):               kind 1: (HasAbility(0x17) || HasAbility(AIRDODGE 0x18)) && !bGround  // 0x141425FD0
TryAirSlide 0x141436D20 (pawn type 1):
  if !gate: return false
  water = InWaterFlag()
  current id in {1,2,3,4,5,15,16,19,20,74,97,111,142}: bKeep kept; attack ids, 18, 40, 122: bKeep = 0   // table 0x1414371FC
  dodge = battle && HasAbility(AIRDODGE 0x18)      (kind 1; 0 when not in battle)                  // 0x14143705B
  if !bKeep: ClearQueue (0x1405A8010)
  st = PushState(20)                                // Reset runs
  st.flags = (st.flags & ~0xE) | water << 3 | slideShot << 2 | dodge << 1                           // 0x1414370CC
  if mag >= 0.1: st+0x8C = yaw; st+0x88 = 1         // comiss mag,0.1; jb skip (0x1414370F8)
  return true                                        // the controller returns: no MoveInput / TryRun this frame
```

`MoveInput(yaw, mag)` 0x141440600, case AIR_SLIDE (0x1414406A7): `+0x8C = yaw; +0x88 = 1; +0x90 = mag`
on every later frame with an active stick (≥ 0.1). The request itself never writes +0x90.

Variant selection, plain Keyblade: not in battle → Air Slide (needs `AIRSLIDE`); in battle with
`AIRDODGE` → Aerial Dodge (flag 2) whether or not `AIRSLIDE` is equipped; in battle with only
`AIRSLIDE` → Air Slide.

##### Begin 0x141332930

```
if pawn invalid: return
if !(flags & 8) && InWater(0.0):                    // component test, 0x1413329A3
    if !Swimming (0x1405AD820): SetMovementMode(4, 0)
    flags |= 8
if (flags & 0x1E) == 0:                             // plain Air Slide
    if !battle && !pawn vt+0x1B48(): Sheathe(1, 1.0)       // 0x141332A14; in battle: nothing
else: Draw(1, 1, 1.0)                               // Aerial Dodge, slide shot, water, back slide (0x141332A26)
ctl = controller; if null: return                   // nothing below runs
[controller mode 0x8B: shooter-mode bookkeeping]
if !(flags & 0x120): pawn bit 1 = 0                 // vt+0x1E68(0), 0x141332AC9
else:                                               // Aerial Recovery (0x20) and the 0x100 variant
    yaw = target(+0x98) ? yaw of (target location - own location) : actor yaw       // 0x141332AD4..0x141332C55
    +0x8C = yaw;  Turn((0, yaw, 0), 1.0)                                             // 0x141332C8E
    if !pawn vt+0x1EE8():                           // not command-restricted (pawn+0x1130 bit 3, restrict bits 0 / 15)
        if flags & 0x100: +0xAC = 0xD; +0xB0 = 0xC6; pawn bit 1 = 0
        elif HasAbility(BLOW_COUNTER 2): +0xAC = 2; +0xB0 = 0x27                     // 0x141332CDE
if (flags & 0x14) == 4: [slide shot: Draw, face the lock-on target]
elif +0x88: Turn((0, +0x8C, 0), 1.0)                // full snap to the stick yaw, 0x141332F20
SetReactionType(pawn, 11)                           // 0x1405B8B00(pawn, 0xB), 0x141332F38: GHOST_INVINCIBLE
flags |= 1
component vt+0xBD0 (cancel a driven move);  pawn+0x660 = 0 (vt+0x2348)
```

##### Tick 0x141340A10

```
pawn vt+0xDC8(dt * 12); pawn vt+0xDB8(dt * 12)                              // 12: 0x141340A72
if sub == 0:
    StopMovement()                                                          // 0x141340C37
    gravity off (vt+0x1728(0))                                              // 0x141340C46
    len = 0
    if flags & 0x20: slot113(1); len = Play("AirRecovery", blendIn 0.0, mode 2, group 0, rate 1.0, scale (1,1))   // 0x141340CB8
    if flags & 0x100: slot113(1)
    group = (weapon kind == 0x12) ? 0x7DA : 0
    if   flags & 8:    len = Play("AirDodge",  -1, 2, group 0x208, 1.0, (1,1))      // water; also REPLACES the recovery clip
    elif flags & 0x10: len = Play("BackSlide", -1, 2, group, 1.0, (1,1))
    elif flags & 4:    len = Play("SlideShotB/R/F/L" by wrap180(+0x8C - actorYaw): |d| >= 135 B, d >= 45 R, d > -45 F, else L)
    elif flags & 2:    len = Play("AirDodge",  -1, 2, group, 1.0, (1,1))            // Aerial Dodge, 0x141340EFA
    if !(len > 0):                                                                    // nothing played (or the name is missing)
        s = pawn vt+0x1B90()                                                          // AIRSLIDE level scale
        if weaponOut (vt+0x1330): len = Play("AirSlideB", -1, 2, 0, 1.0, (s, 1.0))   // 0x141340FE7
        if !(len > 0):            len = Play("AirSlide",  -1, 2, 0, 1.0, (s, 1.0))   // 0x14134105B
        if len > 0: ANIM vt+0x1F0(+0xA4)
    sub = 1                                                                 // then the pole probe below, same tick
else:                                                                       // sub 1
    if flags & 0x80: DoChain; return
    if IsAnimEnd(0):
        if !OnGround: Fall(actor yaw)                                       // 0x141340B0B
        DoChain; return
    if m_bEnableChange:
        if NextState() != null:
            if OnGround && battle: last pose = BATTLE (vt+0x1780(1))
            DoChain; return
        if !(flags & 0x20) && !OnGround && +0x88 && +0x90 > 0.5:            // movss 0.5; comiss 0.5,[+0x90]; jae skip (0x141340BC1)
            j = Fall(+0x8C)
            if j: j+0x114 |= 0x40; DoChain; return                          // Jump flag 0x40: keep the clip until it ends
POLE PROBE (every tick that did not chain):
    P = actor location + RotateVector(actor rotation, (120, 0, 90))         // 0x141341099 / 0x1413410A1
    if !(pawn vt+0x2330(P, P, 100.0, out) && TryPoleTurn(out, 0)):          // iface vt+0x270 (slot 78)
        +0x88 = 0                                                           // 0x14134123B
```

- The stick flag is cleared at the end of every tick, and set again by `MoveInput` in the controller
  phase of the next frame: "stick held" in the exit test means "stick ≥ 0.1 on this frame", and the
  magnitude test uses the last `MoveInput` value (> 0.5).
- On the ground (the slide touched down in Walking mode) nothing ends the state before the clip end
  except a queued state at `m_bEnableChange`.

**Slot 113** 0x14134C5E0 `(bArm)`: if `+0xAC != 0` and a controller exists: arm → 0x141414350(controller,
+0xAC, 2.0, 0), `flags |= 0x40`; disarm (only if flag 0x40) → 0x141414350(controller, 0, 0.0, 0),
`flags &= ~0x40`. (The reaction-command window of `BLOW_COUNTER`: 2.0 s or until End.)
**Slot 114** 0x14134CBC0: `flags |= 0x80` (abort: the next tick chains) and disarm.

**Notify handlers** (state+0x40): `TresEnableChange` +0x238 and `TresEnableInput` +0x240 are the base
ones (0x1411FBA40 / 0x1415124D0). `TresTiming` +0x348 = 0x141345B60: `if flags & 1: restore the reaction
type (0x1405B60D0); flags &= ~1` — **for any parameter value**. +0x228 (`TresAnimNotify_Shot`,
0x141343E10) belongs to the slide-shot form. No other slot differs from a plain state's interface
(vtable diff against Glide's 0x145C936A8).

**Move-blocked handler** (state+0x48 vt+0x48 = 0x1413432C0; called from the pawn's `MoveBlockedBy`
0x1405D2240 ← component `HandleImpact` 0x140587580, for a blocking move hit on a non-pawn actor with
`normalize(TraceEnd − TraceStart) · ImpactNormal < 0`): if not in water (flag 8): the pole probe above
(`TryPoleTurn`); if that did not start a pole turn: `TryWallKick(hit actor, hit component,
&Hit.ImpactNormal, 1, 0)` (iface vt+0x290, slot 82; its gate 0x14142F990 is the flowmotion strand's).
The landed handler (state+0x48 vt+0x258) is empty (0x14041F120).

##### End 0x1413347F0

```
if pawn invalid or no controller: return
if !(flags & 0x20):
    if !OnGround || Floating():                                             // 0x1413348AF / 0x1413348BF
        if !(pawn kind byte (vt+0xD80) == 0x11 && NextId() == 20):
            pawn+0xE38 = pawn+0xC6C (MyAirSlideCoolDownTime 0.6)            // vt+0x1E18, 0x1413348ED
if Floating(): GravityScale = pawn vt+0x1AE8()                              // the floating scale, 0x141334929
if InWater(0.0): SetMovementMode(4, 0)
else:            SetMovementMode(3, 0); gravity on (vt+0x1728(1))           // 0x14133495E / 0x14133496D
restore the reaction type (0x1405B60D0)                                     // 0x141334985
[controller mode 0x8B bookkeeping]
n = NextId()
if n != 3:                                                                  // 0x1413349F5
    if n == 23 (WALL_KICK) && InWater: SetMovementMode(3, 0); gravity on
    pawn bit 0 (second jump) = 0                                            // vt+0x1E58(0), 0x141334A3F
if flags & 0x20:
    slot113(0)
    if pawn vt+0x1360() && next state is neither an attack state nor Damage: Draw(1, 1, 1.0)
        // vt+0x1360 0x141436BF0 = weapon kind not in {1, 0x16, 0x17} && weapon not out: FALSE for the plain Keyblade
if flags & 0x100: slot113(0)
```

Velocity is not touched by End (`bStop = 0`): it is still the zero written at sub-step 0, so whatever
follows an Air Slide in the air starts **from rest**.

##### Clips [A] and numbers

| | `AirSlide` = `AirSlideB` (0055) | `AirDodge` (0054) | `AirRecovery` (0291) |
|---|---|---|---|
| Length / frames (30 fps keys) | 0.7333 s / 23 | 0.7 s / 22 | 0.5 s / 16 |
| Play arguments | blend-in −1 (asset `m_InterpolationTime`: none → 0), mode 2, group 0, rate 1.0, scale (s, 1.0) | −1, mode 2, group 0 (0x208 in water), (1, 1) | blend-in 0.0, mode 2, group 0, (1, 1) |
| Root motion | forward 1130 cm × s; no Z, no yaw | forward 700 cm; no Z | none |
| Root per key (cm) | 0, 200, 344.44, 476.67, 580, 661.2, 736.95, 805.49, 865.02, 913.78, 950, 977.69, 1002.31, 1024.26, 1043.93, 1061.72, 1078, 1092.06, 1103.5, 1113.19, 1122, 1128.17, 1130 | 0, 100, 197.5, 280.64, 351, 408.41, 462.23, 509.25, 546.26, 577.12, 604.7, 624.28, 637.64, 649.71, 660.49, 669.99, 678.19, 685.1, 690.44, 694.33, 697.33, 700 | — |
| `TresEnableChange` | 0.5 | 0.4992 | 0.2667 |
| `TresTiming` | none | none | none |
| `TresLandAnimPose` | 0 (LIGHT) | 0 (LIGHT) | 0 (LIGHT) |
| Sounds / effects | `se00001_007` 0, `se00001_036` 0.3069 (vol 0.7); `vs_spd_lin0` (0–0.2818), `vs_air_slide0`; Bonamik groups 0–0.7333; voice | `se00001_006`; `vs_spd_lin1` ×2 (0.2474), `vs_adg_rng0`, `vp_div_flo4` (group 520 = water only); voice | `se00001_012`; `vs_grd_sor_arv00`; voice |

Level scale `s` = pawn vt+0x1B90 0x141432EB0: equipped count of `AIRSLIDE` ≤ 1 → pawn+0xBD0
`MyAirSlideSpeedScale` 0.725 · 2 → +0xBD4 1.075 · else +0xBD8 1.425 [A values]. Distances [D]: 819.25 /
1214.75 / 1610.25 cm in 0.7333 s; first key interval 145 / 215 / 285 cm in 1/30 s (4350 / 6450 /
8550 cm/s). Aerial Dodge: 700 cm in 0.7 s, first interval 100 cm (3000 cm/s), no levels.

- **Path**: movement mode stays Falling (Swimming in water); gravity disabled and `Velocity = 0` from
  the first tick, so the displacement is the root motion alone, level, along the actor's facing (the
  stick yaw if the stick was ≥ 0.1 at the request, else the facing at the request). Walls: the falling
  slide (movement spec 4.4) plus the move-blocked handler above. A walkable surface met by the sweep
  lands the pawn (Walking); the clip keeps moving it along the floor.
- **Invincibility**: reaction type 11 from Begin to End for all three variants — the only earlier
  restore is the `TresTiming` handler and none of the three clips carries that notify [A]. Whole state:
  Air Slide ≤ 0.7333 s, Aerial Dodge ≤ 0.7 s, Recovery ≤ 0.5 s, shorter when chained at
  `TresEnableChange`.
- **Cooldown**: written only by End (conditions above: not a recovery, airborne or floating). The pawn
  tick part 0x141449CD0 (0x14144A035–0x14144A06C): `if AirSlideBlocked(): t = pawn+0xE38 - dt; store;
  if t < 0: pawn+0xE38 = 0; bit 1 = 1`. Bit 1 (cleared in Begin) therefore comes back on the first pawn
  tick with the timer at 0, i.e. one tick after Begin; what blocks a second slide is the 0.6 s timer
  from End (36 ticks at 60 Hz [D]) and, during the slide, the gate's state rule (AIR_SLIDE passes only
  as a recovery). An Air Slide that ends on the ground writes no cooldown.
- **Second jump**: End clears pawn+0xE48 bit 0 unless the next state is a Jump. With a Jump next, the
  Jump decides: a fall (`Fall` → sub-step 3) leaves the bit as it is; a jump requested out of the slide
  (`JumpRequest` state AIR_SLIDE: type 3) clears it in `Jump::Begin` (0x141386DAA). So after a slide
  that chains into an attack, a guard, a magic or (on the ground) Idle, the air jump is gone until a
  setter runs (landing, `Idle::Begin`, …); after a slide that ends in a fall it is still available if
  it was before the slide.

##### Aerial Recovery

Entry: state DAMAGE, jump button pressed → `JumpRequest` 0x14143D8F0, case 19 (0x14143E022):

```
jump gate 0x14142A3D0, state DAMAGE (table 0x14142B01C): nothing queued, and
    type 1, 9: m_bEnableChange | type 2: (Damage+0x8E && HasAbility(1)) || m_bEnableChange
    | types 7, 8, 10: Damage+0x8E && HasAbility(AIR_RECOVERY 1) | type 19: sub == 4
JumpRequest: if Damage+0x8E != 0 && type(+0x88) in {2, 7, 8, 10} (bt 0x584, 0x14143E089) && controller && HasAbility(1):
    st = SetState(AIR_SLIDE)                                               // queue cleared
    st.flags = (st.flags & ~8) | 0x20 | (InWaterFlag() ? 8 : 0)            // 0x14143E123..0x14143E134
    st+0x98 = actor of the weak pointer Damage+0x1C4 if it is a character pawn, else null     // the attacker
    return true
else: the ordinary airborne branch (type 3 air jump if allowed)
```

`Damage+0x8E` ("recoverable"), Damage side at entry level: the per-kind damage ticks (slots 117–121:
0x14135B300, 0x14135EEA0, 0x14135C410, 0x14135CAB0, 0x14135A870) set it to 1 in their airborne
blow phases once the elapsed time of the reaction exceeds a threshold — `comiss [+0x138],[+0x12C]; jae
skip` (e.g. 0x14135C632) with `+0x12C += dt` each Damage tick (0x1413554DF) and `+0x138 = 0.2 s`, or
0.4667 s when `Damage+0x1C0 & 8` (slot 114 0x141365930, 0x141366004–0x14136603E); the same phases clear
it when the reaction lands (e.g. 0x14135C9F0). The Damage tick tail clears it every tick while
`Damage+0x1C0 & 8` (0x1413554D8–0x1413554F8), so a hit carrying that flag is never recoverable. Who
sets `+0x1C0` bit 3: not in the Damage class's own functions [? combat strand].

In the state: Begin faces the attacker (or keeps the facing when there is none), does **not** consume
the air-slide bit, arms the `BLOW_COUNTER` command (ids 2 / 0x27) if equipped; sub-step 0 zeroes the
velocity, disables gravity and plays `AirRecovery` (in water: `AirDodge`, which replaces it in the same
tick); the tick has no stick exit for a recovery; from `TresEnableChange` (0.2667 s) a queued state
chains (the air-slide gate accepts AIR_SLIDE-as-recovery with `m_bEnableChange`, so guard button + stick
gives an immediate Air Slide; jump: gate case AIR_SLIDE 0x14142A595: bit 0 && `DOUBLEFLIGHT` (0x11) && `m_bEnableChange`, then
`JumpRequest` 0x14143DE05: yaw = AirSlide+0x8C, type 3);
at 0.5 s → `Fall`. End: no cooldown, Falling, gravity on, reaction restored, second-jump bit cleared
unless a Jump follows; no weapon draw for the plain Keyblade.

---

#### Glide (`TresPlayerState_Glide`, id 4)

Vtable 0x14692E478; interfaces are the plain ones (no notify handler, landed and move-blocked handlers
empty).

| Field | Type | Meaning |
|---|---|---|
| +0x88 | u8 | "jump held this frame" (set by `TryGlide`, cleared at the end of each tick) |
| +0x8C | f32 | target yaw |
| +0x90 | f32 | stick magnitude (stored, not read by the state) |
| +0x94 | u8 | cleared in Reset, unused |
| +0x98 | f32 | last yaw delta (blend-space X) |

**Reset** 0x14138F4E0: `+0x88 = 0; +0x8C = +0x90 = 0; +0x94 = 0; +0x98 = 0`.

##### Request

Controller: jump key down and not pressed this frame → `TryGlide(yaw, mag)` slot 67 0x14143C5B0 (before
`JumpHeld`; a true result returns from the input update, so the stick is not forwarded: `MoveInput` has
no GLIDE case anyway, 0x141441268).

```
gate slot 66 0x141429390:
  CanAct && !(pawn+0xE62 & 1) && !restrict bit 9 (vt+0x1F78) && !(pawn+0xE34 > 0) (vt+0x1E10) && HasAbility(GLIDE 0x14)
  && NextState() == null
  id 3 JUMP : Jump+0xF8 == 0 && (sub in {4, 6, 8} || pawn+0x1460 bit 0)      // lea eax,[sub-4]; test eax,~6; then cmp sub,0xA (0x14142947A..0x141429493)
  id 4 GLIDE: pass
  && !(restrict bit 17)
TryGlide 0x14143C5B0:
  JUMP : st = SetState(GLIDE)      // 0x1405B9FE0: queue cleared, Reset runs; begins in this frame's MainProc
  GLIDE: st = current state
  st+0x88 = 1
  if mag > 0.5: st+0x8C = yaw      // comiss mag,0.5; jbe skip (0x14143C680)
  st+0x90 = mag
  return true
```

- `pawn+0xE34` (glide lock): set to pawn+0xC68 (0.6 s, ctor 0x141423D9E) by pawn slot 981
  0x141432780(true) (a combat-state reset; callers: combat strand [?]); zeroed by `Jump::End`
  (0x14138DA0F), Swim End and FireMagic End; counted down at 0x14144A016.
- `pawn+0x1460` bit 0 is set only by the WeaponChange tick (0x1413DA032): a Keyblade switch in the air
  re-enters the glide from any Jump sub-step.
- Jump sub-steps 4 / 6 are the falling ones, 8 the dive-fall loop: the glide cannot start while rising.
  A button held since the jump press starts gliding on the first frame after the Jump reached
  sub-step 4 (the controller runs before the state tick).

##### Begin 0x141384D20

```
Sheathe(1, 1.0)                                                             // vt+0x1358, 0x141384DEF (no-op for the plain Keyblade)
StopMovement()                                                              // 0x141384DFC: Velocity = 0 on ALL axes, Acceleration = 0
SetMovementMode(3 Falling, bStop 0)                                         // 0x141384E10 (no-op when already Falling)
gravity on (vt+0x1728(1))                                                   // 0x141384E1F
GravityScale = pawn vt+0x1AC8() = pawn+0xB90 MyGlideGravityScale            // 0x141384E39
MaxWalkSpeed = pawn vt+0x1AD0() = pawn+0xB94 MyGlideSpeed                   // 0x141384E53
sub = 1;  +0x8C = actor yaw                                                 // 0x141384E5D / 0x141384E6E
if pawn+0x1460 & 1:      // resumed after a weapon change
    if the current animation is not "GlideLoop" (or is not the instance this slot plays): Play("GlideLoop", -1, mode 3, group 0, 1.0, (1,1)); blend X = 0
    pawn+0x1460 &= ~1
else:
    Play("GlideLoop", blendIn -1, mode 3, group 0, rate 1.0, scale (1,1)); ANIM vt+0x120(0, 0, 0, 0)     // 0x141385124 / 0x14138516C
pawn+0xE38 = pawn+0xC6C (0.6)                                               // air-slide cooldown, vt+0x1E18, 0x141385176
pawn+0xE3C = pawn+0xC70 (0.3333)                                            // air-guard cooldown, vt+0x1E28, 0x14138518B
```

##### Tick 0x141391D00

```
if InWater(0.0): if PushState(102 SWIM): DoChain; return                    // 0x141391D6D (+0x88 is not cleared on this path)
pawn vt+0xDC8(dt * 12); pawn vt+0xDB8(dt * 12)
if sub != 1: if IsAnimEnd(0): DoChain                                       // unreachable: Begin always sets 1
elif +0x88 == 0: DoChain                                                    // button not held this frame (0x141391E09)
else:
    d = +0x8C - actorYaw;  +0x98 = d
    if d > 180: d -= 360   elif d < -180: d += 360;  +0x98 = d              // comiss d,180; jbe / comiss d,-180; jae (0x141391ECF..)
    ANIM vt+0x120(d, 0, 0, 0)                                               // GlideLoop blend-space X
    Turn((0, +0x8C, 0), pawn vt+0x1AD8() * dt * 60)                         // MyGlideTurnRate; 0x141391F4B..0x141391F5A
    SetMaxAcceleration(pawn vt+0x1AD0() * 60)                               // MyGlideSpeed * 60; 0x141391F6D
    SetMovementInputXY(RotVec(actor rotation after the Turn), 1.0)          // 0x141392037: travel follows the facing, not the stick
+0x88 = 0                                                                   // 0x14139204D
```

There is **no ground test, no wall test and no timer** in the tick.

##### End 0x14138C840

```
SetMovementMode(3 Falling, bStop = (NextId() != 32 WEAPONCHANGE))           // 0x14138C8A8: velocity and acceleration zeroed unless a weapon change follows
ResetMaxWalkSpeed (vt+0x1578);  ResetGravityScale (vt+0x1658);  gravity on
if !(pawn+0x518 & 1) && pawn vt+0x1360() && NextId() not an attack / damage / weapon-change / cinematic id: Draw(1, 1, 1.0)
        // vt+0x1360 is false for the plain Keyblade
```

##### What actually ends a glide (question Q2)

Statically, the only exits are: the button released (tick: `DoChain`), water (tick), and a request that
replaces the state from outside (guard gate and air-slide gate accept GLIDE as an airborne state; attack
and magic requests: combat strand; damage). The jump gate has no GLIDE case (table 0x14142AF78: id 4 →
fail, 0x14142AA45); a new press needs a release first, and the release already ended the glide.

- **Landing**: `PhysFalling` → `ProcessLanded` 0x140591DC0 → pawn `Landed` (vt+0x9C0 0x1405ADF70: a
  script event with no Sora implementation [A], then pawn interface vt+0x258 0x1405B37B0 → current
  state's second interface vt+0x258) — for Glide that slot is the empty function 0x14041F120; then
  `SetPostLandedPhysics` → Walking. No other code refers to the Glide class (only the constructor
  registration and `TryGlide` call its class getter; no state-id-4 special case after
  `GetCurStateID` / `NextId` anywhere in the game code, scan `idscan.js`). **The state therefore stays
  GLIDE in Walking mode** while the button is held: `MaxWalkSpeed` 1500, `MaxAcceleration` 90000, input
  = facing, the `GlideLoop` pose playing, turning as in the air — Sora skims the ground at 1500 cm/s.
  On release: `End` forces Falling with zero velocity even on the ground → `Idle::Begin` returns (not
  on ground) → Idle tick → `Fall` → Jump sub-step 3 → the pawn drops the 2.15 cm floor offset and lands
  through the normal landing clip. Because gravity is ×0.001 and the vertical velocity starts at 0, a
  glide only reaches the ground where the ground rises to meet it. [the conclusion is I for the code
  paths; a runtime trace should confirm it — see "Still needs the running game"]
- **Wall contact**: the falling slide of movement spec 4.4; `Velocity` is never recomputed from the
  displacement, and the move-blocked handler of Glide is empty: Sora slides along the wall at the
  commanded 1500 cm/s for as long as the button is held.

##### Model and numbers

- Movement mode Falling, `GravityScale` 0.001 [A: `MyGlideGravityScale`, native 0.03 at 0x1414237CB],
  speed 1500 [A: `MyGlideSpeed`, native 1200], turn rate 0.03 [A: `MyGlideTurnRate`, native 0.025].
- Gravity: `−980 × 0.001 = −0.98 cm/s²`. The jump-phase hook (movement spec 4.3) is still armed when the
  glide starts inside the 0.15 s hang of a jump (the mode does not change, so `SetJumpDurationFinish` is
  not called): the remaining hang sub-steps use `× 0.1` (−0.098 cm/s²), then −0.98. A glide out of a
  ledge fall has −0.98 throughout. Sink after t seconds ≈ 0.49 t² cm: 12 cm after 5 s, 49 cm after 10 s
  [D].
- Horizontal [D, 60 Hz, from movement spec 4.4 with `AirControl` 0.95]: the entry velocity is discarded
  (`StopMovement`); acceleration `90000 × 0.95 = 85500 cm/s²` toward the facing; `|V|` = 1425 after the
  first movement tick, 1500 (cap = `MaxWalkSpeed`) from the second. Displacement per tick (midpoint
  rule): 11.875, 24.375, then 25 cm.
- Steering: target yaw = actor yaw at Begin, replaced by the stick yaw on frames with stick magnitude
  > 0.5; facing error × (1 − 0.03·dt·60) per tick; the velocity direction follows the facing through the
  air-control acceleration (friction 0: it closes 1425 / 2925 = 48.7 % of the remaining angle per 60 Hz
  tick [D]).
- Blend space `GlideLoop` [A]: axis −180..180 (the wrapped yaw delta), samples −180 / −90 `GlideLoopL`
  0046, 0 `GlideLoopF` 0043 (1.0 s, `m_InterpolationTime` 0.33, `vs_spd_lin2` ×2), 90 / 180 `GlideLoopR`
  0045; sample interpolation 0.3 s.
- Begin also starts the air-slide (0.6 s) and air-guard (0.3333 s) cooldowns: neither is possible in
  the first 0.6 / 0.33 s of a glide.
- After the release: velocity zero, default gravity (1.5, plain: the hang is not re-armed), Jump
  sub-step 3 → down loop; air control 680 / 3672 as in any fall.

---

#### Superslide (`TresPlayerState_SuperSlide`, id 21)

Vtable 0x14694BC58; anim interface plain; second interface 0x14694BFE8 (move-blocked handler
0x1413DC940, landed handler empty).

| Field | Type | Meaning |
|---|---|---|
| +0x88 | u8 | stick flag (`MoveInput`; not read by the tick) |
| +0x8C | f32 | target yaw |
| +0x90 | f32 | time left |
| +0x94 | u8 | sliding (1 from Begin, 0 once `SlideEnd` plays) |
| +0x98 | i32 | kind: 1 Superslide, 2 sonic slide, 3 / 4 other forms |
| +0x9C / +0xA0 / +0xA4 | f32 | speed / minimum speed / brake |
| +0xA8 | u32 | flags: 1 start in the end clip, 2 yaw given, 4 form weapon (kinds 0xC / 0x12), 8 aim at a target (set only by the pole state, 0x1415104C5) |
| +0xAC | vec3 | position at Begin |

**Reset** 0x1413D0E40: `+0x88 = 0; flags &= ~0xF; +0x8C = +0x90 = 0; +0x94 = 0; +0x98 = +0x9C = 0;
+0xAC = 0`.

##### Request

Controller: guard button pressed → first call `TryFlow(1, 0)` iface slot 72 0x1414417B0 → gate slot 71
0x14142E320.

```
gate 0x14142E320:
  player data (pawn vt+0x8C8) != null && (i8)[data+0x95B0] < 0        // flowmotion unlock bit 7; cmp byte,0; jge fail (0x14142E349)
  && pawn type (vt+0xD70) == 1 && CanAct && !InWaterFlag && !(restrict bit 17)
  id 3 JUMP             : Jump+0xF8 != 0 && pawn bit 4 (vt+0x1EA0) && HasAbility(SUPERSLIDE 0x13)       // 0x14142E423..0x14142E44F
  id 8, 23, 24          : HasAbility(0x13)                               // WALLRUN, WALL_KICK, POLE_TURN
  id 22, 138 RAIL       : HasAbility(0x13) && state vt+0x3A0()
  others                : fail                                           // table 0x14142E4E8 / 0x14142E4D8
TryFlow(kind, bKeep) 0x1414417B0 (table 0x141441A8C / 0x141441A6C):
  JUMP      : ClearQueue; PushSuperSlide(kind, null); return true
  WALLRUN   : WallRun+0x98 |= 0x40; return FALSE                         // 0x141441883: latched; the controller goes on to
                                                                         // TryAirSlide / guard / dodge in the same frame
  WALL_KICK : WallKick+0xA5 = 1; return true
  POLE_TURN : if stick magnitude > 0.5: Pole+0xD4 = 1;  Pole vt+0x390(); return true
  RAIL (22) : if !bKeep: ClearQueue;  PushSuperSlide(kind, null); return true
  138       : state vt+0x3C8()
PushSuperSlide(kind, pYaw) iface slot 73 0x141445B90:
  st = PushState(21)
  if weapon kind in {0xC, 0x12}: equipment vt+0x508(0, 1, 0); st.flags |= 4
  if pYaw: st.flags |= 2; st+0x8C = *pYaw
  st+0x98 = kind
```

`MoveInput` case SUPER_SLIDE (0x141440A15): `+0x8C = yaw; +0x88 = 1` on every frame with an active
stick — **the stick steers the slide** (the tick turns toward +0x8C).

##### Begin 0x1413CC460

```
pawn bit 4 = 0 (vt+0x1E98(0))                                               // 0x1413CC490
StopMovement()                                                              // 0x1413CC49D
SetMovementMode(5 Flying, bStop 1)                                          // 0x1413CC4B2
gravity off (vt+0x1728(0))                                                  // 0x1413CC4C1
Draw(1, 1, 1.0)                                                             // 0x1413CC4E0
if !(flags & 4): equipment vt+0x508(1, 0, 0)                                // pawn vt+0x1388 [what it toggles: ?]
sub = 0;  +0x94 = 1
+0x90 = pawn vt+0x1AA8()        // 0x1414354B0: count(SUPERSLIDE) <= 1 -> pawn+0xBDC, 2 -> +0xBE0, else +0xBE4 (all 1.0)
+0x9C = pawn vt+0x1AB0() = pawn+0xBE8 MySuperSlideSpeed
+0xA0 = pawn vt+0x1AB8() = pawn+0xBEC m_SuperSlideMinSpeed
+0xA4 = max(pawn vt+0x1AC0() = pawn+0xBF0 MySuperSlideBreak, 0)
+0x9C = max(+0xA0, +0x9C)                                                   // 0x1413CC576..0x1413CC592
if !(flags & 1):
    Play(pawn vt+0x1C90(kind): 1 "SuperSlideLoop", 2 "SonicSlideLoop", 3 "SuperFlightLoop", 4 "AirDodge", else "GlideLoopF",
         blendIn 0.0, mode 1, group (weapon kind 0x12 ? 0x7DA : 0), 1.0, (1,1))                       // 0x1413CC60F
else: Play("SlideEnd", -1, mode 2, 0, 1.0, (1,1)); sub = 2; +0x94 = 0
+0xAC = actor location
slot113()                       // 0x1413CD270: acts only if flags & 8 (pole launch): nearest valid target with d² < 6250000 (2500 cm),
                                // lock-on targets (controller+0x8F4 / +0x8F8) first, inside dot >= 0.643 of the direction of +0x8C -> +0x8C = yaw to it
if flags & 2: Turn((0, +0x8C, 0), 1.0)                                      // 0x1413CC773
else:         +0x8C = actor yaw                                             // 0x1413CC784
component vt+0xBD0 (cancel a driven move)
```

##### Tick 0x1413D7C40

```
pawn vt+0xDC8(dt * 12); pawn vt+0xDB8(dt * 12)
if sub != 0:                                                                // end clip
    if IsAnimEnd(0):
        if !OnGround && pawn is a player pawn: pawn+0xE38 = pawn+0xC6C (0.6)        // air-slide cooldown, 0x1413D7D1D
        DoChain
elif !(+0x90 > 0):                                                          // xorps; comiss 0,[+0x90]; jae (0x1413D7D3B)
    StopMovement()
    Play("SlideEnd", blendIn 0.05, mode 2, group 0, 1.0, (1,1))             // 0x1413D8084 / 0x1413D80B1
    sub = 2;  +0x94 = 0
else:
    Turn((0, +0x8C, 0), dt * 1.5)                                           // 0x1413D7D64
    +0x9C = max(+0xA0, +0x9C - dt * +0xA4)                                  // 0x1413D7D88..0x1413D7DA7
    v = max(+0x9C, 0.1) * RotVec(actor rotation)                            // after the Turn; 0x1413D7DB6
    component vt+0x9E8(v)                                                   // pawn vt+0x16B0: direct velocity XYZ, 0x1413D7E7D
    +0x90 -= dt
    if pawn vt+0x1AA8() - 0.1 > +0x90:                                      // comiss (total-0.1),[+0x90]; jbe skip (0x1413D7EAC)
        P = actor location + RotateVector(actor rotation, (120, 0, 90))
        if pawn vt+0x2330(P, P, 100.0, out): TryPoleTurn(out, 0)
```

**Move-blocked handler** 0x1413DC940: only when `total − 0.1 > time left` (0x1413DCA6A): the pole probe
(`TryPoleTurn`); if no pole turn started and `+0x94 != 0` (still sliding): `TryWallKick(hit actor, hit
component, &ImpactNormal, 1, 0)`.

##### End 0x1413CF350

`SetMovementMode(3 Falling, bStop 1)` (velocity and acceleration zeroed); gravity on; equipment
vt+0x508(0, 0, 0); pawn+0x660 = 0.

##### Clips [A] and numbers

| Clip | Asset | Length | Root motion | Notifies |
|---|---|---|---|---|
| `SuperSlideLoop` | 1152 | 0.3333 s, loop (mode 1) | none | `se00001_039` (attached, fade-out 1 s), effect `vp_ffa_rad0` (group 3200), voice |
| `SlideEnd` | 1157 | 0.7667 s (24 keys), `m_InterpolationTime` 0.03 | forward 426 cm: 0, 33.26, 104.85, 172.5, 221.82, 258.01, 280.71, 298.75, 313.04, 324.48, 334.37, 343.46, 351.9, 360.2, 368.48, 376.81, 385.07, 393.18, 401.03, 408.75, 415.6, 420.8, 424.4, 426 | `se00001_015` at 0.0114 (pitch 1.2); no `TresEnableChange` |

Tunables [A]: `MySuperSlideSpeed` 4000 (native 2000), `m_SuperSlideMinSpeed` 1000 (native 2000),
`MySuperSlideBreak` 1000 (native 2000), `MySuperSlideTime` / `Lv2` / `Lv3` 1.0 (native, 0x141423886..).

- Flying mode, gravity irrelevant, velocity = direct value: per tick the pawn moves exactly
  `speed × dt` along its facing (pitch and roll are driven to zero by the two head calls, so the path is
  level unless the actor was pitched at Begin).
- Speed at 60 Hz [D]: the n-th tick (n = 1 is the tick in the `MainProc` call of Begin) writes
  `4000 − n × 16.667`; 60 writes (61 if the f32 countdown leaves a positive residue), 3983.3 → 3000;
  distance Σ = 3491.7 cm (continuous: 3500), then the tick that sees `time ≤ 0` stops the pawn and plays
  `SlideEnd`: 426 cm more, level (Flying, root motion in the flying delta), 0.7667 s without any cancel
  notify. Total ≈ 3918 cm in ≈ 1.78 s.
- Steering: `Turn(dt × 1.5)` → 2.5 % of the remaining angle to the stick yaw per 60 Hz tick; without
  stick input the target is the facing at Begin (flow jump) or the given yaw (pole / wall kick).
- Jump out (jump gate case SUPER_SLIDE 0x14142A7EA: pawn bit 3 (vt+0x1E90) && `SUPERJUMP` 0x12) →
  `JumpRequest` type 2 with the facing as launch direction [C for the request side]; `Jump::Begin` for type 2 clears bits 3 and 0 (0x141386E12 / 0x141386E21).
  Because End zeroes the velocity, the jump's horizontal motion comes only from its own momentum
  parameters.
- Ending in the air writes the 0.6 s air-slide cooldown.

---

#### Floating (`TresPlayerState_Floating`, id 5)

Vtable 0x14691E048; interfaces plain (no handlers).

| Field | Type | Meaning |
|---|---|---|
| +0x88 | u8 | stick flag (set by `MoveInput` when magnitude > 0.5) |
| +0x8C | f32 | target yaw |
| +0x90 | f32 | stick magnitude |
| +0x94 | u8 | unused here |
| +0x95 | u8 | 1 = more than 30 cm above ground (written each tick, not read by the state) |
| +0x98 | rotator | smoothed travel heading |
| +0xA4 | u8 | coasting (skip-`CalcVelocity` flag is on) |
| +0xA8 | f32 | coast speed |

**The mode switch** pawn+0xD80: bit 0 = floating on, bit 1 = "special". Written only by controller slot
705 (vt+0x1608) 0x1413EA6C0 = native of the script function
`ATresPlayerControllerBase::BP_NotifySetFloatingMode(IsOn, IsSpecial)` (exec 0x1428764B0):
`pawn+0xD80 = (pawn+0xD80 & ~3) | IsOn | IsSpecial << 1` (0x1413EA755–0x1413EA771), pawn = the
controller's pawn cast to `TresPlayerPawnBase`. The constructor clears bits 0–3 (0x141423A8F). The
pawn's own `DebugSetFloatingMode` (vt+0x26D8) is an empty function in this build. The only cooked packages that call it are two level scripts [A]:
`Levels/bt/bt_04/umap/bt_04_gameflow` and `bt_04_camera_ms040` — floating is a scripted set piece of that
map, not a field mechanic (no blueprint under `Content/Blueprints` calls it).
Getters: bit 0 pawn vt+0x1BD8; speed pawn vt+0x1AE0 0x1411093B0 = bit 1 ? pawn+0xB94 (`MyGlideSpeed`
1500) : pawn+0xB9C (`MyFloatingSpeed` 680, native 0x1414237E9, no override); gravity scale pawn
vt+0x1AE8 0x141109390 = bit 1 ? pawn+0xBA4 (`MyFloatingGravityScale2` 0.6 [A `p_ex001_Pawn`], native
0.3) : pawn+0xBA0 (`MyFloatingGravityScale` 0.01 [A `p_ex001_PawnBase`], native 0.3).

While bit 0 is set `OnGround()` is false for every caller (it tests the bit first).

**Entries** (`PushState(5)` when `Floating()`): `Idle::Begin` 0x14138610B, Idle tick 0x141394598,
`Jump::Begin` 0x141386A27 (fall entry), Jump tick tail 0x1413981C1.
**Reset** 0x14136ED60: `+0x88 = 0; +0x8C = +0x90 = 0; +0x94 = +0x95 = 0; +0x98 = (0,0,0); +0xA4 = 0;
+0xA8 = 0`.
`MoveInput` case FLOATING (0x141441099): `if mag > 0.5: +0x8C = yaw; +0x88 = 1` · `+0x90 = mag` always.

##### Begin 0x14136ABE0

```
Sheathe(1, 1.0)
SetMovementMode(3 Falling, bStop 0)                                         // 0x14136AC94: the entry velocity is KEPT
GravityScale = pawn vt+0x1AE8()                                             // 0x14136ACB4
sub = 0;  +0x8C = actor yaw
controller vt+0x1830(0); controller vt+0x1820(0)                            // lock-on / camera requests [not read]
```

##### Tick 0x14137A680

```
pawn vt+0xDC8(dt * 12); pawn vt+0xDB8(dt * 12)
if !Floating(): DoChain; return                                             // 0x14137A71D: -> Idle -> Fall
+0x95 = 1
if GroundProbe(pawn, 0, range 800, out) && out.dist <= 30:                  // 0x14063A650; comiss dist,30; ja skip (0x14137A78D)
    pawn bits 0, 1, 2, 3, 4 of +0xE48 = 1                                   // second jump, air slide, air guard, flow jump, superslide
    +0x95 = 0
switch sub:
 0: Play("GlideIdle", -1, mode 3, group 0, 1.0, (1,1));  +0xA4 = 0;  +0xA8 = 0;  sub = 1          // 0x14137AE9D
 1: if +0x88:                                                               // stick > 0.5 this frame
        if +0xA4: +0xA4 = 0; skip-CalcVelocity off (vt+0x16D8(0))
        Play("GlideLoop", -1, mode 3, group 0, 1.0, (1,1));  pawn anim interface vt+0x120(0, 0, 0, 0)   // blend X = 0
        +0x98 = actor rotation;  sub = 2
    Turn((0, +0x8C, 0), dt * 6)                                             // 0x14137AE0A
    if +0xA4:                                                               // coasting
        +0xA8 *= 0.98                                                       // per tick, no dt (0x14137AE46)
        component vt+0x9F8(+0xA8 * RotVec(+0x98))                           // direct velocity XY, 0x14137AE92
 2: if +0x88:
        d = wrap180(+0x8C - actorYaw);  ANIM vt+0x120(d, 0, 0, 0)
        Turn((0, +0x8C, 0), dt * 6)
        +0x98 = Rotator(normalize(Slerp(Quat(+0x98), Quat((0, +0x8C, 0)), clamp01(dt * 3))))      // 0x14137A998..0x14137AAAB
        s = pawn vt+0x1AE0()
        SetMaxAcceleration(s * 60);  MaxWalkSpeed = s                       // 0x14137AAC9 / 0x14137AAE3
        SetMovementInputXY(RotVec(+0x98), 1.0)                              // 0x14137AB13
    else:                                                                   // stick released
        skip-CalcVelocity on (vt+0x16D8(1));  +0xA4 = 1
        +0xA8 = |measured velocity XY| (component+0x9A0)                    // 0x14137AB37..0x14137AB65
        component vt+0x9F8(+0xA8 * RotVec(+0x98))
        Play("GlideIdle", -1, mode 3, group 0, 1.0, (1,1));  sub = 1
 other: if IsAnimEnd(0): DoChain
+0x88 = 0
```

##### End 0x14136CF30

`if +0xA4: +0xA4 = 0; skip-CalcVelocity off` · `ResetMaxWalkSpeed` · `ResetGravityScale` · gravity on ·
`SetMovementMode(3, bStop 0)` · `if pawn vt+0x1360() && next state is neither attack nor Damage:
Draw(1, 1, 1.0)` (never for the plain Keyblade).

##### Model and numbers

- Falling mode with gravity `−980 × 0.01 = −9.8 cm/s²` (special: × 0.6 = −588): a slow sink; the state
  never zeroes the velocity, so the vertical speed at entry carries over (terminal 4000).
- Moving (sub 2): the velocity is steered through air control toward a heading that lags the stick yaw
  (slerp `3·dt` per tick; the actor itself turns with `6·dt`); `MaxWalkSpeed` 680 (special 1500),
  acceleration `s × 60 × 0.95` → 646 cm/s after one 60 Hz tick, 680 after two [D].
- Released (sub 1, coasting): `CalcVelocity` is bypassed and the state rewrites the horizontal velocity
  each tick with a speed that decays × 0.98 per tick (frame-rate dependent as written; at 60 Hz
  half-life 34.3 ticks = 0.57 s [D]); gravity still integrates.
- Clips [A]: `GlideIdle` 0047 (1.6667 s, `m_InterpolationTime` 1.2, loop mode 3), `GlideLoop` blend
  space as in Glide.
- Out of Floating: jump (gate: id 5 passes unconditionally, table 0x14142AF78 → 0x14142AE4E;
  `JumpRequest`: type 3 [C]), guard (gate: yes), Air
  Slide (gate: treated as airborne; its End then writes the cooldown and re-applies the floating gravity
  scale), attack (combat strand). Near the ground (≤ 30 cm under the probe) all five availability bits
  are refreshed every tick. The state ends when the script clears bit 0.

---

### Corrections to Part I

| Section | Part I says | Correct |
|---|---|---|
| 2.4, 16 P6 | slots 726 / 728: "velocity / launch-velocity write; which is which: [?]" | both write `Velocity` at once **and** a one-shot "direct velocity" record that overrides the result of the next `CalcVelocity`: vt+0x9E8 (726) all three axes, vt+0x9F8 (728) X and Y only. There is no separate launch velocity. |
| 2.4 | slot 733 `StopMovement` (movement spec 3.1: "does not touch Acceleration") | also zeroes `Acceleration` (component vt+0x4E0 0x140597BA0) and clears the direct-velocity record |
| 2.4 | slot 756 `SetMovementMode(mode, b)` | `b` = stop first: velocity and acceleration zeroed (used by `Glide::End`, `SuperSlide::Begin` / `End`) |
| 2.3 table | bit 0 cleared by "`AirSlide::End` when the next state is not Jump" only; Q4 / P4 open | also cleared by `Jump::Begin` for type 3 (0x141386DAA–0x141386DB0: the double flight consumes it; other non-fall begins **set** it) and for type 2 (0x141386E21, with bit 3), and by RailSlide slot 135 (0x14150AEC3). Complete setter list: section "Second jump" above / NOTES. P4 is settled statically. |
| 5.2 | air-jump gate: "pawn+0xE48 bit 0 && DOUBLEFLIGHT && !(Jump+0x114 & 8)" | the gate needs the flag **set**: `test [Jump+0x114],8; jne success` (0x14142A589 → 0x14142AA3F). Jump slot 113 0x1413837F0 maintains it: `flags &= ~8; if Jump+0x9C == 0 && Velocity.Z <= 500: flags \|= 8`; a fall-begun Jump in the air starts with it set (0x141386D48). (Jump strand to integrate.) |
| 6.2 | "Plain slide: not in battle → Sheathe, else Draw" | plain Air Slide: not in battle → `Sheathe` (no-op for kind 1); in battle → nothing. `Draw` only for the flagged variants (Aerial Dodge, slide shot, water, back slide). Nothing after the controller test runs without a controller. |
| 6.2 | "Air-slide bit cleared" | not for a recovery (flag 0x20) |
| 6.3 | pole probe "point ahead, 100", every tick | point = actor location + actor rotation · (120, 0, 90), range 100; skipped on a tick that chains; the same probe plus `TryWallKick` also runs from the state's move-blocked handler 0x1413432C0 (answers part of Q6: wall contact in AIR_SLIDE and SUPER_SLIDE reaches `TryWallKick` through pawn `MoveBlockedBy` 0x1405D2240) |
| 6.4 | cooldown "if not recovery and (airborne or floating)"; "Movement mode Falling" | plus the exception (pawn kind 0x11 re-sliding); in water the mode is set to Swimming instead; when floating the gravity scale is set to the floating value; End does not touch the velocity |
| 6.5 | (cooldown / bit 1) | bit 1 returns one pawn tick after Begin; only the 0.6 s timer from End limits repetition |
| 6.6 | "yaw = current, or toward the target for the 0x100 variant" | both recovery variants face the stored target (the attacker, from `JumpRequest`) when there is one, else keep the facing |
| 6.6 | "End re-draws the weapon unless an attack or damage follows" | only when pawn vt+0x1360() — false for the plain Keyblade |
| 6.6 | in water | the `AirRecovery` clip is replaced by `AirDodge` (700 cm root motion) in the same tick |
| 6.6 | "Damage+0x8E set" (writer unknown) | set by the damage sub-ticks once the reaction has lasted > 0.2 s (0.4667 s and never, when Damage+0x1C0 bit 3) |
| 7 | gate: "sub-step 4 or 6" | 4, 6 or 8 |
| 7 | "cap 1500 reached in the first tick" | 1425 after the first movement tick, 1500 from the second (60 Hz) |
| 7 | End: "movement mode Falling, ResetMaxWalkSpeed, ResetGravityScale, gravity on" | End also **zeroes the velocity** (`SetMovementMode(3, bStop)`), except when a weapon change follows |
| 7, Q2 | "Landing while gliding and wall contact: [?]" | nothing ends the glide: see "What actually ends a glide" |
| 7 | pawn+0xE34 "glide gate" | a 0.6 s glide lock written by pawn slot 981; `MoveInput` does not feed the Glide state at all (only `TryGlide`) |
| 8 | "slot 113 aims at the nearest lock-on target … → yaw; else yaw = facing" | the aim runs only with flag 8 (pole launch). From a flow jump the yaw is the facing at Begin, and **the stick steers** (`MoveInput` writes +0x8C; `Turn(dt × 1.5)`) |
| 8 | Begin "movement mode Flying" / End "movement mode Falling" | both with bStop = 1 (velocity zeroed) |
| 8 | TryFlow from WallRun | latches the request and returns **false**: the same guard press continues to `TryAirSlide` / guard / dodge |
| 8 | wall contact | move-blocked handler 0x1413DC940 (after the first 0.1 s; wall kick only while +0x94) |
| 10 | "Movement mode during all of them: Flying" (how Flying moves: unspecified) | `PhysFlying` 0x14058E570 specified above |
| 12 | Floating: "pawn+0xD80 bit 0 (set by volumes / script)"; entry only | full specification above; the writer is `BP_NotifySetFloatingMode(IsOn, IsSpecial)`; speed 680 / 1500 |
| 16 Q1 | sections 6–8 [C] | now [I] |

### Host affordances

| Probe | Exact data read | Needed to place the feature |
|---|---|---|
| Water (Air Slide Begin / End, Glide tick) | pawn vt+0x1A80(0.0) → component vt+0x5A0 0x140589410: `component vt+0x420()` (current physics volume has the water bit, volume+0x42C bit 0) `&& vt+0x678() > max(vt+0x680() + 0, 0)` (immersion depth against a threshold) | a water volume; none → all "in water" branches are off |
| Ground probe (Floating tick) | 0x14063A650(pawn, 0, 800): sphere radius 50 swept from origin + (0,0,55) straight down 800 cm, channel Pawn, blocking WorldStatic / WorldDynamic / TresGimmick, ignoring the pawn; result `dist` = actor Z − impact Z for a surface less steep than 45° (movement spec 4.5); test `dist <= 30` | any floor |
| Pole probe (Air Slide tick + blocked handler, Superslide tick after 0.1 s + blocked handler) | pawn vt+0x2330 0x1405A7500(P, P, 100, out): iterates the registered pole components (pawn vt+0x8E0 → vt+0x9B0 array), calls component vt+0xBD0(P, 100, &dist, …), keeps the nearest that accepts; out = {owner actor (component+0x1B8), component}; P = actor location + actor rotation · (120, 0, 90) | a pole component within 100 cm (by the component's own metric) of the point 120 cm ahead and 90 cm above the actor origin; without poles the probe is a constant false and only clears AirSlide+0x88 |
| Move blocked (Air Slide, Superslide) | pawn `MoveBlockedBy` 0x1405D2240 ← `HandleImpact`: fields of the move's hit result: ImpactNormal (+0x30), TraceStart (+0x3C), TraceEnd (+0x48), actor / component weak pointers (+0x64 / +0x6C); fires when the actor is not a pawn (or is one whose component refuses, vt+0x508) and `normalize(TraceEnd − TraceStart) · ImpactNormal < 0` | a wall in the path; the host forwards (actor, component, impact normal) to `try_wall_kick` (flowmotion strand) |
| Landing | `ProcessLanded` → state landed handler: empty for all four states | nothing to supply beyond the movement core's landing |
| Lock-on targets (Superslide slot 113, pole launches only) | controller target list, ids controller+0x8F4 / +0x8F8, target position (vt+0xB88), targetable bit (+0x868 bit 0); range 2500, cone dot ≥ 0.643 | only with poles |
| Attacker (Aerial Recovery) | weak pointer Damage+0x1C4 → actor location (root component +0x2B0) | the damage system passes the attacker |
| Script switch (Floating) | `set_floating_mode(is_on, is_special)` → pawn+0xD80 bits 0 / 1 | a host command or a trigger volume in the test level |
| Abilities | `HasAbility(kind)` pawn vt+0x14E0: 1 AIR_RECOVERY, 2 BLOW_COUNTER, 0x13 SUPERSLIDE, 0x14 GLIDE, 0x17 AIRSLIDE, 0x18 AIRDODGE; counts (vt+0x1508): 0x17 (levels), 0x13 | host stubs |
| Flowmotion unlock | player data (pawn vt+0x8C8) byte +0x95B0 bit 7 | host flag |
| Battle flag | pawn vt+0x1740 (pawn+0x630 bit 1) | selects Air Slide vs Aerial Dodge |

### Tests

All at a fixed 60 Hz step, every ability equipped (AIRSLIDE ×3 → scale 1.425; in battle AIRDODGE wins),
plain Keyblade, flat floor at Z = 0, no water, no poles. "Tick n" = n-th movement tick after the request
frame (frame 0 = controller sees the input and the state begins and ticks once). Distances are measured
along the facing.

1. **Air Slide, level 3, out of battle.** Setup: jump, at the apex (Jump sub-step 4, 266 cm) push the
   stick forward and press guard. Expect: state AIR_SLIDE in frame 0; reaction type 11 from frame 0;
   `Velocity = 0`, gravity off from tick 1; cumulative forward distance at ticks 1, 2, 4, 10, 20, 30, 44
   = 142.5, 285.0, 490.8, 942.2 (= 661.2 × 1.425), 1353.8 (= 950 × 1.425), 1513.0 (= 1061.72 × 1.425 at
   t = 0.5), 1610.25 cm; Z constant (± 0.001) for the whole slide. Stick neutral after the press:
   state ends at tick 44 (clip end) → JUMP sub 3, velocity (0, 0, 0), reaction type back to default in
   that frame, pawn+0xE38 = 0.6; pawn+0xE48 bit 0 unchanged.
2. **Air Slide stick exit.** As 1 but the stick is held (magnitude 1). Expect the state to end on the
   first state tick with `m_bEnableChange` (tick 30 or 31: clip time ≥ 0.5) → JUMP sub 3 with Jump flag
   0x40 and yaw = stick yaw; distance at that tick ≥ 1512 cm; the clip keeps playing but no further
   root-motion displacement is specified by this strand (Jump strand: flag 0x40).
3. **Levels.** Repeat 1 with 1 and 2 `AIRSLIDE`: totals 819.25 and 1214.75 cm; tick 1: 72.5 and 107.5 cm.
4. **Cooldown.** After test 1 ended at frame E: guard + stick on frames E+1 … E+35 are refused
   (pawn+0xE38 > 0); accepted on frame E+36 or E+37 (timer ≤ 0). A slide that ends on the ground (start
   it 20 cm above a rising ramp so that it lands) writes no cooldown: a new jump + slide is accepted
   immediately.
5. **Aerial Dodge.** Battle flag on. Same input as 1. Expect clip `AirDodge`: forward distance at ticks
   2, 20, 42 = 100, 604.7, 700 cm; ends at tick 42; reaction type 11 for all 42 ticks; `Draw` called in
   Begin; cooldown 0.6 at End.
6. **Neutral stick.** In the air, stick neutral, press guard: `TryAirSlide` false (kind 1 needs
   magnitude ≥ 0.1) → air guard, not a slide.
7. **Second jump after a slide.** (a) jump, slide, let it end in the fall, press jump once
   `Velocity.Z ≤ 500` (immediately: it is 0): double flight starts (bit 0 was set), `Jump::Begin` type 3
   clears bit 0. (b) jump, slide, at tick 31 press jump (queued through the jump gate at
   `m_bEnableChange`): double flight; a later press in the same airtime is refused. (c) jump, double
   flight, slide, fall: a jump press is refused (bit 0 clear).
8. **Aerial Recovery.** Host puts Sora in DAMAGE type 2 (blow), airborne, attacker at yaw 90° from
   Sora; jump press at 0.1 s into the reaction: refused by the recovery branch (Damage+0x8E still 0);
   press at ≥ 0.2 s + one tick: state AIR_SLIDE with flags 0x20, facing snapped toward the attacker,
   velocity 0, gravity off, clip `AirRecovery`, reaction type 11 for the whole state, pawn+0xE48 bit 1
   still set; with no input → `Fall` at tick 30 (0.5 s), no cooldown written; with guard + stick held:
   an Air Slide starts on the first frame after 0.2667 s (tick 16 or 17).
9. **Glide.** Jump with the button held from the press (stick neutral). Expect GLIDE on the first frame
   after Jump sub-step 4 (apex frame + 1); velocity (0, 0, 0) at Begin; `|V_xy|` = 1425 after tick 1,
   1500 from tick 2; forward distance 11.875, 36.25, then +25 cm per tick; `V.z ≈ −0.98 t` (≤ 0.1 cm/s
   in the first 0.1 s of a glide begun inside the hang); height loss < 13 cm after 300 ticks; pawn+0xE38
   = 0.6 and pawn+0xE3C = 0.3333 at Begin. Stick 90° right at magnitude 1: facing error after n ticks =
   90 × 0.97ⁿ (45° after 23 ticks). Release at any tick: velocity (0, 0, 0) in that frame, state JUMP
   sub 3, GravityScale 1.5, MaxWalkSpeed 680.
10. **Glide into rising ground** (Q2). Glide toward a 20° ramp. Predicted by the code: MovementMode
    becomes Walking, state stays GLIDE, speed stays 1500 along the ramp while the button is held; on
    release: mode Falling, velocity 0, Idle → Jump sub 3 → landing clip within a few ticks. (Compare
    with the running game.)
11. **Glide into a wall.** Glide head-on into a vertical wall: state stays GLIDE, commanded velocity
    stays 1500 (position stops); at 45°: slides along the wall at 1500 × cos 45°.
12. **Superslide.** Host: flowmotion unlocked, Jump with Jump+0xF8 = 1 (flow jump) and bit 4 set; press
    guard. Expect SUPER_SLIDE in frame 0, mode Flying, gravity off, bit 4 cleared, clip
    `SuperSlideLoop`; forward distance after ticks 1, 30, 60 = 66.39, 1870.8, 3491.7 cm (speeds 3983.3,
    3500, 3000 cm/s), Z constant; then `SlideEnd`: +426 cm over 46 ticks (0.7667 s), ending ≈ 3918 cm
    from the start at ≈ tick 106; at the end in the air: pawn+0xE38 = 0.6, mode Falling, velocity 0.
    With the stick held 90° right from frame 1: facing error 90 × 0.975ⁿ. Pole probe / wall kick calls
    start at tick 7 (time left < 0.9).
13. **Floating.** Host calls `set_floating_mode(true, false)` during a fall with `V.z = −300`. Expect
    FLOATING on the next state tick, GravityScale 0.01, `V.z` continuing from −300 at −9.8 cm/s², clip
    `GlideIdle`; stick forward (magnitude 1): `GlideLoop`, `|V_xy|` 646 then 680; release: coast speed
    ×0.98 per tick from the measured speed (680 → 340 after 34 ticks) with `GlideIdle`; within 30 cm of
    the floor: pawn+0xE48 bits 0–4 all 1 each tick; `set_floating_mode(false, false)` → next tick
    `DoChain` → Idle → Jump sub 3 with GravityScale 1.5. Special mode (true, true): GravityScale 0.6,
    speed 1500.

### Still needs the running game

| Object | Offset | Type | Why |
|---|---|---|---|
| state queue current id + component | MovementMode component+0x314 | u8 per frame | Q2 confirmation: a glide carried onto rising ground — expected GLIDE with mode 1 while the button is held, then Falling → landing on release |
| component | Velocity +0x214 | vec3 per frame | glide: 1425 / 1500 ramp and Z sink (−0.098 then −0.98 cm/s²); velocity zero at Glide End; Superslide 3983 → 3000 |
| actor | location | vec3 per frame | Air Slide distances with 1 / 2 / 3 abilities (819.25 / 1214.75 / 1610.25 cm in 0.7333 s), Superslide 3492 + 426 cm |
| pawn | +0xE38 | f32 per frame | air-slide cooldown written at End (0.6) and at Glide Begin; refusal window 36 ticks |
| pawn | +0xE48 | u32 per frame | bit 0 through slide → fall → double flight (tests 7a–c); bit 1 returning one tick after Begin |
| pawn | +0x619 | u8 per frame | reaction type 11 for the whole Air Slide / Aerial Dodge / Recovery |
| Jump state | +0x114 | u32 per frame | flag 8 (air-jump permitted when `Velocity.Z ≤ 500`), after a slide's fall |
| Damage state | +0x8E u8, +0x12C f32, +0x138 f32, +0x1C0 u32 | per frame during blow reactions | the 0.2 s recovery threshold and which hits carry flag bit 3 (never recoverable) |
| component | +0xB80 f32, +0x3BC f32 | once | FluidFriction and BrakingDecelerationFlying actually present (matter only for a Flying state that neither writes the velocity nor stops) |
| pawn | +0xD80 | u32, write watch | which level scripts turn floating on (callers of `BP_NotifySetFloatingMode`) |
| pawn | +0xE34 | f32, write watch | when the 0.6 s glide lock (pawn slot 981) is applied in play |
| function entry | 0x1413432C0, 0x1413DC940 | event with state id | how often the move-blocked handlers fire on floors / walls during slides (wall-kick trigger rate) |

---

## II.3 Jump types, second jump, landing variants, stance system

(strand `jump`; working notes in `build/sora_tmp/actions/p2_jump/`)

Second pass, read in the machine code. Tags: **[I]** instruction level (address of the instruction given),
**[C]** decompiled C only, **[A]** asset, **[D]** derived, **[?]** not determined. Conventions of the movement
spec (frame order C → M → S; `comiss a,b; ja` = a > b). Dumps of every function quoted are in
`build/sora_tmp/actions/p2_jump/*.dis`.

Pawn virtuals read here in addition to Part I 2.4 [I, one-instruction functions unless noted]:

| Offset | Function | Does |
|---|---|---|
| +0x1738 (slot 743) | 0x141446CE0 | `SetBattleMode(b)` (9.1) |
| +0x1740 / +0x1750 | 0x1405AD050 | battle flag: `(pawn+0x630 >> 1) & 1` |
| +0x1748 | 0x141435A10 | `battleAnim` (9.1) |
| +0x1758 / +0x1760 | 0x141112B60 / 0x14110A530 | set / get keep-out timer pawn+0x10E0 |
| +0x1778 / +0x1780 | 0x140543410 / 0x14054EA00 | get / set last pose pawn+0x6E4 |
| +0x1788 / +0x1790 | 0x140543400 / 0x14054E9F0 | get / set land pose pawn+0x6E8 |
| +0x1330 | 0x1405ADE80 | weapon out: equipment+0x2C8 bit 1 (equipment vt+0x4A0 0x141784670) |
| +0x1360 | 0x141436BF0 | weapon kind ∉ {1, 0x16, 0x17} **and weapon not out** |
| +0x28B0 | 0x141435DE0 | weapon kind ∈ {1, 0x16, 0x17} |
| +0x1EE8 | 0x1414365D0 | weapon restricted: `(pawn+0x1130 & 8) \|\| (pawn+0xE60 & 0x8001) \|\| vt+0x1FE8(0)` |
| +0x1B48 | 0x141436040 | weapon out && kind ∈ {7, 9, 0xA} (a formchange with carried momentum) |
| +0x1388 | 0x1405BD6C0 | equipment vt+0x508 0x141790690 (dl, r8b, r9b): equipment+0x2E0 = dl, +0x2E6 bit 0 = r9b (flow-weapon display mode) |
| +0x1AA0 | 0x141434320 | `movss xmm0,[rcx+0B7Ch]` = `MyJumpPowerHighLv3` |
| +0x1AF0 / +0x1AF8 | 0x1411099E0 / 0x1411099D0 | pawn+0xBF8 `MyRailSlideCheckRange` 50 / pawn+0xBFC `MyRailSlideCheckAirRange` 150 |
| +0x1B08 / +0x1B10 | 0x140543C90 / 0x140543C80 | pawn+0x644 `m_RailSlideJumpInertialVelocity` (native 200 @0x14059FF6A, blueprint 175 [A]) / pawn+0x648 `m_RailSlideJumpInertialBrake` (native 0.92 @0x14059FF74, no override) |
| +0x1718 | 0x1405A0650 | component vt+0x778 0x140573FD0 = `AddImpulse(v, bVelocityChange)` (adds to `PendingImpulseToApply` +0x4C8) |
| +0x2648 | 0x14110E7A0 | pawn+0xBB4 bit 0 `m_bEnableDiveFall` |
| +0xA48 | 0x140545990 | pawn+0x518 bit 0 |
| +0xD70 | 0x140541A80 | byte pawn+0x5F0 (wear form / costume index [D]) |
| +0x1A10 | 0x1405AD6C0 | component vt+0x948 0x140589380 `IsJumpDurationFinish`: `(+0xBFC & 1) \|\| !(m_MaxJumpDurationTime +0xBEC > timer +0xC00)` |

Component: vt+0x9D8 (slot 315 0x1405894A0) = floor test at the current location (`FindFloor` vt+0x818; true when
the result is blocking and walkable); vt+0x540 0x140589910 = `MovementMode == Swimming`.

---

#### TresPlayerState_Jump (id 3) — types, power, second jump

Fields (additions to movement spec 4.2) [I]:

| Offset | Type | Meaning |
|---|---|---|
| +0x98 | i32 | 1 for a jump requested in the air (written by `JumpRequest`) |
| +0x9C | u8 | "no further air jump": `bAir` of the request; set to 1 on every landing and by a fall Begin on a floor. **This is the byte Part I took for Idle+0x9C** |
| +0xA0 / +0xA4 | i32 / f32 | type / power |
| +0xA8 | f32 | ledge-check delay, 0.2 s, counted on falling ticks only |
| +0xBC | u8 | "fall slide" contact flag, cleared at the end of every tick [? writer] |
| +0xE8 / +0xE9 | u8 | deferred down loop / not requested by the controller (Reset 1 @0x14138F85A, `JumpRequest` 0 @0x14143E46A) |
| +0xEC..+0xF4 | vec3 | actor location at Begin (@0x141386ECC); **+0xF4 = start Z** |
| +0xF8 | u8 | flow-jump flag (type 2 running) |
| +0xFC | u32 | bit 0: the super jump has risen (`Velocity.Z > 0` seen) |
| +0x100 / +0x10C / +0x110 | vec3 / f32 / f32 | launch direction / inertial speed / brake per 60 Hz tick |
| +0x114 | u32 | bit 0 flow jump out of Superslide / rail, bit 1 from Swim, bit 3 (8) **second jump enabled**, 4 (0x10) light landing, 5 (0x20) step-down, 6 (0x40) keep clip (Air Slide exit), 8 / 9 debug, 12 (0x1000) probe cached, 13 (0x2000) `B` clip names, 14 (0x4000) dive camera, 15 (0x8000), 19 (0x80000) scripted height |
| +0x118 | f32 | scripted jump height (`TryJump`) |
| +0x120 | f32 | step-down timer |
| +0x124 / +0x128 | u8 / f32 | formchange momentum (vt+0x1B48) |
| +0x12C | f32 | time in the sub-step |

##### Type and power resolver — pawn slot 1347 0x141434010 [I]

```
out type = requested                                                   // 0x14143401F
if !b6:                                                                // test r8b @0x14143402A
    bad = (type == 2 && !HasAbility(0x12 SUPERJUMP))                   // 0x14143403B / cmp 2 @0x141434045
       || (type == 3 && !HasAbility(0x11 DOUBLEFLIGHT))                // 0x141434055 / cmp 3 @0x14143405F
       || (type == 1 && !HasAbility(0x10 HIGHJUMP))                    // 0x14143406F / cmp 1 @0x141434079
    if bad: type = (type == 2 && HasAbility(0x11)) ? 3 : 0             // 0x141434089..0x14143409F
    if HasAbility(0x10) && type == 0: type = 1                         // 0x1414340AC, cmove @0x1414340BD
power: 0 -> pawn+0xB70 (1480) | 1 -> vt+0x1AA0() = pawn+0xB7C (1950) | 2 -> pawn+0xB80 (2500) | 3 -> pawn+0xB84 (2300) | other -> pawn+0xB70
```

- **Confirmed [I]: with HIGHJUMP every ground jump is type 1 with power pawn+0xB7C = 1950.** Note the order: a type 3
  request without DOUBLEFLIGHT becomes 0 and then 1 with HIGHJUMP.
- **No ability level selects the power.** `MyJumpPowerHigh` (+0xB74) and `MyJumpPowerHighLv2` (+0xB78) are written by
  the constructor (0x141423785, 0x14142378F) and have no reader in the pawn / state / interface code
  (0x140500000–0x140660000, 0x141100000–0x141130000, 0x141330000–0x141520000 scanned for the displacements). The count
  virtual vt+0x1508 is called in 0x141330000–0x141520000 with only two kinds: 0x13 (0x1414354B0) and 0x17
  (0x141432EB0).
- `RequestJump` 0x141444FA0 [I]: power local preset pawn+0xB70; `PushState(3)`; `bFall` → sub = 3; +0x8C = yaw;
  +0x98 = 0; +0xA0 = type; +0xA4 = power; **+0x9C = bAir**; +0xA8 = 0; type 2 with weapon kind 0xC / 0x12 → vt+0x1388(0, 1, 0),
  flags |= 0x8000.
- `JumpRequest` 0x14143D8F0, the cases that matter [I]:

| Current state | yaw | type | bAir | Extra |
|---|---|---|---|---|
| IDLE (also treasure, gimmick, item) | actor yaw (vt+0xDA0 @0x14143D949) | 0 | 0 | |
| RUN | Run+0x8C | 0 | 0 | |
| JUMP, sub-step 11 | **Jump+0x8C** (@0x14143DD68; the last stick yaw of the landed jump) | 0 | 0 | |
| JUMP, other sub-steps | Jump+0x8C | 3 if pawn+0xE48 bit 0 (vt+0x1E60 @0x14143DD82), else 0 | = that | +0x98 = 1 |
| FLOATING | Floating+0x8C | 3 | 1 | **b6 = 1** (the resolver is bypassed) |
| SUPER_SLIDE (and RAIL fallback) | actor yaw | 2 | 1 | direction = `normalize2D(pawn vt+0xE60())`, +0x10C = pawn+0x644 (175), +0x110 = pawn+0x648 (0.92), flag bit 0 |

  Tail @0x14143E421: `ClearQueue` 0x1405A8010; `RequestJump`; Jump+0x98; +0xA8 = 0; +0xE9 = 0; +0x100 = direction
  (zero by default); +0x10C (0 by default); +0x110 (0.92 by default, 0x14143D986); +0x11C = 0; flags bits 0–1.

##### Start clip and launch [I]

`SetSubStep` 0x1413A0BF0: sub = n, +0x12C = 0; for n == 0: `Play(name(type, 0, flag 0x2000), blendIn −1, mode 0, group 0,
rate 1.0)`. Name selector pawn slot 913 0x141434150 `(type, phase, bB)` [I]:

| Type | Phase 0 (start) | Phase 1 (rise) | Phase 2 (fall) |
|---|---|---|---|
| 0 | `JumpNormalStart` | `JumpNormalRiseLoop` | `JumpNormalDownLoop` |
| 1 | `JumpHighStart` 0039 (0.5 s, `TresJumpStart` 0.0647) | `JumpHighRiseLoop` 0040 | as type 0 |
| 2 | `JumpSuperStart` 1161 (1.8333 s, 0.0667) | `JumpHighRiseLoop` (@0x141434201) — **never requested**: sub-step 1 of type 2 goes straight to phase 2 | as type 0 |
| 3 | `JumpDoubleFlightStart` 0041 (0.4333 s, 0.0335) | `JumpDoubleFlightRiseLoop` 0042 | as type 0 |
| 4 | `SuperFlightStart` | `SuperFlightLoop` | as type 0 (not in Sora's set) |

`bB` (flag 0x2000, set in Begin when the weapon is out and pawn+0x5F0 ∉ {2, 6}, @0x1413869B7) appends `B`; in Sora's
AnimSet every `…B` jump / land name maps to the same sequence [A]. `JumpSuperRiseLoop` 1162 exists in the AnimSet but
the selector has no string for it.

`TresJumpStart` handler 0x14139AB60 (all types) [I]:

```
if sub != 0: return                                                   // cmp [rbx+38h],0 @0x14139ABA8
if probe(2000) && Jump+0xF8 == 0:                                      // slot 114: hit && dist < 2 (cached: dist <= 0)
    vt+0x1E88(1); vt+0x1E98(1); flags &= ~8; SetSecondJump(1)          // 0x14139ABE3..0x14139AC08
pawn vt+0x1090 (bPressedJump latch)                                    // 0x14139AC15
if flags & 0x80000 && +0x118 > 0: +0xA4 = component vt+0xC50(+0x118)   // sqrt(2 * (-GravityZ * m_JumpUpGravityScale) * h)
component JumpZVelocity (+0x304) = +0xA4                               // 0x14139AC56
sub = 1; +0x12C = 0; flag bit 9 = bit 8
```

So types 1–3 use the **same latch → `CheckJumpInput` → `DoJump` path** as type 0, and the gravity hook 0x140577F30
[I, state-independent: it reads only `Velocity.Z` (@0x140577F6B), the finished flag +0xBFC, the timer +0xC00,
`m_JumpUpGravityScale` +0xBE8 2.8, `m_MaxJumpDurationTime` +0xBEC 0.15, `m_JumpDurationGravityScale` +0xBF0 0.1] applies
to every type: rise −4116, released −8232, hang −147 for 0.15 s, fall −1470. `DoJump` during a fall (double flight)
replaces `Velocity.Z` and reopens the curve.

Launch frame at 60 Hz [D]: types 0 / 1 / 2: notify in M(4), `DoJump` in M(5); **type 3 (0.0335 s): notify in M(3),
`DoJump` in M(4)** (0.0333 < 0.0335).

Heights, button held [D]: type 1: 1950² / 8232 = **461.9 cm** in 0.474 s; type 3: 2300² / 8232 = **642.6 cm** above the
point of the second launch in 0.559 s; type 2: 2500² / 8232 = 759.2 cm.

##### Begin 0x1413868C0 (non-fall part) [I]

```
if weaponOut && pawn+0x5F0 not in {2, 6}: flags |= 0x2000              // 0x1413869B7
if sub != 3:                                                           // 0x1413869C2
    if flags & 2 && component Swimming: SetMovementMode(Falling); gravity on      // leaving water
    pawn+0x668 = 0 (vt+0x2368: last hop component)
    SetSecondJump(type != 3)                                           // cmp [r14+0A0h],3 @0x141386D99; call @0x141386DB0
common (also after the fall part):
if type == 2:                                                          // 0x141386DB6
    +0xF8 = 1; +0x9C = 1; Draw(1, 1, 1.0)                              // 0x141386DC4..0x141386DE1
    if !(flags & 0x8000): vt+0x1388(1, 0, 0)
    vt+0x1E88(0)                                                       // flow-jump bit 3 consumed
    SetSecondJump(0)                                                   // 0x141386E21
else: +0x124 = vt+0x1B48()
v = vt+0x1588() (class-default MaxWalkSpeed 680); SetMaxWalkSpeed(v); SetMaxAcceleration(v * 5.4)       // 0x141386E5D
+0xEC.. = actor location; SetSubStep(sub); if !(0 < +0xA8): +0xA8 = 0.2                                // 0x141386EEB
pawn+0x1460 &= ~1
```

Fall part (sub == 3) [I]: floating → `PushState(FLOATING)`, `DoFinish`; stick ≥ 0.1 → +0x90, +0x88 (movement spec 4.5);
pawn+0x1460 bit 0 with the jump key down → `TryGlide` (only after a Keyblade switch, 9.4); probe; step-down flag;
then

```
if component vt+0x9D8():                                               // standing on a walkable floor @0x141386BED
    +0x98 = 1; +0x9C = 1; SetSecondJump(1)                             // 0x141386C00..0x141386C16
    if TrySlopeSlide (iface +0x5E0): DoFinish; return
    name = LandName(pawn land pose (vt+0x1788), weaponOut)             // selector 0x141434330, see Landing
    if name != None: StopMovement; Play(name, -1, mode 2, 0, 1.0); m_bEnableChange = 0; sub = 10; flags |= 0x10
else: flags |= 8                                                       // 0x141386D48: a fall may jump at once
```

##### Second jump (Doubleflight): the complete rule [I]

Gate, state JUMP (jump gate 0x14142A3D0 @0x14142A4E3):

```
next queued == null                                                    // 0x14142A4E6
sub == 11 -> allowed (ground jump)                                     // cmp [rdi+78h],0Bh @0x14142A552
else: pawn+0xE48 bit 0 (vt+0x1E60 @0x14142A562) && HasAbility(0x11) (@0x14142A57B)
      && (Jump+0x114 & 8) != 0                                         // test @0x14142A589, jne -> allowed
then !restrict bit 17
```

Flag 8 is produced by Jump slot 113 0x1413837F0, called in sub-step 1 (types ≠ 2, ≠ 3 — @0x141395D59), sub-step 2
(both branches @0x141395ECF / 0x141395F0E) and on every airborne tick of sub-steps 4–6 (@0x1413964CB):

```
flags &= ~8                                                            // 0x1413837F6
if Jump+0x9C == 0 && Velocity.Z <= 500: flags |= 8                     // cmp byte [rcx+9Ch],0 @0x141383800; comiss vz,500 @0x141383859; ja skip
return flag 8
```

- The constant is the literal **500 cm/s** (0x145C9C554). `MyJump2ndEnableMinimumVelocityZ` (pawn+0xB88, −800) has
  **no reader**: the only instruction with that displacement on a pawn is the constructor store 0x1414237B7.
- The second jump is therefore available: (a) in a jump started on the ground, from the first state tick with
  `Velocity.Z ≤ 500` (high jump, button held: 22 movement ticks after the launch — press accepted from frame 27 after
  the jump press at 60 Hz [D]; 1480 jump: frame 20); (b) at once in a fall (`Begin` sets flag 8 when not on a floor;
  recomputed from the first sub-step-4 tick); (c) never in a jump whose `bAir` is set (a double flight, a flow jump,
  a jump out of Dangle / WallRun / air guard / air dodge).
- Every writer of pawn+0xE48 bit 0 (setter vt+0x1E58 0x141112590; all call sites in 0x141330000–0x141450000 and
  0x141500000–0x141520000):

| Function | Site | Argument | When |
|---|---|---|---|
| `Jump::Begin` 0x1413868C0 | 0x141386DB0 | **type != 3** | every Begin that is not a fall: a double flight clears it, any other jump sets it |
| `Jump::Begin` | 0x141386E21 | 0 | type 2 (super jump) |
| `Jump::Begin` | 0x141386C16 | 1 | fall Begin while standing on a floor |
| `TresJumpStart` 0x14139AB60 | 0x14139AC08 | 1 | launch from the ground (probe true, not a flow jump) |
| Jump tick 0x141395450 | 0x14139626B | 1 | landing (sub-steps 4–6 on ground) |
| Jump tick | 0x1413973B6 | 1 | sub-step 10 |
| `AirSlide::End` 0x1413347F0 | 0x141334A3F | 0 | next state ≠ JUMP (cmp eax,3 @0x1413349F5) |
| RailSlide slot 135 0x14150AB80 | 0x14150AEC3 | 0 | rail jump |
| `Idle::Begin` 0x141386050 | 0x141386337 | 1 | on ground only (after the `OnGround` return @0x141386207) |
| `Run::Begin` 0x1413A58A0 | 0x1413A59EE | 1 | |
| Floating tick 0x14137A680 | 0x14137A79F | 1 | |
| Hopping / SlopeSlide / Swim / WallRun / PoleTurn / RailSlide / Cover Begin | 0x141385EF9, 0x1413CB6EC, 0x1413CC8B9, 0x1413CD153, 0x141509AC5, 0x141509CF1, 0x14134F635 | 1 | |

- Sequence of a double flight: press accepted → `JumpRequest` (type 3, `bAir`, yaw = Jump+0x8C) → old `Jump::End`,
  new `Begin` clears bit 0 → `JumpDoubleFlightStart` → notify (probe false in the air: nothing restored) → `DoJump`
  with 2300. In the new Jump +0x9C = 1, so slot 113 keeps flag 8 clear: **one air jump per airtime, restored by
  landing** (or by the states of the table).

##### Tick 0x141395450 — head [I]

```
+0x12C += dt                                                           // 0x14139554F
if component in water (vt+0x1A80(0)):                                  // 0x14139556C
    if !(flags & 2 && sub < 3) && sub != 9 && PushState(SWIM): controller vt+0x1830(0); DoChain; return
else flags &= ~2
if next queued id == 0x43 (CINEMATIC) && 0x143EA19C0(pawn): DoChain; return
if pawn status byte (0x1405A9DD0) not in {0x0C, 0x11}:                 // rail probe
    start = actor location; end = start + (0, 0, 30)                   // 30 @0x14139567B
    dir = +0x88 ? Rot(0, +0x8C, 0).Vector() : actor forward            // 0x14139573E
    if pawn vt+0xEA0(start, end, range = vt+0x1AF8() = 150, dir, out, 1) && TryRailSlide(out, 1, 0) (iface +0x258): DoChain; return
probe(2000)
req = +0x88 && (sub in {10, 11} ? +0x124 != 0 : true)                  // 0x14139583A..0x14139585D
if +0xE9: +0x94 = 1                                                    // not a controller jump: counts as held
falling = false                                                        // r15b
switch sub (table 0x141398334: 0, 1, 2, 3, 4, 5 = 6, 7, 8, 9, 10; > 10 -> sub-11 code)
```

##### Sub-steps 0–2 [I]

```
0: if flags & 1: anim interface vt+0x2A0()                             // flow jump only [? what it does]
1, type == 2 (cmp @0x1413958E3):
     if OnGround && (+0xFC & 1):                                       // came down again during the 1.83 s start clip
         if weapon kind != 0: vt+0x1388(0, 1, 0)
         StopMovement
         if +0x88 && !(800 > fallHeight): Sheathe(1, 1.0); name = "RollLand"        // comiss 800,h; ja @0x14139595E
         else name = flags & 0x2000 ? "LandB" : "Land"
         Play(name, -1, mode 2, 0, 1.0); m_bEnableChange = 0; sub = 10; req = false
     else:
         if IsAnimEnd: Play(name(type, 2, B), -1, mode 1); vt+0x22E8; sub = 4; falling = true
         elif !(+0xFC & 1) && Velocity.Z > 0: +0xFC |= 1
         if Velocity.Z < 0: vt+0x22E8; falling = true                  // comiss vz,0; jae skip @0x141395B3E
         lastZ = actor Z
1, other types (0x141395BCF):
     if IsAnimEnd: Play(name(type, 1, B), -1, mode 1, 0, 1.0); sub = 2
     lastZ = actor Z
     if type == 3: break                                               // cmp @0x141395D00: no height cut, no slot 113
     if +0x94 == 0 && clip time > 0.1: SetGravityScale(pawn+0xB8C = 3.0)           // comiss t,0.1; jbe skip @0x141395D36
     slot 113
2 (0x141395D69):
     if Velocity.Z > 0:                                                // comiss vz,0; ja @0x141395D79
         if type != 3 && +0x94 == 0: SetGravityScale(pawn+0xB8C)       // cmp @0x141395EDC
         slot 113
     else:
         if IsJumpDurationFinish: Play(name(type, 2, B), -1, mode 1); +0xE8 = 0   else +0xE8 = 1
         vt+0x22E8; sub = 4; falling = true; lastZ = actor Z; ResetGravityScale; slot 113
```

Every `type` test of the Tick: `== 2` @0x1413958E3, `== 3` @0x141395D00, `== 3` @0x141395EDC; three writes
`type = 0` (@0x141396228, @0x1413973D8, @0x141397B0E, each with +0xF8 = 0 and vt+0x1388(0, 0, 0)). `Jump::End` tests
`== 2` once (@0x14138DA75). Nothing else depends on the type.

- **High jump: the height cut works in sub-step 1.** `JumpHighStart` is 0.5 s long and the launch is at 0.083 s, so
  the rise of a high jump runs mostly in sub-step 1, where the test "button not down and clip time > 0.1" is live
  (the same test is dead for the 0.1 s `JumpNormalStart`). The cut starts with the state tick S(7) (clip time
  0.1167 s) and acts from M(8): three rise ticks at −4116, then −8232. Tap height [D] ≈ 92 + 185 = **277 cm**
  (held: 462 cm).
- Double flight (type 3): no height cut at all, in either sub-step.
- Super jump (type 2): no height cut (its own branch); the start clip plays for 1.8333 s while Sora rises and falls;
  at the clip end, or on landing, it leaves sub-step 1. No rise loop is played.

##### Sub-steps 3–6 [I]

Sub-step 3 as in the movement spec 4.5, with: flag 0x40 (Air Slide stick exit) → the current clip plays to its end
(`IsAnimEnd` @0x141395F66), then flag 0x40 is cleared and the step continues; the step-down loop name is
`vt+0x1C70(0, weaponOut, battleAnim || +0x124)`.

Sub-step 4 sets `falling = true` (@0x141396192); **sub-steps 5 and 6 enter the same code with `falling = false`**
(no dive check, no ledge / hop / floating exits):

```
if +0xF8 && !(actor Z > +0xF4):                                        // comiss z,[rsi+0F4h]; ja skip @0x14139621F
    +0xF8 = 0; type = 0; vt+0x1388(0, 0, 0)                            // the flow jump ends when Sora is back at its start height
if OnGround:                                                           // 0x141396244
    falling = false; +0x9C = 1; flags &= ~8; SetSecondJump(1); StopMovement
    flag 0x10 = !(fallHeight > 75)                                     // comiss h,75; cmova @0x14139628B
    if +0x88 && flags & 0x20: DoChain; return                          // step-down with the stick held: no landing clip
    if flag 0x10:                        name = LandName(2, weaponOut)             // LightLand / LightLandB
    elif +0x88 && !(fallHeight < 800):   Sheathe(1, 1.0); name = "RollLand"        // comiss h,800; jb @0x1413962E3
    else:                                name = LandName(1, weaponOut)             // Land / LandB
    Play(name, -1, mode 2, 0, 1.0); m_bEnableChange = 0; sub = 10; +0x12C = 0; req = false
else:
    if +0xE8 && IsJumpDurationFinish: Play(name(type, 2, B), -1, mode 1); +0xE8 = 0
    slot 113
    if falling && slot 115 (dive check 0x141383550): controller vt+0x1830(0); Play("Diving", -1, mode 2); sub = 9
```

Sub-steps 5 / 6 ("fall slide") [I]: entered from the tail of sub-step 4 when Jump+0xBC is set, the stick is neutral,
pawn+0x5F0 == 1, the weapon is away or its kind ≤ 1, the object at +0xD0 is valid (vt+0x550), +0xD8 > 10 and
0x14063A1E0(pawn, &+0xC0, 350.0) succeeds (@0x1413978B0..0x141397959). Sub 5: when `forward(actor) · direction(pawn
vt+0xE28()) ≥ 0.643` (@0x1413976D1) play `FallSlide` 0104 (mode 1), sub = 6. Sub 6: while +0xBC stays set and the stick
is neutral, +0x8C = yaw of that direction; otherwise down loop, sub = 4. In sub 6 the pitch target of the restore turn is
+0xD8 − 50. [? no instruction that sets Jump+0xBC was found in the state / interface code: it is 0 in the functions read.
Treat sub-steps 5 / 6 as not reachable until the writer is known.]

##### Sub-steps 7–9 (dive fall, diving) [I for the constants]

Entry from the tail of sub-step 4 (@0x141397963), all of: `m_bEnableDiveFall` (pawn+0xBB4 bit 0; false by default),
`!(pawn+0xE60 & 0x1000)`, fall time > 0.5 (comiss 0.5,+0xAC; jae skip), not (water depth +0xE4 ≠ 0 && +0xE4 < probe
distance && +0xE4 < 2000), `Velocity.Z ≤ 0`, probe distance ≥ pawn+0xBB8 `MyDiveFallStartHeight` 1600 (comiss
1600,+0xDC; ja skip), current clip ∈ {`JumpNormalDownLoop`, `JumpNormalDownLoopB`}. Then Sheathe(1, 1.0),
`Play("DiveFallStart", −1, mode 2)`, sub = 7, flags &= ~8, flow-jump flag dropped.

- 7: at the clip end: `Play("DiveFallLoop", −1, mode 3)`, slot 120(1), sub = 8, flags |= 8 (@0x141396653),
  `MaxWalkSpeed = pawn+0xBC4` (2000), `MaxAcceleration = × 5.4`, `GravityScale = pawn+0xBC0` (0.25), controller
  vt+0x1820(1).
- 7 / 8 on ground (@0x1413966CB): slot 120(0), +0x9C = 1, speeds and gravity reset, StopMovement, Sheathe,
  `Play("RollLand")`, sub = 10. Airborne: if `Velocity.Z < 0` and |pawn vt+0xE28().z| < 300: without flag 0x10000 →
  `DoChain`; blend X = stick magnitude × 10 when the yaw delta < −15°, × −10 when > +15° (needs magnitude ≥ 0.5), else 0;
  restore-turn rate `pawn+0xBBC × dt × 60`; in sub 8 probe(3500), dive check → `Diving` / sub 9, and the lock-on camera
  request (flag 0x4000) when a target is within 1000 of the ground distance.
- 9 `Diving`: `req = false`; on ground → as above with `RollLand`; airborne: horizontal velocity zeroed, `Velocity.Z`
  clamped to ≥ −1500 below the water surface (@0x14139706C), written with vt+0x16B0; at the clip end or under the
  surface, in water: `PushState(SWIM)` (sub-step 3) and `DoChain`. The common tail is skipped for sub 9
  (@0x141397507).

##### Sub-steps 10, 11 [I]

```
10: rumble manager 0x141522B30 (unless pawn vt+0x1328() == 6)
    if TrySlopeSlide(0) (iface +0x5E0): DoChain; return                // 0x1413971AC
    sub = 11; +0x12C = 0
    if actor Z > +0xE0 && actor Z - +0xE0 <= 75: SetActorLocation(z = +0xE0, sweep)      // 0x141397358..0x141397380
    vt+0x1E88(1); vt+0x1E98(1); flags &= ~8; SetSecondJump(1); controller vt+0x1C68()
    (falls into 11)
11: if +0xF8: +0xF8 = 0; type = 0; vt+0x1388(0, 0, 0)
    if +0x88 && (m_bEnableChange || flags & 0x20): StopMovement; DoChain; return
    if m_bEnableChange:
        if pawn+0x10E8 bit 0 && PushState(0x8E COVER): DoChain; return
        if water flow (component vt+0x9B8) && !OnGround && Fall(+0x8C): DoChain; return
    req = false
    if IsAnimEnd: StopMovement; DoChain; return
```

##### Tail and end of tick [I]

```
if status byte in {0x0C, 0x11}: return
(sub 9 jumps to END)
sub 4: fall-slide entry or dive-fall entry (above)
turn:  if pawn+0x518 bit 0: vt+0xDB8(dt * 18); rate2 = dt * 18
       else: Turn(Rot(0, +0x8C, 0), dt * 6) (vt+0xDB0 @0x141397893); rate2 = 0.1 * dt * 60 (dive: pawn+0xBBC * dt * 60)
       vt+0xDC0(zero rotator (pitch = +0xD8 - 50 in sub 6), rate2)     // pitch / roll back to level
if req:
    if +0x124: vt+0x16D8(0); if sub == 11: yaw = actor yaw
    SetMovementInputXY(Rot(0, yaw, 0).Vector(), 1.0, 0)                // 0x141397BC8
elif +0x124: formchange momentum (vt+0x1B48; 0.98 per tick) — not the plain Keyblade
if +0x11C > 0: +0x11C -= dt
END (0x141397E7F):
if +0x88: +0x10C = 0                                                   // the stick cancels the inertial push
slot 118 0x1413832C0 (dt):
    if sub <= 4 && +0x10C > 0 && +0x100 != 0:
        AddImpulse(+0x100 * +0x10C * dt * 60, velocity change)         // pawn vt+0x1718
        +0x10C *= pow(+0x110, dt * 60)
+0x88 = 0; +0x94 = 0
if sub == 4: fall accounting (movement spec 4.2)
+0xBC = 0
if falling:
    if +0xA8 > 0: +0xA8 -= dt                                          // 0x141397FEF: counted on falling ticks only
    else:
        if dangle gate (iface slot 63 0x141427830)
           && ledge probe 0x140638740(pawn, out, up 250, down 50, forward 0, radius 100, debug = flag bit 8)
           && iface slot 65 (out.actor, out.component, &out.point, out.yaw, 0): DoChain; return
        if TryHopping(actor yaw, 0, 0) (iface +0x5C0): DoChain; return  // +0x88 is already cleared: always the actor yaw
    if floating (pawn+0xD80 bit 0) && PushState(FLOATING): DoChain; return
flags &= ~0x1000; +0xE0 = actor Z; flags &= ~0x280
```

- `falling` is true only: on the tick sub-step 2 (or 3, or type-2 sub-step 1) hands over to 4, on every airborne
  sub-step-4 tick, in type-2 sub-step 1 with `Velocity.Z < 0`, and in the dive. So **ledge grab, hop and floating are
  tested only while falling**, and the 0.2 s delay starts at the apex (12–13 ticks at 60 Hz), not at Begin.
- Super jump inertia [D]: with the stick neutral each 60 Hz tick adds `direction × 175` cm/s to the velocity, the amount
  shrinking by 0.92 per tick (sum 175 / 0.08 = 2187 cm/s if never cancelled); `CalcVelocity` brakes the excess over
  `MaxWalkSpeed` 680 with `BrakingDecelerationFalling` 2048. Any stick input zeroes +0x10C for good.

##### End 0x14138D850 [I]

Clear the jump latch (vt+0x1098); vt+0x16D8(0); `ResetMaxWalkSpeed`; pawn+0x668 = 0; `ResetGravityScale`; restore
rotation offset (vt+0x21A8(zero)); slot 120(0); dive camera off; unless the next state is WEAPONCHANGE (0x20): glide
gate timer pawn+0xE34 → 0 if running, and **when on ground and not floating the air-slide and air-guard cooldowns are
zeroed** (vt+0x1E18(0), vt+0x1E28(0) — their bits return on the next pawn tick); type 2 → vt+0x1388(0, 0, 0); else if
vt+0x1360() (never true for kind 1): `Draw(1, 1, 1.0)` by the next state id.

#### Landing variants and the land pose

| Clip | Chosen when [I] | Where |
|---|---|---|
| `LightLand` / `LightLandB` 0120 | fall height ≤ 75 | Jump sub-steps 4–6; name by **weapon out** (selector 0x141434330 arg r9 = vt+0x1330()) |
| `RollLand` 0108 | stick request on the landing tick && fall height ≥ 800 (and > 75); `Sheathe(1, 1.0)` first | Jump sub-steps 4–6 (@0x1413962DC), type-2 sub-step 1 (@0x14139594F) |
| `RollLand` | always | landing out of the dive fall (7 / 8) and `Diving` (9) |
| `Land` / `LandB` 0034 | otherwise | as `LightLand` |
| `Land`, `LightLand`, `LightLandR`, `LightLandL` (+`B`) by pose 1 / 2 / 3 / 4 | a land pose is stored | `Idle::Begin` (on ground) and `Jump::Begin` (fall Begin on a floor) |

Land selector 0x141434330 `(pose, bWeaponOut)` [I]: weapon out: 1 `LandB`, 2 `LightLandB`, 3 `LightLandRB`,
4 `LightLandLB`; away: `Land`, `LightLand`, `LightLandR`, `LightLandL`; other → None. `B` here means **weapon out**, not
the battle pose.

Land pose mechanism [I]:

- Writer: `TresAnimNotify_LandAnimPose::Notify` 0x140560320 (@0x14056036D, value = notify+0x50, constructor default 2
  @0x14055A98B) → pawn anim interface (pawn+0x440, vtable 0x14698E5D0) slot +0x2C8 0x1405B3770 `mov [rcx+2A8h],edx`
  (= pawn+0x6E8). Last pose likewise: slot +0x2C0 0x1405B3870 `mov [rcx+2A4h],edx` (= pawn+0x6E4).
- Cleared: the play-by-name wrapper 0x1405A0E50 calls pawn vt+0x1790(0) @0x1405A0EB9 before every play of a non-empty
  name; also the pawn reset 0x1405B6700 (@0x1405B6989, both poses).
- Readers: `Idle::Begin` @0x1413864C1 and `Jump::Begin` @0x141386C73, both `LandName(vt+0x1788(), weaponOut)`. Idle:
  +0x93 = 1, `Play(name, −1, mode 2)`, +0xA4 |= 2, sub = 1; while +0x93 is set the Idle tick skips the idle-loop logic
  (@0x141395020); at the clip end sub-step 1 plays the idle name and clears +0x92 / +0x93 (@0x141394D70).
- Because every play clears the pose, the pose survives only when the state that ends in the air plays nothing
  afterwards — i.e. the path `X::End → Idle::Begin` directly on the ground, or `X → Fall → Jump::Begin` on a floor.

Jump flag 0x40 [I]: set by the Air Slide stick exit; in sub-step 3 the current clip plays to its end before the down
loop (@0x141395F48..0x141395F74); cleared also when the stick is released (flags &= ~0x60 @0x141395F25).

---

#### Stance system (TresPlayerState_Idle id 1, TresPlayerState_Run id 2)

##### The inputs [I]

1. **Battle flag** pawn+0x630 bit 1. `SetBattleMode(b)` slot 743 0x141446CE0:

   ```
   if b: pawn+0x10E0 = 10.0                                            // 0x141446CF4
   else: if battle flag: pawn+0x10E0 = 10.0                            // 0x141446D0D (falling edge only)
         vt+0x27D8(0); vt+0x27E8(0)
   0x14054E4F0: if flag != b: pawn+0xA7C = b ? world time (vt+0x140()+0x938) : 3.1536e10
   0x1405B8AC0: pawn+0x630 bit 1 = b
   ```

   Callers for the player: the player controller tick 0x14141E950 calls controller slot 630 0x1413F7340 **every
   frame** (@0x14141E9C1): `b = G+0x5BB bit 0 ? (G+0x8D4 & 0x14) != 0 : false; pawn.SetBattleMode(b)`, where G is the
   game object at world+0x158 (static class 0x1422A2E80). The same read in 0x141453B90 (pawn setup). Player states
   that force it on: AFRollerCoaster 0x141317C2A, QuickBattle 0x1413A4489, ShootFlow 0x1413A5D5D; a thunk from the
   pawn sub-object +0x448 (0x1414629A0).
   G's flags are maintained by 0x140EA0B20 per frame [I for the tail, C for the count]: `count` = number of
   `TresEnemyPawnBase` actors whose own battle flag (vt+0x1740) is set and vt+0x17B0() > 0, plus G+0x618;
   `count > 0` → bit 2 on, linger timer G+0x5C0 = 1.0 (@0x140EA0D09); else the timer counts down by G+0x210 and bit 2
   stays on while it is > 0; with neither, bit 2 = bit 4 (forced flag, setter 0x140EB8560). A change switches the
   music between "Battle" and "Field". **For the port: `battle = any engaged enemy, plus 1.0 s`, host-supplied.**
2. **Weapon out**: equipment+0x2C8 bit 1. `Draw(bOut, bEffect, rate)` = pawn vt+0x1340 0x141448060 (`bOut &&
   !restrict bit 15`) → 0x1405BA440: equipment vt+0x4A8 0x14178B670 (no-op when the bit already equals `bOut`; else
   vt+0x608 0x14178B690 writes the bit **at once** and shows / hides the weapon actors with the effect), then pawn+0x10E0
   = 10.0 (@0x1405BA464, on every call, draw or put away). `Sheathe(bEffect, rate)` = vt+0x1358 0x1414480B0: only when
   the weapon is out and the kind ∉ {1, 0x16, 0x17}: `Draw(0, …)` — **a no-op for the plain Keyblade**.
3. **Last pose** pawn+0x6E4 (0 NORMAL, 1 BATTLE): written by `TresLastAnimPose` notifies and by states (vt+0x1780).
4. `battleAnim` = pawn vt+0x1748 0x141435A10: controller mode 0x8B → true; kind ∉ {1, 0x16, 0x17} && weapon out →
   true; restrict bit 15 → false; else `lastPose == 1`.
5. **Keep-out timer** pawn+0x10E0: written 10.0 by the constructor (0x1414236B5), `SetBattleMode` and `Draw`; counted
   down to 0 by pawn slot 1309 0x14144A5D0. Its **only reader** is the Run tick @0x1413AF770 (vt+0x1760), inside a
   block guarded by Run+0xBD — and Run+0xBD is never set: `Reset` zeroes it (word store @0x1413AD3A3), the tick clears
   it (@0x1413AF7A1), no other instruction in 0x141330000–0x141520000 writes it. **The 10 s timer has no effect in
   this build.**

##### Idle fields [I]

+0x88 `battleAnim` at Begin; +0x8C sheathe timer; +0x90 water flow; +0x91 lock-on; +0x92 break played; +0x93 land-pose
clip playing; +0x94 formchange momentum; +0x98 i32 wrap counter; **+0x9C i32: zeroed by Reset (qword store
@0x14138F722) and never written again** — the tests `Idle+0x9C > 0` in CanRun (@0x14142D1D4) and the jump gate
(@0x14142A4C9) are always false; +0xA4 flags; +0xA8 last clip time; +0xAC momentum; +0xB0 lock target.

##### Idle::Begin 0x141386050 [I]

```
pawn+0x660 = 0
if floating && PushState(FLOATING): DoChain; return
if TryHopping(actor yaw, 0, 0): DoChain; return
if !OnGround: return                                                   // the tick falls
rumble; controller: +0x91 = lock-on (vt+0x1680), +0xB0 = target, camera resets
SetSecondJump(1)
bA = battleAnim                                                        // vt+0x1748 @0x141386344
(+0x94 = vt+0x1B48(): formchange momentum, forces bA)
if water flow: Play("WaterFlowIdle", -1, mode 3); +0x90 = 1; Sheathe(1, 1.0); +0x8C = 0; sub = 0; return
if weapon restricted (vt+0x1EE8): if weaponOut: Draw(0, 1, 1.0);  +0x8C = 0
elif bA: Draw(1, 1, 1.0); +0x8C = 3.0                                  // 0x141386579, 0x14138657F
else: +0x8C = 0
wOut = weaponOut
name = LandName(land pose, wOut)
if name != None: +0x93 = 1; Play(name, -1, mode 2, 0, 1.0); +0xA4 |= 2; sub = 1; +0x88 = bA; return
if pawn+0x1460 bit 0:                                                  // set only by WeaponChange::Tick 0x1413DA032
    if stick >= 0.8 && TryRun(yaw, mag): DoFinish; return             // comiss mag,0.8; jb @0x141386602
    Draw(1, 1, 1.0); Play("BattleStart", -1, mode 2); sub = 1; pawn+0x1460 &= ~1
else:
    name = IdleName(wOut, bA, +0x91)                                   // selector 0x141433F80
    if name != current clip (and the current clip is not IdleBreakW / IdleBreakN): Play(name, -1, mode 2, group, 1.0)
    sub = 0; +0xA8 = 0
+0x88 = bA
```

Idle name selector 0x141433F80 [I]: in water → `DiveIdle`; `weaponOut && battleAnim` → `+0x91 ? IdleLockon : IdleB`;
else **`IdleN`** (also with the weapon out: there is no `W` idle). Effect group with `battleAnim`: kind 0x10 → 0x720,
0x11 → 0x71F, 0x15 → 0x76C, else 0.

##### Idle::Tick 0x1413944D0 [I]

Head: in water → SWIM; floating → FLOATING; cover; not on ground → `Fall(actor yaw)`; status 0x0C / 0x11 → states
0x7A / 0x7C; rail probe as in the Jump head but with range vt+0x1AF0() = 50 and the actor forward; `TrySlopeSlide`.

```
bA = battleAnim (vt+0x1748); bt = battle flag (vt+0x1740)              // 0x1413948A3, 0x1413948B4
if controller mode in {0x85, 0x87, 0x88, 0x8B}: bt = true              // bit mask 0x4D over mode - 0x85
(pawn+0x149C bits 0 && 1: bt = false, special mode; water-flow branch)
sub 0: vt+0x1E68(1); vt+0x1E78(1); vt+0x1E88(1); vt+0x1E98(1)          // bits 1-4 every idle tick, not bit 0
all sub-steps:
if weapon restricted:                                                  // vt+0x1EE8 @0x141394DE2
    if weaponOut: Draw(0, 1, 1.0); Play("BattleEnd", -1, mode 2); +0x8C = 0; sub = 1
elif bA == bt: if bA: +0x8C = 3.0                                      // cmp r15b,bl @0x141394E36
elif bt:                                                               // battle began, pose not battle
    Draw(1, 1, 1.0); Play("BattleStart", -1, mode 2, 0, 1.0); sub = 1  // 0x141394E59
else:                                                                  // pose battle, battle over
    if vt+0x1360():            countdown                               // formchange kind with the weapon away
    elif !IsAnimEnd:           nothing                                 // 0x141394EF2: plain Keyblade waits for the clip end
    else:                      +0x8C = 0; countdown
    countdown: +0x8C -= dt; if !(+0x8C > 0): Sheathe(1, 1.0); Play("BattleEnd", -1, mode 2); +0x8C = 0; sub = 1
sub 1: if IsAnimEnd: Play(IdleName(weaponOut, bA, +0x91), -1, mode 2, group); +0x92 = +0x93 = 0; +0x98 = 0; sub = 0; +0xA8 = 0
Turn(Rot(0, actor yaw, 0), dt * 9); vt+0xDC8(dt * 9)
if +0x93: return
if bA:                                                                 // battle pose
    lock = controller vt+0x1908() | vt+0x1680(); +0xB0 = target
    if lock != +0x91: +0x91 = lock; Play(IdleName(weaponOut, true, lock), -1, mode 2, group); +0x92 = 0; +0x98 = 0
    elif IsAnimEnd:   Play(same); +0x92 = 0; +0x98 = 0
else if sub == 0:
    wrapped = clip is looping (iface +0xD0) && clip time < +0xA8;  +0xA8 = clip time
    if !IsAnimEnd && !wrapped: return
    n = (restrict bit 15 && battle flag) ? (+0x98 = 0) : ++(+0x98)
    if n >= 3: Play(weaponOut ? "IdleBreakW" : "IdleBreakN", -1, mode 2); +0x92 = 1; +0x98 = 0       // cmp eax,3; jl @0x14139527A
    else:
        if +0x92:                                                      // the clip that just ended was the break
            if pawn+0x5F0 not in {3, 7, 0xA} && weaponOut: Draw(0, 1, 1.0)        // 0x14139536A: the Keyblade is put away here
            +0x92 = 0; +0x98 = 0
        Play(IdleName(weaponOut, false, +0x91), -1, mode 2)
```

Reconciliation with the weapon research ("Idle shows the weapon with effect and plays BattleStart when pawn+0x630
bit 1 is on, a 3.0 s timer then BattleEnd, Run hides it when the 10 s timer reaches 0"):

- *Compare:* the Idle and Run ticks compare **`battleAnim` (the pose, vt+0x1748) with the battle flag** (plus the
  controller modes); the weapon-out bit is not part of the comparison. Draw with effect + `BattleStart` when the flag
  is on and the pose is not BATTLE: correct [I].
- *3.0 s timer:* it exists (Idle+0x8C, reloaded to 3.0 while pose and flag are both on), but for weapon kinds 1 /
  0x16 / 0x17 — and for any kind with the weapon out — the branch **waits for the current idle clip to end** and then
  fires at once (+0x8C is zeroed first). The 3.0 s countdown runs only when vt+0x1360() is true (a formchange kind
  whose weapon is away). For the plain Keyblade: `BattleEnd` starts when the `IdleB` (5.9333 s) or `IdleLockon`
  (1.3333 s) instance playing at the end of the battle finishes.
- *BattleEnd for the plain Keyblade* does not put the weapon away (`Sheathe` is a no-op for kind 1). Its
  `TresLastAnimPose` (NORMAL) at 0.1 s ends `battleAnim`; the idle after it is `IdleN` with the Keyblade in hand. The
  Keyblade is put away by `Draw(0, 1, 1.0)` at the end of the first idle break: 3 × `IdleN` (3.0 s) + `IdleBreakW`
  (9.6333 s) = **18.63 s of standing** after `BattleEnd`, and by a weapon-restricted zone.
- *Run and the 10 s timer:* dead code (above). Run never puts the plain Keyblade away.

##### Run — stance parts of Tick 0x1413AED70 [I]

```
bA = battleAnim (0x1413AF507)
bt = battle flag || controller mode in {0x85, 0x87, 0x88, 0x8B} || pawn+0x5F0 in {3, 7}      // 0x1413AF663..0x1413AF6B1
sub 0, first: if Run+0xBD: (weapon restricted: Draw(0); elif !bt && weaponOut && keep-out == 0: Draw(0, 1, 1.0), pose NORMAL); +0xBD = 0     // never entered
sub 0, walking (+0x94): if bA != bt: bt ? Draw(1, 1, 1.0) : (kind not in {1, 0x16, 0x17}: Sheathe(1, 1.0), pose NORMAL)      // 0x1413AFF3B: no clip
sub 0, running, |yaw delta| < 75 (0x1413AFA34):
    if weapon restricted: if weaponOut: Draw(0, 1, 1.0)
    elif bA != bt:                                                     // cmp [rbp+0B0h],dil @0x1413AFACF
        if kind not in {1, 0x16, 0x17}: bt ? Draw(1, 1, 1.0) : (Sheathe(1, 1.0); pose NORMAL)
        elif bt:
            Draw(1, 1, 1.0)
            len = Play(weaponOut ? "RunBattleStartW" : "RunBattleStart", -1, mode 2, 0, 1.0)         // tested after the Draw
            if !(len > 0): len = Play("RunLoopB", -1, mode 2); pose = BATTLE (vt+0x1780(1))
            if len > 0: sub = 3
        elif weaponOut:
            len = Play("RunBattleEnd", -1, mode 2)
            if !(len > 0): len = Play("RunLoopB", -1, mode 2); pose = NORMAL
            if len > 0: sub = 3
sub 3 = sub 2 (same table entry 0x1413B025F): at the clip end play the loop, flags &= ~2, sub = 0
```

- `Draw(1, …)` sets the weapon-out bit immediately, so the name test that follows always yields `RunBattleStartW`
  (1013); `RunBattleStart` (0013) is not reachable through this path [D].
- With the existing clips the pose is written by their `TresLastAnimPose` notifies at t = 0 (`RunBattleStartW` /
  `RunBattleStart` → BATTLE, `RunBattleEnd` → NORMAL) [A], i.e. in the next movement tick; the explicit pose writes
  belong to the fallback.
- `RunBattleEnd` needs the weapon out; with the weapon away and the pose BATTLE nothing happens while running.
- Stance changes are evaluated only in sub-step 0 (not during start / turn-around / stop clips), and for the plain
  Keyblade only while running, not walking.

##### Pose and weapon by clip [A + I] (plain Keyblade)

BATTLE at 0: `BattleStart` 1014 (0.4333 s), `IdleB` 1015, `IdleLockon` 1018, `RunBattleStart(W)` 0013 / 1013 (0.5333 s);
`Guard::Begin` writes BATTLE. NORMAL: `BattleEnd` 1028 at 0.1 (1.1 s), `RunBattleEnd` 1027 at 0 (0.5333 s),
`DodgeRoll` at 0.69, landings. `Jump::Begin` of a flow jump (type 2) draws the weapon.

#### Weapon kind byte

`kind(index)` = equipment vt+0x440 0x141780380 [I]: `index < 0` → `index = equipment+0x378` (current slot); out of
0..2 → **1**; else byte `m_EquipSlots[index]` (+0x2E8 + index × 0x30) **+0x0C**. Sub-kind vt+0x448: byte +0x0D.
Writer [I]: equipment slot 190 0x141791120 (the form setter), `mov [rbx+rcx*8+2F4h],r14b` @0x141791535 with r14b =
its 4th argument (the form), `+2F5h` = 5th argument; it also stores equipment+0x229 / +0x22A. Its callers are the
equipment's own form-change entry points (slots 155 0x14177C4A0, 156 0x14177C780, and 0x14177C3B0), which pass the
form through.

The value is an **`ETresWeaponForm`** (0 NONE, `TRES_WEAPON_FORM_01` … `_18` = 1 … 24, MAX 25) — the enum has no
descriptive names. It is the key of `TresEquipKeybladeData.m_EquipMap` in each Keyblade's equip asset
(`Blueprints/Player/p_ex001/w_soNNN_Equip`) [A]:

| Form | Assets holding the key | `m_UIDataAsset` suffix | Wear form | Replace anim sets |
|---|---|---|---|---|
| **0x01** | **all 20 equip assets that have a map** (w_so010 … w_so210; w_so100 has no map) | – | NORMAL | – |
| 0x02 / 0x05 | w_so020 | DW / BZ | 02 | AnimSet2 / AnimSet5 |
| 0x03 / 0x06 | w_so050 | HM / DR | 01 | 3 / 6 |
| 0x0C / 0x04 | w_so060 | CL / YO | 01 | 12 / 4 |
| 0x12 / 0x07 | w_so080 | CL2 / SK | 04 | 12 / 7 |
| 0x08 | w_so170 | GM | 04 | 8 |
| 0x09 | w_so190 | SW | 06 | 9 |
| 0x0A | w_so070 | WA | 02 | 10 |
| 0x0B / 0x13 | w_so030 / w_so140 | SH / SH2 | 03 / 04 | 11 / 11+19 |
| 0x0E / 0x0F | w_so090 | HL / FL | 01 | 14 / 15 |
| 0x10 / 0x11 | w_so040 | DW2 / BZ2 | 03 | 2+16 / 5+17 |
| 0x14 / 0x15 | w_so110 | HM2 / DR2 | 03 | 3+20 / 6+21 |
| 0x16 / 0x17 | w_so200 / w_so210 | L6 / L7 | 08 / 09 | 22 |
| 0x18 | w_so200, w_so210 | L8 | 0A | 23 |

**Every Keyblade without formchange is kind 1** (the only non-formchange key of every asset, and the getter's
default). Kinds 2–24 are formchange forms; 0x16 / 0x17 are the two forms the code treats like the plain Keyblade
(vt+0x28B0, `Sheathe`), 7 / 9 / 0xA are the forms with carried momentum (vt+0x1B48). The form names behind the
suffixes (DW double arrowguns, BZ launcher, HM hammer, DR drill, CL claws, YO yo-yos, SK, GM, SW, WA staff, SH shield,
HL, FL, L6 / L7 / L8) are readings of the asset names [D], not stated by any table.

---

### Corrections to Part I

| Section | Part I | Correct |
|---|---|---|
| 5.2, 2.5, transitions `jump>jump.double` | second jump needs `!(Jump+0x114 & 8)` | needs flag 8 **set** (`test …,8` @0x14142A589, `jne` → allowed); flag 8 = "Jump+0x9C == 0 && Velocity.Z ≤ 500", slot 113 0x1413837F0 |
| 5.2, Q4 / P4 | "where bit 0 is cleared after the second jump was not found" | `Jump::Begin` @0x141386DB0: `SetSecondJump(type != 3)`; type 2 clears it too (@0x141386E21) |
| 0.5, 5.2, Q4 | `MyJump2ndEnableMinimumVelocityZ` (−800): reader not located | no reader exists; the threshold is the literal 500 in slot 113 (`Velocity.Z ≤ 500` enables) |
| 5.2 | launch "second movement tick after the press" | notify in M(3), `DoJump` in M(4) at 60 Hz |
| 5.1 table | type 2 rise loop `JumpSuperRiseLoop` 1162 | the selector has no such name (phase 1 of type 2 is `JumpHighRiseLoop`, and it is never requested); the start clip is followed by the down loop |
| 5.1 | "the variable-height cut … skipped for type 3 (tests in sub-steps 1 and 2)" — implied equal to type 0 for type 1 | right for type 3; for **type 1 the sub-step-1 test is live** (0.5 s clip): the cut starts at clip time > 0.1 |
| 5.1 | type 2 "falls back to type 0 when the actor reaches the height stored at +0xF4" | +0xF4 is the actor Z at Begin; the test `!(Z > +0xF4)` runs in sub-steps 4–6 only (and sub 11 / dive start drop it unconditionally) |
| 5.1 | momentum "+0x110 (0.92)" used "in a flow jump" | mechanism: slot 118 adds a velocity impulse `direction × +0x10C × dt × 60` per tick (sub ≤ 4), +0x10C × 0.92 per tick, cancelled by any stick input; +0x10C = pawn+0x644 = 175 |
| 5.3 sub 4 | "`LandB` instead of `Land` when flag 0x2000" | `Land` / `LandB` and `LightLand` / `LightLandB` by **weapon out at the landing tick** (selector 0x141434330); flag 0x2000 only in the type-2 sub-step-1 landing and as the `B` argument of the jump names |
| 5.3 sub 5 / 6 | "fall variants entered from other states (same code as 4 without the dive check)" | "fall slide" sub-steps entered from the sub-4 tail on Jump+0xBC (`FallSlide` 0104); they run the sub-4 landing code with `falling = false`, so also without ledge / hop / floating exits |
| 5.3 sub 10 | "sets bits 3, 4, 0" | and clears flag 8; slope-slide test comes first |
| 5.4 | dangle after "Jump+0xA8 (0.2 s from Begin)" and gate "sub 1–2 with Velocity.Z ≤ 500" | the tail runs the check only on falling ticks; the 0.2 s is counted on falling ticks only; the sub 1–2 clause of the gate is reachable only for a type-2 jump falling in sub-step 1 |
| 5.4 | `TryHopping(yaw, 0, 0)` | the yaw is always the actor yaw (+0x88 is cleared before the test) |
| 5.5 | `B` variants of the landings "by the battle animation flag" | by weapon out |
| 9.2, Q5 | Idle+0x9C "set to 1 by the Jump landing; its countdown was not located" | the landing writes **Jump+0x9C** (u8). Idle+0x9C is zeroed by Reset and never written: both gate tests are constant |
| 9.2 | "`IdleN` / `IdleB` / `IdleLockon` by (weaponOut, battleAnim, lock-on)" | `IdleB` / `IdleLockon` need weapon out **and** `battleAnim`; everything else is `IdleN` |
| 9.2 | "3.0 s of idle after the battle flag drops → `BattleEnd`" | plain Keyblade: `BattleEnd` at the end of the idle clip that is playing; the 3.0 s countdown only for formchange kinds with the weapon away |
| 9.2 | "`Draw`, +0x8C = 3.0 … weapon not restricted" | the restriction test is vt+0x1EE8 (pawn+0x1130 bit 3, restrict bits 0 and 15, component query 0), not vt+0x1ED8 |
| 9.1, 9.3, sheet `stance.weapon_keep_time` | "10.0 s keep-out timer … read by Run": Run puts the weapon away when it expired | the reader sits behind Run+0xBD, which is never set: no effect |
| 9.1 | battle flag "called by the battle manager when enemies engage or the encounter ends" | mirrored every frame by the player controller (slot 630) from the game object's flags: engaged-enemy count > 0, + 1.0 s linger, or a forced flag |
| 9.3 | "battle began and weapon not out → `RunBattleStart` 0013 / `RunBattleStartW` 1013" | condition is pose ≠ flag (kind 1 / 0x16 / 0x17, running); the name test follows `Draw(1)`, so `RunBattleStartW` always |
| 2.2, P3 | weapon kind of each Keyblade unknown | `ETresWeaponForm` of the current slot; 1 for every Keyblade without formchange [A] |
| 2.3 table, bit 0 | "Cleared by `AirSlide::End` when the next state is not Jump" only | also `Jump::Begin` (type 3, type 2) and the rail jump (0x14150AEC3) |

### Host affordances

| Probe | Called from | What it reads |
|---|---|---|
| Ground probe 0x14063A650 (movement spec 4.5) | Jump slot 114, every tick, range 2000 (3500 in dive sub 8) | sphere r 50 from origin + (0,0,55) down; channel Pawn; blocking WorldStatic / WorldDynamic / TresGimmick; walkable < 45° |
| Floor test component vt+0x9D8 | `Jump::Begin` (fall) | `FindFloor` at the current location: blocking and walkable |
| Ledge probe 0x140638740 → 0x140638900 | Jump tail, falling ticks after 0.2 s; arguments [I]: up 250 (@0x14139807A), down 50 (@0x141398072), forward 0, radius 100 (@0x14139805E) | sphere sweeps, channel Pawn, straight down: r 50 from `location + F × 50 + (0,0,300)` to 150 cm lower (retried with F yawed ±45°), then r 100 from `location + (0,0,250)` to 50 cm lower (per the terrain strand's reading of 0x140638900); a hit counts when its component has **`SQEX_bEnableDangle`** (PrimitiveComponent byte +0x854 bit 2, comp vt+0x4F0) and is walkable; then a free-space test and a grip test (terrain strand, section Dangle, for the instruction-level body) |
| Dangle gate iface slot 63 0x141427830 | before the ledge probe | pawn+0x518 bit 0 clear, CanAct, dangle cooldown pawn+0x6F8 ≤ 0, nothing queued, `Velocity.Z ≤ 0` |
| Rail probe pawn vt+0xEA0 0x1405A7740 | Jump head (range 150), Idle head (range 50), Run | **not a collision query**: walks the registry of rail actors (pawn vt+0x8D8 → list vt+0x978), each active rail's spline components; segment `location → location + (0,0,30)`, direction = stick yaw or facing; accepts a spline within `rail radius + range`; returns actor, component, position, rotation, distance, reversed. A level without registered rail actors always answers false |
| Hop probe (iface slot 184 `TryHopping`) | Jump tail, `Idle::Begin` | hop-point registry (terrain strand) |
| Water | Jump / Idle head: component vt+0x5A0 | physics volume `bWaterVolume` and depth |
| Battle flag | controller tick | host: "an enemy is engaged" (count > 0), kept 1.0 s after the last one; optional forced flag |
| Weapon restricted | Idle / Run | host: restriction bits 0 / 15 of pawn+0xE60 (`TresRestrictActionVolume`), all clear in a test level |
| Lock-on | Idle | host / controller: lock-on active (selects `IdleLockon`) |

To place the features in a test level: a ledge = a walkable-topped primitive flagged "dangle" whose top is met by
a 50 cm sphere travelling from 300 to 150 cm above Sora's location, 50 cm ahead of him, while he falls; high / double jump need only
floor; roll landing needs a drop of ≥ 800 cm (fall height accumulates only while `Velocity.Z < 0`).

### Tests (all abilities, 60 Hz, flat floor unless said; frame 0 = the press, counts as in movement spec 4.1)

| # | Setup / input | Expected |
|---|---|---|
| J1 | Idle, jump held | type 1, `JumpHighStart`; `Velocity.Z = 1950` in M(5); apex 461.9 cm above the floor 0.474 s after the launch (gravity −4116 while rising); `JumpHighRiseLoop` from the tick the 0.5 s clip ends; hang 8 steps at −147; fall −1470 |
| J2 | Idle, jump tapped (1 frame) | as J1 until S(7); `GravityScale = 3.0` from S(7) (clip time 0.1167 > 0.1), rise gravity −8232 from M(8); apex ≈ 277 cm |
| J3 | J1, second press at frame 20 | refused (flag 8 clear: `Velocity.Z` > 500) |
| J4 | J1, second press at frame 27 | accepted (first frame: after M(26) `Velocity.Z` = 1950 − 22 × 68.6 = 440.8 ≤ 500); type 3, `JumpDoubleFlightStart`; `Velocity.Z = 2300` in M(31) (press + 4); apex 642.6 cm above the launch point; pawn+0xE48 bit 0 = 0 from the Begin; releasing the button does not shorten it |
| J5 | J4, third press anywhere in the air | refused (bit 0 clear, Jump+0x9C = 1) |
| J6 | J4, land | `SetSecondJump(1)` on the landing tick; a press from 2 frames after touchdown (sub 11) starts a ground jump with yaw = Jump+0x8C |
| J7 | walk off a ledge (no jump), press on the first airborne frame after the Jump state began | accepted at once (Begin sets flag 8): double flight out of a fall |
| J8 | fall 801 cm with the stick held | `RollLand` (0.7667 s, 300 cm root motion, control back at 0.4325 s); 799 cm → `Land`; ≤ 75 cm → `LightLand` |
| J9 | Superslide → jump | type 2: `JumpSuperStart`, launch 2500 in M(5), Keyblade drawn at Begin, bit 0 and bit 3 cleared; stick neutral: +175 cm/s per tick along the facing, × 0.92 per tick; any stick input ends the push; type returns to 0 on the first sub-4 tick at or below the start height |
| J10 | Air Slide, let it end in the air, press jump during the fall | accepted (next state of the slide was JUMP: bit kept; fall Begin sets flag 8) |
| J11 | Air Slide → air attack (not a jump) → fall → press jump | refused (bit 0 cleared by `AirSlide::End`) |
| J12 | jump next to a "dangle" ledge, rising | no grab while rising; grab test from 0.2 s of falling ticks after the apex |
| S1 | Idle (`IdleN`), battle flag turned on | same tick: Keyblade drawn with effect, `BattleStart` (0.4333 s) → `IdleB`; pose BATTLE from the next movement tick |
| S2 | S1, flag off at t | `IdleB` keeps playing to its end (≤ 5.9333 s), then `BattleEnd` (1.1 s; pose NORMAL at 0.1 s), then `IdleN` with the Keyblade in hand; after 3 × 3.0 s `IdleBreakW` (9.6333 s); at its end the Keyblade is put away with effect |
| S3 | running (not walking), flag turned on | sub-step 0: Keyblade drawn, `RunBattleStartW` (0.5333 s), sub 3 → loop `RunLoopB` |
| S4 | S3, flag off | `RunBattleEnd` (0.5333 s) → normal run loop names with the Keyblade still out; it stays out for any length of running |
| S5 | walking when the flag changes | on: Keyblade drawn, no clip; off: nothing |
| S6 | idle, weapon restricted, Keyblade out | `Draw(0)` + `BattleEnd` at once |
| S7 | land a jump with the Keyblade out | `LandB` / `LightLandB` names (same sequences) |

### Still needs the running game

| Object | Offset | Type | Why |
|---|---|---|---|
| Jump state | +0xBC | u8, write watch | who sets the fall-slide flag (sub-steps 5 / 6, `FallSlide` 0104); +0xC0 vec3, +0xD0 ptr, +0xD8 f32 with it |
| Jump state | +0x114 | u32 per frame | confirm flag 8 timing against `Velocity.Z` (expect set from the first tick ≤ 500) |
| pawn | +0xE48 | u32 per frame | bit 0 through jump → double flight → land (expect clear at the Begin of the second jump) |
| equipment component (pawn+0x598) | +0x378 i32, +0x2F4 u8 (+ index × 0x30) | | confirm kind 1 with each Keyblade equipped and no formchange |
| pawn | +0x630 u32, +0x6E4 u32, +0x10E0 f32 | per frame | stance trace through an encounter: flag from the controller, pose from the notifies, timer unused |
| game object (world+0x158) | +0x5BC i32, +0x5C0 f32, +0x8D4 u8 | per frame | engaged-enemy count and the 1.0 s linger behind the battle flag |
| Run state | +0xBD | u8 | expect always 0 (keep-out block dead) |
| Jump state | anim interface vt+0x2A0 call | event | what the flow-jump start tick does (flag bit 0) |
| component | Velocity vec3, +0x4C8 vec3 | per frame | super-jump inertial push: 175 → × 0.92 per tick, and the resulting horizontal speed |

---

## II.4 Wall Run, Wall Kick, Pole Turn

(strand `wall`; working notes in `build/sora_tmp/actions/p2_wall/`)

Second pass of sections 10.1–10.4. Everything tagged **[I]** was read in the disassembly (address of the
instruction or of the function given); **[A]** asset; **[C]** decompiled C / not re-read; **[D]** derived;
**[?]** not determined. Conventions are those of Part I and of the movement spec (units cm, s, degrees;
`comiss a,b; ja` = a > b). Offsets `+0x..` without an object are state fields. `L` = actor location (root
component world translation, root+0x2B0), `N` = stored wall normal (unit, pointing out of the wall),
`Rot(v)` = `FVector::Rotation()` 0x142D8D950 (pitch, yaw, roll = 0), `R.Rotate(v)` = `FRotator::RotateVector`
0x142D8D890 (local X forward, Y right, Z up), `R.Vector()` = 0x142D93270.

Common engine pieces used below [I]:

- **World hit event.** `cmc HandleImpact` 0x140587580 (0x1405875B3) → pawn vt+0x9D0 0x1405D2240. For a hit
  whose actor is not a pawn: `d = normalize(TraceEnd − TraceStart)`; if `d · ImpactNormal < 0`
  (0x1405D25EA `comiss`, `jae` skips) → pawn sub-object +0x448 vt+0x48 0x1405B3650 → current state
  (queue 0x14150CCA0) → **state interface at state+0x48, slot 9 (+0x48) `OnHit(FHitResult*)`**. It runs
  inside the movement tick M(N), before the state tick S(N). Handlers: Jump 0x14139A730, Run 0x1413B2BE0,
  SlopeSlide 0x1413DC440, Swim 0x1413DCC70, WallRun 0x1413DCE40, AirSlide 0x1413432C0, SuperSlide
  0x1413DC940. Idle, Glide, WallKick, PoleTurn, AthleticFlow have none.
- **Sweeps.** 0x144526380 = `GeomSweepMulti` (PhysX geometry pointer, rotation, out hits, start, end,
  trace channel, params, response params, object params). All probes of this part pass a **sphere**
  (`PxSphereGeometry`: int 0, radius), trace channel **2 (ECC_Pawn)** unless noted, query params ignoring
  the pawn with simple collision (`bTraceComplex` = 0), and a response container set to Ignore for every
  channel then Block for **WorldStatic (0), WorldDynamic (1), TresGimmick (18)** and, for the wall-run
  probes only, **TresBodyCollision (24)** (0x14443E7E0 / 0x14443F9F0). Hit records are `FHitResult`
  (0x88 bytes: +0 bit 0 blocking, bit 1 start-penetrating, +0x18 ImpactPoint, +0x30 ImpactNormal, +0x64
  Actor, +0x6C Component).
- **Component flags** (`UPrimitiveComponent`+0x854): bit 2 `SQEX_bEnableDangle` (get vt+0x4F0), bit 3
  **`SQEX_bEnableWallRun`** (get vt+0x508 0x1404ED830, set vt+0x518), bit 4 **`SQEX_bEnableWallKick`**
  (get vt+0x538 0x1404ED820), bit 5 `SQEX_bEnableAutoRun` (get vt+0x520). Native defaults:
  `UPrimitiveComponent` ctor 0x14402964A all four off; `UMeshComponent` 0x144009D38 and `UModelComponent`
  0x144009DE7: Dangle on, WallKick on, **WallRun off**, AutoRun off.
- **Movement-component writes** (settles P6): vt+0x9E8 0x140592DD0 `SetVelocity(v)`: `Velocity = v`,
  pending +0xC0C = v, flags +0xC18 |= 3; vt+0x9F8 0x140592E10 `SetVelocityXY`: `Velocity.XY`, flags |= 1;
  vt+0xA08 0x140592E40 `SetVelocityZ(f)`, flags |= 2; vt+0xA18 clears the pending value; vt+0xA40 = skip
  `CalcVelocity` (+0xA3C bit 2); vt+0x928(b) = gravity (+0xA3C bit 1 = !b); vt+0xA70(b, v) 0x1405944A0:
  **wall flag** +0xB30 bit 0 = b, **wall direction** +0xB34 = normalize(v). cmc+0x9A0 (pawn vt+0xE28) =
  actual velocity of the last movement tick, `(L − L_prev) / dt` (0x140591991 → 0x140591D3A); cmc+0x9C4
  (pawn vt+0xE58) = root-motion velocity of the last tick (copy of +0xA24, 0x14058D055 → 0x140591D9B).
- **Ability flags.** Player data (pawn vt+0x8C8) word +0x95B0, initialised to **0x7FF** (0x140E84635,
  0x140E8D066, 0x140E92EDA): bit 0 2ndJump, 1 AirSlide, 2 Magic, 3 StyleChange, 4 ShootFlow, 5
  AthleticFlow, 6 AttractionFlow, **7 FreeFlow**, **8 WallRun**, 9 ShortCutMenu, 10 Attack. Writers:
  `ATresPlayerControllerBase::BP_SetAbility…(bool)` (exec 0x142876720–0x142876CC0) → controller
  vt+0x1430…+0x1480 → bit setters 0x140E97BE0…C80; `BP_SetAbilityFreeFlow` → vt+0x1468 0x1413EA950 →
  0x140E97C30 (bit 7); `BP_SetAbilityWallRun` → vt+0x1470 0x1413EAC40 → 0x140E97C80 (bit 8). In the
  cooked data only the Olympus scripts call them (`Levels/he/he_01…03` and their `_gameflow`,
  `sp_01_gameflow`: `false` at the start of the first visit, `true` later) [A].

---

#### Wall Run (id 8)

`TresPlayerState_WallRun`, vtable 0x146945A10; Reset 0x1413D0F80, Begin 0x1413CCD10, Tick 0x1413D87E0,
End 0x1413CF500, `OnHit` 0x1413DCE40. Animation interface: default handlers (`TresEnableChange` →
0x1411FBA40, `TresEnableInput` → 0x1415124D0, `TresTiming` → empty) [I].

##### Fields [I]

| Offset | Meaning |
|---|---|
| +0x88 / +0x90 | wall actor / component (`m_pHitActor`, `m_pHitComponent`) |
| +0x98 | flags: bit 0 debug print, **bit 1 stick this frame** (set by `MoveInput`, cleared in the tick tail), **bit 2 wall valid**, bit 3 entered airborne, bit 4 "roll" from the probe, bit 5 "end" from the probe, **bit 6 guard-button latch**, bit 7 ceiling found, bit 8 early loop start (no writer found in functions with unwind data [?]) |
| +0x9C | wall normal `N` (ImpactNormal of the accepted hit) |
| +0xA8 | direction class: **0 F** = up the wall (actor faces into the wall), **1 R** = along the wall toward the actor's left when facing it (wall on Sora's right; yaw = yaw(N) + 90), **2 L** = the mirror (yaw(N) − 90) |
| +0xAC | stick yaw (world) |
| +0xB0 | input grace timer: 0.2 while the stick is active, then counts down |
| +0xB4 | direction-switch timer |
| +0xB8 | blocked timer |
| +0xBC | normal of the next surface predicted by the probe (corner) |
| +0xC8 | rotator last written to the actor |
| +0xD4 | lateral drift direction (up-run) |
| +0xE0 / +0xEC | rotation of the last hit normal / its slope angle (`OnHit`) |
| +0xF0 | class at entry (−1 after Reset) |
| +0xF8 | `m_Camera` (`ATresCameraWall`) |
| +0x104 / +0x108 / +0x10C | side switch angle 80, down switch angle 155, side move speed 980 (copied from pawn+0xC14 / +0xC18 / +0xC1C at Begin; Reset writes 50 / 150 / 680, 0x1413D107C..) |
| +0x110 | impact point from the probe |

##### Preconditions

`WallRunEnabled` = pawn vt+0xD48 0x141435CE0 [I]: player data ≠ null && **player data +0x95B1 bit 0**
(ability flag 8) && **pawn+0xB64 bit 0 `m_bEnableWallRun`**. `m_bEnableWallRun` is cleared by the
`TresPlayerPawnBase` constructor (0x141423B2E) and **set by the `TresPlayerPawnSora` constructor**
(0x1413E7ECD `or dword ptr [rbx+0B64h],1`); no cooked package overrides it [A]. So for Sora wall running
is on by default; the per-save ability flag (default on) and the geometry flag decide. The FreeFlow bit
(+0x95B0 bit 7), `CanAct` and the pawn-type test are **not** part of the wall-run path.

##### Request: who calls `TryWallRun` [I]

`TryWallRun(FHitResult* hit, bool bProbe)` = interface slot 181 (+0x5A8) 0x141442550: gate (slot 180,
+0x5A0) with `&hit.ImpactNormal`, `&class`, `hit.Component`, `bProbe`; then push (slot 182 0x141445EE0 =
`PushState(8)`); state +0x88 = `hit.Actor`, +0x90 = `hit.Component`, +0x9C = `hit.ImpactNormal`, flags |= 4,
+0xA8 = class. The queue is not cleared; the calling state does `DoFinish`.

| Caller (`OnHit`) | Conditions before the call |
|---|---|
| Jump 0x14139A730 → call 0x14139AA9D, `bProbe` = 1 | pawn status byte ∉ {0x0C, 0x11}; `Velocity.Z < 0` (0x14139A846 `comiss`, `jb`); `pitch = Rot(ImpactNormal).Pitch` ∈ [−41, 41] (0x14139A9CD / 0x14139A9E2); Jump sub-step ∈ {3, 4, 7, 8}, or sub-step 1 with Jump+0xF8 ≠ 0 (flow jump), clip time ≥ 0.2 and `Velocity.Z ≤ 0`; **Jump+0x88 set (stick request of this frame)**; `TryHopping(0, 0, 1)` (slot 184) failed |
| Run 0x1413B2BE0 → 0x1413B2D3A, `bProbe` = 0 | Run+0xAA == 0; walk flag Run+0x94 == 0; sub-step 0; stick magnitude (controller+0x828 vt+0x10) > 0.5 (0x1413B2CEC `comiss`, `jbe`); `TryHopping` failed. Always: pawn+0xDA0 = component, pawn+0xDA8 = ImpactNormal |
| SlopeSlide 0x1413DC440 → 0x1413DC541, `bProbe` = 0 | `TryHopping` failed (head not re-read [C]) |
| Swim 0x1413DCC70 → 0x1413DCCB8, `bProbe` = 0 | Swim sub-step ∈ {1, 2} |

With `Velocity.Z ≥ 0` the Jump handler does something else: ceiling probe (below) with height 200; on a
hit it plays the fall clip and goes to Jump sub-step 4 (`ResetGravityScale`, `SetVelocityZ(0)`).

**Gate** (slot 180) 0x14142FF80 [I]: `NextQueued == 0` && !restrict bit 17 && `WallValid(pawn, &N, &class,
component, 0)` && (`bProbe` ? `WallPoint(pawn, P{normal = N, class})` : true).

##### `WallValid` 0x1406402E0 — entry classification [I]

```
WallRunEnabled (vt+0xD48); the pawn is a player pawn; the player controller exists
if !bNoStickTest (always 0 here): stick magnitude >= 0.5                 // 0x140640417 comiss; jb fail
component != null && component.SQEX_bEnableWallRun                       // 0x140640430 vt+0x508
pitch(N) in [-41, 41]                                                    // 0x1406404B7 jb, 0x1406404CA ja
D  = Rotator(0, stickYaw, 0).Vector()        // stickYaw = controller+0x85C (movement spec 2.3), vt+0x13A8
N2 = normalize(N.x, N.y, 0)                  // only when N.x^2 + N.y^2 > 1e-8
dot = D . N2
if dot < -0.8191520 (0x1406405D7; jae else):  class = 0 (up)          // stick within 35 deg of "into the wall"
elif dot < -0.6427880 or dot > 0.6427880:     fail                    // 0x1406405E9 jb, 0x1406405F6 ja
else: class = (D.x*N2.y - D.y*N2.x >= 0) ? 2 : 1                      // 0x140640613 comiss; setae; inc
```

So: stick within 35° of the direction into the wall → up-run; 35°–50° → **no wall run** (dead zone);
50°–130° → side run on the stick's side; more than 130° (pointing away) → none.

Two sphere sweeps (radius 40, 0x140640622; rotation = quaternion of `(0, stickYaw, 0)`), `Rn = Rot(N)`:

1. **Clearance.** `S1 = L + Rn.Rotate((40, 0, 200))`; `E1 = S1 + R2.Rotate((len, 0, 0))` with class 0:
   `R2 = (Rn.Pitch + 90, Rn.Yaw, 0)`, `len` 150 (0x140640689); class 1: `R2 = (0, Rn.Yaw + 90, 0)`, 500;
   class 2: `(0, Rn.Yaw − 90, 0)`, 500 (0x140640647). Any blocking hit that is start-penetrating, or that
   belongs to another actor and has a component → **fail** (0x14064088B, 0x140640955). (40 cm off the
   wall, 200 above the origin: 150 cm of head room for the up-run, 500 cm of free path for a side run.)
2. **The wall continues.** `R3 = (class 0 ? −Rn.Pitch : 0, Rn.Yaw + 180, 0)` (facing into the wall);
   `S2 = L + R3.Rotate((−40, y, z))` with class 0 `(y, z) = (0, 225)`, class 1 `(−500, 200)`, class 2
   `(+500, 200)` (0x140640A31–0x140640A6B); `E2 = S2 + R3.Rotate((140, 0, 0))`. There must be a blocking
   hit; scanning the hits in order: start-penetrating → fail; own actor or no component → skip; the first
   one whose component has `SQEX_bEnableWallRun` and whose `Rot(ImpactNormal).Pitch` ∈ [−41, 41] →
   **success** (0x140640C86–0x140640CC4).

##### `WallPoint` 0x14063EF00 — per-tick wall tracking [I]

In/out record `P`: +0x00 actor, +0x08 component, +0x10 impact point, +0x1C normal (in: current, out: new),
+0x28 point of the look-ahead hit, +0x34 class (in), +0x38 `roll`, +0x39 `end`, +0x3C next normal.
Requires `WallRunEnabled`. `R = (class 0 ? −pitch(N) : 0, yaw(N) + 180, 0)`. Sphere radius 40 except step C.

```
A. S = L + R.Rotate((-40, 0, 0));  E = S + R.Rotate((140, 0, 0))                 // 0x14063F126, 0x14063F187
   hits in order: start-penetrating -> return false; own actor / no component -> skip;
   first with component.SQEX_bEnableWallRun and Rot(ImpactNormal).Pitch in [-41, 41]:
       P.actor, P.component, P.point = ImpactPoint, P.normal = ImpactNormal;  none -> return false
B. class 0:
     S = L + R.Rotate((-40, 0, 0))
     line test (0x143FD43D0, channel 2) S -> S + R.Rotate((0, 0, 200)): blocked -> P.end = 1; goto C
     sweep S -> S + R.Rotate((0, 0, 300)); for blocking, non-penetrating hits of other actors:
         P+0x28 = ImpactPoint
         if |pitch(P.normal) - pitch(hit.ImpactNormal)| >= 45:                   // 0x14063F874 comiss; jae
             if hit.component.SQEX_bEnableWallRun: P.roll = 1
             elif |ImpactPoint - S|^2 <= 50625 (225^2): P.end = 1                // 0x14063FB40
             break
     if P.roll: sweep L + R.Rotate((-140, 0, 0)) -> + R.Rotate((0, 0, 350));     // room to roll over?
         first blocking non-penetrating hit of another actor:
             if |previous point - start|^2 <= 57600 (240^2): P.end = 1          // 0x14063FBBA
             P.roll = 0;  P+0x28 = its ImpactPoint
   class 1 / 2:
     S = L + R.Rotate((-40, 0, 50));  E = S + R.Rotate((0, class 1 ? -300 : +300, 0))   // 0x14063FCCC / FDA1
     hits in order: start-penetrating -> goto C; first hit of another actor with a component that has
     SQEX_bEnableWallRun and |yaw(P.normal) - yaw(hit.ImpactNormal)| >= 45 (raw difference, no wrap):
         P+0x3C = hit.ImpactNormal;  P.roll = 1;  P+0x28 = ImpactPoint           // corner ahead
C. sphere radius 5 from A0 = L + (0, 0, 50) to A0 - 120 * P.normal               // 0x140640059..0x1406400C5
   hits in order: start-penetrating -> false; first hit of another actor with a component that has
   SQEX_bEnableWallRun -> return true;  none -> return false
```

##### Reset 0x1413D0F80 [I]

Base Reset; flags bits 0–8 cleared; +0x88 / +0x90 = null; +0x9C, +0xBC, +0xD4, +0x110 = zero vector;
+0xC8, +0xE0 = zero rotator; +0xA8 = 0; +0xAC, +0xB0, +0xB4, +0xB8, +0xEC = 0; +0xF0 = −1; +0x100 = 1.8
(no reader in the tick); +0x104 = 50; +0x108 = 150; +0x10C = 680.

##### Begin 0x1413CCD10 [I, camera part C]

```
camera [C for the two helper tests 0x1405EDEC0 / 0x1405E9040]: with a player controller, in the normal
  camera mode and only when class == 0 (0x1413CCE7A): spawn the wall camera (class from 0x141434B70:
  pawn+0x1448 override or ATresCameraWall), m_Camera = it, vt+0x788(owner, controller),
  0x140627FD0(camera, &N) (camera+0x9E0 = N), vt+0x790(1, {0.5, type 3, 2.0})  // blend-in 0.5 s
Sheathe(1, 1.0)                                         // vt+0x1358
flags bit 3 = !OnGround                                  // 0x1413CCFF5 cmove
+0x104, +0x108, +0x10C = pawn+0xC14, +0xC18, +0xC1C      // 80, 155, 980
cmc vt+0xA70(1, -N)                                      // wall flag on, wall direction = into the wall
root+0x990 = 1; mesh vt+0xE20(1)                         // 0x1405B91D0
pawn vt+0x1E58(1), vt+0x1E88(1), vt+0x1E98(1)            // availability bits 0, 3, 4
pawn vt+0x16D8(1)                                        // skip CalcVelocity on
ResetMaxWalkSpeed (vt+0x1578);  MaxAcceleration = class default (vt+0x1648)
```

##### Tick 0x1413D87E0 [I]

`dt` = xmm1. Sub-steps: 0 entry, 6 entry clip, 1 loop, 4 change / roll clip, 7 leave clip.
`Play(name, blendIn, mode, rootScale)` = interface vt+0x18 with slot 0, effect group 0, rate 1.0; the root
scale is a pair (first, second); the default pair is the global at 0x14871E368.

```
head: +0xD4 = 0;  if m_Camera: camera+0x9E0 = N (0x140627FD0)

FACE(class, rot = Rot(N)):                      // used by sub 0 and by every accepted probe
    class 0: (-rot.Pitch, rot.Yaw + 180, 0);  class 1: (0, rot.Yaw + 90, 0);  class 2: (0, rot.Yaw - 90, 0)
    normalised -> +0xC8 -> SetActorRotation (0x143EAD160)

sub 0 (0x1413D9B1B):
    FACE(class);  StopMovement;  SetMovementMode(Flying 5)
    if flags & 8 (airborne entry): Play(WallRunLoopF | WallRunLoopR | WallRunLoopL, -1, mode 2, default)
                                   m_bEnableChange = 1                          // 0x1413D9CF5
    else:                          Play(WallRunF | WallRunR | WallRunL, -1, mode 2, default)
    sub = 6;  +0xF0 = class

sub 6 (0x1413D8903):
    if m_bEnableChange:
        if flags & 2 (stick):  sub = 1;  +0xB4 = 0;  LOOP(blendIn 0)            // 0x1413D891C
        else: cmc vt+0xA70(0, 0)                                                // wall flag off
              if class == 0: sub = 7                                            // let the clip finish
              elif pawn+0xD98 bit 11 (debug flag, false by default): 0x1405A1D30(0.1, 1.0); sub = 7
              else: DoChain; return
    else:
        if !(flags & 4): DoChain; return
        ledge = hop = (class == 0)
    if IsAnimEnd: DoChain; return
    if ledge: LEDGE()                                                           // may DoChain
    if hop && TryHopping(0, 0, 1): DoFinish; return
    -> tail                                                                     // no wall probe in sub 6

LOOP(blendIn): class 0: Play(WallRunLoopF, blendIn, mode 3, (1.0, pawn+0xB68))   // MyWallRunSpeedScaleUp 1.84
               class 1 / 2: Play(WallRunLoopR / L, blendIn, mode 3, (pawn+0xB6C, 1.0))  // ...ScaleSide 1.84

sub 1 (0x1413D8E46):
    if !(flags & 4) or +0xB0 <= 0: DoChain; return                              // 0x1413D8E57 comiss; jae
    if flags & 2 (stick):
        blocked test: class 0: a = rmVel.Z, b = actVel.Z;  else a = |rmVel.XY|, b = |actVel.XY|
                      // rmVel = cmc+0x9C4, actVel = cmc+0x9A0
            if a * 0.3 > b: +0xB8 += dt; if +0xB8 > 0.2: DoChain; return       // 0x1413D8EEC..0x1413D8F0D
            else +0xB8 = 0
        with the wall camera active: +0xAC = yaw of (stick vector rotated by (0, camera+0x6FC, 0))  // 0x1413D900D..913F
        d = wrap180(+0xAC - actorYaw)
        cmd = |d| >= +0x108 (155) ? BACK : d >= +0x104 (80) ? RIGHT : d > -80 ? none : LEFT   // 0x1413D92A7..92D7
        if cmd == none: +0xB4 = 0
        else: +0xB4 += dt
              if +0xB4 >= 0.1 (0x1413D92F0; jb skips): flags &= ~0x100; CHANGE(class, cmd)
        if cmd == none or the timer is still running:
            if class == 0:                                                      // lateral drift
                v = +0x10C (980);  if +0xB8 > 0: v *= 0.35;  v = max(v, 0.1)
                +0xD4 = Rotator(0, +0xAC, 0).Vector() (z = 0)
                cmc SetVelocityXY(v * +0xD4)                                    // pawn vt+0x16C0, 0x1413D950C
                hop = true
                blend X of the loop = clamp(d, min, max of the blend space)     // 0x1405A0A10, vt+0x120
        ledge = (class at tick start == 0)
        if ledge: LEDGE();   if hop && TryHopping(0, 0, 1): DoFinish; return
    PROBE()

CHANGE(class, cmd) (0x1413D9304): all Play(name, -1, mode 2, scale), then sub = 4
    class 0: BACK -> DoChain;  LEFT -> class = 1, WallRunChangeFR (1, 1);  RIGHT -> class = 2, WallRunChangeFL (1, 1)
    class 1: BACK -> class = 2, WallRunChangeL (0.5, 0.5);  LEFT -> DoChain;  RIGHT -> class = 0, WallRunChangeRF (1, 1)
    class 2: BACK -> class = 1, WallRunChangeR (0.5, 0.5);  LEFT -> class = 0, WallRunChangeLF (1, 1);  RIGHT -> DoChain
    after a change: StopMovement                                               // 0x1413D9479
    (LEFT / RIGHT / BACK are relative to the actor's facing: in the up-run "left" starts the run to the
     left, in a side run the side that points into the wall climbs and the other one drops off)

PROBE() (0x1413D965C): flags &= ~4
    P = {normal = N, class = class at tick start};  if !WallPoint(pawn, P): -> tail     // valid bit stays 0:
                                                                                         // DoChain next tick
    dp = wrap180(pitch(N) - pitch(P.normal));  dy = wrap180(yaw(N) - yaw(P.normal))
    if |dp| > 35 or |dy| > 35: -> tail                                          // 0x1413D975A..0x1413D9775
    +0x88, +0x90, N = P.actor, P.component, P.normal;  flags |= 4;  bit 4 = P.roll;  bit 5 = P.end
    +0x110 = P+0x28;  +0xBC = P+0x3C
    if flags & 0x40: if TryWallKick(actor, component, &N, 1, 0) (slot 82):
                         queued WallKick+0xA5 = 1;  return                       // 0x1413D981D, 0x1413D987B
    cmc vt+0xA70(1, -N);  FACE(current class, Rot(P.normal))
    if flags & 0x20: DoChain; return
    if flags & 0x10: +0x110 = P.point;  flags &= ~4
                     Play(WallRunRollF | WallRunRollR | WallRunRollL, 0, mode 2, default);  sub = 4

LEDGE() (0x1413D95A9): if 0x14063E620(pawn, out) && TryDangle(out.actor, out.component, &out.point, out.yaw, 0)
                          (slot 64, +0x200): DoChain; return

sub 4 (0x1413D8AD0):
    StopMovement
    if flags & 0x10 and flags & 4 and class > 0:        // set by OnHit when the predicted surface is touched
        rot = (0, Rot(+0xBC).Yaw + (class 1 ? +90 : -90), 0) -> +0xC8 -> SetActorRotation;  flags &= ~0x10
    if class == 0 and clip time > 0.3 (0x1413D8C14) and !(flags & 0x80):
        if CEILING(pawn, out, 225.0): flags |= 0x80;  ResetGravityScale;  cmc SetVelocityZ(0);  DoChain; return
    if IsAnimEnd or (flags & 0x100 and clip time > 0.2):
        flags &= ~0x100;  sub = 1;  +0xB4 = +0xB8 = 0;  LOOP(blendIn -1)
        if class == +0xF0 and m_Camera: camera+0x804 = 1.0

sub 7 (and any other value): if IsAnimEnd: DoChain

tail (0x1413D9DAD): if flags & 2: +0xB0 = 0.2   else: +0xB0 = max(+0xB0 - dt, 0)
                    flags &= ~0x42                                              // stick and guard latch
```

`OnHit` 0x1413DCE40 [I]: +0xE0 = `Rot(ImpactNormal)`; +0xEC = slope angle of that normal; if flags & 0x10
and sub-step 4 and `ImpactNormal == +0xBC` (exact float compare): +0x88 / +0x90 / N = the hit, flags |= 4.

**Ceiling probe** 0x140634980(pawn, out, H) [I] (Part I called it a top-edge probe): sphere radius 5,
rotation = actor rotation, from `L` straight up `H`; **trace channel 18 (TresGimmick)**; response Block
for WorldStatic, WorldDynamic, TresGimmick. Hits of other actors in order: start-penetrating → false;
`|Rot(ImpactNormal).Pitch| ≥ 45` → true (out: actor, component, point, height above `L`, pitch); otherwise
(a wall-like surface) record the height and continue.

**Ledge probe** 0x14063E620(pawn, out) [I]: sphere **radius 90**, rotation = actor rotation, from
`L + forward2D × 80 + (0, 0, 250)` down 100; channel 2; Block WorldStatic, WorldDynamic, TresGimmick. First
hit of another actor (start-penetrating → false) whose component has **`SQEX_bEnableAutoRun`** (vt+0x520),
passes cmc vt+0x7A8 (0x140589A60, walkable-hit test [C]) and whose surface slope is < 50° → out (actor,
component, adjusted point, yaw).

##### Requests while wall running [I]

- `MoveInput` (slot 35 0x141440600, case 0x1414408ED): flags |= 2, +0xAC = stick yaw (any active stick,
  i.e. magnitude ≥ 0.1).
- Guard button: `TryFlow` (slot 72 0x1414417B0, case 0x141441841) after the flow gate 0x14142E320 (FreeFlow
  bit, pawn type 1, `CanAct`, not in water, !restrict 17, **`HasAbility(SUPERSLIDE 19)`**): flags |= 0x40
  and returns false. The next accepted probe turns it into `TryWallKick` and marks the kick to end in a
  Superslide. The latch lasts one tick (tail clears it).
- Jump: gate 0x14142A6F8: sub-step ∈ {1, 3} (3 is never set; so: the loop). `JumpRequest` 0x14143DE2E:
  **`HasAbility(SUPERJUMP 18)`**, else refused; `ClearQueue`; `RequestJump(actor yaw, type 2, bAir)` with no
  launch direction (Jump+0x100 = 0, +0x110 = 0.92).

##### End 0x1413CF500 [I]

```
m_Camera: vt+0x798(1, {0.5, type 3, 2.0});  m_Camera = null
pawn vt+0x16D8(0);  cmc vt+0xA70(0, 0);  pawn vt+0x1008();  ResetMaxWalkSpeed;  MaxAcceleration reset
root+0x990 = 0; mesh vt+0xE20(0)
if !OnGround:
    v = actVel (cmc+0x9A0)
    if v.Z > 0 (0x1413CF60C; ja): SetMovementMode(Falling 3);  cmc SetVelocity(v * 0.5)
    else: SetMovementMode(Falling 3);  w = rmVel (cmc+0x9C4);  if |w|^2 > 0: cmc SetVelocity(w * 0.5)
    gravity on (vt+0x1728(1))
next queued id: 0 or JUMP 3: if battle flag && vt+0x1360(): Draw(1, 1, 1.0);  DANGLE 6, HOPPING 9: nothing;
                any other: if vt+0x1360(): Draw(1, 1, 1.0)
```

##### Movement model [I for the writes, C for the flying move]

All displacement is root motion in Flying mode with `CalcVelocity` skipped and the stock velocity untouched,
plus the lateral `SetVelocityXY` of the up-run. The flying move (cmc slot 248 0x14058E570) reads the wall
flag: on a blocking hit it slides with a horizontalised normal when the hit normal points up (0x14058EA98),
and without a hit it can add a displacement along the wall direction (cmc+0xC04 / +0xC08, 0x14058EC44)
[C — not re-derived; probe W1]. Facing is `FACE`, rewritten at every accepted probe.

Speeds [D from A]: up-run loop `WallRunLoopF` (0124 / 0124L / 0124R, blend axis −10..10): 317.33 cm per
0.4667 s × 1.84 = **1251 cm/s**; side loops 0125 / 0126: 317.33 × 1.84 → **1251 cm/s**; lateral drift
while running up 980 cm/s (343 while blocked).

##### Clips [A]

| Name | Asset | Length (s) | Root motion in actor space (fwd, right, up, yaw) | Notifies that matter | Used |
|---|---|---|---|---|---|
| `WallRunF` | 0121 | 1.0 | up 451 at frame 16 then back to 0, fwd −350 (run up and drop back) | `TresEnableChange` 0.1671 and 0.3667, `TresLandAnimPose` 0 | sub 0, ground entry, class 0 |
| `WallRunR` / `L` | 0122 / 0123 | 1.1333 | 1238 fwd, ∓314 right, arc up 303, yaw ∓45 | `TresEnableChange` 0.5296 | sub 0, ground entry, class 1 / 2 |
| `WallRunLoopF` | blend space 0124 / L / R | 0.4667 | up 317.33, linear | effect `vs_wrn_smk0` | loop, class 0; scale (1, 1.84) |
| `WallRunLoopR` / `L` | 0125 / 0126 | 0.4667 | fwd 317.33, linear | | loop, class 1 / 2; scale (1.84, 1) |
| `WallRunRollF` | 0127 | 0.7 | up 495 | | roll over an edge |
| `WallRunRollR` / `L` = `WallRunChangeR` / `L` | 0128 / 0129 | 0.6333 | fwd 516, bow ∓176 right, up 110 | | corner roll (scale 1) and reversal (scale 0.5) |
| `WallRunChangeFR` / `FL` | 0136 / 0137 | 0.2333 | fwd 170, bow ∓45, up bump 60 | | up → side |
| `WallRunChangeRF` / `LF` | 0138 / 0139 | 0.2333 | up 170, right ∓30, bow back 54 | | side → up |
| `WallRunJump` / `H` / `F` | 0133–0135 | 0.7 | up 495 | | not referenced by this state |

---

#### Wall Kick (id 23)

`TresCharState_WallKick` (the player class shares its vtable 0x14694D490); Reset 0x14150E2F0, Begin
0x14150A0B0, Tick 0x141511E90, End 0x14150C3E0. No `OnHit`.

##### Fields [I]

+0x88 / +0x90 wall actor / component; +0x98 wall normal `N`; +0xA4 direct entry; **+0xA5 Superslide latch**;
**+0xA6 jump latch**; +0xA7 chain latch (set by the attack request, slot 45 0x1414385DD); +0xA8 stick
latch; +0xAC stick yaw; +0xB0 bit 0 form weapon.

##### Request [I]

Callers of `TryWallKick` (slot 82, +0x290), all with `bDirect` = 1, `bKeep` = 0:

| Caller | When |
|---|---|
| WallRun Tick 0x1413D981D | guard latch on an accepted probe (then `+0xA5 = 1` on the queued state) |
| AirSlide `OnHit` 0x1413435C0 | any world hit, not in water (AirSlide+0xA8 bit 3 clear), after the pole probe + `TryPoleTurn` failed |
| SuperSlide `OnHit` 0x1413DCC3E | more than 0.1 s into the slide (`SuperSlideTime − 0.1 > time left`, 0x1413DCA62), pole probe failed, byte SuperSlide+0x94 ≠ 0 (meaning not read [?]) |

DodgeRoll and AthleticFlow are in the gate's state mask but have no caller.

`TryWallKick(actor, component, &normal, bDirect, bKeep)` 0x141442480: gate; `ClearQueue` unless `bKeep`; push
(slot 83 0x141445D80): +0x88, +0x90, N, +0xA4 = `bDirect`; if `bDirect` and stick magnitude > 0.5
(0x141445E4E): +0xAC = stick yaw (controller+0x85C), +0xA8 = 1.

**Gate** (slot 81) 0x14142F990(component, &normal) [I], in order: !pawn+0x518 bit 0; `CanAct`; not in
water; pawn type == 1; !(pawn+0xE62 bit 0); !vt+0x2540; `NextQueued == 0`; FreeFlow bit (player data
+0x95B0 bit 7, 0x14142FA80); **`HasAbility(0x61 = WALL_KICK 97)`** (0x14142FA90 — Part I's open question:
it *is* tested, here); current state ∈ {WALLRUN 8, DODGEROLL 16, AIR_SLIDE 20, SUPER_SLIDE 21,
ATHLETICFLOW 42} (`bt` mask 0x40000310100); !restrict 17; component ≠ null and
**`SQEX_bEnableWallKick`** (0x14142FAF0). Then a sphere sweep, **radius 5**, rotation = actor rotation,
from `L + (0, 0, 75)` to that point `− 105 × normal` (0x14142FB62, 0x14142FB7E), channel 2, Block
WorldStatic / WorldDynamic / TresGimmick: true on the first blocking hit of another actor whose component
is null or has `SQEX_bEnableWallKick` and whose `Rot(ImpactNormal).Pitch` ∈ [−30, 30] (0x14142FEAE jb,
0x14142FEB3 jbe). (The ±30° is the wall's steepness, not a yaw window.)

##### Reset / Begin / End [I]

Reset: flags &= 0xFFFFF810; sub = 0; +0x88 / +0x90 = null; N = 0; +0xA4…+0xA7 = 0; +0xA8 = 0; +0xAC = 0.

Begin: pawn vt+0x1E98(1) (bit 4: Superslide allowed); vt+0x19E0(1); with +0xA4: `StopMovement`,
`SetMovementMode(Flying)`, `Draw(1, 1, 1.0)`, vt+0x1388(1, 0, 0) unless +0xB0 bit 0,
`Play(WallKickF, −1, mode 2, effect group 0x139)`, sub = 2.

End: `SetMovementMode(Falling 3)`; vt+0x1388(0, 0, 0); vt+0x19E0(0).

##### Tick 0x141511E90 [I]

```
sub 0 (only without +0xA4; no caller): SetActorRotation((pitch(N), yaw(N) + 180, roll(N)));  StopMovement;
       Flying;  Play(WallJumpStart, -1, mode 2);  sub = 1
sub 1: if IsAnimEnd: Draw(1, 1, 1.0);  vt+0x1388(1, 0, 0);  sub = 2  (and continue with sub 2 in this tick)
sub 2 (0x14151203B) -- ONE tick, no waiting:
       D = actor forward;  if +0xA8: D = Rotator(0, +0xAC, 0).Vector()
       dir = 0;  if D . N >= -0.819 (0x141512118 / 0x141512193; jb keeps 0):
                     dir = (D.x*N.y - D.y*N.x >= 0) ? 2 : 1
       yaw = Rot(N).Yaw + (dir 0: 180 | dir 1: +90 | dir 2: -90);  SetActorRotation((0, yaw, 0))
       StopMovement;  SetMovementMode(Flying)
       Play(dir 1: WallKickR | dir 2: WallKickL | dir 0: WallKickF, -1, mode 2, effect group 0x139);  sub = 3
sub >= 3 (0x141511EE9):
       if nothing queued: if +0xA6 and clip time > 0.1 (0x141511F15; jbe skips):
                              RequestJump(actor yaw, type 2, bFall 0, bAir 1) (slot 170);  DoChain;  return
       elif +0xA7 and m_bEnableChange: DoChain;  return
       if IsAnimEnd: if nothing queued and +0xA5: push SuperSlide (slot 73, kind 1, no yaw)
                     DoChain
tail: +0xA8 = 0
```

Because a state begun in `MainProc` is ticked in the same call, the `WallKickF` of Begin is replaced at
once by the direction clip. `dir` = 0 means "the facing (or stick) points into the wall within 35°": Sora
faces the wall and `WallKickF` turns him around; otherwise he is turned parallel to the wall and kicks
off diagonally.

Requests: `MoveInput` case 0x141440D03: magnitude > 0.5 → +0xAC = yaw, +0xA8 = 1 (only matters before the
sub-2 tick). Jump: gate 0x14142A757: pawn+0xE48 bit 3 && `HasAbility(SUPERJUMP)` && sub-step > 2;
`JumpRequest` 0x14143DF4F: +0xA6 = 1. Guard: flow gate (needs `SUPERSLIDE`) → `TryFlow` 0x14144189C:
+0xA5 = 1.

##### Clips [A]

| Name | Asset | Length | Root motion (fwd, right, up, yaw) | Notifies |
|---|---|---|---|---|
| `WallKickF` | 1169 | 0.5 | still for 12 frames, then −80 fwd with +180° yaw | `TresShot` 0.199, `TresEnableChange` 0.4674; SE `se00001_003`, `_013`, `_039`; effect `vp_ffa_wal0` (group 313), `vp_ffa_rad0` |
| `WallKickR` / `L` | 1170 / 1171 | 0.3333 | 286 fwd, ∓112 right, yaw ∓45 | `TresShot` 0.1338 / 0.1327, `TresEnableChange` 0.2663 |
| `WallJumpStart` | 1168 | 0.4667 | up 200.8 (starts at frame 4) | unused entry |

---

#### Pole Turn (id 24)

`TresPlayerState_PoleTurn` 0x146935FE8 over `TresCharState_PoleTurn`; Reset 0x14150DFE0; Begin 0x1413A41F0
→ 0x141509AA0; Tick 0x1415102E0; End 0x1413AA820 → 0x14150BF30; slots 113 0x141514540, 114 0x141514890,
115 0x14150E590, 116 0x14150E910.

##### Fields [I]

+0x88 / +0x90 pole actor / component; +0x98 grab rotation (rotator); +0xA4 attach offset in component
space; +0xB0 bit 0 horizontal (swing bar); +0xB4 attach radius; +0xB8 target relative location; +0xC4
loop count (int; Reset 3, player Begin: `MyPoleTurnRotMax` 4); +0xC8 loop length; +0xCC approach timer;
**+0xD0 remaining time**; +0xD4 / +0xD8 stick flag / yaw; **+0xDC latches: bit 0 jump, bit 1 guard**;
+0xE0 Superslide yaw; +0xE4 rotator written to the actor (+0xE8 = release yaw); +0xF0 bit 0 "rotate on
release".

##### The pole component and its probe [I, A]

`UTresPoleComponent` (vtable 0x146638700; `TresFlowComponentBase` → `TresLockonTargetComponent`), reflected:
`m_bEnableNearDetectOffset` (+0x8B8 bit 0), `m_AttachOffset` (+0x8BC), `m_AttachRadius` (+0x8C8),
`m_BaseOffset1` (+0x8CC), `m_BaseOffset2` (+0x8D8), `m_CheckRadius` (+0x8E4), `m_bIsHorizontality`
(+0x8E8), order numbers, colour, `OnTresStartPoleTurn` (+0x900), `OnTresEndPoleTurn` (+0x910). Constructor
0x140F8C870: +0x8B8 bits 0–1 = 0, `m_AttachRadius` 0, `m_CheckRadius` 50.

| Virtual | Does |
|---|---|
| +0xBD0 0x140F932B0 `Test(point, range, &distSq, &near)` | needs component+0x868 bit 0 (active); `W1, W2` = world positions of `m_BaseOffset1 / 2`; `near = ClosestPointOnSegment(point, W1, W2)` (0x142D74FB0); true when `|point − near|² ≤ (range + m_CheckRadius)²` (0x140F9347D) |
| +0xBA0 0x140F965E0 `IsHorizontal` | +0x8B8 bit 1 clear: `m_bIsHorizontality`; set: `|axis.z| ≤ 0.7` |
| +0xBC0 0x140F94B90 `AttachOffset(pos)` | `m_bEnableNearDetectOffset`: closest point on the segment to `pos`, in component space; else `m_AttachOffset` |
| +0xBC8 0x140F94D00 `GrabRotation` | +0x8B8 bit 1 clear: the component's world rotation |
| +0xB88 / +0xB90 / +0xB98 | world `m_AttachOffset` / broadcast start / broadcast end |

**Probe** pawn vt+0x2330 0x1405A7500(point, point, range, out): **no collision query** — it walks the
registered pole list (pawn vt+0x8E0 → vt+0x9B0), skipping inactive components, the pole stored in
pawn+0x660 (the last one used) and components without owner, calls `Test` and keeps the smallest
`distSq`; out = (owner actor, component). Callers: AirSlide `OnHit` and SuperSlide `OnHit` with
`point = L + actorRotation.Rotate((120, 0, 90))`, range 100 (0x141343408, 0x1413DCA74); AirSlide and
SuperSlide ticks (Part I 6.3 / 8).

##### Request [I]

`TryPoleTurn(out, bKeep)` slot 78 0x14143F0C0: `out.component.IsHorizontal()` ?
**`HasAbility(0x60 POLE_SWING)`** : **`HasAbility(0x5F POLE_SPIN)`** (0x14143F10B / 0x14143F131); gate (slot
77 0x14142C640): !pawn+0x518 bit 0, `CanAct`, not in water, pawn type 1, !(pawn+0xE62 bit 0), !vt+0x2540,
`NextQueued == 0`, FreeFlow bit, state ∈ {AIR_SLIDE 20, SUPER_SLIDE 21, ATHLETICFLOW 42}, !restrict 17;
`ClearQueue` unless `bKeep`; push (slot 80 0x141445270): +0x88 / +0x90, +0x98 = `GrabRotation()`, +0xA4 =
`AttachOffset(L)`, +0xB0 bit 0 = `IsHorizontal()`, +0xB4 = `m_AttachRadius`.

##### Begin [I]

Parent 0x141509AA0: availability bits 0, 3, 4 set (vt+0x1E58 / +0x1E88 / +0x1E98); `Draw(1, 1, 1.0)`;
vt+0x1388(1, 0, 0); vt+0xDC8(1.0); pawn+0x660 = the component (vt+0x2340); horizontal: pawn vt+0xCA8(0);
camera calls (0x14060B860(camera, 1); horizontal: 0x140627E30 with blend 0.5). Player 0x1413A41F0: then
+0xC4 = pawn vt+0x28F8() = pawn+0xBF4 `MyPoleTurnRotMax` (4) and two controller camera requests.

##### Tick 0x1415102E0 [I; the quaternion algebra of sub 0 is C]

```
sub 0: rot = +0x98
       horizontal: v = rot.Rotate((0, 1, 0));  if F.x*v.y - F.y*v.x < 0 (F = actor forward): rot turned 180 deg about Z
                   if +0xB4 != 0: pawn vt+0xB88((+0xB4, 0, 0))          // mesh offset
       vertical:   rot = heading with forward = cross-product of the actor's right vector and the pole axis [C]
                   if +0xB4 != 0: pawn vt+0xB88((0, -(+0xB4), 0))
       SetActorRotation(rot);  +0xE4 = rot;  AttachToComponent(pole) (0x143E8FC60)
       +0xB8 = +0xA4;  +0xCC = 0.5;  StopMovement;  SetMovementMode(Flying)
       Play(horizontal ? PoleSwing : PoleTurn, -1, mode 2)
       vertical: +0xB8 += relativeRotation.Rotate((0, 0, -100))                  // 0x141510DD1
       pole.BroadcastStart();  sub = 1
sub 1: if IsAnimEnd: SetActorRelativeLocation(+0xB8) (0x143EAC880);  StopMovement
                     Play(horizontal ? PoleSwingLoop : PoleTurnLoop, -1, mode 3)
                     +0xC8 = clip length;  +0xD0 = (float)+0xC4 * +0xC8;  sub = 2
       else: +0xCC -= dt;  a = clamp(2 * +0xCC, 0, 1)
             SetActorRelativeLocation(+0xB8 + (currentRelative - +0xB8) * a)
sub 2: release = slot 116: (+0xD0 <= 0) or (pole no longer valid -> latches cleared) or a latch is set
       if !release: +0xD0 -= dt
       elif +0xD0 > 0:
           if nothing queued:
               if latch bit 0 (jump):  +0xF0 |= 1;  if +0xD4: +0xE8 = +0xD8
                   slot 171 0x1414459A0(yaw +0xE8, bFall 0, pawn+0x644, pawn+0x648)   // RequestJump type 2 bAir + launch direction
               elif latch bit 1 (guard): push SuperSlide(kind = horizontal ? 1 : 2, yaw +0xE0); its +0xA8 |= 8
           finish
       else:                                                    // time is up
           if nothing queued: +0xF0 |= 1
               vertical: push SuperSlide(kind 2, no yaw); its +0xA8 |= 1
               horizontal: the jump of the latch branch (yaw +0xE8)
           finish
       finish = stop slot 0 (interface vt+0xB8(0, 0.0)); DoFinish
tail: +0xD4 = 0
```

Slot 113 (jump latch): +0xDC |= 1. Slot 114 (guard latch): if not yet latched: +0xDC |= 2, +0xE0 = +0xD4 ?
+0xD8 : actor yaw. Slot 115: pole valid = component+0x868 bit 0. `MoveInput` case 0x141440D7B: +0xD8 = yaw,
+0xD4 = 1. `TryFlow` case 0x1414418F9 (flow gate: needs `SUPERSLIDE`): if stick magnitude > 0.5: +0xD4 = 1;
slot 114. Jump gate 0x14142A808: sub-step > 1 && pawn+0xE48 bit 3; `JumpRequest` 0x14143DFA7: slot 113.

##### End [I]

Player 0x1413AA820: unless the next state is SUPER_SLIDE: pawn+0x660 = 0 (vt+0x2348) — so the pole just
left cannot be re-grabbed by the Superslide it launched; next state an attack, horizontal and
`HasAbility(0x5E SUMMERSALT)`: special exit [C]; controller camera request. Parent 0x14150BF30: stop slot
0; if attached: `StopMovement`, detach (0x143E93FF0), `SetActorLocation` (bone "Center" based, [C]), and if
+0xF0 bit 0: `SetActorRotation((0, +0xE8, 0))`; pawn vt+0xCA8(1); `SetMovementMode(Falling)`; vt+0x1388(0,
0, 0); camera restore (blend 0.5); `pole.BroadcastEnd()`.

##### Clips [A]

| Name | Asset | Length | Root motion | Notifies |
|---|---|---|---|---|
| `PoleTurn` | 1172 | 0.1667 | none | effects `vp_ffa_rad0`, `vp_atf_hjk01` |
| `PoleTurnLoop` | 1173 | 0.3 | yaw 360° per cycle (40° per frame) | `TresAtk` `FF_POLE_SPIN` (collision group `SphereM`) 0–0.3; SE `se00001_017` |
| `PoleSwing` | 1174 | 0.1667 | none | |
| `PoleSwingLoop` | 1175 | 0.4 | none | `TresAtk` `FF_POLE_SWING` 0–0.4; SE `se00001_018` |

Budget: 4 × 0.3 = **1.2 s** (spin), 4 × 0.4 = **1.6 s** (swing), counted only while no latch is set.

---

### Corrections to Part I

| Section | Part I says | Correct |
|---|---|---|
| 0 item 7, 10.2, sheet `wallrun.enabled` | wall run is off in the pawn defaults; "the level turns it on" | `m_bEnableWallRun` is **set by the `TresPlayerPawnSora` constructor** (0x1413E7ECD). The gates are the ability flag (player data +0x95B1 bit 0, default on, toggled only by Olympus scripts) and `SQEX_bEnableWallRun` on the hit component |
| 0 item 7, 2.5, 8, 10 | "flowmotion needs an unlock bit (+0x95B0 bit 7)" for all of it | bit 7 = `BP_SetAbilityFreeFlow`; it gates the flow trigger, wall kick and pole. **Wall run does not test it** (it tests bit 8 `BP_SetAbilityWallRun`) |
| 2.5 | `WALL_KICK` 97 "was not seen tested" | tested in the wall-kick gate, 0x14142FA90 |
| 2.5 | "bits 0 / 1 mask the counts of kinds 0x13 / 0x17" | the word is the script ability mask listed above (bit 0 2ndJump, bit 1 AirSlide …) |
| 10 (head) | common conditions "pawn type 1, unlock bit, not in water, `CanAct`" | true for the flow gate, wall kick and pole gates; the wall-run gate has none of them (only nothing queued, !restrict 17, `WallRunEnabled`) |
| 10.1 | wall kick: "wall yaw within ±30°", "overlap query" | a radius-5 sphere sweep 105 cm along −normal from 75 cm above the origin; ±30 is the **pitch** of the hit normal; component must have `SQEX_bEnableWallKick` |
| 10.1 | wall-kick callers include ATHLETICFLOW / DODGEROLL contact | only WallRun (guard latch), AirSlide `OnHit`, SuperSlide `OnHit` call it |
| 10.1 / Q6 | callers of `TryWallRun` unknown | `OnHit` of Jump (falling, stick held), Run (stick > 0.5), SlopeSlide, Swim |
| 10.2 | "in the entry clip after 0.3 s a top-edge probe (0x140634980, 225) ends the run over the edge" | it runs in **sub-step 4** (change / roll clip) of the up-run and is a **ceiling probe** (sphere r 5 straight up 225, surface pitch ≥ 45°) |
| 10.2 | "no stick for 0.2 s ends it" | two 0.2 s timers: the input grace (+0xB0) and the blocked timer (actual speed < 30 % of the root-motion speed, +0xB8) |
| 10.2 | "pitch / yaw deviation > 35° → end" | the probe result is rejected and the run ends on the **next** tick (valid bit) |
| 10.2 | guard button → wall kick | needs `SUPERSLIDE` (flow gate) and the wall-kick gate; the kick then ends in a Superslide |
| 10.3 | "Sub 2 waits for input: jump …, guard …, stick → kick direction" | sub-step 2 lasts one tick and picks the direction from the facing (or the stick latched at the push); jump and guard are honoured in sub-step 3 (jump: clip time > 0.1; Superslide: at the clip end) |
| 10.3 | kick direction "by the angle to the wall normal … more than 35° off the wall line" | `D · N ≥ −0.819` → side kick (sign of `D.x·N.y − D.y·N.x`: ≥ 0 → `WallKickL`); else `WallKickF` |
| 10.4 | "position interpolated to the grab pose over 0.5 s" | the relative location approaches the target with a weight falling from 1 to 0 at 2/s, but the entry clip (0.1667 s) ends it with a snap |
| 10.4 | "budget exhausted → release" | vertical pole: automatic Superslide (kind 2); bar: automatic directed jump |
| 10.4 | pole probe = "point query" (host collision) | a registry walk with a point-to-segment distance test; no collision |
| 2.4 / P6 | vt+0x9E8 / +0x9F8 "which is which" | +0x9E8 sets the full velocity, +0x9F8 only XY, +0xA08 only Z |

### Host affordances

**Runnable wall.** A collision component that (1) blocks sphere sweeps on channel Pawn with responses
WorldStatic / WorldDynamic / TresGimmick / TresBodyCollision (i.e. its object type is one of those four),
simple collision; (2) has **`SQEX_bEnableWallRun` = true** — the only "tag" the code reads (no physical
material, no actor class, no component tag; `SQEX_OverridePhysMaterialWallClimb` /
`TresMapID_DefaultWallRun` is not read by these probes); (3) presents impact normals whose pitch is within
±41° of horizontal (wall between 49° overhang and 49° lean-back). In the cooked game the flag is set per
instance on plain `StaticMeshActor` components of the map packages (`Maps/<world>/umap/…`: he 29
packages, ts 60, bt 17, mi 15, ra 11, ca 11, fz 9, bx 5 …; e.g. `Maps/he/umap/he_02/he_02_map_areaA`,
actor `SM_he_Rock_ClimbA3`, with `SQEX_bEnableDangle = false`) and on gimmick templates
(`Blueprints/Gimmick/bt/…`, 51 packages) [A]. Minimum extent from the probes, measured from the actor
origin at the moment of contact, wall normal `N`:

| Entry | The wall surface must be hit by a r-40 sphere moving 140 cm into the wall, starting 40 cm off the origin, at | And free space |
|---|---|---|
| up-run | 225 cm above the origin | r-40 sphere path from 200 to 350 cm above the origin, 40 cm off the wall |
| side run | 500 cm to that side, 200 cm above the origin | r-40 sphere path of 500 cm along the wall at height 200 |

While running: every tick a r-40 sweep from 40 cm behind the origin 140 cm along the facing must find the
flagged surface, and a r-5 sweep from origin + 50 up, 120 cm along −N, must hit a flagged component; the
new normal may differ at most 35° in pitch and yaw per tick. A test wall: a vertical box face at least
6 m wide and 6 m high, flagged, object type WorldStatic, with an unflagged floor in front; for the
"run over the top" exit add a top surface flagged `SQEX_bEnableAutoRun` (slope < 50°) — the ledge probe is
a r-90 sphere dropped from 250 to 150 above the origin, 80 cm ahead.

**Kickable wall.** Any blocking component with `SQEX_bEnableWallKick` (default **on** for every mesh and
BSP component) whose normal pitch is within ±30°, reached by a r-5 sweep of 105 cm along −normal from 75 cm
above the origin; channel Pawn, responses WorldStatic / WorldDynamic / TresGimmick.

**Pole.** A `UTresPoleComponent` registered in the pole list (active: component+0x868 bit 0). Data: world
transform, `m_BaseOffset1`, `m_BaseOffset2` (the segment), `m_CheckRadius` (default 50), `m_AttachRadius`,
`m_AttachOffset`, `m_bEnableNearDetectOffset`, `m_bIsHorizontality`. It is caught when the probe point
(120 cm ahead and 90 cm above the actor origin) is within `100 + m_CheckRadius` of the segment while Sora
is in AirSlide / SuperSlide (on a world hit, and in their ticks). Examples [A]:

| Type | Blueprint | Properties |
|---|---|---|
| vertical (spin, `POLE_SPIN`) | `Blueprints/Gimmick/he/g_he_hasira/g_he_hasira_01`, component `TresPole1` | NearDetect true, AttachOffset (0,0,0), AttachRadius 90, Base1 (0,0,−100), Base2 (0.31,0,850), CheckRadius 100, RelativeLocation (0,0,250) |
| vertical, thin | `Blueprints/Gimmick/bt/g_bt_Light/g_bt_LightB_Pole` | NearDetect true, Base1 (0,0,−100), Base2 (0,0,100), CheckRadius 50 (default), AttachRadius 0, RelativeLocation (0,0,300) |
| horizontal (swing, `POLE_SWING`) | `Blueprints/Gimmick/tt/g_tt_PoleSwing/g_zz_PoleSwing` | AttachRadius 30, Base1 (0,−200,0), Base2 (0,200,0), `m_bIsHorizontality` true, NearDetect false, RelativeLocation (0,0,258.35), RelativeRotation roll −90 |

The host must also deliver the **world-hit event** (hit record of every blocking move hit whose travel
direction opposes the impact normal) to the current state inside the movement tick, and answer the sweeps
with all hits in order, including start-penetrating ones.

### Tests

Assume every ability equipped, ability flags 0x7FF, 60 Hz, flat floor, the test wall and poles above.

1. **Up-run from a run.** Run straight at the wall (stick 1.0 into the wall). Frame of contact: Run `OnHit`
   → WALLRUN queued; next `MainProc`: class 0, facing yaw = yaw(N) + 180, Flying, `WallRunF`. With the
   stick held, `m_bEnableChange` at clip time 0.1671 (11th clip advance) → loop on that tick; height gained
   by the entry ≈ 231 cm; then **1251 cm/s** upward (20.85 cm per tick). Releasing the stick: the run
   ends 12–13 ticks later (0.2 s) with `Velocity = actual velocity × 0.5` (≈ 626 cm/s up), mode Falling.
2. **Dead zone.** Same approach with the stick 40° off the wall direction: no state change. At 30°:
   up-run; at 60°: side run on that side (class 1 when the stick points to the left of the wall
   direction).
3. **Side run from a jump.** Jump, fall against the wall with the stick 70° off: Jump `OnHit` (needs
   `Velocity.Z < 0`) → class 1 / 2, airborne entry: the loop clip is played once with `m_bEnableChange`
   set; next tick the loop proper: **1251 cm/s** along the wall, level.
4. **Direction change.** In the up-run turn the stick 90° to the left for 0.1 s (6 ticks, ±1):
   `WallRunChangeFR` (0.2333 s, 170 cm), class 1, then the side loop. Stick opposite to the facing
   (≥ 155°) in a side run: `WallRunChangeL / R` at half scale (258 cm in 0.6333 s) and the run reverses;
   in the up-run: the state ends.
5. **Blocked.** Run up under a ceiling: on the first loop tick where the actor origin is less than 200 cm
   below it (line test from 40 cm off the wall), `WallPoint` sets `end` and the state ends on that tick.
   With an obstacle that only stops the motion: the state ends after 0.2 s below 30 % of the
   root-motion speed.
6. **Jump off.** Jump press in the loop: Jump type 2 (`Velocity.Z` = 2500 at its launch notify), yaw
   unchanged. Without `SUPERJUMP`: no effect.
7. **Wall kick from an air slide.** Air slide into a wall with `SQEX_bEnableWallKick`, stick neutral after
   the push: on the contact frame WALL_KICK begins and its first tick plays `WallKickF` if the slide
   direction is within 35° of the wall's inward normal (0.5 s, ends 80 cm off the wall facing away),
   otherwise `WallKickR / L` (0.3333 s, 286 cm along the facing, 112 cm sideways, 45° turn). Jump press
   during the clip: flow jump (type 2) at the first tick with clip time > 0.1. Guard press: Superslide at
   the clip end. Nothing: Falling at the clip end. Without `WALL_KICK` (97) equipped: no kick.
8. **Wall kick out of a wall run.** Guard press in the loop (needs `SUPERSLIDE`): WALL_KICK on the same
   tick (after the probe), then a Superslide at the end of the kick clip.
9. **Vertical pole.** Air slide past the pole so that the point 120 ahead / 90 up passes within
   `100 + m_CheckRadius`: POLE_TURN; `PoleTurn` 0.1667 s, then `PoleTurnLoop`, 360° per 0.3 s; after
   **1.2 s** of loop without input a Superslide (kind 2) starts by itself. Jump press: directed flow jump
   (stick yaw if the stick is active on that frame, else the grab yaw). Guard press: Superslide (kind 2)
   along the stick or the facing. The same pole is not caught again by that Superslide.
10. **Horizontal bar.** As 9 with `PoleSwing` / `PoleSwingLoop` (0.4 s); after **1.6 s** an automatic
    directed jump; guard press: Superslide kind 1.
11. **Ability gates.** Without `POLE_SPIN` (95) the vertical pole is ignored, without `POLE_SWING` (96)
    the bar; with the FreeFlow flag cleared neither pole nor wall kick nor Superslide work, the wall run
    still does; with the WallRun flag cleared (bit 8) no wall run.

### Still needs the running game

| # | Object | Offset | Type | Why |
|---|---|---|---|---|
| W1 | actor position; movement component | Velocity +0x214 vec3, +0x9A0 vec3, +0x9C4 vec3, +0xB30 u8, +0xB34 vec3, +0xC04 f32, +0xC08 f32 | per frame through an up-run and a side run | the flying move's use of the wall direction (0x14058E570) was not re-derived; expect 1251 cm/s |
| W2 | WallRun state | +0x98 u32 (bit 8), +0x78 i32, +0xA8 i32, +0xB0 / +0xB4 / +0xB8 f32 | per frame | writer of flag bit 8 (early loop start) not found; timer behaviour at tick granularity |
| W3 | root-scale pair | argument of the play call (state interface vt+0x18) | which element scales the vertical root motion | assumed second = Z from the up-run (1.0, 1.84) versus side (1.84, 1.0) |
| W4 | pawn | +0x644 f32, +0x648 f32 | once | launch speed and factor passed to the pole jump (slot 171); defaults not located |
| W5 | pole component | +0x8B8 u8 (bit 1), +0x868 u8 | once per pole | the unreflected "auto orientation" bit and the active bit |
| W6 | PoleTurn state | +0x98 rotator, +0xA4 vec3, +0xB8 vec3, +0xE4 rotator, +0xD0 f32 | per frame on both pole types | grab pose algebra of sub-step 0 (quaternion part read as C) |
| W7 | player data | +0x95B0 u16 | at load of a normal save and during Olympus | expect 0x7FF |
| W8 | WallKick state | +0x78 i32, +0xA4…+0xA8 bytes, actor rotation | per frame for the three kick directions | confirms the one-tick sub-step 2 |

---

## II.5 Rail Slide, Athletic Flow (airstep travel and arrival), Hopping

(strand `rail`; working notes in `build/sora_tmp/actions/p2_rail/`)

Second pass over sections 10.5, 10.6 and 11.4 of Part I. Conventions and tags as in Part I
([I] instruction level with address · [C] decompiled C only · [A] asset · [D] derived · [?] open).
`comiss a,b; ja` = a > b, `jae` = a ≥ b, `jb` = a < b, `jbe` = a ≤ b.
Reading aids of this strand (disassemblies, compact C, tools): `build/sora_tmp/actions/p2_rail/`.

Evidence level reached: **Rail Slide** — request path, gate, push, probe, attach, the per-frame motion
model, the main Tick path (sub-steps 0, 1, 2), turn-around, both ways off the rail (jump, guard), end of
rail and End are [I]; the on-rail ("relative") jump sub-machine, the camera and the stick-to-camera
correction of the turn test are [C]. **Hopping** — gate, probes, callers, constants of the Tick are [I];
the control flow of the Tick and End is [C] with the constants checked. **Athletic Flow** — see its
section (read by a sub-agent of this strand; its own evidence statement is kept).

---

#### Rail Slide (id 22)

##### R0. What a rail is [I, A]

A rail is a **chain of `ATresRailSlideActor` nodes** (vtable 0x146629500, constructor 0x140F8B5B0, super
`ASQEX_SplineActor` 0x1477CB568). A node has no geometry of its own; for every outgoing link it owns one
`USQEX_SplineComponent` (vtable 0x1477CBF20) — the **segment** to the next node:

- `ASQEX_SplineActor::Connections` (actor+0x3E8, count +0x3F0): array of `{SplineComponent*, ConnectTo*}`
  (0x10 bytes each); `LinksFrom` (actor+0x408, count +0x410): array of the nodes that connect *to* this
  one. The last node of a rail has no connection.
- A segment is a two-point cubic curve **in world space** (`SplineInfo`, `FInterpCurveVector`, both
  points `CIM_CurveUser`, `InVal` 0 and 1, `OutVal` = the two node locations, `ArriveTangent` /
  `LeaveTangent`), plus `SplineReparamTable` (distance → key, linear points; the last point's `InVal`
  is the segment length — getter comp vt+0xA38 0x14451B8C0 [I]) and `m_SplineReparamRateToDistance`,
  `m_qBaseQuat` / `m_qNextQuat` (the orientations of the two end nodes; the orientation along the segment
  is interpolated between them — comp vt+0xA58 0x14451A3E0 [C]).
- Segment virtuals used by the player code [I for the slots]: +0xA38 length · +0xA40 location at distance
  (0x1445199A0: reparam table → key → `SplineInfo` eval; **no component transform is applied**) ·
  +0xA48 direction at distance · +0xA50 enabled (comp+0x90C bit 1) · +0xA58 quaternion at distance
  (arg `bReverse` turns it around) · +0xA60 key (0..1) → distance · +0xA88 0x144518920 closest point of
  the spline to a segment (returns the squared distance, writes the key).
- Node virtuals [I, one-instruction getters unless noted]: +0x788 `m_bActive` (actor+0x430 bit 1) ·
  +0x7D0 segment leading to a given neighbour · +0x7D8(seg, bRev) the node at the other end of a segment
  (0x144517A50) · +0x800(dir, bRev) choose the next node: `bFixedNextConnect` / `bFixedPrevConnect` →
  the fixed index, otherwise the connection whose direction matches `dir` best (0x1445183A0 [C]) ·
  +0x860 `m_ID` · +0x878 `m_StartDir` · +0x888 `m_RailType` · +0x8A0…+0x8C8 action bits (below) ·
  +0x900 usable (alive && active, 0x140F923E0) · +0x908 / +0x918 / +0x920 / +0x928 speed (R4) ·
  +0x930…+0x968 slope acceleration (R4) · +0x970 `m_JumpLimitType` · +0x9A0 `m_DetectRangePlus`.
  Getters called with `bRev = 1` answer for the *reverse-side* node (0x140F95800 finds the `LinksFrom`
  node whose connection uses the segment).

Reflected properties of `ATresRailSlideActor` (offset; constructor default [I 0x140F8B5E6…0x140F8B6A4]):

| Property | Offset | Default | Read by |
|---|---|---|---|
| `m_fCameraRollRate` | +0x438 | 1.0 | camera |
| `m_RailType` (`ESQEX_RAIL_SLIDE_TYPE`) | +0x43C | 0 NORMAL | probe, push (R1, R2) |
| `m_LinkedActor` / `m_RevLinkedActor` | +0x440 / +0x448 | null | transfer (MI rails) |
| `m_StartDir` (`ESQEX_RAIL_SLIDE_DIR`: 0 BOTH, 1 FORWORD, 2 REVERSE) | +0x450 | 0 | probe |
| `m_DetectRangePlus` | +0x454 | 0 | probe (added to the range) |
| flags dword | +0x458 | 0 | bit 0 `m_bDisableReverse` · 1 `m_bDisableActAttack` · 2 `m_bDisableActJump` · 3 `m_bDisableActAirSlide` · 4 `m_bDisableActAthleticFlowMove` · 5 `m_bDisableActTransferOnRelativeJump` · 6 `m_bDisableSpawnRailSlideConnectTo` · 7 `m_bDestroyAfterThePassage` · 8 `m_bIsNarrow` · 9 `m_bIsDisableAction` |
| `m_JumpLimitType` (0 NONE, 1 RELATIVE) | +0x45C | 0 | R6 |
| `m_bSetSlideSpeed` / `m_bResetAccDec` | +0x460 bits 0 / 1 | false | R4 |
| `m_SetSlideSpeed` / `m_SetRevSlideSpeed` | +0x464 / +0x468 | 2000 / 2000 | R4 |
| `m_bSetSpeedAccDec` | +0x46C bit 0 | false | R4 |
| `m_SetSpeedRateMin` / `Max` | +0x470 / +0x474 | 1 / 1 | R4 |
| `m_SetSpeedAccRate` / `DecRate` / `RevAcc` / `RevDec` | +0x478…+0x484 | 3 / 0.5 / 3 / 0.5 | R4 |
| nav-mesh fields (`m_bGenerateNavData` …) | +0x488… | 0, spacing 500 | AI only |

Derived action bits (node vt, `test [node+0x458], mask` [I 0x140F963C0…0x140F96410]): no reverse = 0x301 ·
no attack = 0x302 · no jump = 0x304 · no air slide = 0x308 · no athletic flow = 0x310 · narrow = bit 8
(vt+0x890). So `m_bIsNarrow` and `m_bIsDisableAction` each switch off every action.

Rail types handled by the plain state: 0 NORMAL (and 8+). Type 1 ATTRACTION, 2 GIMMICK and 7 EX05 are
never returned by the probe; types 3–6 (EX01–EX04) push `TresPlayerState_RailSlideMI` (id 138) instead
[I 0x1414455A8]. Out of scope here: id 138, the shooter-form branches and everything keyed to
`m_RailType` 5 / 6 (transfer by the reaction key, the on-rail Superslide).

Example placed rail [A]: `Levels/bt_DLC/bt_07/umap/bt_07_rail_02.umap`: nodes `TresRailSlideActor18 → 19 →
3 → 22 → 9 → …`; node 18 at (−11092.7, −4405.6, 1880) yaw −154.7°, `m_DetectRangePlus` 150,
`m_bSetSlideSpeed`, `m_SetSlideSpeed` = `m_SetRevSlideSpeed` = 1500, `m_ID` 2, `Connections` =
[(`SQEX_SplineComponent_23`, node 19)]; node 19 at (−11832.9, −4668.0, 1903.4), 1800, range plus 100,
`LinksFrom` = [18]; node 22: 2250 / 2500. Segment 18→19: points (−11092.7, −4405.6, 1880) tangent
(−271.3, −128.1, 0) and (−11832.9, −4668.0, 1903.4) tangent (−1104.9, −211.9, 0); 10 reparam points.
The root component of a node is an `SQEX_SplineRootComponent`.

##### R1. Probe — pawn vt+0xEA0 = `ATresCharPawnBase` 0x1405A7740 [I]

`bool Probe(start*, end*, range, dir*, out*, bCheckType)`; `out` (0x30 bytes): +0 node, +8 segment,
+0x10 point on the spline (vec3), +0x1C rotator of the travel direction, +0x28 distance along the
segment in the travel direction, +0x2C reversed flag.

```
list = pawn.vt+0x8D8().vt+0x978()                 // world registry: weak pointers to every rail node
last = (cmc.vt+0xBC0(), cmc.vt+0xBC8())           // m_LastRailSlideActor / Component (work+0x38 / +0x40)
box  = AABB of the segment [start, end]
best = -1; near = 40000                                                  // 0x1405A787A
for node in list (valid weak pointer, node.vt+0x900()):
    if bCheckType: t = node.m_RailType
        t == 0: if node has the actor tag "BlizzardRail" (FName 0x1487E43C0) and its life span is set:
                    require remaining life >= 0.1 s                      // 0x1405A7A03, 0x140F967C0 [C]
        t == 1 or 2 or 7: skip node                                      // 0x1405A79DE..E9
    r = node.m_DetectRangePlus + range;  pad = max(r, 200)               // 0x1405A7A54
    for (seg, to) in node.Connections with to != null, seg valid, seg.vt+0xA50():
        if seg.Bounds (origin +0x27C, extent +0x288) does not overlap box grown by pad: continue
        d2 = seg.vt+0xA88(start, end, &key, ..)                          // squared distance spline <-> probe segment
        if d2 <= r*r and (best < 0 or d2 < best): best = d2; hit = (node, seg, key)      // 0x1405A7BD5..EA
        elif d2 < near: near = d2; nearNode = node; nearSeg = seg                        // 0x1405A7C03
if no hit:
    if nearNode != last.actor and nearSeg != last.comp: pawn.vt+0x22E8()   // forget the last rail
    return false
len = seg.length; dist = seg.vt+0xA60(key); v = seg.vt+0xA48(dist)
switch node.m_StartDir:  1 FORWORD: keep;  2 REVERSE: v = -v, rev = 1
                         0 BOTH: if dot(v, dir) < 0: v = -v, rev = 1      // comiss dot,0; jae keep 0x1405A7D58
pos = seg.vt+0xA40(dist)
if rev: same = (dist <= 0 and (node == last.actor or seg == last.comp))
        node = the ConnectTo node of that connection (must be a TresRailSlideActor); dist = len - dist
dist = clamp(dist, 0, len)
if node == last.actor or seg == last.comp or same: return false           // 0x1405A7E9F..EB3
if dist >= len - 150:                                                     // 0x1405A7EB9 (comiss dist,len-150; jb skip)
    forward: next = node.vt+0x7D8(seg, 0) must exist and have >= 1 connection
    reverse: the original node must have >= 1 LinksFrom
out = (node, seg, pos, Rotator(v), dist, rev)
```

Consequences: the rail must pass within `range + m_DetectRangePlus` of the probe segment; within 150 cm
of the end of a rail (in the travel direction) it is only taken when the rail continues; the rail just
left is refused until a probe finds no rail within its reach whose nearest out-of-range candidate
(< 200 cm) is that rail — in practice until Sora is more than 200 cm away from it.

Callers [I]. Start = actor location (capsule centre), end = start + (0, 0, 30) in all of them:

| Caller | Call | Range | Direction |
|---|---|---|---|
| Idle tick 0x1413944D0 | 0x141394825 (30 @0x1413946F2) | pawn vt+0x1AF0 = pawn+0xBF8 `MyRailSlideCheckRange` **50** | actor forward |
| Run tick 0x1413AED70 | 0x1413AF427 (30 @0x1413AF2B7) | 50 | stick yaw (Run+0x8C) when Run+0x88, else actor forward |
| Jump tick 0x141395450 | 0x1413957F6 (30 @0x14139567B) | pawn vt+0x1AF8 = pawn+0xBFC `MyRailSlideCheckAirRange` **150** | stick yaw (Jump+0x8C) when Jump+0x88, else actor forward |
| SlopeSlide 0x1413D1EA0 @0x1413D210C, Swaying 0x1413D8110 @0x1413D8375 | | 50 | as Run |
| Rail state, sub-steps 7 and jump phase (R6) | 0x141511388, 0x1415080FD | 150 / pawn vt+0x1AF0 | actor forward |

Every caller then calls `TryRailSlide(out, bKeep = 1, bSkipStart = 0)` (iface +0x258) and, on success,
`DoChain` (vt+0x270). Native defaults 50 / 150 [I 0x1414238CC / 0x1414238D6], no blueprint override [A].

##### R2. Request path [I]

```
Gate (slot 74, 0x14142C800):
    pawn.vt+0x29C0() (CanAct)                                             // 0x14142C81A
    player data (pawn.vt+0x8C8()) byte +0x95B0 has bit 7 (cmp byte,0; jge fail)   // flowmotion unlock 0x14142C83D
    !pawn.vt+0x1C30() (not in water)
    id = current state id: 0 <= id <= 3 (none, IDLE, RUN, JUMP)           // cmp eax,3; jle ok 0x14142C86E
        or id == 0x8E (COVER) with its own conditions; anything else fails
    pawn.vt+0x2788() == 0 (restriction bit 17)
TryRailSlide(out, bKeep, bSkipStart) (slot 75, 0x14143F4E0):
    if !Gate: return false
    if !bKeep: 0x1405A8010 (ClearQueue)
    if out.node and out.node.m_RailType == 1: return false                // 0x14143F53B
    return Push(out, bSkipStart) != null
Push (slot 76, 0x141445570):
    t = out.node ? out.node.m_RailType : 0
    st = PushState((t - 3) <= 3 unsigned ? 0x8A : 0x16)                    // 0x1414455A8..B2 / 0x141445611
    st+0x90 = out.node (m_SplineActor); st+0x98 = out.seg (m_SplineComponent)
    st+0xA0 = out.pos; st+0xAC = out.rot; st+0xC4 = out.dist
    st+0xC8 = (st+0xC8 & ~3) | out.rev | (bSkipStart << 1);  st+0x170 = t
```

Because of the state test, **the SlopeSlide and Swaying probes can never start a rail slide** (their
state ids 10 / 11 fail the gate); the working entries are Idle, Run and Jump.

##### R3. State fields

`UTresCharState_RailSlide` (size 0x190) / `UTresPlayerState_RailSlide` (0x1A0):

| Offset | Meaning |
|---|---|
| +0x88 (u8), +0x8C (f32) | stick active this frame, stick yaw — written by `MoveInput` (slot 35 0x141440600 [C]) |
| +0x90 / +0x98 | `m_SplineActor`, `m_SplineComponent` (from the probe) |
| +0xA0, +0xAC, +0xC4 | attach point, attach rotator, distance along the segment |
| +0xB8 / +0xBC / +0xC0 | blend-space X (curve), Y (pitch), Z (0) |
| +0xC8 | flags: 1 reversed · 2 skip the start clip · 4 waiting for a one-shot clip · 8 turn-around armed · 0x10 shooter variant · 0x20 narrow · 0x40 no reverse · 0x80 no attack · 0x100 no jump · 0x200 no air slide · 0x400 no athletic flow · 0x800 camera turn flag pending · 0x1000 rail is `RELATIVE` · 0x2000 transfer probe active (jump phase) · 0x4000 stick must be released before the next turn |
| +0xCC | turn-around lock-out timer |
| +0xD0 / +0xD4 / +0xD8 | on-rail jump: type, phase, flags |
| +0xDC, +0xE4, +0xE8, +0xEC, +0xF0 | MI-only sub-machines (damage, on-rail Superslide) |
| +0x114…+0x154 | attack / shot / magic sub-machines (forms; out of scope) |
| +0x160 | effect group passed to every play: 0xC81 [I 0x1413AD0C9] |
| +0x170 (u8) | `m_RailType` of the start node |
| +0x174 (vec3), +0x180 (quat) | actor location / rotation at the end of the last tick (used by the obstacle sweep) |
| +0x190 | `m_pCamera` (`ATresCameraRailSlide`) |

**Reset** 0x1413AD0B0 → 0x14150E0C0 [C; +0x160 I]: clears all of the above; `+0xC8 = (+0xC8 & ~0x3FFF) |
0x4000`; +0x150 = 10; +0x160 = 0xC81.

Movement side: `FTresRailSlideWork` at movement component +0xA40 (`m_RailSlideWork`):

| Offset | Meaning |
|---|---|
| +0x00 bit 0 | attached |
| +0x08 / +0x10 | `m_LeadActor` and its work (followers of a leader; not used by Sora) |
| +0x1C | bit 0 paused · bit 1 reversed · bit 2 slope acceleration on · bits 4 / 5 pause-point check · bit 6 speed lock |
| +0x20 | 0 on the rail · 1 on-rail jump · 2 settling after a landing |
| +0x24 | bit 0 type > 2 · bit 1 stop at the end instead of leaving · bit 2 end reached · bit 3 reversed (copy) · bit 4 offset +0xC4 active |
| +0x28 / +0x30 | `m_RailSlideActor`, `m_RailSlideComponent` (current node, segment) |
| +0x38 / +0x40 | `m_LastRailSlideActor`, `m_LastRailSlideComponent` (kept after leaving; cleared by 0x1405C98F0) |
| +0x5C / +0x60 / +0x64 | base speed, node speed offset, current speed |
| +0x68 | speed rate (1.0 after a detach) |
| +0x74 | fraction along the segment |
| +0x78 / +0x7C, +0x80 / +0x84 | slope rate min / max, acceleration / deceleration rate |
| +0x88 / +0x8C / +0x90 / +0x94 | distance on the segment (travel direction), distance moved this tick, segment length, total |
| +0xA0 | gravity direction (cmc vt+0xC18) |
| +0xAC / +0xB8 | jump offset from the rail point (vec3), its velocity (vec3) |
| +0xD0 | rail type |

Pawn forwarders [I]: vt+0x21C0 attached (work bit 0) · +0x21C8 `work+0x20 == 1` · +0x21D0 `work+0x20 != 0`
· +0x21E0 attach (0x141448920 → cmc vt+0xA98 → 0x1405D94A0) · +0x21E8 detach (0x141432A40 → 0x1405CA7E0)
· +0x21F0 reverse (0x1405D79E0) · +0x21F8(b) pause (0x1405D3280) · +0x2218(d, pos, rot) look ahead
(0x1405D8E90) · +0x2220 current speed (0 while paused, 0x1405CF090) · +0x2230(v) set base speed · +0x2238(r)
set rate · +0x2248(b) speed lock · +0x2258 narrow · +0x2260…+0x2280 no reverse / attack / jump / air slide /
athletic flow · +0x2298 jump limit type · +0x22F0 current node · +0x2320(b) stop-at-end bit · +0x2328 end
reached (work+0x24 bit 2; true when not attached) · +0x1B00 / +0x1B08 / +0x1B10 = pawn+0x640 / +0x644 /
+0x648.

Tunables on the pawn [I ctor `ATresCharPawnBase` 0x14059F950; A]: `MyRailSlideSpeed` pawn+0x640 = **2000**
(0x14059FF60, no blueprint override); `m_RailSlideJumpInertialVelocity` +0x644 = 200 native (0x14059FF6A)
→ **175** in `p_ex001_PawnBase`; `m_RailSlideJumpInertialBrake` +0x648 = **0.92** (0x14059FF74).

##### R4. Motion model [I]

`StartNewPhysics` 0x140596E10: when `m_RailSlideWork` is attached (0x140596ED4…EE2) the component calls
**`PhysRailSlide`** = component slot 408 (vt+0xCC0) 0x14058ECF0 instead of the movement-mode dispatch —
the `MovementMode` value (5, set by Begin) is not what moves Sora.

```
PhysRailSlide(dt):                                               // 0x14058ECF0
    work.gravityDir = cmc.vt+0xC18()
    Step(work, dt, &pos, &rot)                                   // 0x1405DAA90
    if attached and no lead actor: rot = Slerp(current rotation, rot, 0.5)   // FUN_142D90CA0(.., 0.5), normalised
    work+0x20 == 1: jump offset integration (R6);  == 2: settle (offset * remaining / duration)
    if !attached or work+0x20 == 0: clear the jump offset (0x1405C9900)
    MoveUpdatedComponent(pos - current, rot, bSweep = false)     // vt+0x450(.., 0, 0, 0): no collision
Step: work.moved = 0; if no node: detach
      Advance(work, work.speed * work.rate * dt, dt, &pos, &rot)             // 0x1405DC650
Advance(d):
    next = node.vt+0x7D8(seg, rev); len = seg.length
    if next and seg:
        if !paused: dist += d; moved += d                        // work+0x1C bit 0 tested here
        frac = clamp((rev ? len - dist : dist) / len, 0, 1)
        if dist >= len:                                          // comiss dist,len; jb stay
            if stop-at-end bit: n2 = next.vt+0x800(Vector(rot), rev); s2 = next.vt+0x7D0(n2, rev)
                                if !n2 or !s2: end-reached bit = 1; dist = len; (stay on this segment)
            else / if a continuation exists:
                over = dist - len; last = (node, seg); node = next
                seg = node.vt+0x7D0(node.vt+0x800(Vector(rot), rev), rev); dist = 0
                return Advance(over)                             // a node without continuation: falls to "leave"
        pos = seg.vt+0xA40(rev ? len - dist : dist)
        rot = seg.vt+0xA58(rev ? len - dist : dist, rev)
        speed update (below); last = (node, seg)
    else (leave):  pos / rot = end of the last segment; attached = 0; node = seg = null
```

There is no sweep and no floor test: on a rail Sora ignores level collision, and his **actor location
(capsule centre) is put exactly on the spline** — no code of the player path sets the offset work+0xC4
(setter cmc vt+0xBD8 0x1405D7740 has no caller in the player, state or pawn code) [I]. The slide is
frame-rate independent in distance (speed × dt); only the 0.5 rotation slerp is per tick.

Speed [I 0x1405D94A0, 0x140F91A00, 0x1405DC650]:

```
base = MyRailSlideSpeed (2000), written at the attach (work+0x5C)
node speed: if node.m_bSetSlideSpeed:
    forward: v0 = node.m_SetSlideSpeed; v1 = (next node has m_bSetSlideSpeed) ? next.m_SetSlideSpeed : base
             target = v0 + (v1 - v0) * clamp(dist / len, 0, 1)
    reverse: the same with m_SetRevSlideSpeed and (1 - dist / len)
    work.offset(+0x60) = target - base, re-evaluated every step; work.speed follows the change
else: speed = base
slope (only if node.m_bSetSpeedAccDec, work+0x1C bit 2; rates from the node, interpolated like the speed):
    g = dot(travel direction, gravityDir)
    V = base + offset
    speed = clamp(speed + (RateMax - RateMin) * V * g * dt * (g >= 0 ? AccRate : DecRate), RateMin * V, RateMax * V)
distance per tick = speed * work.rate(+0x68) * dt
```

A rail placed with constructor defaults therefore moves Sora at a constant **2000 cm/s** (33.33 cm per
60 Hz tick) in both directions; the example rail of R0 runs at 1500 → 1800 → … cm/s, interpolated node
to node.

##### R5. Begin, Tick, End [I unless marked]

**Begin** 0x1413A4650 → 0x141509CA0: pawn bits 0, 3, 4 of +0xE48 set (vt+0x1E58 / +0x1E88 / +0x1E98 with
1: second jump, flow jump, Superslide available) · `pawn.vt+0xDC8(1.0)` (pitch / roll of the actor
restored at once) · `SetMovementMode(5, 1)` (vt+0x17A0) · mesh flag (0x1405B9420(pawn, 1, 1): MyMesh+0x1270
bit 1 [? meaning]) · state slot 146 (weapon: `pawn.vt+0x1388(1, 0, 0)` unless `pawn.vt+0x1380()`) · +0x174 /
+0x180 = actor location / rotation · slot 152 0x1413A3E20 spawns `ATresCameraRailSlide` and binds it [C].
The player override then clears a pending focus / MP-charge state of the controller (vt+0x1828 / +0x1830 /
+0x1820) [C].

**Tick** 0x141510EE0 (jump table 0x141511BE4: 0 → 0x141511412, 1 → 0x1415114DE, 2 / 4 → 0x14151157B,
3 → 0x14151105A, 5 → 0x1415111A0, 6 → 0x141510F93, 7 → 0x1415111DD, 8 → 0x14151114A). The tick does **not**
call the common pitch / roll restore: the actor keeps the rail's orientation.

```
sub 0 (attach):
    pawn.vt+0xDB0(rot(+0xAC), alpha 1.0)                         // snap the facing to the rail  0x14151143B
    ok = pawn.vt+0x21E0(&pos(+0xA0), &rot(+0xAC), node, seg, dist(+0xC4), flags & 1, MyRailSlideSpeed)   // 0x141511496
    if !ok: DoChain; return
    slot 129 (0x141514E90)(bSkip = flags bit 1):
        pawn.vt+0x21F8(1) (pause); StopMovement
        if !bSkip: Play(slot 147 name "RailSlideStart", blendIn -1, mode 0, group +0x160, rate 1.0); flags |= 4
        else flags &= ~4
    slot 154 (camera blend in {0.5, 3, 2.0, 2}) [C]
    sub = 1
sub 1 (one-shot clip: start or turn):
    if flags & 4: if IsAnimEnd: flags &= ~4;  if still flags & 4: goto TAIL
    flags bit 5 = pawn.vt+0x2258() (narrow)
    slot 130 StartLoop(slot 148 name: narrow ? "RailSlideNarrowLoop" : "RailSlideLoop", blendIn -1)      // 0x14151155B
    sub = 2; fall into sub 2 in the same tick
sub 2 (slide):
    if !pawn.vt+0x21C0() or pawn.vt+0x2328() or slot 124(2) (obstacle sweep, R7): goto LEAVE
    slot 128 (camera roll) [C]
    if slot 137() (always false for the player) or slot 141() (shot / magic sub-machine, forms): goto TAIL
    flags bits 6..10 = pawn.vt+0x2260 / 2268 / 2270 / 2278 / 2280 (no reverse / attack / jump / air slide / athletic flow)
    slot 156: HUD prompts (jump, attack, air slide availability) [C]
    flags bit 12 = (pawn.vt+0x2298() == 1)
    n = pawn.vt+0x2258(); if n != flags bit 5: flags bit 5 = n
        Play(loop name for n, blendIn 0.16666667 (0x141511769), mode 3, group +0x160); set blend (+0xB8, +0xBC, +0xC0)
    slot 132 lean (below)
    if slot 123() (reaction key pressed and a transfer target exists; needs m_RailType 6): goto TAIL
    turn-around (below)
    +0x88 = 0
TAIL: +0x174 / +0x180 = actor location / rotation; +0x11C &= ~1
LEAVE (0x141511AE6):
    slot 128; slot 151 (attraction hand-over, controller mode 0x54 only) [C]
    if NextQueued() <= 0                                                   // jg skip 0x141511B10
       and pawn.vt+0xE70() (actor up vector).z >= 0                        // comiss z,0; jb skip 0x141511B31
       and -(pawn.vt+0xE60() (actor forward).z) < 0.86:                    // comiss; jae skip 0x141511B56
        slot 145 (0x141513460)(pawn, &forward, pawn+0x644, pawn+0x648)     // automatic launch, R6
    DoFinish (vt+0x268)
```

`StartLoop(name, blendIn)` = slot 130 0x141514AD0: `StopMovement`; refresh flags bits 5…10 and 12 from the
node; `Play(name, blendIn < 0 ? -1 : blendIn, mode 3, group, rate 1.0)`; blend values zeroed;
`pawn.vt+0x21F8(0)` (resume); `pawn.vt+0x2320(0)`; if !(flags bit 14): +0x88 = 0; **+0xCC = 0.33333334**
(0x141514D27); flags &= ~8; if flags bit 11: slot 155(1), clear bit 11.

So an attach costs the start clip: the slide is **paused for `RailSlideStart` (0.2333 s)**, snapped to the
spline point (first movement tick after the attach), then moves.

Lean, slot 132 0x141516600 [I for constants, C for the vector algebra]:

```
R0 = actor rotation * (0,1,0)
(posA, rotA) = pawn.vt+0x2218(200.0)                 // pose 200 cm further along the rail, 0x14151679A; crosses nodes (0x1405D8EE0)
R1 = rotA * (0,1,0); U1 = rotA * (0,0,1)
a = acos(clamp(dot(R1, R0), -1, 1)) * 57.29578       // 0x141516840
if a > 0 and dot(U1, normalize(cross(R1, R0))) > 0: a = -a          // right curve positive, left negative [D]
+0xB8 = a;  +0xBC = wrap180(pitch of the actor forward vector)
ANIM.vt+0x120(+0xB8, +0xBC, +0xC0)
```

Turn-around [I 0x141511802…0x1415119FD]:

```
if (flags & 8) and !(flags & 0x60):                  // armed, not narrow, reverse allowed
    if +0x88 (stick this frame):
        if !(flags bit 14):
            F = Vector(actor rotation); S = Vector((0, +0x8C, 0))
            slot 126 (0x1413AC1A0)(pawn, &F, &S)     // player: unless dot(F,S) < -0.5 already, re-expresses both in the
                                                     // camera frame (raw stick x / y, camera matrix); stick^2 < 0.01 -> S = 0 [C]
            if dot(F, S) < -0.5                      // comiss dot,-0.5; jae skip 0x14151193A
               and pawn.vt+0x21F0(&rot, 1):          // reverse on the same segment, R5a
                slot 155(0); flags |= 0x800
                SetActorRotation(rot)                // 0x143EAD160, the reversed rail orientation
                slot 131 (0x141514D70): pause; StopMovement; Play("RailSlideTurn", -1, mode 2); flags |= 4
                flags |= 0x4000; sub = 1
    if !+0x88: flags &= ~0x4000
else:
    if +0xCC > 0: +0xCC -= dt; if +0xCC <= 0: +0xCC = 0; flags |= 8      // 0x1415119C3..ED
    if !+0x88: flags &= ~0x4000
```

R5a. Reverse 0x1405D79E0 [C, comparisons I]: forward → reverse needs `!node.vt+0x8A0(0, 0)` and the far node
of the segment; then work bit 1 set, node = far node, `dist = max(len - dist, 0)`, fraction and node speed
recomputed, speed reset to base + offset. Reverse → forward symmetrically (the `LinksFrom` node that owns the
segment must allow it). The returned rotator is the segment orientation at the new distance, reversed.

Timeline of a turn: stick pushed against the travel direction (more than 120° off) → same tick: paused,
facing snapped, `RailSlideTurn` (0.3 s) → loop restarts, 0.3333 s lock-out, and the stick has to go
neutral for one tick before another turn is accepted. At the first attach bit 14 is set by Reset, so a
stick held while landing on the rail does not turn Sora around.

**End** 0x1413AAF40 → 0x14150C1F0 [I for the calls]: slot 127; slot 140; `pawn.vt+0x21E8()` (detach
0x1405CA7E0: `m_bDestroyAfterThePassage` nodes are destroyed, last node / segment kept, speed rate = 1.0);
`pawn.vt+0x1388(0, 0, 0)`; `ResetGravityScale` (vt+0x1658); mesh flag off; if the component `IsFlying`
(cmc vt+0x548): cmc vt+0x758 (`SetDefaultMovementMode`); slot 153 (camera end {0.5, 3, 2.0, 2}) [C].
No cooldown is written.

##### R6. Ways out [I for the dispatch, C for bodies]

| Input / event | Path | Result |
|---|---|---|
| **Jump** | jump gate 0x14142A3D0, state 22: actor up.z ≥ 0 and state slot 113 0x141507BC0 (`sub == 2 && !(flags & 0x120)`: not narrow, jump allowed) → `JumpRequest` 0x14143D8F0: if slot 114 0x1413A3D00 (`flags & 0x1000 && sub == 2`: rail is `RELATIVE`) → on-rail jump (below); else `ClearQueue`; `RequestJump(yaw, type 2, bFall 0, bAir 1)`; Jump+0x114 \|= 1; Jump+0x100 = actor forward, XY normalised, z = 0; Jump+0x10C = pawn+0x644 (175); Jump+0x110 = pawn+0x648 (0.92) | rail state ends at the next `MainProc` (queue[0] ≠ current); Jump type 2 → 3 without `SUPERJUMP` but with `DOUBLEFLIGHT`, → 0 with neither (resolver 0x141434010) |
| **Guard button** | `TryFlow(1, 0)` 0x1414417B0, state 22: gate 0x14142E320: flowmotion unlocked, pawn type 1, `CanAct`, not in water, `HasAbility(SUPERSLIDE 19)`, state slot 116 0x141507AF0 (`!(flags & 0x220) && sub ∈ {2, 3}`) → `ClearQueue`; push Superslide (iface +0x248, kind 1) | plain Superslide (state 21) from the rail position, aim = its own rule (Part I 8). The on-rail Superslide (slot 121 / 136, `SuperSlideLoop` then `SlideEnd`) belongs to id 138 |
| Guard + stick (air slide) | air-slide gate 0x141425780, RAIL: state slot 116 | Air Slide off the rail (Part I 6) [C] |
| **End of rail** | LEAVE branch of the Tick | slot 145 → `RequestJump(actor yaw (vt+0xDA0), type 2, bFall 0, bAir 1, b6 0)` (0x1415135EC) with the same direction / 175 / 0.92 block; then `DoFinish`. If a state is queued, the actor is upside down or pointing down steeper than asin 0.86 (59.3°): no launch, `DoFinish` only → Idle → `Fall` |
| Obstacle (Blizzard rails only) | slot 124(2) | as end of rail (R7) |
| Athletic flow (airstep) | slot 118 0x141507B10 (`sub == 2 && !(flags & 0x420)`), asked by 0x141436B20 | see Athletic Flow |
| Water / damage / cinematic | queue (`SetState`) | End |

On-rail jump (rails with `m_JumpLimitType = RELATIVE`) [C; constants I]: `JumpRequest` resolves the power
with pawn vt+0x2A18(2, 0, …) and calls state slot 120 0x1413AA0A0 → 0x14150AAC0: `pawn.vt+0x22A8(power)`
(work+0x20 = 1, work+0xC0 = power = vertical speed of the offset, component +0x304 = power), stop-at-end
bit on, sub = 3, +0xD0 = type, +0xD4 = 0. Sub 3 (slot 135 0x14150AB80) plays the three clips of pawn
vt+0x1C88(type, phase) (start mode 2 → rise loop mode 3 → fall loop when the offset speed ≤ 0), keeps
advancing along the rail (`PhysRailSlide` integrates the offset with the component's gravity:
`offset += (v_old + v_new) / 2 × dt`), and returns to sub 2 with `StartLoop(name, 0.1)` (0x141511129) when
the offset is back to 0 (`work+0x20 == 0`). During the jump slot 133 0x141507EA0 probes for another rail
from 50 cm above the pawn toward the rail point (range pawn vt+0x1AF0 [? slot identity]) when flag 0x2000
is set. Type-2 (super) jumps clear the second-jump and flow-jump bits. Not needed for a rail with default
properties.

##### R7. Obstacle sweep, slot 124 0x1413A8E00 [C]

In sub-step 2 (argument 2) it runs only when the current node has the actor tag **`BlizzardRail`** (the ice
rail made by Blizzard magic): a sphere sweep of radius 50 from local (0, 0, 120) to (60, 0, 120) of the
actor (query parameters of `MyRoot`, world sweep 0x143FDC0C0); a hit pauses the slide and takes the LEAVE
branch. Level rails are never swept.

##### R8. Clips [A]

| Name | Asset | Length | Play | Notifies / notes |
|---|---|---|---|---|
| `RailSlideStart` | 1158 | 0.2333 s (8 frames) | −1, mode 0, group 0xC81 | SE `se00001_015` (pitch 1.2) at 0; `m_InterpolationTime` 0.03; no root motion |
| `RailSlideLoop` | blend space 2-D | X −30…30 (curve), Y −30…30 (pitch), interpolation 0.4 s, `m_InterpolationTime` 0.667 | −1 / 0.1667 / 0.1, mode 3 | samples 1159 (centre, 1.2667 s, auto-SE surface record `railslide`, fade-out 0.3), 1159L / R / F / B / FL / FR / BL / BR (0.4 s each) |
| `RailSlideNarrowLoop` | blend space 1-D | X −30…30 | same | 1160 (0.4667 s, auto-SE `railslide`), 1160L, 1160R |
| `RailSlideTurn` | 1180 | 0.3 s (10 frames) | −1, mode 2 | SE `se00001_015` at 0; no root motion |
| `RailSlideTransfer` | 1164 | 1.0 s | MI only | SE `se00001_015` |
| `SlideEnd` | 1157 | 0.7667 s, root 426 cm | MI only (on-rail Superslide) | |

No `TresEnableChange` / `TresEnableInput` / `TresTiming` in any of them: the state is driven by clip ends
and the movement component only.

---

#### Athletic Flow (id 42) — the airstep travel / arrival state

`TresPlayerState_AthleticFlow`, vtable 0x146901158, constructor 0x1413308C0 [I]: parent constructor 0x141506780 =
abstract `TresCharState_SuperSlide` (vtable 0x146A11238, id 0x15) — **not** the player Superslide class 0x14694BC58;
`MyStateID` = 0x2A; state name `AthleticFlow`; anim interface (state+0x40) vtable 0x146901560; a second interface at
state+0x48, vtable 0x1469019A8 (arrival / enemy call-backs). Reset, Begin, Tick and End are overridden and do not call
the Superslide ones (Reset calls only `TresStateBase` reset 0x14150DF80: flags &= 0xFFFFF810, sub-step 0) [I].
Anim-interface handlers [I, `ivt.js`]: `TresEnableChange` → default 0x1411FBA40 (`m_bEnableChange` = 1),
`TresEnableInput` → default 0x1415124D0 (`m_bEnableInput` = 1), `TresTiming` / `TresLastAnimPose` / `TresLandAnimPose`
→ empty 0x14041F120. Slot 73 (0x141333490) is a debug canvas print (speed, distance, target list) — no behaviour [I].

Tags: [I] instruction level · [C] decompiled C only · [A] asset · [D] derived · [U] stock UE 4.17 on function shape ·
[?] not determined.

##### Fields

| Offset | Type | Meaning |
|---|---|---|
| +0x8C | f32 | actor yaw at launch of a leg [I 0x141349A8F] |
| +0x90 | f32 | leg time left; loaded with `MyAthleticSlideTime` at each leg start [I 0x141349AA4] |
| +0x98 | i32 | pose variant of the leg: 1 level, 2 downward, 3 upward |
| +0x9C / +0xA0 | i32 | number of upward / downward legs so far (parity selects the clip) |
| +0xA8 / +0xAC | i32 | counters (kick / enemy end; enemy turn) — only incremented |
| +0xB0 | u8 | 1 while a non-travel mode runs |
| +0xB1 | u8 | jump-press latch, written by `JumpRequest` 0x14143E323, cleared at the end of every Tick 0x141341313 |
| +0xB8 | ptr | → controller target list (controller+0xB18: data pointer, count at +8) |
| +0xC0 / +0xC8 | ptr | current / next list entry (0x34 bytes each; next = null on the last) |
| +0xD0, +0xD4, +0xD8 | i32 | mode (0, 1, 2, 4), index of the current entry (starts −1), list count read at Begin |
| +0xDC | u8 | `m_MoveFlowType` of the current target (entry+4) |
| +0xEC, +0xF8 | vec3 | actor location and reference point at Begin |
| +0x104, +0x110 | vec3 | fall-back actor location / reference point (re-captured when Sora attaches to an enemy or pole) |
| +0x11C, +0x11D | u8 | "attached, must be put back" flags used by End |
| +0x120 | ptr | `m_pTargetComponent` (reflected) |
| +0x128 | u8 | attack latch (slot 113) |
| +0x135 | u8 | guard latch (slot 114; no reachable writer, see Follow-ups) |
| +0x12C, +0x130 | f32 | hit slow-motion rate and time: Reset 0.5 / 0.1 [I 0x14133D0D2, DC], Begin overwrites with `MyAthleticAttackSlowRate` / `MyAthleticAttackSlowTime` (pawn+0xD34 / +0xD38) [I 0x1413331E4..F6] |
| +0x138 | u32 | flags: 1 attack prompt shown · 2 target is an enemy type (1–3) · 4 debug text · 0x10 attack accepted · 0x20 first-stop flourish still available (set by Reset) · 0x40 current target is the last · 0x80 hit slow motion active |
| +0x13C | f32 | "not moving" timer |
| +0x140 | vec3 | target point of the leg |
| +0x158, +0x160, +0x180 | ptr | `m_pHitActor`, `m_pHitComponent`, `m_pPoleComponent` (reflected) |
| +0x168 | vec3 | unit direction used by the stop (kick facing, enemy side) |
| +0x174 | vec3 | reference point at the moment of arrival |
| +0x1B4, +0x1C0, +0x1C4, +0x1C8 | vec3, i32, f32, f32 | attach offset, loop count, 0.5 s attach blend, loop timer (pole / enemy-turn stops) |
| +0x1D0, +0x1D8 | ptr | `m_pTargetEnemy`, `m_pEnemyTurn` (reflected) |
| +0x1E0 | delegate | per-mode update, bound by slot 115 |

"Reference point" = pawn vt+0x2110 (0x141448550): for state 42 the default case (byte table 0x1414486A0[42−27] = 1) =
actor location + (0, 0, 100) [I 0x14144866C]. "Target point" = slot 123 (below).

##### Tunables (all `TresPlayerPawnBase`, native constructor 0x141423210, **no blueprint override** in
`p_ex001_PawnBase` / `p_ex001_Pawn` [A])

| Property | Pawn offset | Value | Constructor store | Reader |
|---|---|---|---|---|
| `MyAthleticSlideTime` | +0xD28 | 1.0 s | 0x141423912 | pawn vt+0x1B98 (0x141108E90) ← travel 0x141349A9E |
| `MyAthleticSlideSpeed` | +0xD2C | 6000 cm/s | 0x14142391C | pawn vt+0x1BA0 (0x141108E80) ← travel 0x141348D01 |
| `MyAthleticSlideKickAnim` | +0xD30 | 0.13333 | 0x141423926 | pawn slot 1314 (0x141108E70); **not read by this state**; only caller found: interface slot 207 0x14142E990 @0x14142EA52 [I scan] |
| `MyAthleticAttackSlowRate` | +0xD34 | 0.25 | 0x141423930 | Begin 0x1413331E4 → state+0x12C |
| `MyAthleticAttackSlowTime` | +0xD38 | 0.05 s | 0x14142393A | Begin 0x1413331F0 → state+0x130 |
| `MyAthleticLockonLimit` | +0xC9C | 15000 cm | 0x141423E20 | pawn vt+0x29A8 → controller+0x984 (0x141410E50) |
| `MyAthleticLockonRange` | +0xCA0 | 3750 cm | 0x141423E2A | pawn vt+0x29B0 → controller+0x988 (0x141410F00) |
| `MyFocusPointNeedAF` | +0xD10 | 10 | 0x141423D5C | no reader on this path [I scan of the state / controller ranges] |

##### Request path

```
controller aim mode 0x1414165E0 (magic spec S2.3), airstep key with >= 1 airstep target:
    call [iface+0x308] at 0x141418F5C (normal aim) and 0x14141773E (focus-syphon aim)         [I]
slot 97 0x1414372F0:  if !gate(): return false;  SetState(0x2A) (0x1405B9FE0);  return true    [I 0x1414372FC..14]
gate slot 96 0x141426280 [I]:  pawn vt+0x29C0() (CanAct)                 0x14142629A  je fail
                               !(pawn+0xE62 bit 0)  (vt+0x2770)           0x1414262AA
                               !vt+0x2540()  (state or queued state == 59 DIE, or 0x1405AD270)  0x1414262BA
                               pawn type byte (pawn+0x5F0, vt+0xD70) == 1 0x1414262D0
                               !(pawn+0xE60 bit 17) (vt+0x2788)           0x1414262DA
                               current state id != 6 and != 0x28          0x1414262EC / F1
                               controller != null && controller+0xB20 > 0 0x141426307  jle fail
slot 193 0x1414446B0: PushState(0x2A) (0x1405B4F80), returns the state object — no caller found in sheets/exe/calls
                      (indirect only) [?]
```

##### Reset 0x14133CFB0 [I]

Zeroes +0x88..+0xDC, +0x120, +0x128, +0x134, +0x135, +0x13C, +0x158, +0x160, +0x180, +0x1A4, +0x1C8, +0x1D0, +0x1D8,
+0x11C, +0x11D; vectors +0xE0, +0xEC, +0xF8, +0x104, +0x110, +0x140, +0x14C, +0x168, +0x174, +0x194, +0x1B4 = zero
vector; +0x188, +0x1A8 = global 0x149B5C7A8; +0x12C = 0.5; +0x130 = 0.1; +0x1C0 = 2; flags +0x138 =
(old & ~0xC0 | 0x20) & ~7 & ~0x18 → **0x20 set**, 1 / 2 / 4 / 8 / 0x10 / 0x40 / 0x80 clear.

##### Begin 0x141332F90 [I]

```
pawn vt+0x16E8()                StopMovement
pawn vt+0x17A0(5, 1)            SetMovementMode(Flying)                       0x141332FF5
pawn vt+0x1340(1, 1, 1.0)       Draw                                          0x14133301C
pawn vt+0x1388(1, 0, 0)         equipment vt+0x508: equip+0x2E0 = 1, +0x2E6 bit0 = 0   0x141333031   [meaning ?]
pawn vt+0x1728(0)               gravity off (component+0xA3C bit 1 = 1)       0x141333040
pawn vt+0x2068(1)               pawn+0x630 bit 2 = m_bIsInvincible = 1        0x14133304F
pawn vt+0x19E0(1)               component+0xB70 bits 2, 3, 4 = 1              0x14133305E   [char-push switches ?]
ctl = controller (0x14150DAF0); if null: return
+0xEC = actor location;  +0xF8 = reference point;  +0x104 = +0xEC;  +0x110 = +0xF8
MyBodyColl (pawn+0x590) vt+0x720(8): bodycoll+0x4A4 = 8   (0x1405B8B30; pawn+0x619 is NOT written)   0x141333157
+0xB8 = &ctl+0xB18;  +0xD8 = ctl+0xB20;  +0xD4 = -1
if !slot115(0): return                          // first target; also runs travel sub-step 0 at once
if (u8)(+0xDC - 1) <= 2: flags |= 2             // ENEMY / ETURN / ERIDE      0x141333196 ja skip
+0x12C = pawn+0xD34;  +0x130 = pawn+0xD38
```

Reaction type 8 = `ETresBodyCollReactionType` 8 (ghost, Part I 2.4) on the component only; the invincibility of the
state is `m_bIsInvincible` [A reflection name of pawn+0x630 mask 4], on from Begin to End.

##### Tick 0x141341270 [I]

```
if list pointer null or count <= 0 (0x1413412CA jle): DoChain (vt+0x270); return
pawn vt+0xDC8(dt * 12);  pawn vt+0xDB8(dt * 12)           // 12.0 @0x1413412DE
delegate(+0x1E0)(dt)
+0xB1 = 0                                                 // jump latch lives one frame
```

##### Slot 115 (vt+0x398) `SetMode(mode)` 0x1413311D0 [I]

```
+0x1D0 = 0;  if mode != 4: +0x1D8 = 0
mode 1: +0xB0 = 1, delegate = slot 117 (kick stop)        mode 2: +0xB0 = 1, delegate = slot 118 (pole stop)
mode 4: +0xB0 = 1, delegate = slot 119 (enemy stop)       other non-zero: return false
mode 0 (next travel leg), delegate = slot 116:
    list present and count > 0:
        index = ++(+0xD4);  entry = list[index] (0x34 bytes);  +0xC0 = entry;  +0xDC = entry+4;  +0x11C = 0
        +0x120 = component of the entry's weak pointer (entry+0x1C)
        if index + 1 >= +0xD8 (0x1413313A8 jl else):  flags |= 0x40;  +0xC8 = 0
              pawn vt+0x1010(1, 0);  vt+0xFF0(1);  controller 0x141411FD0(ctl, 1, 0)
        else: +0xC8 = list[index + 1];  pawn vt+0x1010(0, 0);  vt+0xFF0(0)
    list empty:  pawn vt+0x1010(1,0), vt+0xFF0(1), StopMovement, Play("SlideEnd", blend 0, mode 0, group 0),
                 (not last: landing probe, SetActorLocation(+0x104)), delegate = travel, +0xD0 = 0, sub = 2,
                 run delegate, return false
+0xD0 = mode;  sub = 0;  delegate(world+0x948 = world delta seconds) is run immediately;  return true
```

Note the index is never bounds-checked against the count in mode 0: the travel code calls `SetMode(0)` only when the
flag 0x40 is clear [I].

##### Slot 116 — travel 0x141348BA0 (mode 0)

```
sub 0  (0x141349A57) [I]
    0x1405DA4E0(pawn, 1, 1, rate 1.0, time 0.0)            // ends the 0.1 slow motion set by the controller
    +0x128 = 0;  +0x8C = actor yaw;  +0x90 = MyAthleticSlideTime (1.0)
    +0x140 = TargetPoint();  variant = 1
    pitch = pitch of (target - reference point)            // 0x142D8D950 vector -> rotator
    if pitch > 45  (comiss pitch,45; jbe else  0x141349B27): variant = 3; ++(+0x9C)
    elif pitch < -30 (comiss pitch,-30; jae skip 0x141349B42): variant = 2; ++(+0xA0)
    clip:  variant 3 and (+0x9C odd): "AthleticSlide"   blend 0, mode 0 (once, no root motion), effect group 0xC80
           variant 2 and (+0xA0 odd): "AthleticSlide2"  same arguments
           else pawn vt+0x1C90(variant) (0x141433C00): 1 "SuperSlideLoop", 2 "SonicSlideLoop", 3 "SuperFlightLoop",
                blend 0, mode 1 (loop), effect group 0xC80
    play rate 1.0;  +0xB0 = 0;  sub = 3
sub 3  (0x141348CFA) [I]
    S = MyAthleticSlideSpeed (6000);  +0x140 = TargetPoint();  d = |+0x140 - reference point|
    v0 = max(2 * d / (+0x90),  min(60 * d, S))             // addss, divss [+0x90] 0x141348D84, 60.0 0x141348D8D,
                                                           // minss 0x141348D99, maxss 0x141348D9D
    Turn((0, yaw of (target - ref), 0), alpha = dt * 60)   // 0x141348DE0 (60 Hz: alpha 1 = snap)
    pawn vt+0x1620(v0):  component+0x390 MaxFlySpeed = v0          0x141348DFF
    pawn vt+0x1640(v0 * 60): component+0x39C MaxAcceleration       0x141348E15
    +0x13C = 0;  sub = 1
sub 1  (0x141348E2D) [I]
    if 0 >= +0x90 (comiss 0,[+0x90]; jb skip): StopMovement;
            if !(flags & 0x40): SetMode(0); return         // leg timed out: go on to the next target
            else +0xB1 = 1                                  // last leg timed out: abort below
    +0x120 = component of the current entry
    if +0xB1 or component invalid or Stuck(dt) (slot 122):                       -> ABORT
    +0x140 = TargetPoint();  +0x90 -= dt
    if IsAnimEnd and current clip name (pawn+0x440, slot 0) == "AthleticSlide":   [name compare C]
            Play(vt+0x1C90(variant), blend -1, mode 1, group 0xC80)
    d = |+0x140 - reference point|
    R = component vt+0xB18() (m_CloseRange, +0x89C) + 100.0                       0x1413490A8
    if d >= R (comiss d,R; jae 0x1413490B5):                                     -> MOVE
    ARRIVAL:
      flags & 2 (enemy type):
          owner = component+0x1B8; if not a live TresCharPawnBase:               -> MOVE   (keeps flying)
          dir = normalize(+0xF8 - +0x140)   (from the target back to the start reference point; 1e-8 guard)
          iface48 vt+0x40(dir, owner, component)  = 0x141342EA0  ("ENEMY")       -> enemy stop (mode 4)
      else:
          +0x174 = reference point
          SetActorLocation(pawn, +0x140, bSweep = 1) (0x143EAC3D0, r8b = 1 @0x141349312)
          pawn vt+0x1010(1, 0);  vt+0xFF0(1)
          if slot126():  "POLE"
          elif slot127(): pawn vt+0x1010(1,0); vt+0xFF0(1); StopMovement;
                          Play("SlideEnd", blend 0, mode 0, group 0, rate 1);  sub = 2          "END"
          elif slot128(): "KICK"
          else StopMovement; SetMode(0)                                                         "NEXT"
    MOVE (0x1413495A5):
      dir = normalize(+0x140 - reference point)
      v = max(6 * d, pawn vt+0x1630() = MaxFlySpeed)        // 6.0 @0x14134964E, maxss 0x141349663
      if d < 600 (comiss d,600; jae skip 0x14134966E): v = 3000            // 0x141349670
      if v * dt * (1/60) > d (0x14134968A ja): nothing this tick ("Adjust!!")
      else: Turn((0, yaw of dir, 0), dt * 12);  pawn vt+0x1668(dir * v, 0)   // component vt+0x500
    ABORT (0x141349847):
      pawn vt+0x1010(1,0); vt+0xFF0(1); StopMovement; Play("SlideEnd", blend 0, mode 0, group 0)
      if !(flags & 0x40): if component and owner valid: LandingProbe(component location, ref, pawn, owner, &+0x104, 60)
                          and, when that fails, LandingProbe(+0xF8, ref, pawn, null, &+0x104, 60)
                          SetActorLocation(pawn, +0x104, no sweep);  +0x11D = 0
      sub = 2
sub 6: IsAnimEnd -> SetMode(0)                              sub 2 (and any other): IsAnimEnd -> DoChain (vt+0x270)
```

**How the pawn moves.** Movement mode Flying, gravity off, no root motion (all travel clips are played without the
root-motion bit and have no root track [A]). The state never writes `Velocity`: each tick it calls
`RequestDirectMove(dir · v, bForceMaxSpeed = false)` — component vt+0x500 = 0x140592BC0, whose body is the stock one
[I: Falling → clamp and air control; otherwise `RequestedVelocity` (+0x5AC) = v, +0x594 bit 19 set, bit 20 = arg, Z
zeroed only when walking]. The speed actually reached is therefore decided by the component's velocity step with
`MaxFlySpeed` = v0 and `MaxAcceleration` = 60·v0: in stock UE 4.17 `ApplyRequestedMove` the requested speed is clamped
to `MaxFlySpeed`, an increase is limited by `MaxAcceleration`·dt (= v0 per 1/60 s: full speed after one 60 Hz step) and
a decrease is immediate [U — the Tres `CalcVelocity` was not re-read for this path: probe P1]. Resulting profile [D]:

- v0 (fixed for the leg, from the distance d0 at leg start): d0 < 100 → 60·d0; 100 ≤ d0 < 3000 → 6000; d0 ≥ 3000 →
  2·d0 (so that d0 is covered in 0.5 s);
- while d ≥ 600: speed min(max(6·d, v0), v0) = **v0**; below 600 cm: **min(3000, v0)**;
- arrival when d < `m_CloseRange` + 100 (200 cm for the default 100); d is measured from the reference point (actor
  + 100 cm up) to the target point;
- the collision of the move is the ordinary Flying sweep of the capsule (`PhysFlying`; not re-read) [U]; a blocked
  leg is detected by slot 122 and aborted.

Slot 122 `Stuck(dt)` 0x141336DB0 [I]: `if 0.5 · |pawn vt+0x2B8() (GetVelocity)| > |component+0x9A0 vector|
(comiss; jbe else 0x141336E4B): +0x13C += dt; if +0x13C > 0.2 (0x141336E5D): return true ("No Moving!!")` else
`+0x13C = 0`. (component+0x9A0: the component's measured movement vector — field not otherwise identified [?].)

Slot 123 `TargetPoint(out)` 0x141336520 [I]: out = actor location; if the entry's component and its owner are alive:
out = component world location (component vt+0xB88 = `ComponentToWorld` translation +0x2B0); if the flag 0x40 is clear
and the next entry's component is alive: `return pawn vt+0x2750(mid = (this + next) / 2, this, pawn, null, out, 60.0,
0)` (0x141431630: a blocking probe that may pull the point off geometry [C, not read]).

Slot 124 `LandingProbe(a, b, ignoreActor1, ignoreActor2, out, r)` 0x141331780 [C]: world sweep 0x144526380 from a to
b, trace tag `Athletic`, responses of channels 0, 1 and 18 set to 2 (block), shape parameter r (60.0 at every call
site [I]); first hit on an actor other than the two ignored ones → out = hit location with Z = hit.Z − r + 1, return
false; no hit → out = b, return true.

##### Arrival branches

| Debug string | Test (in this order, non-enemy targets only) | Result |
|---|---|---|
| POLE | slot 126 0x1413321B0 [I]: `+0xDC == 6` (0x14133220B); r = max(`m_CloseRange` + 100 − 75, 125) (0x14133233D / 5D / 65); pawn vt+0x2330(P, P, r, out) with P = component location must return a pole whose actor is the component's owner | last target: `TryPoleTurn` (interface slot 78 0x14143F0C0) → `DoChain` into POLE_TURN (24); else, first stop of the chain (flag 0x20): in-state pole stop (mode 2); else `StopMovement`, next leg |
| END | slot 127 0x141331C40 [I]: flag 0x40 and component + owner alive (returns true whatever the ledge probe says) | ledge probe 0x140638740(pawn, out, (target.Z − ref.Z) + 300, 300, horizontal distance − 45, 90.0, debug): hit → `StopMovement`, interface slot 65 0x141444810 (push DANGLE with the ledge data) → `DoChain`. Always: `SlideEnd` (mode 0), sub 2, `DoChain` at the clip end |
| KICK | slot 128 0x141331ED0 [I]: flag 0x20 | `m_pHitComponent` = component, `m_pHitActor` = owner, +0x168 = normalize(component location − reference point), mode 1 |
| NEXT | otherwise | `StopMovement`; `SetMode(0)`: the next leg starts in the same tick |
| ENEMY | flag 2 (types 1–3), tested before the others | interface+0x48 slot 8 0x141342EA0 |

The flag 0x20 is set only by Reset and cleared by the kick stop (0x1413475C0) and the pole stop (0x14134828E): **only
the first intermediate target of a chain gets the kick / pole flourish**; later ones are passed without a pause [I].
Readers of +0xDC are complete [I scan]: Begin (1–3), slot 126 (6), the stops (1, 2). Types 4 GROUND, 5 WALL, 7 RAIL,
8 ROPE, 9 BALL are treated alike by the state; there is no "arrive on a rail" and no wall-run branch. The state never
calls `TryWallKick` (interface +0x290) or the rail request [I scan of its functions].

**Kick stop — slot 117 0x141347510 (mode 1)** [I]: sub 0: ++(+0xA8); `StopMovement`; if flag 0x20: clear it and
controller 0x141411FD0(ctl, 1, 1); `Play("WallKickF", blend 0, mode 2 (root motion), group 0xC80, rate 1)`; anim
vt+0xF0(slot 0, 0.2) [meaning ?]; `SetActorRotation(rotator of +0x168)` (0x143EAD160) — Sora faces the point; sub 2.
Sub 2: clip end → `SetMode(0)`. `WallKickF` 1169 [A]: 0.5 s, root motion 0 until frame 12, then −80 cm and a 180°
turn over the last three frames; `TresEnableChange` 0.4674 (not used here).

**Pole stop — slot 118 0x141347750 (mode 2)** [C, structure only]: clears flag 0x20; attaches to the pole component;
`PoleTurn` (vertical) or `PoleSwing` (bar), mode 2; 0.5 s blend of the relative location (+0x1C4); then the loop clip
(mode 3) for `length × (+0x1C0 = 1)`; detach; not last → `SetMode(0)`, last → `SlideEnd` → `DoChain`.

**Enemy arrival — interface+0x48 slot 8 0x141342EA0(dir, owner, component)** [I]: requires mode 0, sub-step 1,
component.owner == owner, flag 2. `m_pTargetEnemy` = owner; if interface slot 110 (0x1414476C0, the SMALLAIRPLANE
request) accepts the owner: `DoFinish`; else `m_pHitActor` = owner, `m_pHitComponent` = component, +0x168 = dir,
+0x180 = 0, slot 120(); if `+0xDC == 2` (ETURN): owner+0x448 interface vt+0x58(pawn) (the enemy may answer through
interface+0x48 slot 11 0x141345A40, which sets `m_pEnemyTurn`); `SetMode(4)`.

**Enemy stop — slot 119 0x141346600 (mode 4)**, jump table 0x1413474EC [I for the lines with an address]:

```
sub 0 (0x141346642): StopMovement;  sub = 6;  +0x134 = 0;  +0x128 = 0;  Prompt(1) (slot 121)
sub 6 (0x141346677): m_pEnemyTurn invalid:
                         ++(+0xA8);  sub = 3
                         Play("AthleticSlideEnd", blend 0, mode 2 (root motion), group 0xC80, rate 1)   0x1413469DE..A3D
                     m_pEnemyTurn valid (ETURN enemies) [C]: slow motion as below, attach to the component,
                         "PoleTurn" (mode 2), offset (0, 0, -100) in the attach frame, 0.5 s blend (sub 5),
                         "PoleTurnLoop" (mode 3) for 2 loops + 0.2 s (sub 2), detach, "SlideEnd" (mode 2), sub 7
sub 3 (0x14134717E): if m_bEnableInput (state+0x84 & 0x20):
                         if flags & 0x10: 0x1405DA4E0(pawn, 1, 1, rate = +0x12C (0.25), time = +0x130 (0.05)); flags |= 0x80
                         sub = 4                                                    (falls through)
sub 4 (0x1413471C7): if m_bEnableChange:
                         if flags & 0x80: 0x1405DA4E0(pawn, 1, 1, 1.0, 0.0); flags &= ~0x80
                         if flags & 1: Prompt(0)
                         sub = 7
sub 7 (0x141347231): IsAnimEnd -> DoChain
                     elif +0x135: if nothing queued (0x1405AB6E0 <= 0): interface slot 73 (push SUPER_SLIDE)(1, 0);  DoChain
                     elif +0x128: a = SetState(12 ATTACK) (0x1405B9FE0); controller vt+0x16D8();
                                  a+0x118 = m_pTargetComponent;  a+0x1E8 = 0x38 if +0xDC == 2, 0x36 if +0xDC == 1;
                                  (pawn kind 0xB: tutorial "Tutorial_AthleticFlow");  DoChain
                     elif +0xDC == 1 and pawn vt+0xE48() (component+0x9A8) == 0.0: DoChain
```

Slot 121 `Prompt(b)` 0x1413342C0 [I]: `b` and flag 2 and not flag 1 and the component's vt+0xA80() (active,
`m_bPermissionLockOn`, owner alive): controller 0x14140DEF0(ctl, 0); flags |= 0x11; controller vt+0x18F0(owner, 0, −1).
Otherwise, if flag 1: 0x14140DEF0(ctl, 1); flag 1 cleared. So the attack window (flag 0x10) exists only for an enemy
target that may be locked on.

`AthleticSlideEnd` 1163 [A]: 0.6333 s, 20 frames, root motion on (interp 0.03): a hop back and up — forward 0, 0, 0,
−5.58, −15.03, −27.55, −41.71, −56.55, −71.65, −86.35, −100, −111.83, −121.67, −129.7, −136.13, −141.17, −145,
−147.76, −149.44, −150 cm; up 0, 0, 0, 25.85, 53.32, 71.96, 82.51, 88.71, 93.45, 96.2, 96.43, 94.11, 89.61, 82.99,
74.32, 63.67, 51.12, 35, 16.51, 0 cm; `TresEnableInput` 0.1, `TresEnableChange` 0.2667; sound `se00001_008` (pitch
1.1) at 0.0093; effect `vp_atf_hjk00` (group 3200) at 0.1235. Slow motion [D]: on from clip time 0.1 to 0.2667 when
no attack has been pressed yet (rate 0.25, the 0.05 is the second argument of the dilation call — blend / duration
[?]); 0.1667 s of clip time.

##### Follow-ups out of the state [I]

| Input | Path | Effect |
|---|---|---|
| jump | `JumpRequest` 0x14143D8F0 case AthleticFlow @0x14143E2D9: `+0xB1 = 1`, return true (jump gate 0x14142A3D0 @0x14142ABB3: always true for this state) | read only in travel sub 1 on the same frame: ABORT (`SlideEnd`, then `DoChain`). No jump is pushed; in a stop the press is lost |
| attack | gate slot 44 0x141426330 @0x141426D0E: nothing queued and state flag 0x10; request slot 45 0x141437330 @0x1414387A1: state vt+0x388 = slot 113 0x1413435F0: flags &= ~0x18, `+0x128 = 1`, prompt removed (0x14140DEF0(ctl, 3)) | at sub 7 of the enemy stop: `SetState(ATTACK)` with the target component and command 0x36 / 0x38 |
| guard | `TryFlow` slot 72 0x1414417B0 has a case for this state (@0x141441A03 → state vt+0x390 = slot 114 0x1413436C0: `+0x135 = 1`), but it first calls the flow gate slot 71 0x14142E320, whose byte table 0x14142E4E8 maps state 42 (index 39) to "fail" | **unreachable**: the guard button does nothing in state 42; the Superslide push of sub 7 is dead unless another caller of slot 114 exists [? none in the interface / controller ranges] |
| pole | only the state's own slot 126 (last target of type POLE) → `TryPoleTurn` (ability `POLE_SPIN` 95 / `POLE_SWING` 96 by pole type) | POLE_TURN |
| wall kick | the gate 0x14142F990 accepts state 42 in its mask [C], but nothing in this state requests it | caller outside the state [?] |

##### End 0x141334B60 [I]

```
if m_pEnemyTurn: enemy+0x448 interface vt+0x60(pawn)
anim vt+0xB8(slot 0, 0.0)
if pawn vt+0x968() (attached: [pawn+0x468]+0x200 != 0): p = pawn vt+0xC68("Center"); StopMovement; detach (0x143E93FF0);
        pawn vt+0x2C8(...); SetActorLocation(p, no sweep); pawn vt+0xB90()
pawn vt+0x17A0(3, 1)   SetMovementMode(Falling)            pawn vt+0x1388(0, 0, 0)
pawn vt+0x1728(1)      gravity on                          pawn vt+0x1628(): MaxFlySpeed = vt+0x1638() (default)
pawn vt+0x2068(0)      m_bIsInvincible = 0                 pawn vt+0x19E0(0)
pawn vt+0x1010(1, 0);  vt+0xFF0(1);  vt+0xCA8(1)
if +0x11C: (component alive: pawn vt+0x2750(+0x110, component location, pawn, owner, &+0x104, 60, 0));
           SetActorLocation(+0x104, no sweep); +0x11C = 0
elif +0x11D: landing probes as in ABORT; SetActorLocation(+0x104, no sweep); +0x11D = 0
MyBodyColl vt+0x720(byte [[pawn+0x4D0]+0x98])   (0x1405B6100: default reaction type back)
controller: 0x141411FD0(ctl, 0, 0);  ctl vt+0x1948();  slot 121(0)
```

`MaxAcceleration` is **not** restored by End [I: no vt+0x1640 call]; it stays 60·v0 until another state writes it.

##### Clips [A unless noted]

| Name | Seq | Length | Root motion | Play arguments [I] | Notifies |
|---|---|---|---|---|---|
| `SuperSlideLoop` | 1152 | 0.3333 s | none | blend 0 (−1 after an intro), mode 1, group 0xC80 | effect `vp_ffa_rad0` (group 3200) 0; attached sound `se00001_039` (fade out 1.0); voice `bt1011810..40sr0` |
| `SonicSlideLoop` | 1155 | 0.3333 s | none | same (downward variant) | `vp_ffa_rad0`; attached `se00001_039` (fade out 0.7) |
| `SuperFlightLoop` | — | — | — | same (upward variant) | **not in `p_ex001_AnimSet`** (the name exists only in the executable, strings 0x146982F38 / 0x146982DF0): what Sora shows on an upward loop is [?] |
| `AthleticSlide` | 1156 | 0.7333 s | none | blend 0, mode 0, group 0xC80; upward legs with an odd count | `vp_ffa_rad0` 0; `se00001_039` 0.0097 |
| `AthleticSlide2` | 1153 | 1.0 s | none | same; downward legs with an odd count | `vp_ffa_rad0` 0; `se00001_016` (volume 0.6) 0.0315 |
| `AthleticSlide3` | 1150 | 0.8 s | none | **never played**: the string is not in the executable [I strings sheet] | — |
| `SlideEnd` | 1157 | 0.7667 s | 426 cm forward in the asset | END / ABORT: blend 0, **mode 0** (root motion not applied), group 0; enemy-turn / pole exit: mode 2 | `se00001_015` (pitch 1.2) 0.0114 |
| `AthleticSlideEnd` | 1163 | 0.6333 s | −150 cm, arc +96 cm | blend 0, mode 2, group 0xC80 | above |
| `WallKickF` | 1169 | 0.5 s | −80 cm, 180° at the end | blend 0, mode 2, group 0xC80 | `se00001_003` 0; `se00001_013` 0.0337; `vp_ffa_wal0` (313) 0.0339; `TresShot` 0.199; `se00001_039` 0.3972; `vp_ffa_rad0` 0.4479; `TresEnableChange` 0.4674 |
| `PoleTurn`, `PoleTurnLoop`, `PoleSwing`, `PoleSwingLoop` | 1172–1175 | Part I 10.4 | | modes 2 / 3, group 0xC80 | |

Time dilation calls: travel sub 0 `0x1405DA4E0(pawn, 1, 1, 1.0, 0)` (off); enemy stop 0.25 / 0.05 and off (above).
0x1405DA4E0 forwards to the world object [world+0x110] vt+0x8A0(bool, bool, rate, time) [I]. Camera calls:
controller 0x141411FD0(ctl, bOn, b) at the last-leg start (1, 0), at the first kick (1, 1) and at End (0, 0): with
bOn = 0 it ends the camera at controller+0xA90 (vt+0x798 with {0.3, 3, 2.0, 2}) [I first block]; with 1 it starts
`TresCameraAthleticFlow` [C, camera strand]. pawn vt+0x1010(b, 0) / vt+0xFF0(b) (0x1405C9D10 / 0x1405C9B00, on
pawn+0x478 / +0x468) are switched 0 on a non-last leg and 1 otherwise [meaning ?].

##### Athletic Flow — Corrections to Part I

| Section | Part I says | Correct |
|---|---|---|
| 10.5 | "Begin … takes the target list … and moves toward the chosen target" with `MyAthleticSlideKickAnim` 0.1333 as a parameter of the state | the state does not read `MyAthleticSlideKickAnim`; the list is walked target by target (a chain of legs), each leg with its own 1.0 s budget |
| 10.5 | clips `AthleticSlide` 1156, `AthleticSlide2` 1153, `AthleticSlide3` 1150, `AthleticSlideEnd` 1163 | the cruise clips are `SuperSlideLoop` / `SonicSlideLoop` / `SuperFlightLoop`; `AthleticSlide` / `AthleticSlide2` are intro clips of odd-numbered upward / downward legs; `AthleticSlide3` is never played; `AthleticSlideEnd` is the enemy arrival only; `SlideEnd` (mode 0) is the point arrival and the abort |
| 10.5 | "Jump press sets +0xB1" (read as a jump follow-up) | the latch aborts the travel (`SlideEnd`, then `DoChain`); no jump is requested |
| 10.5 | "pole and wall-kick gates accept this state" | the state itself asks for a pole turn only at the last target of type POLE (6) and never for a wall kick; an intermediate pole / point is handled inside the state (modes 2 / 1) |
| 10.5 / Q6 | "What fills the target list and who requests the state: [?]" | controller aim mode 0x1414165E0 (call sites 0x141418F5C, 0x14141773E); list built by 0x14141FDC0 (Host affordances) |
| 10.1 | pole trigger row lists AthleticFlow with "pawn vt+0x2330(point, point, 100, out)" | for this state: P = component location, range max(`m_CloseRange` + 25, 125), only for type POLE |
| 10.1 | wall-kick row: "wall contact in … ATHLETICFLOW" | not requested by this state |
| 10 (intro) | "Movement mode during all of them: Flying, gravity off, velocity zero; displacement is root motion or direct SetActorLocation / velocity writes" | airstep travel is a **requested velocity** (`RequestDirectMove`) with `MaxFlySpeed` / `MaxAcceleration` rewritten per leg; `SetActorLocation` (swept) only snaps onto a point target at arrival |
| 8 | (implicit) "guard button → Superslide" from flow states | not from state 42: the flow gate rejects it |
| 2.4 | invincibility = body-collision reaction type | here: `m_bIsInvincible` (pawn+0x630 bit 2) plus reaction type 8 written straight into `MyBodyColl`+0x4A4 (pawn+0x619 unchanged) |
| 15 | "AIR_SLIDE / SUPER_SLIDE / ATHLETICFLOW --pole in reach--> POLE_TURN" | ATHLETICFLOW: only "last target is a pole" |

##### Disagreements with the magic spec (Part D, S2.3 / S4)

1. **Travel speed.** Magic: "each tick: `speed = max(2·dist / T, min(dist · 60, MyAthleticSlideSpeed))`". The formula is
   evaluated **once per leg** (sub-step 3, 0x141348D01..0x141348D9D) and stored as `MaxFlySpeed` (and ×60 as
   `MaxAcceleration`). The per-tick request (sub-step 1, 0x14134964B..0x141349678) is `max(6·d, MaxFlySpeed)`, replaced
   by 3000 when d < 600.
2. "`60·dist` inside the last 100 cm" — no: the 60·d term matters only when the leg *starts* closer than 100 cm; the
   final approach is 3000 cm/s from 600 cm and the leg ends at d < `m_CloseRange` + 100.
3. Range 0x141349A60..0x141349B42 quoted for the formula is the launch block (sub-step 0: slow motion off, yaw, time,
   pitch variant, clip) — it contains no speed computation.
4. S2 list entry "+0x14 move-flow type (component vt+0xAF8)": vt+0xAF8 (0x140F50C30) returns the float
   `m_ShootLockOnRange` (+0x88C); the move-flow type is the byte entry+4 (vt+0xAD0 = +0x87C) [I].
5. S4 request: "→ PushState(0x2A)": slot 97 calls `SetState` 0x1405B9FE0; `PushState` is the separate slot 193.
6. S4 targets: "searched … within `MyAthleticLockonLimit` 15000 / `MyAthleticLockonRange` 3750": the aim update uses
   the component's `m_ShootLockOnRange` when it is > 0 and ≠ 3750, else 15000 (0x141420304..0x141420327); 3750 is only
   the "unset" sentinel there (and the minimum range of the out-of-aim prompt 0x14141F6F0).
7. S4 clips list `AthleticSlide3`, `PoleSwing(Loop)` as travel animations: `AthleticSlide3` has no reader; the cruise
   clips (`SuperSlideLoop`, `SonicSlideLoop`, `SuperFlightLoop`) are missing from the list.
8. Missing in S4: the unlock bit (player data +0x95B0 bit 5, 0x14141FE3B) and pawn kind 1 required by the target
   update; the screen test of airstep targets (160 px candidate circle, 80 px for the aimed one — the shotlock circle
   is 150 px); Begin also sets reaction type 8 on `MyBodyColl`.
9. S2.3 "airstep key … mode = 3 … `TryAthleticFlow`": agreed [I 0x141418F5C]; the second call site 0x14141773E is in
   the focus-syphon branch.

##### Athletic Flow — Host affordances

**What an airstep target is.** A `UTresLockonTargetComponent` (size 0x8C0, vtable 0x146628970) or subclass registered
with the lock-on manager. Level "airstep points" are `UTresShootFlowComponent` (vtable 0x1465E4250; `TresFlowComponentBase`
← `TresLockonTargetComponent`), placed by blueprint actors; the native actor `ATresShootFlowActor` (`TresGimmickActor`)
has one as `m_Root`. Example [A]: `Maps/he/umap/he_01/he_01_gimmick_Env.umap.json`, actors `g_he_anchor_01_8`,
`g_he_anchor_141`, `g_he_anchor_248` of `/Game/Blueprints/Gimmick/he/g_he_anchor/g_he_anchor_01` (parent
`g_TresGimmickStaticBase`): component `TresShootFlow`, template `RelativeLocation` (0, 105, 76) under the mesh;
instance overrides `m_bMoveFlowDestination` = true and `m_ShootLockOnRange` = 9000 / 9000 / 14000.

| Property (offset) | `TresLockonTargetComponent` default [I 0x140F8C720] | `TresShootFlowComponent` default [I 0x140F477F0] | Used by |
|---|---|---|---|
| active bit (+0x868 bit 0, unreflected) | 1 | 1 | every permission getter |
| `m_bPermissionLockOn` (+0x870 bit 0) | 1 | **0** | attack prompt at an enemy (vt+0xA80) |
| `m_bPermissionShootFlow` (+0x878 bit 0) | 1 | **0** | shotlock candidates |
| `m_bPermissionMoveFlow` (+0x878 bit 1) | 1 | 1 | airstep candidates (vt+0xAC0) |
| `m_MoveFlowType` (+0x87C, `ESQEX_FreeFlowKind`) | 1 ENEMY | **5 WALL** | state flag 2 (1–3), pole test (6), attack command (1 / 2) |
| `m_bMoveFlowDestination` (+0x880 bit 0) | 0 | 0 (true on the placed anchors) | only the out-of-aim prompt 0x14141F6F0 (vt+0xAE0); not the state, not the aim update [I scan] |
| `m_ShootLockOnRange` (+0x88C) | 0 | 0 (9000 / 14000 on the anchors) | airstep selection range |
| `m_CloseRange` (+0x89C) | 100.0 | 100.0 | arrival radius = value + 100; pole probe range |
| world location (+0x2B0) | | | target point |
| owner actor (+0x1B8) | | | must be alive; team; ignored by the probes |

Enum `ESQEX_FreeFlowKind`: 0 NONE, 1 ENEMY, 2 ETURN, 3 ERIDE, 4 GROUND, 5 WALL, 6 POLE, 7 RAIL, 8 ROPE, 9 BALL [A].

**Selection (controller).** Candidates are collected when the aim mode opens (0x14141942E..0x141419613) [I]: every
lock-on component of the manager list (vt+0x9D0) with the active bit, `m_bPermissionMoveFlow`, and a team value
(0x140F75F30(component)) different from the player's. Each aim tick 0x14141FDC0 rebuilds the airstep target list
(controller+0xB18 / +0xB20):

```
needs player data (controller vt+0x1420) +0x95B0 bit 5 and pawn kind 1                  [I 0x14141FE3B, 0x14141FE98]
s = min(viewportW * 0.00078125, viewportH * 0.0013888889)                               [I 0x141420011..21]
for each candidate (vt+0xAC0 again, team again, position 0x1413FE8F0):
    range = m_ShootLockOnRange if (> 0 and != controller+0x988 (3750)) else controller+0x984 (15000)   [I 0x14142030A..20]
    skip if |pos - reference point|^2 > range^2            (comiss d2,r2; ja skip 0x141420327)
    project (0x140ECF670); skip if w <= 0
    skip if (sx - cx)^2 + (sy - cy)^2 > 25600 * s^2        (160 px at 1280x720; 0x141420386 ja skip)
    skip unless LineOfSight(player -> component) (0x1413EE400)
    keep as candidate; the aimed one = smallest screen distance below 6400 * s^2 (80 px)  [I 6400 @0x14142002E]
no aimed target: list empty, return false
aimed target of type < 4 (enemy): list = [aimed]                                          [C]
else: candidates sorted (0x14062E940, key = entry distance; order not read [?]); the aimed one is appended in its
      place, every other candidate of type > 3 is appended when LineOfSight(candidate -> aimed) holds   [C]
```

`LineOfSight(from, to)` 0x1413EE400 [C]: sweep 0x144526380 with trace tag `Athletic`, from the player reference point
(actor + 100 cm up) — or from the first component's location — to the target component's location, shape parameter
35.0, channels 0 (WorldStatic), 1 (WorldDynamic) and 18 blocking, ignoring the start actor and the target's owner
(and, for a `TresPoleActor` owner, its attach parents); any other blocking hit = no line of sight.

**What the host must supply for a test level.**

| Feature | Data |
|---|---|
| airstep point | position; type (5 for a plain point, 6 for a pole point, 4 ground); `m_CloseRange` (100); selection range (0 → 15000 cm); move-flow permission; owner handle; team ≠ player |
| enemy target | the enemy's lock-on component: position, type 1 (2 = "turn" enemies), `m_CloseRange`, lock-on permission (attack prompt), owner pawn alive |
| screen test | view-projection, viewport size and centre |
| line of sight | sphere sweep r 35 (static + dynamic world), start = player + 100 Z, end = component position, ignoring both owners |
| travel | Flying move of the capsule with a requested velocity (speed cap = `MaxFlySpeed`, acceleration = `MaxAcceleration`), blocking sweep; measured displacement for the stuck test (0.5 × speed for 0.2 s) |
| arrival snap | swept teleport of the actor to the component position (point targets) |
| landing probe | sweep r 60 between two points ignoring two actors → hit position (Z − 59) |
| ledge at the last point | probe of Part I 14 (0x140638740) with up range (ΔZ + 300), 300, forward range (horizontal distance − 45), 90 → DANGLE |
| pole at a type-6 point | pole query at the component position, range max(`m_CloseRange` + 25, 125), must belong to the same actor |
| time dilation | world hook (rate, time): 1.0 at launch; 0.25 / 0.05 in the enemy attack window |
| flags | `m_bIsInvincible`, body reaction type 8, weapon drawn, gravity off, mode Flying / Falling |
| unlock | "airstep available" bit (player data +0x95B0 bit 5; script `BP_SetAbilityAthleticFlow`) |

##### Athletic Flow — Tests (60 Hz, dt = 1/60; frame B = the frame whose state step runs Begin; speeds [D] under the stock velocity step)

**T1 — enemy 1000 cm away, same height as the reference point, `m_CloseRange` 100 (R = 200), type 1.**

| Frame | State step | Movement step of that frame |
|---|---|---|
| B | Begin; travel sub 0 inside Begin: yaw stored, time 1.0, variant 1, `SuperSlideLoop` loop | — |
| B+1 | sub 3: v0 = max(2·1000/1.0, min(60000, 6000)) = 6000; `MaxFlySpeed` 6000; `MaxAcceleration` 360000; yaw snapped to the target (alpha 1) | velocity 0 |
| B+2 | sub 1: time 0.9833; d = 1000; request 6000 cm/s | 0 |
| B+3 … B+7 | d = 900, 800, 700, 600, 500 after the move; requests 6000, 6000, 6000, 6000 (600 is not < 600), then 3000 at d = 500 | 100 cm per frame |
| B+8 … B+13 | d = 450, 400, 350, 300, 250, 200 (200 is not < 200: move) | 50 cm per frame |
| B+14 | d = 150 < 200: ARRIVAL → ENEMY → mode 4 sub 0 in the same step (`StopMovement`, prompt on) | 50 cm |
| B+15 | sub 6: `AthleticSlideEnd` starts (clip time 0) | root motion |
| +0.1 s clip | `TresEnableInput`: slow motion 0.25 (if attack not pressed), sub 4 | |
| +0.2667 s clip | `TresEnableChange`: slow motion off, prompt off, sub 7; a latched attack → ATTACK now | |
| +0.6333 s clip | clip end → `DoChain` → End: Falling, gravity on, `m_bIsInvincible` off | |

Expected numbers: 850 cm flown in 12 movement frames (0.2 s): 5 frames at 100 cm, 7 at 50 cm; stop 150 cm (reference
point to component) short of the target; hop back 150 cm with a 96.4 cm arc; `m_bIsInvincible` and reaction type 8
from B to End (about 0.25 + 0.6333 s without input). Time left when arriving: 1.0 − 13/60 = 0.783 s.

**T2 — single airstep point (type 5, R = 200) 1000 cm away.** Same travel. At B+14: actor location := component
location (swept; the reference point jumps from 150 cm away to 100 cm above the point), END: `SlideEnd` in mode 0 —
0.7667 s (46 frames) hanging with zero velocity, Flying, no gravity — then `DoChain` → Falling. With a ledge inside the
probe at the point: DANGLE instead.

**T3 — point 5000 cm away.** v0 = 2·5000 / 1.0 = 10000 cm/s (166.7 cm per frame) down to d < 600, then 50 cm per
frame; 27 movement frames to d = 500, then 7 more (arrival at d = 150).

**T4 — two points A (1000 cm) and B (a further 1000 cm), controller list = [A, B] (list order: probe P3).** Leg 1 ends at A with KICK:
`WallKickF` 0.5 s facing A (root −80 cm and 180° at the end), flag 0x20 cleared; leg 2 starts at the clip end with a
fresh 1.0 s; at B: END. A third point would be passed with NEXT (no pause).

**T5 — jump pressed during travel.** That frame: ABORT → `SlideEnd` (mode 0, 0.7667 s), actor moved to the landing
point when the leg was not the last, then `DoChain`. No jump.

**T6 — blocked leg.** Velocity 6000 requested but measured movement below half of it for more than 0.2 s (13 frames):
ABORT as T5. **T7 — time out:** a leg that has not arrived after 1.0 s: not last → next leg from where Sora is; last →
ABORT.

**T8 — guard pressed anywhere in the state:** nothing (flow gate fails). **T9 — attack pressed after arrival at a
lockable enemy:** accepted once (flag 0x10), ATTACK starts at `TresEnableChange` (0.2667 s into `AthleticSlideEnd`)
with command 0x36 and the target component.

##### Athletic Flow — Still needs the running game

| # | Object | Offset | Type | Why |
|---|---|---|---|---|
| P1 | movement component | +0x214 Velocity, +0x390 MaxFlySpeed, +0x39C MaxAcceleration, +0x5AC RequestedVelocity | vec3, f32, f32, vec3 | per frame through T1 and T3: confirms the capped-speed reading (100 → 50 cm per frame) and the one-frame acceleration |
| P2 | state AthleticFlow | +0x78, +0x90, +0xD0, +0xD4, +0xD8, +0xDC, +0x138, +0x140 | i32, f32, i32, i32, i32, u8, u32, vec3 | sub-step / mode timeline, arrival frame, flag history (0x20, 0x40, 0x10, 0x80) |
| P3 | controller | +0xB18 list (count +0xB20; entries 0x34: +0 f32, +4 u8, +0x1C weak pointer) | | order of a multi-point chain (sort direction of 0x14062E940) |
| P4 | movement component | +0x9A0 | vec3 | identity of the vector compared with the velocity by the stuck test |
| P5 | movement component | +0xB70 | u8 | what bits 2–4 switch (set by pawn vt+0x19E0) — expected: pawn-to-pawn push off |
| P6 | pawn | +0x630 u32, MyBodyColl (pawn+0x590) +0x4A4 u8, pawn+0x619 u8 | | invincible flag and reaction types during the state |
| P7 | world time dilation | actor `CustomTimeDilation` of the pawn and global dilation | f32 | meaning of the 0.05 argument and of the two bool arguments of 0x1405DA4E0 |
| P8 | animation | clip name and time in slot 0 on an upward (> 45°) leg | | what plays for the missing `SuperFlightLoop` |
| P9 | function-entry events | 0x1413436C0 (guard latch), 0x1414446B0 (push slot 193), 0x141442480 (TryWallKick) with state id 42 | | confirm they never fire in state 42 |
| P10 | pawn | +0x468, +0x478 | ptr, i32 | objects toggled by pawn vt+0xFF0 / vt+0x1010 |


---

#### Hopping (id 9)

##### H0. What a hop point is [I, A]

`ATresHopActor` (vtable 0x146635B20, constructor 0x140F8AD60 [C]): root `SceneComponent0`, a
**`UTresHopComponent`** named `TresRoot` (vtable 0x146634EE0, constructor 0x140F8C5D0, super
`UTresGimmickComponentBase`) and a `BoxComponent0` (extent 100 × 100 × 1, collision profile `TresNavMesh`
— navigation only, not read by the player). Actor properties `CheckLength` 150, `CheckRange` 100,
`HopID` −1 (not read by the player probe). `ATresHopActorEw` adds `PermissionAngle` /
`MyInverseUpVectorDir`. Any actor may carry a hop component (`TresGimmickLauncher` actors do: Begin and
End treat them specially).

`UTresHopComponent` properties (offset; default):

| Property | Offset | Default | Use |
|---|---|---|---|
| `m_bIsGimmickActive` (base class) | +0x868 bit 0 | | must be set for every probe |
| `m_AttachOffset` | +0x870 | (0, 0, 0) [D: not written by the constructor] | where the pawn's **actor location** ends, relative to the component (comp vt+0xBA0 0x140F94B40) |
| `m_BaseOffset1` / `m_BaseOffset2` | +0x87C / +0x888 | 0 / 0 | ends of the check segment, component space |
| `m_CheckRadius` | +0x894 | 100 [I 0x140F8C625] | check radius |
| `m_bIsOrder` / `m_OrderNumber` | +0x89C bit 0 / +0x8A0 | false / 0 | ordered chains |
| `m_IdleMotionType` (`ETresHopMotionType`) | +0x8A4 | 1 HOPPING | idle clip on the point |
| `m_bIsEndPoint` / `m_EndMotionType` | +0x8A8 bit 0 / +0x8AC | false | last point of a chain |
| flags | +0x8B0 | | bit 0 `m_bIsClosing` · 1 `m_bIsSurfing` · 2 `m_bIsTripping` · 3 `m_bIsDangle` · 4 `m_bIsHassou` · 5 `m_bDisableEffect` |
| `m_HassouMotionType`, `m_pFreeRunEffect` | +0x8B4 / +0x8B8 | | Caribbean ship jumps; the marker effect |

Component virtuals [I]: +0xB90 world location (0x140ACF250) · +0xB98 world attach point · +0xBA0
`m_AttachOffset` (dangle points: adjusted) · +0xBA8 component rotator · +0xBB0 0x140F930A0
`Check(pos, radius, &d2, &outLocation)`: needs `m_bIsGimmickActive`; `d2` = squared distance from `pos` to
the world segment [`m_BaseOffset1`, `m_BaseOffset2`] (component transform with scale); true when
`d2 ≤ (radius + m_CheckRadius)²`.

So a plain hop point is **a point** (the component location) with a radius; Sora's capsule centre is moved
onto `location + m_AttachOffset`. Points with `m_bIsDangle` (state slot 113 0x14139B450) and
`m_bIsHassou` (slot 114 0x14139EBF0, probe pawn vt+0x2370) run their own tick functions — not read.

Example [A]: `Levels/ca/ca_02/umap/ca_02_island01_gimmick01.umap`: `TresHopActor_1` at (−131416, 77608,
952), `HopID` 21, component `m_CheckRadius` 200; `TresHopActor_13` at (−134055, 75544, 643), radius 200;
`TresHopActor_15` radius 120. Every other property at its default (plain points, no order, no end point).

##### H1. Probes [I]

Pawn vt+0x2350 = 0x1405A6F40 `Probe(pos*, rot*, radius, out*)`; `out` = {owner actor, component, rotator}:

```
list = pawn.vt+0x8E0().vt+0x9B8()                 // world registry of hop components (weak pointers)
dir  = rot * (1, 0, 0)
last = pawn+0x668 (m_LastHitHopComponent);  order = (last and last.m_bIsOrder) ? last.m_OrderNumber : -1
limit = radius * radius * 36                                             // 0x1405A7068
for c in list: c valid, c.m_bIsGimmickActive, !c.m_bIsHassou, c != last, c's owner valid,
               (!c.m_bIsEndPoint or last != null):
    if order == -1:
        if !c.Check(pos, radius): continue
        if best and c.location.z < best.location.z: continue             // higher point wins  0x1405A7250
        d2 = XY distance^2 (pos, c.location)
        if d2 > limit: continue                                          // comiss d2,limit; ja 0x1405A72B3
        to = d2 > 0 ? normalize2D(c.location - pos) : dir
        if d2 <= c.m_CheckRadius^2 or dot(dir, to) >= 0:                 // inside the radius, or in front
            if d2 < 10000: to = dir                                      // 0x1405A73AC
            best = c; outRot = Rotator(to); limit = d2
    elif c.m_bIsOrder and c.m_OrderNumber - order == 1:
        return (owner, c, c.vt+0xBA8())                                  // next of an ordered chain, no distance test
return best ? (owner, best, outRot) : false
```

Pawn vt+0x2370 = 0x1405A6C70: the same loop restricted to `m_bIsHassou` components with
`Check(pos, 0)` and `d2 ≤ m_CheckRadius²` — only tried when the caller passes the flag (never from Idle /
Run / Jump). `m_LastHitHopComponent`: get vt+0x2358, set vt+0x2360, clear vt+0x2368.

##### H2. Request path [I for constants and call sites, C for the flow]

```
Gate (slot 183, 0x14142A140)(yaw, .., out):
    CanAct (vt+0x29C0); nothing queued (0x1405AB6C0 == 0); restriction bit 17 clear
    state == 9: return true                                              // cmp eax,9  0x14142A1B4
    pos = actor location
    if out.hassouFlag: if pawn.vt+0x2370(&pos, state != 8 ? 1000 : 0, out): return true     // 0x14142A231
    rot = (0, yaw, 0)
    IDLE (1): radius 100                                                 // 0x14142A37C
    RUN (2), SLOPESLIDE (10), SWAYING (11): pos += actor rotation * (120, 0, 0); radius 100   // 0x14142A29C
    JUMP (3): radius 220                                                 // 0x14142A30C
    other states: radius 0
    return pawn.vt+0x2350(&pos, &rot, radius, out)
TryHopping(yaw, bReverseLeg, bHassou) (slot 184, 0x14143D7F0):
    if !Gate: return false
    st = Push(out.actor, out.comp, ..) (slot 186, 0x141444EF0: PushState(9); st+0x88 = actor (m_pHitActor);
                                         st+0x90 = component (m_pHitComponent); m_bReverseLeg = argument)
    if bHassou: st+0xC4 = (state was WALLRUN)
    else: st+0xA8 = (state was IDLE or JUMP) ? out.rot.yaw : yaw;  st+0xA4 = (state was IDLE or JUMP)
```

Since the probe multiplies the radius by 6 for the distance limit, the reach is governed by
`Check`: the pawn (or the point 120 cm ahead of a running pawn) must be within
**radius + `m_CheckRadius`** of the point: 200 cm standing / running, 320 cm in the air for the default
radius 100; in the example level (radius 200) 300 / 420 cm.

Callers [I]: `Idle::Begin` 0x141386050 @0x1413861E1 (yaw = actor yaw, 0, 0) → `DoChain`; Jump tick
0x141395450 @0x141398192 (actor yaw, 0, 0), in the falling tail after the dangle test (Part I 5.4);
Run tick 0x1413AED70 @0x1413AF62F, after `TryClimbing` failed and only while the controller stick
magnitude ≥ 0.1 (comiss mag,0.1; jb skip 0x1413AF603): (Run+0x8C stick yaw, Run `m_bReverseLeg`, 0);
also the SlopeSlide (0x1413D2305), Swaying (0x1413D84C1) and WallRun (0x1413D9634) ticks.
`MoveInput` (slot 35) writes Hopping+0xA8 = stick yaw, +0xAC = magnitude, +0xA5 = 1 [C].

##### H3. State

Fields: +0x84 bit 1 `m_bReverseLeg` (which foot hops next) · +0x88 `m_pHitActor` · +0x90
`m_pHitComponent` · +0x98 start location (relative to the hop component) · +0xA4 entered from Idle / Jump ·
+0xA5 stick this frame · +0xA8 yaw · +0xAC stick magnitude · +0xB0 moving · +0xB4 hop counter · +0xB8 walk
speed in use · +0xC1 chained hop (blend-in 0.2) · +0xC2 / +0xC3 / +0xC5 tripping / dangle / hassou copies ·
+0xD4 sprint flag · +0xD5 launcher.

**Reset** 0x14138F650 [C]: all cleared; +0xB8 = 600.0.

**Begin** 0x141385D80 [C; 5.4 I 0x141385ECD]: `pawn.vt+0xDC8(1.0)`; `m_LastHitHopComponent` = the
component; copies the tripping / dangle / hassou bits; speed: if not already sprinting and
`pawn.vt+0xE38()` (current speed) ≤ `pawn.vt+0x1588()` (base walk speed): +0xB8 = base speed, else sprint:
+0xB8 = pawn+0xB28 `MySprintSpeed` (980); `SetMaxWalkSpeed(+0xB8)` (vt+0x1570);
`SetMaxAcceleration(+0xB8 × 5.4)`; Sheathe(1, 1.0); pawn bits 0, 3, 4 set; the hop actor is told
(0x140F926A0(actor, pawn, 2): marker state).

**Tick** 0x141393270 [C for the flow; constants I]:

```
if component invalid: DoChain
dangle point: slot 113; hassou point: slot 114; return
sub 0: attach the pawn to the component (0x143E8FC60); +0x98 = pawn RelativeLocation
       StopMovement; SetMovementMode(5, 1)
       from Idle / Jump (+0xA4): m_bReverseLeg = 1; Play("HopL", blendIn 0.2, mode 0)
       else: toggle m_bReverseLeg; Play(was set ? "HopR" : "HopL", blendIn +0xC1 ? 0.2 : -1, mode 0)   // 0x141394360..413
       sub = 1; +0xB0 = 1
sub 1: if +0xB0:
           t = clamp(clip time / clip length, 0, 1); target = component.m_AttachOffset
           A = start + (target - start) * t                               // follows the clip
           B = current - normalize(start) * GetMaxWalkSpeed() * dt        // constant-speed step toward the point
           p = (|B|^2 <= |A|^2) ? B : A                                   // whichever is nearer to the point
           if |p|^2 < step^2 or |current|^2 < |p|^2: p = target; +0xB0 = 0
           SetRelativeLocation(p)
           if !MyHopRotateModeB (pawn+0xB38): if !+0xA4: +0xA8 = yaw toward the point; SetRelativeRotation((0, +0xA8, 0))
           elif +0xAC > 0.2: Turn((0, +0xA8, 0), dt * 6)
       if IsAnimEnd or !+0xB0:
           SetRelativeLocation(target); StopMovement; other hop actors are told (0x141513A80)
           end point (m_bIsEndPoint): clear m_LastHitHopComponent; sub = 4; detach; SetMovementMode(1, 1);
               ResetMaxWalkSpeed; m_EndMotionType 4 -> Play("BalanceBreak"), 5 -> Play("RollLand") (-1, mode 2),
               otherwise DoChain
           else: Play(idle clip, -1, mode 3): m_IdleMotionType 2 SURFING "Balance", 3 NORMAL "IdleN",
                 4 "BalanceBreak", else m_bReverseLeg ? "BalanceL" : "BalanceR"
                 +0xB4 += 1; sub = 2; +0xB0 = 0
sub 2 (standing on the point):
       if !component.m_bIsGimmickActive: DoChain
       mag = +0xAC
       mag <= 0.2: Turn((0, actor yaw, 0), dt * 6)                         // comiss mag,0.2; jbe  0x141393564
       mag > 0.85:                                                         // comiss mag,0.85; jbe 0x141393571
           for d = 150; d < 400; d += 50:                                  // 0x1413935D8, 0x141393707..0C
               if pawn.vt+0x2350(actor location + (0, +0xA8, 0) * (d, 0, 0), (0, +0xA8, 0), 100, &o):
                   keep the result with the highest component location z   // comiss z,best; jbe 0x1413936F1
           found: if Gate(0, m_bReverseLeg, &o) and Push(o.actor, o.comp, m_bReverseLeg):
                      new+0xB4 = +0xB4; new+0xD4 = +0xD4; DoChain; return
           not found, !m_bIsClosing: hop off: StopMovement; detach; set location; SetMovementMode(3);
                      toggle m_bReverseLeg; Play("HopR" / "HopL", blendIn 0, mode 0);
                      Turn((0, +0xA8, 0), dt * 60); sub = 3                 // 0x141393982
       otherwise: Turn((0, +0xA8, 0), dt * 6); if sprinting: back to the base walk speed (x 5.4 acceleration)
sub 3 (hop off): pawn.vt+0x16C0(GetMaxWalkSpeed() * actor forward)         // velocity write every tick
       if IsAnimEnd: Fall(+0xA8) (iface +0x560); DoChain
sub 4 (end point): IsAnimEnd -> DoChain; or (+0xA5 and m_bEnableChange) -> DoChain
every tick: pawn.vt+0xDC8(dt * 6); +0xA5 = 0; +0xAC = 0
```

**End** 0x14138D300 [C]: if still attached: `StopMovement`, detach, keep the world location; leave Flying;
skip-CalcVelocity off; `ResetMaxWalkSpeed`; unless the next state is another Hopping: markers restored,
and if the clip playing is `HopR` / `HopL`: `SetMovementMode(3)`; the hassou variant gives the fall half of
its velocity. `m_LastHitHopComponent` is cleared unless the next state is HOPPING (9), DANGLE (6), or
(none / JUMP while not in battle) — so the point just left cannot be re-taken in the same airtime, and an
end point can be reached only with a previous point remembered.

Jump: gate 0x14142A3D0 HOPPING: sub ∈ {2, 3} [C, Part I]; `JumpRequest` uses yaw = Hopping+0xA8.

Motion summary [D]: a hop is a straight-line move of the capsule centre from where the pawn was to the hop
point, attached to the point (it follows a moving owner), in Flying mode without gravity; its duration is
at most the clip (0.4333 s) and shorter when the distance is less than `MaxWalkSpeed × 0.4333` (the
constant-speed branch: 600 cm/s walk → 260 cm, 980 cm/s sprint → 425 cm). The hop has no arc of its own:
the arc is the animation. Standing on a point, the stick (> 0.85) chooses the next point within
150…350 cm (+ 100 + `m_CheckRadius`) ahead; without one Sora hops off forward at walk speed and falls.

##### H4. Clips [A]

| Name | Asset | Length | Play | Notifies |
|---|---|---|---|---|
| `HopR` | 0113 | 0.4333 s (14 frames) | −1 / 0.2 / 0, mode 0 | `TresLandAnimPose` LIGHT_R at 0; auto-SE `step` at 0.033; no root motion; `m_InterpolationTime` 0.07 |
| `HopL` | 0114 | 0.4333 s | same | `TresLandAnimPose` LIGHT_L at 0; auto-SE `step` at 0.035 |
| `BalanceR` / `BalanceL` | 0111 / 0112 | 3.4667 s | −1, mode 3 | voice `bt1011940sr0` / `bt1011950sr0` at 0.070; auto-SE `step` |
| `Balance` | 0115 | 2.2667 s | −1, mode 3 (SURFING points) | same voice; auto-SE land |
| `BalanceBreak` | not in Sora's anim set | | | the play request returns length 0 |
| `IdleN`, `RollLand` | 0001, 0108 | | NORMAL points / ROLLING end points | Part I 5.5 |

The marker effect (`m_pHopEffect` of the pawn, `m_pFreeRunEffect` of the component, shown within
`MyHopEffectRange` 2000, `BP_SetDisableHopPointDisp`) was not read [?].

---

#### Corrections to Part I

Rail and Hopping below; the Athletic Flow corrections and the disagreements with `docs/sora-magic-spec.md` are listed in its section ("Athletic Flow — Corrections to Part I", "Disagreements with the magic spec").

| Section | Part I says | Correct |
|---|---|---|
| 10.1 (rail row), 5.4, 15 | rail probe callers: Idle / Jump / SlopeSlide / Swaying ticks | Idle, **Run** (0x1413AF427) and Jump; SlopeSlide and Swaying also probe but the gate 0x14142C800 accepts only state ids 0–3 (and COVER), so they never start a rail [I] |
| 10.6 | "Guard button: state vt+0x3A0 → Superslide" as a state decision; "Jump: the state decides (vt+0x390 / +0x3C0 …) or falls back" | for state 22 `TryFlow` itself pushes a plain Superslide after the gate (`SUPERSLIDE` + slot 116); the state-side Superslide (slot 121) is reached only for id 138. The on-rail jump (slot 114 / 120) exists only on rails with `m_JumpLimitType = RELATIVE`; on a default rail the jump always leaves it [I] |
| 10.6 | "Begin: bits 0, 3, 4 set, Flying" as the motion mode | Flying is only the label: while attached `StartNewPhysics` runs `PhysRailSlide` (slot 408) and moves the actor without sweep [I 0x140596ED4] |
| 10.6 | clip list includes `RailSlideTransfer` for the state | `RailSlideTransfer` needs `m_RailType` 6 (state 138) |
| 10 intro | "displacement is root motion or direct SetActorLocation / velocity writes by the state" | the rail state writes nothing: the movement component advances `m_RailSlideWork` (speed × dt along the spline) |
| 2.3 table, bit 0 | set again by "WallRun / RailSlide / SlopeSlide / Swim Begin" | confirmed for RailSlide [I 0x141509CF1]; also by `Hopping::Begin` [C] |
| 11.4 | hop points "and balance beams" | there is no beam object: `BalanceR/L`, `Balance` are the idle clips played while standing on a hop point (`m_IdleMotionType`) [C / A] |
| 11.4 | "`TryHopping` from `Idle::Begin`, the Run tick and the Jump tail" | also SlopeSlide, Swaying and WallRun ticks; the Run call needs stick magnitude ≥ 0.1 and passes the stick yaw [I] |
| 14 (rail probe row) | "segment actor → actor + (0,0,30), range 50 / 150" | plus the node's `m_DetectRangePlus`; the query is against a registry of rail nodes, not a collision channel [I] |

#### Host affordances

**Rail** (no physics channel is involved: the game iterates a registry of rail nodes):

- Data per node: location, rotation (quaternion), `m_RailType` (0), `m_StartDir` (0 BOTH / 1 / 2),
  `m_DetectRangePlus`, the ten action bits (`m_bDisableReverse` … `m_bIsNarrow`, `m_bIsDisableAction`),
  `m_JumpLimitType`, speed set (`m_bSetSlideSpeed`, forward / reverse speed), slope set (min / max rate,
  four acc / dec rates), `m_bActive`, the list of outgoing links (segment + target node, optional fixed
  index) and the back links.
- Data per segment: a world-space cubic Hermite curve between the two node locations with the two
  tangents (key 0…1), a distance → key table (length = last distance), enabled flag, bounds, the two end
  orientations.
- Queries the core needs from the host (or computes itself from that data): closest point of a segment to
  the probe segment (squared distance + key); location, direction and orientation at a distance; length;
  next node for a travel direction.
- Probe shapes: segment from the capsule centre to 30 cm above it; accepted when the squared distance to
  the spline ≤ (R + `m_DetectRangePlus`)², R = 50 on the ground (Idle, Run), 150 in the air (Jump); the
  travel direction is chosen by the sign of dot(spline direction, wanted direction) with wanted = stick
  direction (Run, Jump with stick) or facing; not within 150 cm of the end unless the rail continues.
- Placement rule: the spline is the path of the **capsule centre**. Minimal test rail: two nodes, one
  segment, all properties default → 2000 cm/s, both directions, jump and guard exits available, automatic
  launch at either end.
- Also needed: flowmotion unlock flag, `has_ability(SUPERSLIDE)`, `has_ability(SUPERJUMP)` /
  `DOUBLEFLIGHT` for the exits.

**Hop point**: a registry of hop components; per component: world transform, `m_AttachOffset`,
`m_BaseOffset1/2`, `m_CheckRadius`, active flag, order (flag + number), idle motion type, end-point flag +
end motion type, closing flag, owner handle (the pawn attaches to it and follows it). Probe: point test
`dist(pos, segment[base1, base2]) ≤ radius + m_CheckRadius` with pos = capsule centre (Idle, radius 100;
Jump, radius 220) or centre + forward × 120 (Run, radius 100), then "in front of the wanted direction or
inside `m_CheckRadius`", highest point first, nearest next. Placement rule: the component location (+
`m_AttachOffset`) is where the **capsule centre** rests. Minimal test: two components 300 cm apart,
defaults.

**Airstep target** (a `TresShootFlowComponent` / lock-on target component with the move-flow permission): subsection "Athletic Flow — Host affordances" above.

#### Tests

Assumptions: 60 Hz, every ability, flowmotion unlocked, plain Keyblade. "Tick n" counts `MainProc` calls
from the one in which the state begins (n = 0).

**Rail 1 — attach from a run and constant speed.** Level: nodes A (0, 0, 100) and B (3000, 0, 100), one
straight segment (tangents (3000, 0, 0)), defaults. Sora runs along +X on the ground so that his capsule
centre passes within 50 cm of the spline (centre height 100 → on the line). Expected: on the first Run
tick with the centre between x = 0 and x < 2850 the probe hits (distance ≤ 50; nearer than 150 cm to B
it is refused); state 22 begins in that `MainProc` (sub 0 → 1): facing snapped to yaw 0, attach at
`dist = x`, `RailSlideStart` playing, movement paused. Next movement tick: actor location = (x, 0, 100)
exactly. The clip ends after 0.2333 s (14 ticks at 60 Hz); from then on the location advances **33.333 cm
per tick** (2000 cm/s), `Velocity` is not what moves him (location deltas only). Movement component:
`MovementMode` 5, work+0x64 = 2000, work+0x68 = 1.

**Rail 2 — end of rail.** Continue test 1 without input. When `dist ≥ 3000` the work detaches in the
movement tick (location = B); in the following `MainProc` the state takes the LEAVE branch: a Jump state of
type 2 is pushed (`JumpSuperStart`, power 2500 with `SUPERJUMP`), Jump+0x100 = (1, 0, 0), +0x10C = 175,
+0x110 = 0.92, and the rail state finishes. Total time on the rail from x0: 0.2333 + (3000 − x0) / 2000 s
(x0 = 1000 → 1.2333 s, 74 ticks). The rail cannot be re-taken while Sora stays within 200 cm of it.

**Rail 3 — turn-around.** Attach as in test 1 at x0 = 500, wait 0.6 s after the start clip (stick neutral),
then hold the stick toward −X. Expected in that tick: reverse (work+0x1C bit 1), `dist' = 3000 − dist`,
actor yaw 180°, `RailSlideTurn` (0.3 s, paused), then 2000 cm/s toward A. Holding the stick toward +X now
does nothing until it has been neutral for one tick and 0.3333 s have passed since the loop restarted.
Holding the stick toward −X from before the attach: no turn until released once (bit 14).

**Rail 4 — jump off.** On the rail (sub 2), press jump. Expected: same frame `ClearQueue` + Jump pushed;
at that `MainProc` rail End (detach) and Jump Begin: type 2 (`JumpSuperStart`), launch direction = travel
direction in XY, inertial speed 175, brake 0.92; pawn+0xE48 bits 0, 3, 4 are set (from Begin), so a
Superslide out of this flow jump is allowed.

**Rail 5 — guard off.** On the rail press guard (stick neutral): state 21 Superslide begins next `MainProc`
(4000 → 3000 cm/s over 1.0 s per Part I 8) from the rail position; with the stick held the air-slide gate
is asked first by the controller order (Part I 2.1: `TryFlow` precedes `TryAirSlide`), so guard always
gives the Superslide when `SUPERSLIDE` is equipped.

**Rail 6 — node speeds.** Two nodes with `m_bSetSlideSpeed`, 1500 and 1800, 1000 cm apart: speed at
distance d = 1500 + 300 × d / 1000 cm/s forward; per-tick advance at d = 500: 27.5 cm.

**Rail 7 — air pick-up.** Jump so that the capsule centre passes 140 cm from the spline: attach (range
150); at 160 cm: none. With `m_DetectRangePlus` 150 (example level): 300 cm.

**Hop 1 — reach.** Hop component H1 at (500, 0, 100), defaults (radius 100). Sora idle at (310, 0, 100)
facing +X: `Idle::Begin` probes radius 100 → Check passes (190 ≤ 200) → state 9. At (290, 0, 100): fails
(210 > 200). Running toward it: the probe point is 120 cm ahead, so the hop starts when the centre is
within 320 cm. In the air: 320 cm from the centre.

**Hop 2 — motion.** From (310, 0, 100) (walk speed 600): relative start (−190, 0, 0); the constant-speed
branch covers 10 cm per tick, the clip branch 190 × t / 0.4333 = 7.3 cm per tick → arrival after 19 ticks
(0.3167 s), before the clip ends (26 ticks); then `BalanceL` (entered from Idle: `HopL`,
`m_bReverseLeg` = 1 → `BalanceL`) loops, `MovementMode` 5, actor location = (500, 0, 100).

**Hop 3 — chain.** Second component H2 at (800, 0, 100). On H1 push the stick toward +X at magnitude 1:
probe points at 150…350 cm ahead (650…850) with radius 100 → H2 found (first hit d = 150: |650 − 800| =
150 ≤ 200) → new Hopping state, `HopR`, blend-in −1, counter 2; H1 is `m_LastHitHopComponent` only until
the new Begin replaces it. Stick at 0.5: Sora only turns (rate dt × 6).

**Hop 4 — hop off.** On H2 with no further point, stick > 0.85: `HopL`/`HopR` (blend 0), Falling, velocity
= 600 cm/s forward for the 0.4333 s clip, then `Fall` → Jump sub-step 3; landing uses the land pose of the
hop clip (`LightLandR/L`).

Athletic Flow tests: subsection "Athletic Flow — Tests" above.

#### Still needs the running game

| Object | Offset | Type | Why |
|---|---|---|---|
| movement component | +0xA40 (u8), +0xA5C (u8), +0xA60 (i32), +0xA64 (u8) | flags | attached / paused / reversed / jump mode through attach, turn and end of rail |
| movement component | +0xA9C, +0xAA0, +0xAA4, +0xAA8 (f32) | | base speed, node offset, current speed, rate on a default rail (expect 2000, 0, 2000, 1) and on the bt_07 rail |
| movement component | +0xAC8, +0xACC, +0xAD0 (f32) | | distance on the segment, per-tick advance, segment length |
| pawn root | location z − spline z | f32 | confirms the capsule centre rides on the spline (offset work+0xC4 unused) |
| state RailSlide | +0x78 (i32), +0xC8 (u32), +0xCC (f32), +0xB8 / +0xBC (f32) | | sub-step, flags, turn lock-out, lean values |
| MyMesh | +0x1270 (u8) | bit 1 | what the mesh flag set by Begin does (leg IK / rail foot mode) |
| Jump state after a rail | +0x100 (vec3), +0x10C, +0x110 (f32), +0xA0 (i32) | | launch block and resolved type at the end of a rail |
| function entry 0x1405A7740 | return value, out+0x28, out+0x2C | | probe results while walking next to a rail (range, last-rail lock) |
| pawn | +0x668 (ptr) | | `m_LastHitHopComponent` life through hop → fall → land |
| state Hopping | +0x78, +0x98 (vec3), +0xB0 (u8), +0xB4 (i32), +0xB8 (f32) | | sub-step, start offset, moving flag, counter, speed |
| hop component | +0x870 (vec3), +0x894 (f32), +0x8A4 (u8), +0x8B0 (u32) | | attach offset and motion types of a point in a real level |
| hop marker effect | pawn+0xB3C (f32) | | reader of `MyHopEffectRange` not found |

---

## II.6 Dangle, Climbing, Slope Slide, Swaying, Swim

(strand `terrain`; working notes in `build/sora_tmp/actions/p2_terrain/`)

### Part II — terrain strand: Dangle, Climbing, Slope Slide, Swaying, Swim

Second pass, read in the machine code (`build/sora_tmp/actions/p2_terrain/d_<addr>.txt` are the dumps). Tags as in
Part I. **[I]** = the instruction was read (address given); **[C]** = control flow taken from the decompiled C, with
the constants it uses located in the disassembly where an address is given; where neither, the tag says so.
Conventions used below: `pawn` = state+0x68; "iface" = request interface at pawn+0x450 (vtable 0x14685EB18);
`cmc` = movement component pawn+0x578; the pawn's actor location is at the **feet** (capsule r 60, half height 89,
centre +91: movement spec 6). `Play(name, blendIn, mode)` = ANIM[0x18](name, slot 0, blendIn, mode, effect group 0,
rate 1.0, root scale default); after every Play the code calls ANIM[0x1F0](0) when the returned length is > 0.
`SWEEP(r, A → B, block {…})` = Square Enix multi-sweep 0x144526380(world, shape {0, r}, rotation, hits, A, B,
trace channel **2 (ECC_Pawn)**, query params (simple collision, ignore the pawn, plus the pawn root's own ignore list
0x1405CFAA0 where noted), response container, object params 0x149D86288) — a sphere of radius r (shape dword 0 = 0
followed by the radius; the meaning of the first dword was not decoded [?], every call passes 0).

Engine helpers identified in this pass [I]:

| What | Address | Does |
|---|---|---|
| pawn vt+0x9D8 | 0x1405D57D0 | "can the pawn's own body be at / move to": overlap test of the pawn root at `start` (blocking overlap → hit flags \|= 3, time 0), else `ComponentSweepMulti` of the root from `start` to `end` (retry once with a 0.25 cm pull-back); output `FHitResult` (0x88 bytes: +0 bit 0 blocking, bit 1 start-penetrating, +4 Time, +0xC Location, +0x18 ImpactPoint, +0x30 ImpactNormal, +0x64 actor, +0x6C component) |
| pawn vt+0xD20(hit) | 0x1405ADE40 → cmc vt+0x7A8 0x140589A60 | stock `IsWalkable(hit)`: blocking, not start-penetrating, ImpactNormal.Z ≥ 1e-4 and ≥ `WalkableFloorZ` (cmc+0x310, with the component's slope override) |
| component vt+0x4F0 / +0x520 / +0x550 / +0x580 | 0x1404ED7C0 / 0x1404ED790 / 0x1404ED810 / 0x1404ED7E0 | bits 2 / 5 / 6 / 8 of the dword at `UPrimitiveComponent`+0x854: `SQEX_bEnableDangle`, `SQEX_bEnableAutoRun`, `SQEX_bEnableSlopeSlide`, `SQEX_bEnableForceSlopeSlide` (setters vt+0x500, +0x530, …) |
| cmc vt+0x9E8 / +0x9F8 | 0x140592DD0 / 0x140592E10 | write Velocity XYZ (cmc+0x214..21C, copy at +0xC0C, flags +0xC18 \|= 3) / write Velocity XY only (flags \|= 1). Pawn vt+0x16B0 → 9E8, vt+0x16C0 → 9F8. **Settles Part I P6.** |
| cmc vt+0xA08 | 0x140592E40 | write Velocity.Z only (pawn vt+0x16D0) |
| cmc vt+0x500 | 0x140592BC0 | stock `RequestDirectMove(velocity, bForceMaxSpeed)` (pawn vt+0x1668) |
| cmc vt+0x530 / +0x538 / +0x540 / +0x548 | 0x1405892B0 … | IsFalling (mode 3) / IsMovingOnGround (mode 1–2) / IsSwimming (mode 4) / IsFlying (mode 5), each also requiring `UpdatedComponent` |
| `StartNewPhysics` table 0x14059703C | | mode 1 → vt+0x7B0 PhysWalking 0x1405900B0; 2 → vt+0x7B8; 3 → vt+0x6F0 PhysFalling 0x14058D0C0; **4 → vt+0x7C8 PhysSwimming 0x14058FC40**; 5 → vt+0x7C0 PhysFlying 0x14058E570; **6 (Custom) → vt+0x7D0 = empty stub**: no movement at all |
| pawn vt+0x17A0(mode, b) | 0x14054EAF0 | if b: stop movement immediately first (cmc vt+0x3F0 with flag +0x26C bit 3 cleared); then cmc `SetMovementMode(mode, 0)` |

---

#### Dangle (ledge hang, id 6) — `TresCharState_Dangle` (player class `TresPlayerState_Dangle` only wraps Begin / End)

Fields [I: Reset 0x14150DEA0, push 0x141444810]:

| Offset | Type | Meaning | Written by |
|---|---|---|---|
| +0x88 | AActor* | ledge actor | push |
| +0x90 | UPrimitiveComponent* | ledge component | push |
| +0x98 | vec3 | hang position of the pawn (feet), world | push; refined in Begin |
| +0xA4 | vec3 | the same in the component's local space (inverse of the component transform at +0x2A0 / +0x2B0 / +0x2C0) | push, when the component is alive |
| +0xB0 | f32 | yaw of the hang (facing the wall) | push |
| +0xB4 | u8 | stick active this frame | `MoveInput`; cleared at the tick end (0x14150F66B) |
| +0xB8 | f32 | stick yaw | `MoveInput` |
| +0xBC | u8 | variant flag: 1 = pushed from WALLRUN (go straight to the climb-up) | push |
| +0xBD | u8 | bit 0 component valid (local point usable), bit 2 pitch levelling wanted (wall-run variant) | push / Begin |

Reset: m_bEnableChange and bits 1–3, 5–10 of state+0x84 cleared (`and 0xFFFFF810`), sub-step 0, pointers null, vectors
zero, +0xBD &= ~7, +0xB0 = +0xB4 = +0xB8 = +0xBC = 0.

##### Request path

```
gate (iface slot 63, 0x141427830) [I]:
    !pawn vt+0xA48()                      // pawn+0x518 bit 0 (stick locked)
    && CanAct (pawn vt+0x29C0)
    && !(pawn+0x6F8 > 0)                  // dangle cooldown, pawn vt+0x2388 0x1405ADA00
    && next queued state (0x1405AB6C0) == none
    && current state:  IDLE (1) | WALLRUN (8) : yes
                       JUMP (3): sub-step 1 or 2: Velocity.Z <= 500   (0x141427905 comiss vz,500; jbe pass)
                                 otherwise     : Velocity.Z <= 0      (0x14142791D comiss vz,0; ja fail)
                       anything else: no
    && !(pawn+0xE60 bit 17)               // pawn vt+0x2788
TryDangle (slot 64, 0x14143A0D0)(actor, component, &pos, yaw, bKeep) [I]:
    gate; IDLE / JUMP: (!bKeep: ClearQueue 0x1405A8010) push(..., variant 0)
          WALLRUN:     (!bKeep: ClearQueue)             push(..., variant 1)
push (slot 65, 0x141444810)(actor, component, &pos, yaw, variant) [I]: PushState(6); fields as in the table
```

Callers [I]:

| Caller | Call | Probe arguments |
|---|---|---|
| Jump tick 0x141395450, tail (0x141397FE6–0x1413980C6) | gate slot 63, then the ledge probe, then **slot 65 directly** (not slot 64); on success `DoChain` | `probe(pawn, out, up 250, down 50, fwd 0, radius 100, debug = Jump+0x114 bit 8)` (constants at 0x14139805E / 72 / 7A) |
| WallRun tick 0x1413D87E0 (0x1413D9609) | `TryDangle` slot 64 (variant 1) | wall-run top edge (wall strand) |
| Hopping helper 0x14139EBF0 (0x14139F29E) | probe + slot 65 | hop strand |
| AthleticFlow 0x141331C40 (0x141331EA0) | probe(up = target dz + 300, down 300, fwd = distance − 45, radius 90) + slot 65 with variant 1 | flow strand |

Jump tail conditions [I]: pawn status byte (0x1405A9DD0) ∉ {0x0C, 0x11}; the tick's "airborne" flag (r15b, set in the
rising / falling sub-steps); `Jump+0xA8 > 0` → `Jump+0xA8 -= dt` and no test this tick (0x141397FEF–0x141398009), else
the gate. If the gate or the probe fails the code goes on to `TryHopping` (slot 184).

##### Ledge probe 0x140638740 → 0x140638900 (6.3 KB) [constants and tests I; vector arithmetic C]

`probe(pawn, out, up, down, fwd, radius, debug)`; `out` = {actor, component, vec3 pos, f32 yaw}. `L` = actor
location (feet), `F` = actor forward (full rotation), `O` = `m_DangleOffsetPos` (pawn+0x64C through pawn vt+0xE88;
blueprint (−60, 0, −150)).

```
precondition: pawn+0xA6 bit 1 && !(pawn+0x3B4 & 0x10) && pawn+0x3CC == -1          // 0x1406389D7..F0 [? actor flags]
query params: ignore the pawn; pawn root ignore list; default response container
attempt 0 (direction D = F), attempts 1 and 2 (D = F yawed +45 / -45 degrees: 0x140638C72 / 7C) — player pawns only:
    A = L + D*50 + (0, 0, up + 50);  B = A - (0, 0, down + 100)                    // 50: 0x140638B64; 100: 0x140638C31
    hits = SWEEP(r 50 (0x140638C4C), A -> B)                                        // straight down, 300 -> 150 above the feet
    take the first hit that is blocking, not start-penetrating, actor != pawn, component != null,
        component vt+0x4F0()  == SQEX_bEnableDangle            (0x140638ED2)
        IsWalkable(hit)       (pawn vt+0xD20, 0x140638EF1)
    dir  = normalize2D(hit.ImpactPoint - B)            // from the probe axis to the contact, horizontal
    yaw  = dir.Rotation().Yaw
    pos  = hit.ImpactPoint + RotateYaw(yaw) * (O * (1,1,1))    // 60 cm back from the contact, 150 below it
    attempt 1 stores dot1 = dir . D;  attempt 2 replaces an accepted attempt-1 result only if dir . D >= dot1 (0x1406391AB, jb keeps)
  SPACE test (0x1406395A2): response channel 0x18 set to block (0x14443F9F0(resp, 0x18, 2))
    SWEEP(r = |O.x| - 10 (= 50; 0x1406395BB/C2), (pos.xy, pos.z - O.z) -> (pos.xy, pos.z - 10))
    any blocking hit (start-penetrating, or of another actor) -> "Space:NG": attempt 0 -> 1 -> 2, after 2: fail
  GRIP test (0x140639B06..): SWEEP(r 5, (pos.xy, ledgeZ + 5) -> toward the contact horizontally, length dist2D - 2.5)
    any blocking hit -> "Grip:NG": next attempt as above
attempt 3 (only when attempt 0 found no ledge at all, or for a non-player pawn):
    A = L + F*fwd + (0, 0, up);  B = A - (0, 0, down);  SWEEP(r = radius, A -> B); same hit tests, same SPACE / GRIP
```

What a ledge is, in numbers [D from the above, Jump-tail arguments]: a component that blocks the **Pawn** channel
and has `SQEX_bEnableDangle`, with a walkable top. The probe sphere (r 50) drops from 300 to 150 cm above the feet on
the axis 50 cm in front of the pawn. With `d` = horizontal distance from that axis to the ledge edge (d < 50) and
`s = sqrt(2500 − d²)`, the top must lie at `150 − s ≤ h < 300 − s` above the feet (h ≈ 101–251 cm when Sora's body
touches the wall, d = 10; the upper limit is "not start-penetrating"). The edge must be within 100 cm of the pawn's
axis; Sora's own capsule radius (60) keeps the axis in front of the wall face, so the contact is on the edge and `dir`
points into the wall. A cylinder of radius 50 from the ledge top down to 10 cm below the hanging feet (160 cm) must be
free, and nothing may stand on the ledge within 5 cm above its top between the hang axis and the edge (a railing
blocks the grab). Fallback attempt 3: a sphere r 100 from 250 to 200 above the feet on the pawn's own axis.

##### Begin (player wrapper 0x1413685A0 → 0x141508DE0) [I]

Wrapper: pawn vt+0x2008(6, 0) (controller notification of the state id), the controller object 0x14150DAF0: if
vt+0x1828() then vt+0x1830(0); vt+0x1820(0) (camera-side resets, not decoded [?]).

```
if component null or pending-kill: ok = false
else:
    if +0xBD & 1: +0x98 = componentTransform * (+0xA4 * scale)             // follow a moving base
    rot = (0, +0xB0, 0)
    blocked = pawn vt+0x9D8(hit, +0x98, +0x98, rot)                        // body overlap at the hang point
    if blocked:
        v = rot * (-10, 0, 0)                                              // 0x141508FB7
        if pawn vt+0x9D8(hit, +0x98 + v, +0x98, rot):                      // sweep from 10 cm behind
            if hit start-penetrating:
                variant 0: ok = false
                variant 1: retry with the ledge point re-derived (- rot*O, + actorRot*O), back-off 16 (0x141509060),
                           pitch < 0 -> +0xBD |= 4; still penetrating -> ok = false
            else: +0x98 += v * clamp(1 - hit.Time, 0, 1)
    if variant 0 and component Mobility (+0x348) == Movable (2):
        ok = 0x1406385E0(pawn, FTransform(rot, +0x98, rootScale), 10.0)    // base support test for moving bases
    if ok (or variant 1): SetActorLocationAndRotation(+0x98, rot, no sweep, teleport) (0x143EAC700)
m_bEnableChange = 0
if variant 0 and !ok: sub = 3; return                                      // the tick chains out at once
StopMovement; pawn vt+0x1A78(0); pawn vt+0xDC8(1.0); Sheathe(1, 1.0)
variant 1: SetMovementMode(5 Flying, 1); Play("GripRise", -1, mode 2); sub = 2
variant 0: SetMovementMode(6 Custom, 1)                                    // empty Phys: Sora does not move
           pawn vt+0xA60(component, 0, 1)                                  // base / attachment 0x1405B88C0
           Play("GripStart", -1, mode 2) (through the pawn's anim interface pawn+0x440); sub = 0
```

So the snap is exact: feet at `contact + Rot(yaw)·(−60, 0, −150)`, rotation (0, yaw, 0), pushed out of the wall by at
most 10 cm along −forward if the body overlaps. Hanging uses movement mode **Custom (6)** whose Phys function is an
empty stub; a movable ledge carries Sora through the base attachment.

##### Tick 0x14150F220 [I]

```
sub 0: if IsAnimEnd: Play("GripLoop", -1, mode 3); sub = 1
       if !m_bEnableChange: (tail)                           // GripStart: TresEnableChange at 0.199 s
       -> input block
sub 1: input block
input block:
    if +0xB4 (stick, magnitude > 0.5):
        a = |wrap180(+0xB8 - actorYaw)|
        special = pawn vt+0x1A20()          // (controller byte +0x4E0 (pawn vt+0x1A18)) in {1, 2}  [? what the byte is]
        special:  a < 45  -> CLIMB          (0x14150F4D8 jb)
                  a >= 135 -> RELEASE       (0x14150F4E1 jb to "stay")
                  else     -> STAY
        !special: a >= 90 -> RELEASE        (0x14150F50B jae);  else CLIMB
    else STAY
    CLIMB:   pawn+0x6F8 = 0 (vt+0x2380); SetMovementMode(5 Flying, 1); Play("GripRise", -1, mode 2); sub = 2
    RELEASE: pawn+0x6F8 = max(pawn+0x6F8, 0.6) (vt+0x2378(0.6, 1), 0x14150F4F5); Fall(actorYaw) (iface slot 172); DoChain
    STAY:    pawn+0x6F8 = max(pawn+0x6F8, 0.6)               // every hanging tick (0x14150F5AA)
             movable component: if !0x140638690(pawn, 10.0): Fall(actorYaw); DoChain     // the base moved away
sub 2: if +0xBD & 4: Turn((0, actorYaw, 0), dt * 6)          // level the pitch
       if IsAnimEnd: DoChain
sub 3: DoChain
tail:  +0xB4 = 0
```

`MoveInput` (iface slot 35, 0x141440600) DANGLE branch [I]: only when the stick magnitude > 0.5 (0x1414406D0
`comiss mag,0.5; jbe`): +0xB8 = stick yaw, +0xB4 = 1.

- **Climb-up**: Flying mode, `GripRise` root motion (100 cm forward, 200 cm up in 0.5 s; mode 2). From the hang
  position Sora ends 40 cm past the contact point and 50 cm above the ledge top [D]; the state chains at the clip end
  to the default state (Idle), whose tick calls `Fall` while not on the ground → Jump sub-step 3 → lands.
- **Jump press** while hanging: jump gate 0x14142A3D0 DANGLE branch [C]: (sub 0 with m_bEnableChange) or sub 1;
  `JumpRequest` 0x14143D8F0 DANGLE branch [C]: yaw = actor yaw, type 0, **bFall = 1**, bAir = 1 → a Jump pushed at
  sub-step 3: Sora simply drops (no upward launch). Jump+0xA8 is 0 in that Jump, but the cooldown blocks a re-grab.
- **Cooldown** pawn+0x6F8: set to 0.6 on every hanging tick and on release, zeroed by the climb; decremented in the
  pawn tick 0x1405BC470 (0x1405BC4DF: `max(t − dt, 0)`); read only by the gate. So after letting go (stick away, jump
  press, End) a ledge can be grabbed again 0.6 s later; after a climb-up immediately.

##### End (0x14136B710 → 0x14150BB80) [I]

Wrapper: pawn vt+0x2010(6, 0). `SetMovementMode(3 Falling, 0)`; if the next state is none or JUMP and the battle flag
is set, or the next state is anything except DANGLE / HOPPING: if pawn vt+0x1360() (weapon away and kind ∉ {1, 0x16,
0x17}) → Draw(1, 1, 1.0). For the plain Keyblade (kind 1) nothing is drawn.

##### Clips [A]

| Name | Seq | Length | Mode | Blend-in | Root motion | Notifies |
|---|---|---|---|---|---|---|
| `GripStart` | 0035 | 1.8 s | 2 | −1 (clip default 0.07) | none | **`TresEnableChange` 0.199**, SE `se00001_001` 0.053 (`L_ude2`), Bonamik ground collision |
| `GripLoop` | 0036 | 1.3333 s | 3 | −1 | none | — |
| `GripRise` | 0037 | 0.5 s (16 frames) | 2 | −1 | fwd 100, up 200; per frame (fwd, up): (0,0) (0,0) (−0.2,25.8) (−0.1,79.4) (3.4,131.6) (12.2,172.7) (23.6,198.1) (37.4,212.5) (51.3,223.6) (65.6,231.4) (78.9,236.1) (89.6,237.4) (97.3,235.2) (99.6,227.6) (100,215) (100,200) | SE `se00001_002` 0.006 |

---

#### Climbing (vault, id 7) — `TresPlayerState_Climbing`

Fields [I: Reset 0x141352DA0, `TryClimbing` 0x1414398D0]:

| Offset | Type | Meaning |
|---|---|---|
| +0x88 / +0x90 | AActor* / component* | obstacle |
| +0x98 | vec3 | top point of the obstacle (probe output); **+0xA0 = its Z** is the lift target |
| +0xA4 | vec3 | actor location at Begin |
| +0xB0 | f32 | approach yaw (probe output +0x1C) |
| +0xB4 | f32 | obstacle height above the feet (probe output +0x20) |
| +0xB8 | f32 | lift factor: 1.0 at Reset, **0.5** from Begin (0x14134F424) |
| +0xBC | f32 | stick yaw (`MoveInput`) |
| +0xC0 | u32 | bit 0 high obstacle (height > 150), bit 1 middle (100 < height ≤ 150), bit 2 stick forward this frame, bit 3 chained vault |

##### Switch and tunables [I]

`m_bEnableClimbing` = pawn+0xB44 bit 0 (getter pawn vt+0x2610, 0x14110E780). The constructor clears it
(0x141423B5A `and [+0xB44], ~3`). **No native code writes it** (scan of 0x1405A0000–0x1405F0000, 0x141100000–
0x141115000, 0x141420000–0x141460000: only the constructor and the getter touch +0xB44); Sora's blueprints do not
override it, and no cooked package contains the string `m_bEnableClimbing` (full-tree search of
`assets_converted/packages` [A]). In the shipped data the vault is therefore **never enabled**; it is reachable only
through the reflected property (or `DebugSetClimbingParam`). A port exposes it as a host switch, default off.

| Property (pawn) | Offset | Getter | Value (native ctor 0x141423210; no blueprint override) | Used for |
|---|---|---|---|---|
| (int, `DebugSetClimbingParam.InVersion`) | +0xB48 | — | 3 (0x141423B61) | probe variant ("version") |
| `m_fClimbingRadius` | +0xB4C | vt+0x2618 | 35 | top-surface sweep radius |
| `m_fClimbingBottom` | +0xB50 | vt+0x2620 | 44 | minimum obstacle height |
| `m_fClimbingFront` | +0xB54 | vt+0x2628 | 150 | face-search reach |
| `m_fClimbingFrontFar` | +0xB58 | vt+0x2630 | 210 | forward clearance at full speed (version 3) |
| `m_fClimbingFrontNear` | +0xB5C | vt+0x2638 | 120 | forward clearance at low speed (version 3) |
| `m_fClimbingFinalSpace` | +0xB60 | vt+0x2640 | 59.5 | landing-space sweep radius |

##### Request path [I]

```
gate (slot 174, 0x141427630)(out): next queued == none && !(pawn+0xE60 bit 17) && obstacle probe 0x140634E60(pawn, out)
TryClimbing (slot 175, 0x1414398D0)(yaw, bReverseLeg):
    out.yaw = yaw; gate(out)
    if yaw != 0.0 (ucomiss, 0x14143993E): Turn((0, yaw, 0), 1.0)          // snap; a yaw of exactly 0 means "keep facing"
    st = push (slot 176, 0x1414447B0: PushState(7))
    st+0x88.. = out.actor, out.component, out.point; st+0xB0 = out.yaw; st+0xB4 = out.height
    st+0xC0 bit 0 = out.bHigh; bit 1 = out.bMiddle;  st.m_bReverseLeg = bReverseLeg
```

Callers [I]:

| Caller | When | Arguments |
|---|---|---|
| Run tick 0x1413AED70 (0x1413AF5EC) | sub-step 0; Run+0xD4 bit 3 clear (if set it is cleared and the test skipped once); stick magnitude > 0.5 (0x1413AF586); the playing clip is not `IdleN` / `IdleLockon` / `IdleB` | (Run+0x8C stick yaw, m_bReverseLeg); success → DoChain; failure → `TryHopping` |
| SlopeSlide tick 0x1413D1EA0 (0x1413D22DF) | sub-step 0, every tick | (0.0, 0) → DoFinish |
| Swaying tick 0x1413D8110 (0x1413D84F3) | sub-step 0, after slope slide and hopping | (Sway+0x98, m_bReverseLeg) → DoFinish |
| Climbing tick (0x14135394A) | chained vault, direct call of the probe | out.yaw = +0xBC |

##### Obstacle probe 0x140634E60 (7.5 KB) [tests and constants I; vector arithmetic C]

A pawn that is not a `TresPlayerPawnBase` (and a null pawn) does not fail here: both branches (0x140634EEB null test,
0x140634F26 IsA test) jump to 0x140636B76, which tail-calls **FUN_140636BF0(pawn, out)** = the NPC climb probe (its
own sphere sweep, no `m_bEnableClimbing` gate; ported in kh-party `climb::climb_probe`) and returns its answer. A
`TresPlayerPawnBase` whose `m_bEnableClimbing` (vt+0x2610, 0x140634F54) is false returns false (0x14063575F); only
then do the seven steps below run (corrected 2026-10-15, party I1). `ver` =
pawn+0xB48. `L` = feet, `F` = forward of the actor rotation, or of (0, out.yaw, 0) when out.yaw != 0.

```
1 floor slope. response: ignore all, block channels 0 (WorldStatic), 1 (WorldDynamic), 0x12
    G1 = first hit of SWEEP(r 5, L + (0,0,50) -> L - (0,0,50)) whose component has SQEX_bEnableAutoRun (vt+0x520, 0x1406353A7)
    G2 = the same at L + F*10 (0x14063541D)                    // not found: G = L / G1 respectively
    R  = rotation of (G2 - G1);  if its pitch >= 50 degrees: fail        (0x140635723: comiss -pitch,-50; jbe fail)
2 reach = ver > 2 ? Near + (Far - Near) * clamp01((|cmc+0x9A0 .xy| - Near) / (680 - Near)) : Front     // 680: 0x14063586C
    (cmc+0x9A0: a velocity vector of the component, pawn vt+0xE28 [? which: probe])
3 headroom. response: block 0, 1, 0x12, 0x18
    SWEEP(r 50, L + (0,0,Bottom + 50) -> L + (0,0,300)): any blocking hit of another component -> fail   // 300: 0x1406358E7
4 forward clearance: P = L + R*(-50,0,0) + (0,0,275);  SWEEP(r 50, P -> P + R*(reach,0,0)): any hit -> fail   // 275: 0x140635B85
5 face search: r = (ver == 1) ? Radius : 50;  heights tried in order (0x140635E30.., 0x140636296..):
        h = 225 - r;  [ver == 1 only: h = Bottom/2 + 112.5];  h = r + Bottom;  [ver >= 2 only: r = 5, h = Bottom + 5]
    for each: SWEEP(r, L + R*(-50,0,0) + (0,0,h) -> that + R*(Front,0,0)); a start-penetrating hit aborts the height
      take a hit of another component whose ImpactNormal is at least 50 degrees from vertical (0x14063610D)
      dir = normalize2D(ImpactPoint - Location);  require dir . F >= 0.707107 (0x140636241)      // within 45 degrees
      out.point = ImpactPoint; out.yaw = dir yaw; out.actor / component
6 top surface: SWEEP(r = Radius 35, (point.xy, L.z + 225 + Radius + dzSlope) -> (point.xy, L.z + Bottom + Radius + dzSlope))
      (dzSlope = Z of R*(distance2D,0,0): the floor slope carried to the obstacle)
      take a hit, not start-penetrating, whose component has SQEX_bEnableAutoRun (0x14063662B) and IsWalkable(hit) (0x14063664F),
      H = ImpactPoint.z - L.z - dzSlope;  require Bottom <= H <= 225 (0x14063669D ja / 0x1406366A7 jb fail)
      and the top no steeper than 50 degrees (0x14063671F)
      out.point = ImpactPoint (top); out.yaw = yaw of normalize2D(top - L); out.height = H
      H > 150 -> out.bHigh (0x140636948);  else H > 100 -> out.bMiddle (0x140636958)
7 landing space (only if FinalSpace > 0, 0x14063696E):
      SWEEP(r = FinalSpace 59.5, (top.xy, top.z + 120) -> (top.xy, top.z + 90))       // 90: 0x14063698A, +30: 0x1406369AD
      a start-penetrating hit -> fail; ver < 3: any blocking hit of another component -> fail (0x140636A88)
return true (0x140636B6F)
```

So a vaultable obstacle is geometry flagged **`SQEX_bEnableAutoRun`** (the floor Sora stands on and the obstacle
top), 44–225 cm high with a walkable top, whose near face is within about 100 cm ahead (the 150 cm sweep starts 50 cm
behind the feet), faced within 45°, with 300 cm of headroom over Sora and a clear path 275 cm up for 70–160 cm ahead.

##### Begin 0x14134F120 [I]

```
pawn must be a TresPlayerPawnBase
StopMovement; SetMovementMode(5 Flying, 0); gravity off (pawn vt+0x1728(0))
+0xA4 = actor location;  weaponOut = pawn vt+0x1330()
if +0xC0 & 1 (high):  m_bReverseLeg = 0; sub = 0; name = selector(3)
else:                 sub = 2;             name = selector((+0xC0 & 2) ? 2 : 1)
len = Play(name, blendIn 0, mode 2)                                  // 0x14134F33F xorps xmm3
if len == 0: m_bReverseLeg = 0; sub = 0; Play("ClimbingH", 0, mode 2)
m_bEnableTraction = 1; m_bEnableChange = 0; +0xB8 = 0.5
selector = pawn vt+0x1CA0 (0x1414334C0)(class, weaponOut, bReverseLeg):
    class 1: ClimbingR / ClimbingL (reverse leg);  2: ClimbingMR / ClimbingML;  3: ClimbingH;  weapon out: the same names + "B"
```

(`ClimbingRB` … `ClimbingHB` are not in Sora's anim set → length 0 → the fallback `ClimbingH` plays with the weapon
out [A].)

##### Tick 0x1413533B0 [I]

```
sub 0 (high) and sub 2 (low / middle) — same code:
    if IsAnimEnd: m_bEnableTraction = 0
    if m_bEnableTraction:
        dz = +0xA0 - actorZ
        if dz > -1:                                    // 0x141353530 comiss dz,-1; jbe -> traction off
            v = GetVelocity()  (pawn vt+0x2B8)
            if dz >= 5: dz *= +0xB8 (0.5)              // 0x141353549 jb skips
            v.z += dz;  cmc RequestDirectMove(v, false)        // pawn vt+0x1668
            -> tail
        m_bEnableTraction = 0
    SetMovementMode(3 Falling, 0); gravity on; sub = 3      // sub 2 runs the sub-3 body in the same tick
sub 3: if IsAnimEnd:
           if !OnGround: if Fall(actorYaw) (slot 172): DoChain          // skips the tail
           StopMovement; SetMovementMode(1 Walking, 1)
           name = landing selector pawn vt+0x1C98(landPose (vt+0x1788), weaponOut)     // 1 Land, 2 LightLand, 3 LightLandR, 4 LightLandL (+B)
           if none: name = vt+0x1C98(m_bReverseLeg ? 4 : 3, weaponOut)
           Play(name, -1, mode 2); m_bEnableChange = 1; sub = 4
sub 4: if IsAnimEnd or (+0xC0 & 4): finish = true
tail (every sub-step):
    if m_bEnableChange && actor XY != +0xA4 XY && (+0xC0 & 4):
        out = {yaw: +0xBC};  if probe 0x140634E60(pawn, out):
            copy out as TryClimbing does; m_bReverseLeg = !m_bReverseLeg; +0xC0 = (+0xC0 & ~7) | flags | 8
            Begin()  (vt+0x300, slot 96);  return                         // chained vault
    +0xC0 &= ~4
    if finish: DoChain
```

`MoveInput` CLIMBING branch [I]: stick magnitude > 0.5 (0x141440748) and |wrap180(stick yaw − actor yaw)| < 75
(0x141440875 `jae` skips): +0xC0 |= 4, +0xBC = stick yaw.

The vault clips carry `TresEnableChange` (0.2 / 0.3 / 0.4 s) and the default handler (0x1411FBA40) sets
m_bEnableChange, so a held stick chains a second vault from that time on (if the probe finds a next obstacle), and
the jump / dodge gates that test m_bEnableChange see it. The clips carry `TresLandAnimPose` LIGHT_R / LIGHT_L, which is
what the landing selector returns (`LightLandR` after R / MR / H, `LightLandL` after L / ML).

Motion [I for the writes, D for the path]: Flying mode without gravity; forward from the clip's root motion (250 cm,
with an up–down arc of 82 / 110 / 161 cm in the root track); on top of it, each tick while "traction" the requested
velocity is the current velocity plus (0, 0, 0.5 × remaining height) (the full remainder under 5 cm) — a per-tick
feedback, frame-rate dependent. Traction ends when the feet are more than 1 cm above the obstacle top (or the clip
ends); from then on Falling with gravity while the clip finishes. The exact trajectory through `PhysFlying` with a
requested velocity and root motion was not derived (R-probe T2).

##### End 0x141350570 [I]

```
if cmc IsFalling:  v = cmc+0x9A0;  if v.z > 0: SetMovementMode(3,0); Velocity = v * 0.5
                   else: SetMovementMode(3,0); w = cmc+0x9C4; if |w| > 0: Velocity = w * 0.5        // 0.5: 0x14135066E / 0x1413506AB
else: SetMovementMode(1 Walking, 1)
gravity on;  if battle flag: last pose = BATTLE (pawn vt+0x1780(1))
```

##### Clips [A]

| Name | Seq | Length | Root (fwd; up peak) per frame (fwd, up) | `TresEnableChange` | Land pose | Other |
|---|---|---|---|---|---|---|
| `ClimbingR` / `ClimbingL` | 0105 / 0106 | 0.3667 s (12 f) | 250; (0,0) (37.3,38.1) (80,75) (101,81.1) (119.2,82) (135.3,81.2) (150,78.5) (165.4,74) (183.6,62) (203.7,37.4) (225.6,12.6) (250,0) | 0.2 | LIGHT_R / LIGHT_L | interp 0.03 |
| `ClimbingMR` / `ClimbingML` | 0117 / 0118 | 0.5 s (16 f) | 250; (0,0) (40.7,59) (80,100) (94.2,105.8) (106.2,109.2) (116.9,110.3) (127.2,109) (137.9,105.6) (150,100) (163.6,92.8) (178.2,84.2) (193.1,73.6) (208.2,62.1) (223,47.9) (237,26.6) (250,0) | 0.3 | LIGHT_R / LIGHT_L | SE `se00001_005` 0 |
| `ClimbingH` | 0107 | 0.5333 s (17 f) | 250; (0,0) (19.6,50.9) (50.3,114.2) (80,150) (91.6,155.5) (102,159.1) (111.4,160.7) (120.5,160.5) (129.6,158.6) (139.3,155) (150,150) (162.4,142) (176.9,129.9) (193.5,113) (215.7,75.7) (238.4,27.5) (250,0) | 0.4 | LIGHT_R | effect `vs_wrn_smk0`, voice `bt1011830sr0` / `bt1011840sr0`, SE `se00001_002` |
| `LightLandR` / `LightLandL` | 0109 / 0110 | 0.6333 s | none | 0.0989 / 0.1005 | — | `TresLastAnimPose` 0, `TresIK` from 0.30 / 0.33 |

---

#### Slope Slide (id 10) — `TresPlayerState_SlopeSlide`

Fields [I: Reset 0x1413D0BA0]:

| Offset | Type | Meaning |
|---|---|---|
| +0x88 | u8 | stick active this frame (`MoveInput`: any magnitude); cleared at the tick end |
| +0x89 | u8 | sliding (pitch follows the slope); 1 from Begin, 0 when the end clip starts |
| +0x8A | u8 | weapon-form variant (pawn vt+0x1B48 at Begin: weapon out and kind ∈ {7, 9, 10}); 0 for the Keyblade |
| +0x8C | f32 | target yaw: actor yaw at Begin, then the stick yaw |
| +0x94 | f32 | speed increment per tick |
| +0x9C | f32 | slope angle of the last probe, degrees (negative = downhill) |
| +0xA0 | f32 | time in state (+= dt) |
| +0xA4 | f32 | end speed (form variant) |

##### What makes a floor a slide slope — probe 0x14063D9C0 [tests and constants I; arithmetic C]

Called by the gate (iface slot 187, 0x14142DE50: next queued == none && !(pawn+0xE60 bit 17) &&
`probe(pawn, out, 0, zero)`). `out`: +0x28 f32 angle, +0x2C u8 forced, +0x2D u8 "no slide floor here".

```
response: ignore all, block channels 0, 1, 0x12;  all sweeps r 5 (0x14063DD09)
P1 = first blocking hit of SWEEP(L + (0,0,50) -> L - (0,0,50)) whose component has SQEX_bEnableSlopeSlide
        (component vt+0x550 = bit 6 of component+0x854; 0x14063DE7A);  none: out.angle = 0, out+0x2D = 1, fail
limit = -25 degrees, minDot = 0.642788 (cos 50)                                   // 0x14063DF73 / 7C
if the component also has SQEX_bEnableForceSlopeSlide (vt+0x580 = bit 8, i.e. byte +0x855 bit 0; 0x14063DF8A):
        out.forced = 1; limit = -15; minDot = 0.173648 (cos 80)                   // 0x14063DF9C / A5
D = fall line of the hit: the ImpactNormal's rotator with pitch - 90, X axis
require atan2(D.z, |D.xy|) in degrees <= limit                                    // 0x14063E01B
require (forward of (0, actorYaw, 0)) . D >= minDot                               // 0x14063E12F: facing downhill
P2 = the same floor sweep at P1 + F*10 (0x14063E13D); none: out.angle = 0, fail
out.angle = atan2(P2.z - P1.z, |P2.xy - P1.xy|) in degrees; require <= limit      // 0x14063E402
clearance: SWEEP(P1 + (0,0,10) -> along (P2 - P1) for 120 cm (50 when forced; 0x14063E412)): any blocking hit -> fail
```

A slide slope is therefore a floor **component flagged `SQEX_bEnableSlopeSlide`** whose surface under Sora descends
at 25° or more along its fall line, with Sora facing downhill (within about 40–50° of the fall line) and 120 cm clear
ahead. With `SQEX_bEnableForceSlopeSlide` in addition: 15°, almost any facing (80°), 50 cm clear, and the slide starts
regardless of the state rules below. Nothing reads the physical material, and the walkable-angle limit is not part
of the test (a slide floor must still be walkable, 49° max, for `PhysWalking` to keep Sora on it — steeper flagged
floors make him fall, not slide [D]).

##### Request path [I]

```
TrySlopeSlide (slot 188, 0x141441490)(bRunArg):
    latch = pawn+0xD80 bit 2 (pawn vt+0x2920)
    if gate(out):
        start = out.forced
             || current IDLE (1)  && latch
             || current RUN (2)   && (bRunArg || latch)
             || current JUMP (3)                          // and newLatch = !latch
             || any other state
        if start && push (slot 189, 0x1414458E0: PushState(10)):
            pawn+0xD80 bit 2 = out.forced || (JUMP && !latch) ; return true
    // not started:
    IDLE / RUN: latch = 0;   JUMP && latch: latch = out.forced;   return false
```

`Begin` then sets the latch to 1 (0x1413CB6DD). So, on a non-forced slope: landing on it always slides; running or
standing on it slides only while the latch from a previous slide is still set (it is cleared by the first failed test
in Idle / Run), or — from Run — when the Run tick passes `bRunArg`.

Callers [I]: Idle tick 0x1413944D0 (0x141394861, arg 0, after the rail test); Run tick (0x1413AF4C8, arg =
`Run+0x98 > 0 || Run+0x94 == 0` — 0x1413AF495..AB); Jump tick landing path (0x1413971AC, arg 0); Swaying tick sub 0
(0x1413D848C, arg 0, unless Sway+0xA9).

##### Begin 0x1413CB400 [I]

```
Keyblade: Sheathe(1, 1.0); Play("SlopeSlide", blendIn 0.1, mode 1)                 // 0x1413CB526
(form variant: +0x8A = 1; loop clip from selector pawn vt+0x1C70, -1, mode 1)
+0x89 = 1
T = MySlopeSlideAccelTime (pawn+0xC0C, vt+0x1B20) * 60                              // 0x1413CB586
if T > 0: +0x94 = (MySlopeSlideSpeed (pawn+0xC08, vt+0x1B18) - |Velocity| (3-D)) / T
+0x8C = actor yaw;  latch (pawn+0xD80 bit 2) = 1
pawn+0xE48 bits 0, 3, 4 set (vt+0x1E58 / 1E88 / 1E98 (1))
SetMovementMode(1 Walking, 0)
mesh (pawn+0x580) 0x1405D6E50(mesh, 1, 1);  mesh vt+0xE40(20.0)                     // slope alignment of the mesh [? not decoded]
```

Tunables: `MySlopeSlideSpeed` pawn+0xC08 native 680 → blueprint **1600**; `MySlopeSlideAccelTime` pawn+0xC0C →
**0.5**; `MySlopeSlideMinSpeed` pawn+0xC10 (vt+0x1B28) native 340 → **680** [A].

##### Tick 0x1413D1EA0 [constants I; flow C]

```
if (in water (pawn vt+0x1A80) && PushState(SWIM)) || pawn vt+0x1B48() != +0x8A: DoChain; return
dir = +0x88 ? forward of (0, +0x8C, 0) : actor forward
rail: pawn vt+0xEA0(L, L, MyRailSlideCheckRange (pawn+0xBF8, 50), dir, out, 1) && TryRailSlide (slot 75)(out, 1, 0): DoChain; return
speed = |Velocity| (3-D);  minSpeed = MySlopeSlideMinSpeed
sub 1 (end clip): if IsAnimEnd || (+0x88 && m_bEnableChange): DoFinish; return
                  Keyblade: return        (form variant: +0xA4 *= 0.98 (0x1413D225E), velocity XY = +0xA4 * facing)
sub 0:
    if TryClimbing(0.0, 0) || TryHopping(+0x8C, 0, 0): DoFinish; return             // 0x1413D22DF / 0x1413D2305
    if |cmc+0x9A0| < speed * 0.5: END                                               // blocked; 0x1413D2334
    ok = continuation probe 0x14063D1F0(pawn, out{angle: +0x9C})
           // the two floor sweeps of the start probe; out.angle = slope over the next 10 cm;
           // ok while angle < -15 (forced floor: < 0); 0x14063D98F
    if out+0x2D (no slide floor underfoot) && Fall(...) (slot 172): DoChain; return
    if ok: +0x9C = out.angle
    else if +0x9C >= 30: END                                                        // 0x1413D268F comiss 30,angle; ja continues
    else if +0x88 && speed < 980: TryRun(+0x8C, 1.0) (slot 40); DoChain; return     // 0x1413D28EF
    else if speed >= minSpeed: +0x9C = out.angle                                    // 0x1413D292D jae: keep gliding
    else END
END: +0x89 = 0; sub = 1; StopMovement; Play("LightLandR", blendIn 0.1, mode 2); SetActorRotation((0, actorYaw, 0))
     (form variant: +0xA4 = |cmc+0x9A0 .xy|, clip from selector vt+0x1C78)
TAIL (sub 0 that did not return, and END):
    targetPitch = +0x89 ? +0x9C + 25 : 0                                            // 0x1413D2A91
    q  = lerp-normalize(actorQuat, Quat(targetPitch, +0x8C, 0), 0.02)               // 0x1413D2B17: yaw eases 2 % per tick
    q2 = lerp-normalize(q, Quat(targetPitch, q.yaw, q.roll), +0x9C > -5 ? 0.3 : 0.03)     // 0x1413D2AA0..B2: pitch
    SetActorRotation(q2)  (0x143EAD160)
    if !ok:  dec = 2.0, target = 0;  forced floor: dec = 0.5, and target = minSpeed when +0x9C < 0      // 0x1413D2D4F..6A
             if speed > target: speed = max(speed - dec * +0x94, target)
    else:    speed = min(speed + +0x94, MySlopeSlideSpeed)                          // 0x1413D2D41
    MaxWalkSpeed = speed  (pawn vt+0x1570)
    Velocity = normalize(Velocity) * speed       (cmc vt+0x9E8: keeps the current 3-D direction)
    Velocity.XY = speed * forward of (0, actorYaw, 0)   (cmc vt+0x9F8)
    +0x88 = 0;  +0xA0 += dt
```

`MoveInput` SLOPESLIDE branch [C]: +0x8C = stick yaw, +0x88 = 1 for any active stick (magnitude ≥ 0.1).

Model [D]: Walking mode; the state overwrites the velocity every tick: magnitude `speed`, horizontal direction = the
actor's facing. `speed` starts at the entry speed and gains `+0x94 = (1600 − v0) / 30` **per tick** (not per second:
1600 is reached after 30 ticks = 0.5 s at 60 Hz and the ramp scales with the frame rate), capped at 1600. Steering:
the facing yaw moves 2 % of the remaining error toward the stick yaw per tick (time constant ≈ 0.82 s at 60 Hz);
without the stick the target yaw stays. The body pitches to slope angle + 25°. When the floor ahead flattens (probe
angle ≥ −15°) the speed drops by `2 × +0x94` per tick: with the stick held Sora breaks into a run as soon as the
speed is under 980; without the stick he glides until the speed is under 680 and then stops dead with `LightLandR`
(0.633 s; the stick ends it from `TresEnableChange` 0.099 s). A slide that is blocked (actual movement under half of
the commanded speed) ends the same way.

Jump: jump gate 0x14142A3D0 SLOPESLIDE: OnGround [C]; `JumpRequest`: yaw = +0x8C, type 0 (1 with HIGHJUMP) [C].

##### End 0x1413CE320 [I]

pawn vt+0xB38(0) (cmc flag +0xA3C bit 0 cleared); `ResetMaxWalkSpeed`; actor pitch set to 0; mesh
0x1405D6E50(mesh, 0, 1) and mesh vt+0xE40(0.0); form variant with nothing queued: pawn+0xD88 = +0xA4; next state ∉
{none, JUMP, DANGLE, HOPPING} and pawn vt+0x1360(): Draw(1, 1, 1.0).

##### Clips [A]

| Name | Seq | Length | Mode / blend-in | Notifies |
|---|---|---|---|---|
| `SlopeSlide` | 0103 | 0.5333 s | 1 (loop) / 0.1 | `TresIK` whole clip (target alpha 0.3), auto SE surface table record `slide` |
| `LightLandR` | 0109 | 0.6333 s | 2 / 0.1 | `TresEnableChange` 0.0989 |

---

#### Swaying (side-step around a pawn, id 11) — `TresPlayerState_Swaying`

Fields [I: Reset 0x1413D0EA0, `TrySway` 0x141441B20]: +0x88 other pawn; +0x90 i32 side (1 → `RunSwayR`, 2 →
`RunSwayL`); +0x94 u8 stick active this frame (1 from the request; `MoveInput` sets it for any active stick; cleared
at the tick end); +0x98 f32 travel yaw (the Run yaw at the request, then the stick yaw); +0xA9 u8 skip-slope-test
flag (cleared each tick).

##### The caller of `TrySway` [I] — found

The request comes from the **Run state's handler of slot 8 (+0x40) of the state's second interface** (the sub-object
at state+0x48; Run's vtable for it is 0x14693BC28): 0x1413B2960(iface, ctx, otherActor). Swaying has its own handler
in the same slot (0x1413DC2F0, vtable 0x14694C690); Idle's (0x141273E70) only refreshes its pawn pointer. Which engine
event fires slot 8 (a body-collision contact with another actor, by its argument) was not traced [? S1].

```
Run handler 0x1413B2960:
    pawn is a TresPlayerPawnBase; otherActor valid; Run+0xAA == 0; controller present
    |Velocity.xy| >= 0.9 * GetMaxWalkSpeed()                  // 0x1413B2AB4..BC, jb fail
    pawn vt+0x28B0() || !weaponOut
    pawn type byte (pawn+0x5F0, vt+0xD70) not in {2, 3, 6, 7}
    Run+0x94 == 0;  stick magnitude > 0.5 (0x1413B2B52);  Run sub-step == 0
    if TryHopping(0.0, 0, 1) (slot 184): DoFinish
    else if TrySway(Run+0x8C, otherActor) (slot 178): DoChain
Sway handler 0x1413DC2F0 [C]: the same speed and stick tests, sub-step 0, m_bEnableChange, otherActor != +0x88 -> TrySway -> DoChain
```

```
gate (slot 177, 0x14142E570)(other, &side) [I]:
    pawn type byte == 1 (vt+0xD70);  character kind (pawn+0x15B4, vt+0xD80) != 7
    next queued == none;  !battle flag (vt+0x1740)
    other vt+0x1980()                         // TresCharPawnBase base implementation returns true (0x14041F1E0)
    !(pawn+0xE60 bit 17)
    d = normalize2D(other.location - L);  F = actor forward
    F.x*d.x + F.y*d.y >= 0.643               // 0x14142E7ED comiss; jb fail: the other pawn within 50 degrees of the facing
    side = (F.x*d.y - F.y*d.x >= 0) ? 2 : 1  // 2: the other pawn is to the right -> RunSwayL; 1: to the left -> RunSwayR
TrySway (slot 178)(yaw, other): gate; st = push (slot 179, 0x141445C80: PushState(11));
    st+0x88 = other; +0x90 = side; +0x98 = yaw; +0x94 = 1
```

There is no test of the other pawn's team, size or class beyond its vt+0x1980 (true unless a subclass overrides it)
and no distance test: the distance is that of the contact event. Not in battle only.

##### Begin 0x1413CC7B0, Tick 0x1413D8110, End 0x1413CF3B0 [I]

```
Begin: Play(side == 1 ? "RunSwayR" : "RunSwayL", blendIn 0.1, mode 2)
Tick:
    if !OnGround: if Fall(+0x98): DoChain; return
    dir = +0x94 ? forward of (0, +0x98, 0) : actor forward
    rail probe (range pawn+0xBF8) && TryRailSlide (slot 75): DoChain; return
    sub 0: if !+0xA9 && TrySlopeSlide(0): DoChain; return
           if TryHopping(+0x98, m_bReverseLeg, 0): DoChain; return
           if TryClimbing(+0x98, m_bReverseLeg): DoFinish; return
    sub 0 or 3: if IsAnimEnd: sub = 3; if +0x94: TryRun(+0x98, 1.0) (slot 40);  DoChain; return
    if +0x94: Turn((0, +0x98, 0), dt * 6)                         // 0x1413D83D9
    else if m_bEnableChange: sub = 3; TryRun(+0x98, 1.0); DoChain; return
    Velocity.XY = GetMaxWalkSpeed() * actor forward                (cmc vt+0x9F8)
    +0x94 = 0; +0xA9 = 0
End: pawn vt+0xB38(0)
```

So: full run speed along the facing for the whole state, the facing turning toward the stick (rate `dt × 6`), plus
the clip's lateral root motion (out 109.5 cm by frame 9, back to 2.7 cm at the end: an arc around the other pawn).
With the stick **released** the state ends at `TresEnableChange` (0.39 s); with the stick **held** it runs to the
clip end (0.9 s) and continues into Run. (Part I had this the other way round.) Other pawns never block the capsule
(movement spec 6), so the arc is purely the clip.

##### Clips [A]

| Name | Seq | Length | Mode / blend-in | Root (right) per frame | Notifies |
|---|---|---|---|---|---|
| `RunSwayR` | 0101 | 0.9 s (28 f) | 2 / 0.1 | 0, 18.7, 39.1, 64.0, 87.8, 96.5, 102.3, 106.2, 108.6, 109.5, 108.2, 104.7, 99.5, 93.1, 86.0, 78.8, 69.7, 61.2, 53.2, 45.6, 38.4, 31.8, 25.6, 20.0, 14.8, 10.2, 6.1, 2.7 | `TresReverseLeg` 0.165–0.499 and 0.735–0.9, `TresEnableChange` 0.3935 |
| `RunSwayL` | 0102 | 0.9 s | 2 / 0.1 | mirrored (negative) | `TresEnableChange` 0.3969 |

---

#### Swim (id 102) — `TresPlayerState_Swim` and movement mode Swimming

##### Water: how "in water" and the surface are obtained [I]

- **Water volume** = the movement component's current `APhysicsVolume` (cmc vt+0x408 `GetPhysicsVolume`; the engine
  keeps it from the overlaps of the pawn root with physics volumes) with **`bWaterVolume`** (volume+0x42C bit 0;
  cmc vt+0x420 `IsInWater` 0x1405892F0). `SQEX_bOceanVolume` (volume+0x42C bit 1) selects the ocean path, whose
  surface comes from a wave query 0x1445CEA40(world, location, out) instead of the brush.
- **Surface offset** `off` = `m_SwimSurfaceOffset` (cmc+0x3D4; native 50, blueprint **100**) × root scale Z
  (cmc vt+0x680, 0x140584200).
- **Depth of the feet** cmc vt+0x678 (0x140582300), clamped to [0, off + 50]: plain volume — line trace against the
  volume's brush component (volume+0x3F8, component vt+0x950 `LineTraceComponent`) from (x, y, z + off + 50) down to
  the feet: depth = (1 − Time) × (off + 50); ocean — surfaceZ × clamp01(cmc+0x41C × cmc+0x410) − z.
- **Swim test** cmc vt+0x5A0(margin) (0x140589410; pawn vt+0x1A80): `IsInWater && depth > max(off + margin, 0)`.
  With margin 0: **the water surface more than 100 cm above the feet**. Idle, Run, Jump (0x14139556C), Glide and
  SlopeSlide call it with 0 and push SWIM; the Swim tick uses −10 (leaves at 90 cm or less).
- **Near surface** pawn vt+0x1A88 (0x1405AD900): in water and depth < off + 50 (the surface less than 150 cm above
  the feet). **Fast entry** pawn vt+0x1A90: Velocity.Z ≤ `m_SwimSurfaceToDiveVelZ` (cmc+0x3D0; native −1500,
  blueprint **−2400**).
- **Immersion** cmc vt+0x670 (0x140587AB0; pawn vt+0x1A98) = stock `ImmersionDepth`: 0–1 over the column from the
  feet to 2 × half-height above them (half-height from the actor's `GetSimpleCollisionCylinder`, pawn vt+0x548; 1.0
  if that is 0 or `Buoyancy` is 0; no hit of the trace = 1.0). [? the half-height value the root returns: probe W1]

##### Fields [I: Reset 0x1413D0EF0, input 0x141441C90]

| Offset | Type | Meaning |
|---|---|---|
| +0x88 | rotator | actor rotation at entry |
| +0x94 / +0x98 | f32 | stick yaw / second yaw argument of the swim input (pitch target underwater) |
| +0x9C / +0xA0 | f32 | raw stick X / Y (cleared at the tick end) |
| +0xA4 | f32 | stick magnitude |
| +0xA8 | f32 | dash / hold timer |
| +0xAC | f32 | clip lock timer (counted down at the tick end) |
| +0xB0 | i32 | vertical command: 2 = key 0x149C707A8 held (down, pitch −80), 1 = key 0x149C707C0 held (up, pitch +80); cleared at the tick end |
| +0xB4 | u8 | bit 0 controller flag (vt+0x1680), bit 1 stick active, bit 2 key 0x149C707A8 pressed, bit 3 key 0x149C70790 held, bit 4 slot-226 request, bit 5 = bit 3 of the previous tick, bit 6 underwater mode, bit 7 dashing |
| +0xB8 | i32 | clip id playing: 1 SwimIdle, 2 SwimNormal, 3 SwimTurn, 4 SwimToDive / Diving entry, 5 DiveIdle, 6 DiveMoveHigh, 7 / 8 DiveDash |
| +0xBC | i32 | speed mode last applied (slot 122) |
| +0xC0 / +0xCC / +0xD0 | vec3 / f32 / f32 | external push (direction, strength, decay 0.92 from Reset) applied by slot 119 |
| +0xD4 | f32 | dash re-arm timer |

Reset: sub 0, rotators / vectors zero, +0xD0 = 0.92 (0x1413D0F6A), the rest 0.

##### Requests [I]

```
gate (slot 222, 0x14142E870): iface slot 32 (+0x100) && !(pawn+0xE60 bit 17) &&
    ( current SWIM  |  current GUARD (15) && in water (vt+0x1A80(0)) && nothing queued && guarded hit (+0xDC)
                        && m_bEnableInput && controller+0x8A0 <= 0 )
swim input (slot 223, 0x141441C90)(yaw, yaw2, mag, stickX, stickY, k790held, jumpPressed, k7A8held, k7A8pressed, k7C0held):
    gate;  pawn vt+0x2770() (restrict all): the four key flags = 0
    GUARD: only if k790held || k7A8held || k7C0held: st = SetState(SWIM);   SWIM: st = current
    if mag > 0: flag bit 1; st+0x9C = stickX; +0xA0 = stickY; +0x94 = yaw; +0x98 = yaw2
    flag bit 3 = k790held; flag bit 2 = k7A8pressed; st+0xA4 = mag
    st+0xB0 = k7A8held ? 2 : (k7C0held ? 1 : 0)
slot 226 (0x141441BB0)(yaw, mag): SWIM only: (mag > 0: +0x94 = yaw, +0x98 = 0, flag bit 1); flag bit 4; +0xA4 = mag     // dash request, caller not traced [?]
dive allowed (slot 224, 0x14142E850) = !pawn vt+0x1FB8() = !(pawn+0xE60 bit 13 || restriction query 0x0D)
```

The controller (0x1413F1AB0) calls slot 223 whenever the stick, the jump press or one of the three keys is active.
Pad bindings of the FKey globals 0x149C70790 (dash), 0x149C707A8 (dive / down), 0x149C707C0 (up): [? W3].

##### Begin 0x1413CC890 [C; setters I]

```
pawn+0xE48 bits 0, 3 set (and vt+0x1E98);  pawn vt+0xCA8(..) (mesh / equipment water switch)
flag bit 7 = 0
if in water (vt+0x1A80(0)):
    +0x94 = actor yaw; +0x98 = 0;  0x1405A5C40(pawn, 0)                      // leave the base
    if !IsSwimming: SetMovementMode(4 Swimming, 0)
    |Velocity.xy| clamped to [0, MySwimSurfaceSpeed 370]   (pawn vt+0x16F8 -> 0x14057CD20)
    camera: controller vt+0x1AD8(0, 0); 0x14140C040(controller, 0, -1.0)     (unless pawn+0x518 bit 0)
    Velocity.Z = max(Velocity.Z, -1000)   (pawn vt+0x16D0)                   // a clamp, not a branch
    gravity on (vt+0x1728(1))
    if sub == 3 (pushed by Jump sub-step 9 "Diving"): splash effect at socket vfx_center; +0xB8 = 4
    else:
        cmc.Buoyancy (+0x3C8) = MySwimSurfaceBuoyancy 1.5  (pawn vt+0x15B0)
        cmc.m_bIsSwimSurface (+0x3CC bit 0) = 1            (pawn vt+0x15D8)
        speed mode 1 (slot 122): MaxSwimSpeed (cmc+0x38C) = MySwimSurfaceSpeed 370
        +0x88 = actor rotation; sub = 0
        if immersion (vt+0x1A98) < 1.0 || (near surface && !fast entry): slot 120(0): surface
        else: slot 120(1): underwater; sub = 4
slot 120 (0x1413E4380)(bUnder): flag bit 6 = bUnder
    bUnder: pawn vt+0x1C40(1) (diving on: pawn+0xD70 bit 0, HUD, diving camera if allowed); ResetBuoyancy (vt+0x15B8); vt+0x15E0
    else:   pawn vt+0x1C40(0); Buoyancy = 1.5; m_bIsSwimSurface = 1
```

##### Tick 0x1413D8630 [I] and the sub-steps [C; constants I]

```
flag bit 7 = 0
if next queued == CINEMATIC (0x43) && 0x143EA19C0(pawn): DoChain; return
if !in water (vt+0x1A80(-10.0), 0x1413D8677) and sub not in {3, 5}:
    if no physics volume, or it is not a water volume, or cmc vt+0x9D8() (0x1405894A0: `FindFloor` at the location is blocking and walkable; via 0x1405ADAA0): DoChain; return
flag bit 0 = controller vt+0x1680() & 1
dispatch(dt, first = 0): sub 0 -> slot 124, 1 -> 125, 2 -> 126, 3 -> 127, 4 -> 128, 5 -> 129
tail: flag bit 5 = flag bit 3; bits 1-4 cleared;  +0x9C = +0xA0 = 0; +0xB0 = 0
      if +0xAC > 0: +0xAC = max(+0xAC - dt, 0)
```

Transitions are tested only on the first dispatch of a tick; a sub-step that changes `sub` calls the new one at once.

```
common head of sub 0 / 1 (surface):
    if near surface && fast entry: slot 120(1); +0x88 = actor rotation; sub = 4
    if flag bit 4: sub = 5
    if flag bit 2 (dive key pressed) && dive allowed (slot 224): sub = 3
sub 0 SURFACE IDLE (0x1413DA860):  stick: speed mode 1; sub = 1
    Play("SwimIdle", -1, mode 3) once; Draw(0, 1, 1.0) (weapon away)
    slot 130 (turn)((0, +0x94, 0), dt * 3)                                   // 0x1413DAB00
    slot 119 (external push)
sub 1 SURFACE SWIM (0x1413DB020):  no stick: sub = 0
    if forward(0, +0x94, 0) . velocityDir(root) < -0.7 && near surface: sub = 2        // stick against the motion
    slot 130((0, +0x94, 0), dt * 3)                                          // 0x1413DB2AD
    Play("SwimNormal", -1, mode 3) once; Draw(0, 1, 1.0)
    pawn vt+0x16A0(forward of (0, +0x94, 0), 1.0, 0)                         // movement input along the STICK yaw, full scale
sub 2 SURFACE TURN (0x1413DB730):
    Play("SwimTurn", -1, mode 2) once; at the clip end: +0x94 = actor yaw; speed mode 1; sub = 1
    dive key pressed && dive allowed: sub = 3
    pawn vt+0xDC0(zero rotator, dt * 6)                                      // 0x1413DB96A
sub 3 DIVE START (0x1413DB3C0):
    underwater mode on (flag bit 6; ResetBuoyancy)
    first: flag bit 2 = 0; SetMovementMode(5 Flying, 1); Play("SwimToDive", -1, mode 2); +0xB8 = 4
    while it plays: if !diving && immersion >= 0.95 && cmc+0x9C4 .z < 0: pawn vt+0x1C40(1, 0)
    at the clip end: not in water -> DoChain
                     near surface && !fast entry -> slot 120(0); SetMovementMode(4); speed mode 1; sub = 1
                     else pawn vt+0x1C40(1, 0); SetMovementMode(4); +0xA8 = 0.2; +0x88 = rotation; sub = 4
    pawn vt+0xDC0(zero rotator, dt * 6)                                      // 0x1413DB4ED
sub 4 UNDERWATER (0x1413DA070):
    allowed = slot 224
    near surface && !fast entry: slot 120(0); speed mode 1; sub = 1          // surfacing
    flag bit 4 && allowed: sub = 5
    +0xD4 counts down;  !allowed: +0xB0 = 1 (forced rise)
    moving = (dash key && +0xD4 <= 0) || stick || +0xB0 != 0 || +0xA8 > 0
    moving, not dashing: speed mode = +0xB0 == 2 ? 2 (MySwimDiveDiveSpeed 600) : +0xB0 == 1 ? 3 (MySwimDiveRiseSpeed 800)
                                      : 5 (MySwimDiveHighSpeed 500);  Play("DiveMoveHigh", -1, mode 3)
    dash (dash key, +0xD4 <= 0, allowed): Draw(1, 1, 1.0); speed mode 6 (MySwimDashSpeed 1250); Play("DiveDash", -1, mode 3);
                                      +0xAC = 0.2; +0xA8 = max(+0xA8, 0.15)             // 0x1413DA2E9
    not moving: Play("DiveIdle") (slot 123)
    dash key released: +0xD4 = MySwimDiveDashDisableInputTime 0.5 (pawn vt+0x1C50)
    steering, normal: target = (pitch: +0xB0 == 1 ? +80 : +0xB0 == 2 ? -80 : +0x98, yaw +0x94) (pitch 0 when idle); slot 130(target, dt * 3)
    steering, dash:   pitch += MySwimDivingHighSpeedMoveRotSpeedPitch 240 * stickY * dt * (inverted ? -1 : 1), clamped to +-85;
                      yaw += MySwimDivingHighSpeedMoveRotSpeedYaw 300 * stickX * dt; roll = stickX * 10; slot 130(target, 0.4)
    moving: pawn vt+0x1690(forward of the 3-D rotation, 1.0, 0)              // swims where the body points
    pawn+0xE48 bits 1, 2 set (air slide / air guard available)
sub 5 DASH (0x1413DAB60): first: Draw; SetMovementMode(5 Flying, 1); MaxFlySpeed = 1250; MaxAcceleration = 1250 * 120 (0x1413DAC1A);
    speed mode 6; Play("DiveDash", -1, mode 3); +0xA8 = MySwimDashTime 3
    while +0xA8 > 0: steer (as the dash steering above, or level with slot 130(.., dt * 1.5) on the surface); move forward; +0xA8 -= dt
    then: ResetMaxAcceleration; SetMovementMode(4); +0xA8 = 3.0; sub = 4
slot 122 speed modes (0x1413E3E40): MaxSwimSpeed = 1 Surface 370 | 2 Dive 600 | 3 Rise 800 | 4 DiveLow 300 | 5 DiveHigh 500 | 6 Dash 1250
slot 130 turn (0x1413E4F50)(rot, alpha): Turn((0, rot.yaw, 0)) (vt+0xDB0); pitch through vt+0xDC0(rot.pitch, alpha); mesh offset from the mesh roll
slot 119 push (0x1413CA330): if +0xCC > 0: pawn vt+0x1718(+0xC0 * +0xCC * dt * 60, 1); +0xCC *= pow(+0xD0, dt * 60)
```

##### `PhysSwimming` 0x14058FC40 [I]

```
if dt < 1e-6: return
if m_bIsSwimSurface: cmc+0x41C = max(cmc+0x41C + dt/3, 1.0) when it is < 1;  swim-ring fluctuation (+0x410 -> +0x414 over +0x418 s)
depth = ImmersionDepth()                      // 0..1
net   = Buoyancy (cmc+0x3C8) * depth
if Velocity.Z > GetMaxSpeed() * 0.5 && net != 0: Velocity.Z *= depth
CalcVelocity(dt, friction = 0.5 * (cmc+0xB80) * depth, bFluid = true, braking = BrakingDecelerationSwimming (cmc+0x3B8, blueprint 700))
Velocity.Z += GetGravityZ() * dt * (1 - net)
delta = (Velocity + (base moving ? 0 : cmc+0xB90 vec3)) * dt
if root motion (cmc+0x458 bit 2): delta += rootMotionVelocity (cmc+0xA24) * dt
delta.xy += (cmc+0xBB4, +0xBB8) * dt          // water-current velocity
if !m_bIsSwimSurface: delta.z += cmc+0xBBC * dt
delta.z = SurfaceClamp(dt, delta.z)           // 0x1405749F0, surface mode only
remaining = Swim(delta, hit)                  // 0x140597C10: SafeMove; if it left the water, back to the water line
if IsSwimming: if hit.Time < 1: HandleImpact; SlideAlongSurface(delta, 1 - hit.Time, hit.Normal)
               if !IsInWater && !(cmc+0xBF4 bit 0 m_bDisableAutoChangeSwimMoveMode): SetMovementMode(3 Falling)
if !IsSwimming: StartNewPhysics(remaining * dt, Iterations + 1)
```

Differences from stock UE 4.17: the fluid friction is a component field (cmc+0xB80 [? value: W2]), not the
volume's; the water-current and surface terms; no velocity recomputation from the moved distance at the end.

`SurfaceClamp` 0x1405749F0 (plain volume) [I constants, C flow]: line trace against the volume brush from
(x, y, z + off + 100) down to the feet. Hit, not start-penetrating: `target = hit.z − off`; `new = z + dz`;
`new > target` → `dz = target − z` (snap down); `new < target` → `dz += (target − new) × 0.125` (0x140574F6C; per
tick). Start-penetrating (surface more than off + 100 = 200 cm above the feet): `dz = min(800 × dt, 800 × dt + dz)`
(0x140574F29) — pushed up at 800 cm/s. Net effect on the surface: Sora's feet settle **100 cm below the water
surface**, he cannot rise above that line (so he cannot leave the water by swimming up), and he follows a moving
surface.

Surface model [D]: Buoyancy 1.5 with full immersion gives gravity × (1 − 1.5) = +490 cm/s² upward when submerged,
−980 … +490 by immersion; horizontal: stock `CalcVelocity` with MaxSwimSpeed 370, MaxAcceleration (unchanged by the
state: the pawn's current value), braking 700 cm/s² when the stick is released; facing turns at `dt × 3`.

##### Leaving the water

- Tick head: depth ≤ 90 cm (and not in the dive-start / dash sub-steps) and the volume is gone or the pawn has a floor
  (cmc vt+0x9D8) → `DoChain` → Idle. The pawn walks out where the bottom rises to within 90 cm of the surface.
- Jump: jump gate SWIM branch → state slot 114 (0x1413CA550): sub < 3 and near surface. `JumpRequest` SWIM branch
  [C]: Jump flags (Jump+0x114) |= 3; bit 1 makes the Jump tick head skip the in-water test until sub-step 3
  (0x141395576–83), so Sora jumps out of the water; it is cleared as soon as he is out (0x1413955C4).
- Guard in water: guard gate SWIM branch = diving (pawn vt+0x1C30) && dive allowed; a guarded hit returns to SWIM
  through slot 223. Air slide in water: Part I section 6.
- End 0x1413CF3C0 [I]: flag bit 7 cleared; slot 132 (pawn vt+0xB90; pitch / roll reset vt+0xDC8(1.0)); glide,
  air-slide and air-guard timers zeroed if running (vt+0x1E08 / 1E18 / 1E28 (0)); if not in water (vt+0x1A80(0)):
  pawn vt+0xCA8(0), and if the mode is still Swimming or Flying: floor (cmc vt+0x9D8) → SetMovementMode(1 Walking, 1)
  else SetMovementMode(3 Falling, 0); then reset MaxSwimSpeed (vt+0x1598), MaxFlySpeed (vt+0x1628), MaxAcceleration
  (vt+0x1648), Buoyancy (vt+0x15B8), vt+0x15E0.

##### Diving: where it works

pawn+0xD70 bit 0 is **not a world switch**: it is the "diving now" flag, written only by pawn vt+0x1C40
(0x141447B10), which the Swim state (slot 120, sub-step 3) and Jump sub-step 9 (0x141397146) call; it also tells the
HUD (0x141584F50) and, when dive is allowed, spawns the `ATresCameraDiving` (bit 1). The switches are: (a) the
restriction bit 13 of pawn+0xE60 (or restriction query 0x0D) — iface slot 224; (b) the clips: `SwimToDive`,
`DiveIdle`, `DiveMoveHigh`, `DiveDash` are **not in Sora's base anim set**; they exist only in the world sets
`p_ex001_AnimSet_CA` (The Caribbean) and `p_ex001_AnimSet_BT` [A]. Elsewhere a dive request plays nothing; (c) the
swan dive from a jump (Jump slot 115, 0x141383550 [C]) needs an **ocean** volume: height over the surface 500–700 cm,
at least 500 cm of water, fall speed ≤ −900 → clip `Diving`, then SWIM at sub-step 3.

##### Clips (base set) [A]

| Name | Seq | Length | Mode / blend-in | Root motion | Notifies |
|---|---|---|---|---|---|
| `SwimIdle` | 0082 | 2.0 s | 3 / −1 (clip default 2.0) | none | SE `se00001_076` (attached, 0.3 volume; 1.303) |
| `SwimNormal` | 0083 | 1.3333 s | 3 / −1 (0.5) | none | SE `se00001_076` 0 and 0.541 |
| `SwimTurn` | 0092 | 0.6667 s (21 f) | 2 / −1 (0.2) | −289.9 cm forward, yaw 180°; yaw per frame 0, 3.1, 11.6, 24.0, 38.9, 55.2, 71.3, 86.0, 100.9, 120.0, 142.9, 160.8, 171.8, 178.0, 180 … | SE `se00001_076` 0 and 0.396 |
| `Diving` | 0081 | 0.8667 s (27 f) | Jump sub-step 9 | 133.7 cm forward from frame 10 | SE `se00001_076` |

---

#### Corrections to Part I

| Section | Part I said | Correct |
|---|---|---|
| 5.4 | The Jump tail calls the dangle gate and then pushes through `TryDangle` | It calls gate slot 63, the probe, then **push slot 65 directly** with variant 0 (0x1413980BD) |
| 11.1 | "Entry … from Idle / WallRun through `TryDangle`" | The gate allows IDLE, JUMP and WALLRUN; `TryDangle` (slot 64) is called by the WallRun tick only. Other pushers: Jump tail, Hopping helper 0x14139EBF0, AthleticFlow 0x141331C40 |
| 11.1 / movement spec 7 | geometry flag only named | `UPrimitiveComponent` dword +0x854 bit 2 `SQEX_bEnableDangle`, tested by component vt+0x4F0 in the probe (0x140638ED2), together with `IsWalkable` on the hit and trace channel Pawn |
| 11.1 | "position = ledge + `m_DangleOffsetPos`" | position = contact point + **Rot(yaw)** × offset; then a body-overlap test and a push-out of up to 10 cm; failure → sub-step 3 (immediate chain) |
| 11.1 | movement mode during the hang not given | **Custom (6)**, whose Phys function is an empty stub; Flying only for the climb-up |
| 11.1 | "< 90° → climb; ≥ 135° (≥ 90° with a pawn flag) → release" | Without the controller flag: < 90° climb, ≥ 90° release. With it (pawn vt+0x1A20): < 45° climb, ≥ 135° release, otherwise stay. Stick magnitude must be > 0.5 |
| 11.1 | "release: cooldown 0.6 s" | The cooldown (pawn+0x6F8) is refreshed to 0.6 on **every** hanging tick, zeroed by the climb, and blocks the gate; jump press = drop (bFall) |
| 11.2 | vaultable geometry not identified | floor and obstacle top must have **`SQEX_bEnableAutoRun`** (+0x854 bit 5); heights 44–225 cm; classes ≤ 100 / ≤ 150 / > 150 → R·L / MR·ML / H |
| 11.2 / 0.7 | "`m_bEnableClimbing` … the level turns them on" | Nothing in the shipped code or cooked data sets `m_bEnableClimbing`: the vault is dead in retail; host switch only |
| 11.2 | "the actor is lifted toward the ledge height (rate scale +0xB8)" | A velocity request: `RequestDirectMove(Velocity + (0, 0, 0.5 × dz))` per tick while traction; traction ends at dz ≤ −1 or the clip end |
| 11.2 | "a new obstacle found … restarts the state" | Only with m_bEnableChange, the stick forward (within 75°, > 0.5) and the actor moved; it toggles the leading leg and calls Begin directly |
| 11.3 | slope criterion not given; "floor angle term < 30" | Start: `SQEX_bEnableSlopeSlide` floor, fall line ≤ −25° (forced floors `SQEX_bEnableForceSlopeSlide`: −15°), facing dot ≥ 0.643 (0.174), 120 cm (50) clear. Continue while the slope ahead is < −15° (forced: < 0°). "30" is the test "last slope angle ≥ 30° → end at once" |
| 11.3 | "acceleration per tick = (1600 − speed) / (0.5 × 60)" | Confirmed, and it is a constant increment computed once at Begin from the 3-D entry speed, added **per tick** (frame-rate dependent); deceleration 2 × (0.5 × on forced floors) the increment per tick |
| 11.3 | "ends when … the speed drops under 680 / 980 … velocity × 0.98 per tick" | Stick held and speed < 980 → `TryRun` and chain; no stick and speed < 680 → `LightLandR`. The × 0.98 belongs to the weapon-form variant only |
| 11.3 | `TrySlopeSlide` "from Idle, Run, Jump landing, Swaying" | Plus the latch rule (pawn+0xD80 bit 2): Idle slides only with the latch, Run with the latch or its own argument, a landing always |
| 11.5 | "The caller of `TrySway` was not traced" | Run state handler 0x1413B2960 of slot 8 of the state's second interface (state+0x48); conditions above |
| 11.5 | "from `TresEnableChange` (0.39 s) or at the clip end → `TryRun`" | Stick released: ends at `TresEnableChange`; stick held: continues to the clip end (0.9 s), steering |
| 11.5 | "the other pawn (its vt+0x1980) in front within 50°" | Confirmed (dot ≥ 0.643); also pawn type 1, kind ≠ 7, not in battle; the base vt+0x1980 returns true for every character pawn |
| 12 | "if the entry speed `Velocity.Z ≤ −1000` the dive branch starts (sub-step 4)" | −1000 is a **clamp** of the entry `Velocity.Z`. Underwater start (sub 4) = immersion ≥ 1.0 and not (near surface and Velocity.Z > −2400) |
| 12 / 2.4 | "pawn+0xD70 bit 0" as a water / world flag | It is the "diving now" flag set by the state through pawn vt+0x1C40; the permission is restriction bit 13 (iface slot 224) and the presence of the dive clips (CA / BT world anim sets) |
| 12 | water test not specified | `bWaterVolume` physics volume and water surface > 100 cm above the feet (leave at ≤ 90); surface from a line trace on the volume brush |
| 2.4 | "+0x16B0 / +0x16C0 … which is which [? P6]" | +0x16B0 → cmc vt+0x9E8 = Velocity XYZ; +0x16C0 → cmc vt+0x9F8 = Velocity XY |
| NOTES_facts | "pawn vt+0x15B0: cmc+0x3C8 (braking?)" | cmc+0x3C8 is `Buoyancy`; `BrakingDecelerationSwimming` is cmc+0x3B8 |

---

#### Host affordances (what each world probe reads)

All sweeps: trace channel **Pawn (ECC 2)**, simple collision, the pawn ignored. "Flag" = bit of the
`UPrimitiveComponent` dword at +0x854 (reflected bool properties; default false in the engine class [? per-asset
defaults: the level's collision components set them]).

| Affordance | Probe | Shape, start, direction, length | Tests on the hit | Abstract data a host needs |
|---|---|---|---|---|
| **Ledge** | 0x140638900 | sphere r 50 straight down from 300 to 150 cm above the feet, on the axis 50 cm ahead (also ±45°); fallback sphere r 100 from 250 to 200 on the pawn axis | default Pawn-channel blocking; not start-penetrating; other actor; **`SQEX_bEnableDangle`** (bit 2); `IsWalkable` (normal Z ≥ walkable limit) | ledge edge polyline + top plane; answer: contact point on the edge nearest the probe axis (within 50 cm), top height in the window, dangle flag |
| ledge space | same | sphere r 50 from the ledge-top level down 160 cm at the hang axis (contact − 60 cm) — also blocks channel 0x18 | any blocking hit fails | free column in front of the wall |
| ledge grip | same | sphere r 5, 5 cm above the top, from the hang axis to 2.5 cm short of the contact | any blocking hit fails | nothing standing on the lip |
| hang body | pawn vt+0x9D8 | the pawn's own capsule (r 60, hh 89) at the hang point; sweep from 10 cm behind | own collision profile | capsule overlap / sweep |
| **Vault obstacle** | 0x140634E60 | see the seven steps: floor spheres r 5 ±50 cm; headroom sphere r 50 to +300; clearance r 50 at +275 for 120–210 cm (from 50 cm behind); face sweeps r 50 / 5 at 175 / 94 / 49 cm for 150 cm; top sweep r 35 from 260 to 79 cm; landing sphere r 59.5 | channels 0, 1, 0x12 (+0x18) block; **`SQEX_bEnableAutoRun`** (bit 5) on the floor and on the top; top walkable, 44–225 cm high, ≤ 50° | box obstacle: near face distance and yaw, top height, top flag; the switch `m_bEnableClimbing` |
| **Slide slope** | 0x14063D9C0, 0x14063D1F0 | spheres r 5 from +50 to −50 cm at the feet and 10 cm ahead; clearance r 5 along the slope for 120 cm (50) | channels 0, 1, 0x12 block; **`SQEX_bEnableSlopeSlide`** (bit 6), optionally **`SQEX_bEnableForceSlopeSlide`** (bit 8); fall line ≤ −25° (−15°); facing | floor patches with a slide flag, a forced flag and their plane; answer: contact point + normal at two points |
| **Pawn in the way** | event → 0x1413B2960 | none (contact event of the body collision with another actor) | other pawn's vt+0x1980 (true); direction dot ≥ 0.643 | host event `on_bumped_pawn(position)` while running at ≥ 90 % speed with the stick > 0.5, outside battle |
| **Water volume** | cmc vt+0x420 / +0x678 / +0x670; 0x1405749F0 | line traces against the volume's brush: from 150 cm above the feet down to the feet (depth), from the capsule top to the feet (immersion), from 200 cm above down (surface clamp) | `APhysicsVolume` with **`bWaterVolume`** (volume+0x42C bit 0); `SQEX_bOceanVolume` (bit 1) → wave-surface query instead | volume (any convex brush) + surface Z at (x, y); swim when surface − feet > 100 cm; wade below; dive allowed flag (restriction bit 13) and the dive clips |
| rail (from SlopeSlide / Swaying) | pawn vt+0xEA0 | segment at the feet, range `MyRailSlideCheckRange` 50 | rail strand | — |

Test-level recipes: a **ledge** = a static box whose collision blocks Pawn, flagged dangle, top 100–250 cm above the
floor Sora jumps from (any height when jumping: the window is relative to the feet), flat top, free wall face below
it for 160 cm and 50 cm of free space in front. A **pool** = a `bWaterVolume` physics volume whose top face is the
surface, with the bottom more than 100 cm below it where Sora should swim (≥ 190 cm deep keeps his feet off the
bottom: feet settle 100 cm under the surface; the shore must rise to within 90 cm of the surface for him to walk
out). A **slide** = a walkable ramp of 25–49° flagged slope-slide. A **vault box** = flagged auto-run, 44–225 cm
high, on a floor flagged auto-run, with `m_bEnableClimbing` switched on by the host.

---

#### Tests (60 Hz, every ability equipped, level 99, plain Keyblade)

**Dangle**

1. *Grab.* Wall with a flagged ledge whose top is 220 cm above the floor; Sora stands with his axis 70 cm from the
   wall face, facing it; jump press, no stick. Expect: no grab during the first 0.2 s of the jump (Jump+0xA8) nor
   while Velocity.Z > 500 in sub-steps 1–2; state DANGLE on the first tick after both conditions hold with the top
   between ≈ 101 and 251 cm above the feet. On Begin: feet at (contact − 60 cm along the facing, top − 150 cm), yaw =
   toward the wall, MovementMode 6, velocity 0. `GripStart` plays; `GripLoop` from 1.8 s.
2. *No flag.* Same wall without `SQEX_bEnableDangle` → never DANGLE. Same with a 10 cm railing on the lip → never.
3. *Climb.* Hanging ≥ 0.2 s (after `TresEnableChange` 0.199 s), stick toward the wall (|Δyaw| < 90°, magnitude > 0.5)
   → `GripRise`, MovementMode 5; after 0.5 s (30 ticks) the feet are 100 cm forward and 200 cm up from the hang
   position (= 40 cm past the edge, 50 cm above the top); then Idle → Jump sub-step 3 → landing on the ledge.
   pawn+0x6F8 = 0 after the climb.
4. *Release.* Hanging, stick away from the wall (|Δyaw| ≥ 90°) → Fall, Jump sub-step 3 from the hang position;
   pawn+0x6F8 = 0.6 and counts down; a second grab of the same ledge is impossible for 0.6 s (36 ticks).
5. *Drop by jump.* Jump press while hanging (≥ 0.2 s) → Jump sub-step 3 with Velocity.Z starting at 0 (no launch).
6. *Stick before 0.199 s* is ignored (m_bEnableChange clear).

**Climbing** (host switch `m_bEnableClimbing` on; without it: Run into the box → no state change)

1. Box 80 cm high, 300 cm deep, flagged; run straight at it (stick > 0.5). Expect CLIMBING when the near face is
   within ≈ 100 cm; clip `ClimbingR` or `ClimbingL` by m_bReverseLeg (0.3667 s, blend-in 0); MovementMode 5, gravity
   off; root motion 250 cm forward; height class: 80 → low.
2. Box 120 cm → `ClimbingMR` / `ML` (0.5 s); box 180 cm → `ClimbingH` (0.5333 s); box 230 cm or 40 cm → no vault.
3. Approach at 50° to the face normal → no vault (needs ≥ 0.7071 = within 45°).
4. After the clip, on the ground: `LightLandR` (after R / MR / H) or `LightLandL`; with the stick still forward the
   landing clip is skipped (DoChain on the first tick of sub-step 4).
5. Two boxes 250 cm apart, stick held: the second vault starts from the first clip's `TresEnableChange` time
   (0.2 / 0.3 / 0.4 s) with the other leg (R → L).

**Slope Slide**

1. Ramp 30°, flagged, 30 m long; Sora falls onto it facing downhill → SLOPESLIDE on the landing tick. Entry speed
   v0: `+0x94 = (1600 − v0) / 30`; |Velocity| after n ticks = min(v0 + n × +0x94, 1600): 1600 from tick 30 (0.5 s).
   Pitch → slope angle + 25°. Clip `SlopeSlide` looping.
2. Same ramp, Sora runs onto it from flat ground (no previous slide) → no slide while Run's argument is 0 (latch
   clear); after one slide, re-entering from Run or Idle slides (latch set) until a test fails once.
3. Ramp 20° flagged → no slide; 20° with the forced flag → slide (also from Idle, any facing within 80°).
4. Facing 60° off the fall line on the 30° ramp (dot 0.5 × cos 30° = 0.43 < 0.643) → no slide.
5. Steering: stick 90° to the side → facing yaw error shrinks by 2 % per tick (after 60 ticks 30 % of the error
   remains: yaw turned ≈ 63°).
6. Run-out: ramp ends in flat flagged or unflagged ground. With the stick held: speed falls by `2 × +0x94` per tick
   from 1600; `TryRun` on the first tick with speed < 980 (for v0 = 680: +0x94 = 30.67, 61.3 per tick → tick 11).
   Without the stick: glide until speed < 680 (tick 16), then `LightLandR` and velocity 0; Idle after 0.633 s.
7. Jump press while sliding → Jump with yaw = slide target yaw.

**Swaying**

1. NPC pawn standing in the open, not in battle; Sora runs into it at full speed (≥ 90 % of MaxWalkSpeed, stick
   > 0.5) with the NPC 20° left of his facing → host fires the bump → SWAYING, side 1, `RunSwayR` (0.9 s, blend 0.1).
   Velocity each tick = MaxWalkSpeed along the facing; lateral root offset peaks at 109.5 cm at frame 9 (0.3 s).
2. NPC 60° off the facing (dot 0.5) → no sway. In battle → no sway. Walking (< 90 % speed) → no sway.
3. Stick released during the sway → state ends on the first tick after 0.3935 s (`TryRun` + chain → Run → Idle).
   Stick held → ends at 0.9 s into Run; the facing follows the stick at rate dt × 6.

**Swim**

1. Pool: surface 300 cm above the bottom. Sora walks in down a ramp: Idle / Run → SWIM on the first tick with the
   surface more than 100 cm above his feet; MovementMode 4; Buoyancy 1.5; MaxSwimSpeed 370; `SwimIdle` / `SwimNormal`.
   At rest the feet settle 100 cm below the surface (rise by 1/8 of the gap per tick from below).
2. Stick held: horizontal speed → 370 cm/s along the stick yaw; release: deceleration 700 cm/s² (0.53 s to stop)
   [plus fluid friction if cmc+0xB80 ≠ 0: W2]. Stick reversed (dot < −0.7) while moving → `SwimTurn` (0.6667 s, 180°).
3. Jump into the pool from 3 m: Velocity.Z at entry clamped to ≥ −1000, |Vxy| ≤ 370; surface swim (sub 0) since
   Velocity.Z > −2400; sinks, then is pushed up at ≤ 800 cm/s and settles at the surface line.
4. Swim to the ramp: SWIM ends on the first tick with the surface ≤ 90 cm above the feet and a floor under them →
   Idle, MovementMode 1.
5. Jump press on the surface (sub 0–2, surface < 150 cm above the feet) → Jump (type 1 high jump) that leaves the
   water; no SWIM re-entry until Jump sub-step 3.
6. Dive key in the base anim set: sub-step 3 runs with no clip (length 0) [behaviour without the CA clips: W4].

---

#### Still needs the running game

| # | Object | Offset | Type | Why |
|---|---|---|---|---|
| T1 | Dangle state | +0x98 vec3, +0xB0 f32; pawn location vec3 | per frame | confirm the snap position / yaw and the ledge window on a real ledge (the contact-direction arithmetic is [C]) |
| T2 | pawn / cmc | location vec3, cmc+0x214 Velocity vec3, cmc RequestedVelocity, MovementMode cmc+0x314 u8 | per frame through a vault (needs `m_bEnableClimbing` forced on: pawn+0xB44 u8 \|= 1) | trajectory of the traction phase through `PhysFlying` |
| T3 | cmc | +0x9A0 vec3, +0x9C4 vec3 | per frame while running and falling | which velocities these are (vault reach, vault End, slide block test) |
| T4 | pawn | +0xD80 u32 bit 2 | per frame on a slide slope | the slide latch rule; Run+0x94 u8 / Run+0x98 f32 for Run's argument |
| T5 | controller | byte +0x4E0 (pawn vt+0x1A18) | u8 | the value selecting the 45° / 135° dangle input rule |
| S1 | function entry 0x1413B2960 | call stack / caller address | event | which engine event fires slot 8 of the state's second interface (bump source, contact distance) |
| W1 | pawn vt+0x548 result | radius f32, half-height f32 | once | immersion column height (surface-vs-underwater start) |
| W2 | cmc | +0xB80 f32, +0xB90 vec3, +0xBB4 vec3, +0x41C f32, +0x410 f32 | per frame in water | fluid friction, external velocity, surface factors |
| W3 | FKey structs | 0x149C70790, 0x149C707A8, 0x149C707C0 | key names | pad bindings of dash / dive / rise |
| W4 | Swim state | +0x78 i32, +0xB4 u8, +0xB8 i32; pawn+0xD70 u32; pawn+0xE60 u32 | per frame in a non-Caribbean pool with the dive key | what the dive request does without the dive clips; whether bit 13 is set there |
| H1 | level collision components | +0x854 u32 | sample of floor / wall components in a level | how common the dangle / auto-run / slope-slide flags are (defaults per asset) |

---

## II.7 Default input layout

(strand `input`; working notes in `build/sora_tmp/actions/p2_input/`)

### Default input layout

Sheet: `sheets/port/sora-input-layout.json` (tool `tools/sora_input_layout_sheet.js`, 32 rows). Marks: [I] read
at instruction level, [C] decompiled C only, [A] read from an asset / ini file, [U] stock UE 4.17 on confirmed
function identity. Evidence listings and scan tools: `build/sora_tmp/actions/p2_input/` (`NOTES.md`).
Everything below is for `i.GamePadDecideType` = 0 (the default).

#### L1. How keys are organised [I]

- **Pad buttons are not rebindable.** The "FKey globals" the gameplay code reads at 0x149C70xxx are the engine's
  own `EKeys` statics (array of 0x18-byte `FKey` from `EKeys::AnyKey` 0x149C6F9F8; `Gamepad_LeftX` 0x149C705F8 …
  `Gamepad_RightStick_Left` 0x149C708E0). They are constructed by the engine's static initialisers
  (0x1402D3F90 …, one per key) and never reassigned.
- **Two game variables** hold a pad key: attack / decide 0x1487D7760 and jump / cancel 0x1487D7778. Static
  initialisers: 0x1400C79C0 (attack = `Gamepad_FaceButton_Right`), 0x1400C7980 (jump =
  `Gamepad_FaceButton_Bottom`); then 0x140EDD580 rewrites both from the cvar `i.GamePadDecideType`
  (int 0x1487D7370): `>= 1` → attack = Right, jump = Bottom; else attack = **Bottom**, jump = **Right**.
- **Pad table** 0x1486DAA00: 30 `const FKey*` in `.data`, used by the key emulation and by mini-game / hold-camera
  code (`mov rdx,[0x1486DAA00 + 8·i]`): 0 LeftX, 1 LeftY, 2 RightX, 3 RightY, 4–7 LeftStick Up / Down / Left /
  Right, 8–11 RightStick Up / Down / Left / Right, 12 FaceButton_Top, 13 Bottom, 14 Left, 15 Right, 16
  LeftShoulder, 17 LeftTrigger, 18 LeftThumbstick, 19 RightShoulder, 20 RightTrigger, 21 RightThumbstick, 22
  LeftTriggerAxis, 23 RightTriggerAxis, 24–27 DPad Up / Down / Left / Right, 28 Special_Left, 29 Special_Right.
- **Key table** 0x1487D7790: 21 slots × {main `FKey`, sub `FKey`}; defaults written by 0x140EDF630. This is
  the only rebindable part (L5).
- Xbox pad → `EKeys` names is the stock XInput mapping [U] (A = FaceButton_Bottom, B = Right, X = Left, Y = Top,
  LB / RB = Left / RightShoulder, LT / RT = Left / RightTrigger, L3 / R3 = Left / RightThumbstick, View =
  Special_Left, Menu = Special_Right).

Readers: `APlayerController` wrappers over `UPlayerInput` [I] — `WasInputKeyJustPressed` 0x144445000 (true
for one frame), `IsInputKeyDown` 0x1444369D0, `WasInputKeyJustReleased` 0x1444450A0, `GetInputAnalogKeyState`
0x144433300, `GetInputAnalogStickState(0 | 1)` 0x1444333A0 (left / right stick); state helpers 0x14150E670
(down) and 0x14150E750 (pressed) forward to them.

#### L2. Pad table (Xbox)

| Pad | `EKeys` static | Action | Semantics | Consumer |
|---|---|---|---|---|
| Left stick | LeftX 0x149C705F8, LeftY 0x149C70610 | move | axis | input update 0x1413F1AB0 (movement spec 2.3); walk below magnitude 0.8 |
| Right stick | RightX 0x149C70628, RightY 0x149C70640 | camera | axis | camera slot 258 0x1405CD3F0 (camera spec 2.5) |
| A | variable 0x1487D7760 (= FaceButton_Bottom 0x149C70718) | attack; command-menu confirm | just-pressed | 0x1413F1B9B; HUD button 27 → `ExecuteCommand` 0x14141B470 [C] |
| B | variable 0x1487D7778 (= FaceButton_Right 0x149C70730) | jump (hold: glide, jump height); menu cancel | just-pressed + held | 0x1413F1BD9 / 0x1413F1E65 |
| X | FaceButton_Left 0x149C70748 | guard (stick neutral) / dodge (stick active), air slide; hold: guard held | just-pressed + held | 0x1413F1C15 / 0x1413F1EA1 |
| Y | FaceButton_Top 0x149C70760 | situation command | just-pressed | 0x1413F1C50 → controller slot 674 0x141404460 (only with LB and RB up) [I]; HUD button 0 [C] |
| LB | LeftShoulder 0x149C70778 | shortcut modifier | held | 0x1413F1D75: clears the X / Y / B presses [C]; HUD button 4 pressed or held [C] |
| LB + B / Y / X / A | | shortcut 1 / 2 / 3 / 4 (index 0–3) | just-pressed | HUD 0x1416220D0 → 0x14161D450 [C] |
| RB tap | RightShoulder 0x149C70790 | lock-on | released within 0.2667 s | slot 712 0x1413F0700 [I] → target search 0x141404E50 [C] |
| RB hold | same | shotlock aim | held > 0.2667 s | slot 800 0x1414165E0 (magic spec S2) |
| RT tap | RightTrigger 0x149C707C0 | lock-on target change | released within 0.2667 s | slot 713 0x1413F0D00 [I] → 0x141404E50 [C] |
| RT hold | same | mode at controller+0x968 (slot 715 0x141406F80) | held > 0.2667 s | **not identified** |
| LT | LeftTrigger 0x149C707A8 | swim input argument; HUD button 6 → 0x14162BAF0 | just-pressed + held | 0x1413F1DB0 / 0x1413F1DEC; HUD action **not identified** |
| D-pad up / down | DPad_Up 0x149C707D8, DPad_Down 0x149C707F0 | command cursor −1 / +1 | UI pressed | HUD 0x1416236C0 [C] |
| D-pad right / left | DPad_Right 0x149C70808, DPad_Left 0x149C70820 | enter / leave the sub-menu | UI pressed | HUD 0x1416236C0 [C] |
| R3 | RightThumbstick 0x149C706A0 | camera reset | action, pressed | `ActionCameraReset` [A] → 0x14140ECC0 |
| L3 | LeftThumbstick 0x149C70688 | none found for Sora on foot | | UI button 8 is polled; mini-games read it |
| View | Special_Left 0x149C706B8 | gummiphone camera (state 144 HOLD_CAMERA) | just-pressed | 0x1413F1F14 → pawn slot 1349 0x14143D630 [C] |
| Menu | Special_Right 0x149C70700 | pause | UI pressed | UI manager 0x1415B0ED0 @0x1415B1483 [C] |

Timed buttons [I]: slot 712 (RB) keeps the hold time at controller+0x938 and, on the release frame, sets the
tap byte +0x934 when `0 < time <= 0.26666668` (16 frames at 60 Hz); slot 713 (RT) does the same with +0x940 /
+0x93E and additionally sets +0x93C / +0x93D once the hold exceeds 0.26666668 s (cleared on release). The
controller clears the RB flag in states 95, 96, 103, 144, 145, 150 [C]; while the controller's move input is
ignored (vt+0x888 → 0x1440878A0) it clears X pressed / held, B pressed / held, RB held, RT held and the stick,
but not A or Y [C]. Slot 713 and slot 800 run only when pawn vt+0xD70() == 1.

Reads in the input update whose result is discarded [I]: `Gamepad_Special_Right` pressed (0x1413F1C8C),
`Gamepad_RightTrigger` pressed (0x1413F1D3D), `Gamepad_FaceButton_Top` down (0x1413F1EDC). `AnyKey` pressed
(0x1413F1F79) sets controller+0x878 bit 5. `Gamepad_LeftShoulder` pressed (0x1413F1E2A) calls request-interface
slot 101 (+0x328) [C, meaning not determined].

#### L3. Keyboard and mouse table [I]

Defaults of the key table (0x140EDF630) and the pad key each slot is emulated as (registration = controller
slot 415 0x141074520, calling 0x144440EB0 for buttons, 0x144441050 for axes, 0x144440F90 for the mouse):

| Slot | Main | Sub | Emulates | Action |
|---|---|---|---|---|
| 0 | A | – | LeftX −1 | move left |
| 1 | D | – | LeftX +1 | move right |
| 2 | W | – | LeftY +1 | move forward |
| 3 | S | – | LeftY −1 | move back |
| 4 | LeftAlt | – | axis modifier × 0.5 | walk |
| 5 | RightMouseButton | RightShift | FaceButton_Left | guard / dodge |
| 6 | F | RightControl | FaceButton_Top | situation command |
| 7 | LeftMouseButton | Enter | attack / decide variable | attack, confirm |
| 8 | SpaceBar | BackSpace | jump / cancel variable | jump, cancel |
| 9 | Up | MouseScrollUp | DPad_Up | command up |
| 10 | Down | MouseScrollDown | DPad_Down | command down |
| 11 | Left | ThumbMouseButton | DPad_Left | command left |
| 12 | Right | ThumbMouseButton2 | DPad_Right | command right |
| 13 | LeftShift | – | LeftShoulder | shortcut modifier |
| 14 | C | – | LeftTrigger | LT |
| 15 | X | – | LeftThumbstick | L3 |
| 16 | R | – | RightShoulder | lock-on / shotlock |
| 17 | E | – | RightTrigger | target change |
| 18 | RightAlt | MiddleMouseButton | RightThumbstick | camera reset |
| 19 | N | Three | Special_Left | gummiphone camera |
| 20 | M | Four | Special_Right | pause |
| mouse | MouseX / MouseY | | RightX / RightY | camera |

"–" = the slot holds the invalid key (0x149C71048). When the cvar `i.GameKeyboardDisplayType` (name at
0x14655BCA0) is 1, slot 0 main becomes Q and slot 2 main becomes Z (0x140EE0870, 0x140EE08DC). The key names
that have no string in the extracted list (W, S, X, SpaceBar, Up, RightMouseButton, RightShift, RightControl,
ThumbMouseButton(2), Three) follow from the position in the `EKeys` array and, independently, from the
alphabetical order of the per-key static initialisers.

Emulation (movement spec 2.2 for the details): run by `APlayerController` slot 414 from `PreProcessInput`, so
**keyboard input reaches the game one frame later than pad input**. Buttons (0x14442B8F0): main or sub
just-pressed → `InputKey(pad key, IE_Pressed)`; both up after a release → `IE_Released`. Axes (0x14442C190):
`(pos − neg) × (modifier down ? 0.5 : 1)` injected raw every frame while a key is held (it then passes the
same dead zone as the stick: 1.0 → 1.0, 0.5 → 0.3333). The pause emulation of the other controllers differs:
0x1414B6570 (cooking mini-game) binds `Special_Right` to the fixed key M with no sub key.

#### L4. Axis processing

- Pad: XInput short × 1/32767 (1/32768 when ≤ 0), no dead zone at that layer [I, movement spec 2.1];
  `UPlayerInput::MassageAxisInput` 0x1445AF120 [U]: `v = sign(v)·max(|v| − dz, 0)/(1 − dz)`, exponent,
  sensitivity, invert; per axis.
- `DefaultInput.ini` `[/Script/Engine.InputSettings]` [A] (lines 31–36; `BaseInput.ini` lines 18–23 hold the same
  values and are removed and re-added): `Gamepad_LeftX`, `Gamepad_LeftY`, `Gamepad_RightX`, `Gamepad_RightY`:
  DeadZone 0.25, Sensitivity 1, Exponent 1, bInvert False; `MouseX`, `MouseY`: DeadZone 0, Sensitivity 0.07,
  Exponent 1, bInvert False. No trigger axis has an `AxisConfig` (the Sora controller reads the triggers as
  buttons; `Gamepad_LeftTriggerAxis` / `RightTriggerAxis` are referenced by ride states, the gummi-ship
  controller, a gimmick sensor and debug cameras only).
- `ActionMappings` [A]: `ActionCameraReset` = `Gamepad_RightThumbstick` (line 38; bound in 0x1414160F0 with
  event byte 0 = pressed [I for the byte, U for its meaning]); `ActionCameraPreview` = `Gamepad_Special_Left`
  (line 39) is **not bound by any code** (the name does not occur in the executable). There are no
  `AxisMappings` in either ini file; the axes "Turn" and "LookUp" that 0x1414160F0 binds therefore stay 0.
  `DebugExecBindings` (lines 9–15) are development commands.
- Left stick in the controller: vector (LeftY, LeftX), inactive below magnitude 0.1, not clamped (movement
  spec 2.3). Right stick: camera spec 2.5 (Y negated at 0x1405CD525).
- Mouse → right stick [C] (0x14442BC20, registered with stick index 1, flags "invert Y" set and "invert X"
  clear at 0x1410748AC): `mx`, `my` = massaged `MouseX` / `MouseY` (× 0.07); nothing is sent while both are
  below 0.01 in magnitude (the stick keys are then released); otherwise each is divided by the frame time
  relative to 1/60 when the world's value at +0x94C is positive, multiplied by the scale `*0x1486DAAF0` = 0.5,
  clamped to ± `*0x1486DAAF4` = 3.0 and injected as `Gamepad_RightX` / `Gamepad_RightY` (so the mouse can
  exceed the stick's ±1). The two floats are written by 0x140EDF630; `UTresGameSettings.MouseSensitivityCustom`
  (+0x59) exists, its path to the scale was not read.
- UI layer [I]: the UI manager polls a pad object (manager+0x408) in 0x14158E390: 29 buttons × 12 bytes
  `{i32 state: 0 none, 1 pressed, 2 released, 3 held; f32 repeat timer; u8 repeat; u8 cleared each poll}`,
  state at `pad + 8 + 12·i`. Indices: 0 Top, 1 Bottom, 2 Right, 3 Left, 4 LB, 5 RB, 6 LT, 7 RT, 8 L3, 9 R3,
  10 DPad_Up, 11 DPad_Down, 12 DPad_Right, 13 DPad_Left, 14–17 LeftStick Up / Down / Right / Left, 18–21
  RightStick Up / Down / Right / Left, 22 Special_Right, 23 Special_Left, 24 Escape, 25 Insert, 26 Delete,
  **27 decide (variable 0x1487D7760), 28 cancel (variable 0x1487D7778)**. Repeat: 0.6 s after the press, then
  every 0.06 s (0x14158EED0). Accessors: pressed 0x141583B80 (`state == 1`), held 0x141583A60 (`state == 3`).
  Buttons 1 and 2 are special (0x14158F000 [C]): they read the *physical* `FaceButton_Bottom` / `FaceButton_Right`
  from the viewport client's raw key map and additionally the keyboard keys of slot 8 / slot 7. Consequence
  for the shortcuts: LB + B = LeftShift + LeftMouseButton (index 0) and LB + A = LeftShift + SpaceBar (index 3).
- Viewport client `InputAxis` 0x141523130 [C]: besides forwarding the axis it marks, in its own raw key map,
  `Gamepad_DPad_Left` / `Right` as down while raw `LeftX` < −0.5 / > 0.5 and `DPad_Up` / `Down` from raw `LeftY`
  (consumers of that map other than 0x14158F000 were not traced).

#### L5. Confirm / cancel swap and where rebinding is stored [I]

- `UTresGameSettings` (ctor 0x140EC22D0, defaults 0x140EE0CA0): `GamePadDecideType` u8 +0x50 (default 0),
  `GamePadIconType` +0x51, `KeyboardDispayType` +0x52, `KeyConfigCustom` +0x58, `MouseSensitivityCustom` +0x59,
  `KeyConfigMain` +0x80 (21 × `FKey`), `KeyConfigSub` +0x278 (21 × `FKey`) [offsets from reflection].
- The apply function 0x140EC5EC0 sets the cvar, calls 0x140EDF630 (defaults) and 0x140EDE4E0 (settings arrays
  → key table); 0x140ECAFE0 copies the key table back into the settings (key-config menu, caller 0x14170D3B0).
  Order of the settings arrays (key-table slot): 7, 8, 5, 6, 14, 13, 17, 16, 2, 3, 0, 1, 4, 15, 18, 9, 10, 11,
  12, 20, 19. 0x140EDE4E0 has no write to the sub keys of slots 9 and 10 (the mouse wheel stays).
- `i.GamePadDecideType` ≥ 1 swaps exactly the two key variables, hence at once: attack ↔ jump on the pad, and
  UI decide (27) ↔ cancel (28). Nothing else moves; the keyboard slots 7 / 8 follow the variables (slot 7
  stays "attack / decide").

#### L6. Not determined

1. RT hold (> 0.2667 s) mode: controller+0x968, slot 715 0x141406F80, needs request-interface vt+0x5F8() ≠ 0.
2. What HUD button 6 (LT) does: 0x14162BAF0(hud, 1, …) when hud+0x9C8 is 0.
3. `Gamepad_LeftShoulder` just-pressed → request-interface slot 101 (+0x328).
4. Any reader of L3 for Sora on foot; controller slot 716 0x141404520 (RT just-pressed) and slot 914
   0x141404600 (View just-pressed) have no direct caller, and no virtual call was found in the decompiled C
   of the 0x1405E–0x14061 and 0x1413E–0x14141 ranges (other ranges not searched).
5. Pause: what the bit 0x40 set by 0x1441B6B80 starts; View in the UI (0x1415BD110 reads UI button 23 when
   its own flag +0x498 is set, and buttons 24 Escape and 3).
6. `UTresPlayerState_Run` tick 0x1413AFAB5: `Gamepad_RightTrigger` down multiplies the speed passed to pawn
   vt+0x1570 by 5.0 in one branch [C]; the branch condition was not established.
7. Shortcut contents (three sets, save data), item / link shortcuts: only the four execution indices were read.
8. Whether the camera-reset binding fires on press: the event byte is 0 [I]; its identity as `IE_Pressed`
   rests on the stock `FInputActionBinding` layout [U].
9. No first-person toggle key was found other than the gummiphone camera (View).
10. Runtime: none of the above was checked in the running game (probe fields: controller+0x934 / +0x938 /
    +0x93C..+0x940, key states of the two variables).

---
