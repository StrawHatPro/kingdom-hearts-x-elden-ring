# Enemy-side hit reaction: what a KH3 enemy does when one of Sora's hits lands

Written 2026-10-12 (enemy-reaction implementer). Companion of `docs/sora-combat-spec.md`: that spec ends where the
attacker has decided a hit (row, damage, reaction KIND, classification against the victim's body: its sections 8.2 – 8.7)
and describes the victim side only for Sora himself (section 9). This document is the victim side of a generic ENEMY:
the state `UTresEnemyState_Damage`, its physics, its timers, and what gates it.

Evidence tags, on every statement or block:

- **[I]** read at instruction level (disassembly with iced-x86; constants as the bytes in the image).
- **[C]** decompiled C only (`re/decomp/…`; every function below has verdict `not-compiled`, so [C] is weaker than [I]:
  control flow as Ghidra shows it, not proven).
- **[A]** read from an asset (cooked package JSON under `assets_converted/packages`, a data table under `sheets/entities`,
  an animation under `assets_converted/animations`).
- **[U]** stock UE 4.17 behaviour assumed, function identity not checked.
- **[D]** derived from the above by arithmetic or by stepping the implementation (`crates/kh-react`, test
  `tests/spec_numbers.rs`; the numbers quoted are the ones `cargo test -p kh-react -- --nocapture` prints).
- **[?]** not determined.

Constants live in `sheets/port/sora-enemy-reaction.json` (53 rows, written by `tools/sora_enemy_reaction_sheet.js`,
generated into `build/gen/rust/sora_enemy_reaction.rs`). Scratch tools and listings: `build/sora_tmp/react/`.

---

## 0. Summary

**KH3 has no "impulse" and no weight.** A hit carries a damage KIND (one of `ETresDamageKind`) and a few row fields; the
victim's own damage state turns the kind into motion with numbers from the VICTIM's parameter object
(`TresDamageCommonParam`), not from the attack. Almost every enemy uses one shared parameter asset, so the smallest and the
largest enemy that react fly the same arcs; what differs per enemy is whether the hit starts a reaction at all (body reaction type,
reaction endurance, armour) and how long its own clips are. There is no hit-stop (combat spec 8.5.3), no damage scaling
or invulnerability while lying, no juggle counter and no bounce.

What each kind does to a generic enemy (power scale 1, 60 Hz; distances along the hit direction):

| Kind (rows of Sora) | Motion | Numbers [D from I / A] | Ends |
|---|---|---|---|
| SMALL on the ground (every basic combo hit) | slides back, stays standing | 550 cm/s × 0.95 per frame: 175 cm during the 1.0 s clip (183 cm limit) | when the flinch clip ends (median 1.0 s); a new hit restarts it |
| SMALL in the air (every air combo hit) | **fall stopped, gravity off, pushed away and UP** | 550 cm/s away and 200 cm/s up, both × 0.95 per frame; gravity returns when the horizontal speed is ≤ 150 (after 0.47 s); rises 51 cm, drifts 135 cm | air clip (0.77 s) → fall loop → lands **on its feet** (`Land`) |
| BLOW on the ground (ground finishers) | knock-down slide | 1500 cm/s × 0.91 per frame, no lift: 278 cm | down + get-up clip (1.67 s); total 1.8 s |
| BLOW in the air (air finishers) | thrown away, tumbling | 2000 cm/s × 0.95 per frame, 300 cm/s up, gravity off until ≤ 250 cm/s: 663 cm away, +120 cm | falls, lands lying, 1.67 s clip |
| TOSS (Rising Spiral and the other launchers) | straight up | 1500 cm/s against 3 g: **383 cm after 0.51 s**, 0.2 s hover with gravity off, falls at 1 g; airborne 1.6 s | lands lying, 1.7 s clip |
| LAUNCH (no row of Sora) | straight up, higher | 2500 cm/s: 1063 cm after 0.85 s, 0.2 s at ¼ g; airborne 2.45 s | as TOSS |
| BEAT (air-down attacks, Groundbreaker) | driven straight down | 1500 cm/s down, spaced out from under the attacker | at the landing (most enemies have no slam clip): no down time |
| SIDESPIN / VERTICALROLL (Triple Slash end, Magic Flash, counters) | **the clip's root motion**: spun away at constant height | 470 / 450 cm in 0.87 s (medians), decelerating | falls, lands lying, 1.37 s clip. Enemies without the clip: BLOW |
| PARABOLA (no row of Sora) | ballistic | 1500 cm/s at 45°, × 0.98 per frame above 300 cm/s: 968 cm, apex 574 cm | lands lying |
| wall, while BLOW / PARABOLA | sticks to the wall | needs > 100 cm/s and ≤ 60° from head-on; 0.5 s stuck with gravity off | drops, 1.8 s clip |

**The juggle** is the second row: every air hit zeroes the victim's velocity, switches gravity off for about half a
second and gives it 200 cm/s upward. Hits less than about half a second apart therefore make the enemy GAIN height (about
43 cm per hit at a 0.35 s rhythm [D]) instead of dropping, for as long as the hits keep coming. Nothing in the damage
state limits this. What ends a juggle in the game is one of: the combo's finisher (a BLOW / spin throws the enemy out of
reach), Sora's own fall, the enemy's death, or — only on enemies that have one — the **revenge value** (section 5.3).

Bosses and large enemies: a STRONG hurt shape with a reaction endurance (`m_MaxBodyStrongValue`). Sora's basic hits
(reaction power 0) never move them; finishers (1) and counters (5) wear the endurance down, the hit that empties it and
every later hit WITH reaction power lands in full for 10 s; big kinds without power become a flinch (section 5.1).

---

## 1. Objects [I for vtables and slots, A for assets]

| Thing | Where |
|---|---|
| Enemy pawn | `ATresEnemyPawnBase` (vtable 0x145D99398, constructor 0x140DDE9B0) : `ATresAIPawnBase` : `ATresCharPawnBase`. Interfaces at pawn+0x440 (anim), +0x448 (notify), +0x450 (request; vtable 0x145D90CA0), +0x460 (actor; vtable 0x145D91460) |
| Damage state | `UTresEnemyState_Damage` (vtable 0x145D98688, size 720) : `UTresAICharState_Damage` : `UTresCharState_Damage` : `UTresStateBase`; state id 0x13. Registered in the pawn constructor with the other default states (`Idle`, `Die`, `Reflect`, `BadStatStun`, `BadStatFreeze`, `Bind` …). 63 classes derive from it directly (`TresEnemyState_e_*_Damage`; 83 damage-state classes in all); those subclasses were NOT read |
| Parameters | `UTresEnemyCommonParam` : `UTresDamageCommonParam` (constructors 0x140DC4270, 0x14064C130); the object is state slot 141 (0x140DD1D10 [C]): the class default object of the pawn's `m_EnemyCommonParam` (pawn+0xB50, a class property) when set, else the game mode's. Asset: `/Game/Blueprints/Enemy/EnemyCommonParam` |
| Body row | `TresChrBaseParamDataTable` (`sheets/entities/DataTable/Enemy/Base/*_BaseDataTable.json`): `m_BodyPushPower`, `m_MaxBodyStrongValue`, `m_MaxArmorHP`, `m_RevengeLimit`, `m_MaxRevengeCount`, `m_RevengeCoolDownTime`, `m_RevengeCoefficient*`, `m_ResistComboParam` |

State fields used below [I for the offsets that appear in instructions quoted, C otherwise]:

| Offset | Meaning |
|---|---|
| +0x68 | the pawn |
| +0x78 | sub-step |
| +0x88 … +0xEF | the hit's `TresDamageInfo` (+0x92 kind, +0x93 attribute, +0x98 power scale, +0x9C parabola angle, +0xA8 effect time, +0xC0 bad-status time); +0xF0 the previous hit's |
| +0x158, +0x164 | hit direction raw / normalised in XY |
| +0x17C | info.effectTime |
| +0x180 / +0x188 | array of every info of this reaction chain, count |
| +0x198 | clip being played |
| +0x1A0 | instigator (weak) |
| +0x1A8, +0x1B4, +0x1C0 | knock direction, held facing, start location |
| +0x1CC, +0x1D0, +0x1D4 | power (XY speed), brake, vertical speed |
| +0x1D8, +0x1DC | direction index, flinch variant |
| +0x1EC | handler timer |
| +0x1F0 | bit 0 brake every tick, bit 1 was Flying at Begin, bit 2 **landed** (set by the state's event interface, 0x1404444A0 [C]) |
| +0x27C | hits in this state |

Handlers (bound in Setup by the damage kind; thunks 0x140430C70 … jump through the vtable [I]):

| Kind | Handler | Slot | Address |
|---|---|---|---|
| 2 BLOW, 5 BILLIARD, 21 SHIELD_BLOW | Blow | 117 | enemy override **0x140DD8980**, base 0x140446FE0 |
| 3 SIDESPIN | SideSpin | 118 | 0x14044EDC0 (BLOW if the enemy has no clip) |
| 4 VERTICALROLL | VerticalRoll | 119 | 0x140452E00 (BLOW if no clip) |
| 6 FLOAT, 7 TOSS, 22 SHIELD_TOSS | Toss | 120 | 0x1404527F0 |
| 8 LAUNCH | Launch | 121 | 0x1404499F0 |
| 9 BEAT | Beat | 122 | 0x1404463F0 |
| 10 PARABOLA, 11 PARABOLA_NO_RECOVERY | Parabola | 127 | 0x14044B990 |
| freeze on the pawn | Frozen | 123 | 0x140446BF0 (not read) |
| 16, 19, 20, 23 – 40 (MAGNET, SUCTION, EAT, the attraction and bind kinds) | own handlers, slots 124 – 140 | | **not read, not ported** |
| everything else (0 NONE, 1 SMALL, …) | Small | 116 | 0x14044F650 |

---

## 2. From the hit to the reaction

### 2.1 `TakeDamage` [C]

`ATresCharPawnBase::TakeDamage` (0x1405BB850) is shared with the player and specified in combat spec 8.4. For this
document: HP is subtracted first; the revenge bookkeeping runs (section 5.3); then, unless the pawn says "no reaction"
(pawn vt+0x2070, `m_bIsNoDamageResponse`) or the victim result is NOREACTION / a guard result, the pawn's request
interface slot 88 is called: the reaction gate. A lethal hit goes to the die request instead (2.4).

### 2.2 The enemy's gate (request interface slot 88, 0x140DEF290) [C; the STRONG rule I]

```
if !iface.slot87():  return false                 // 0x140DE2E80: current / next state 0x3B..0x3E or 0x40 (dying, vanishing ...),
                                                  // pawn vt+0x2540, a damage state of kind 0x17, an action that refuses
if info.kind in {0x23, 0x24}: return false
if current or next state id is 0x3D / 0x3E: return false
if Freeze(pawn, info) (0x140DEF530): return true  // pawn vt+0x20C0 says the hit freezes: state 0x7B BadStatFreeze, time = info.badStatusTime
if pawn.vt+0x2618(...): return false              // base enemy: stub, false
if !pawn.vt+0x2728(...): return false             // 0x140DF84F0, the STRONG-body rule, below
if current state is DAMAGE (0x13): a few kind pairs are swallowed (0x10 / 0x27 / 0x1D .. 0x20 with SMALL), else the state is re-used
if iface.slot153() (0x140DF0030: a random "damage action" of the enemy's own list starts instead): return false
state = current DAMAGE state, or SetState(0x13)
state.Setup(info, amount, hit, dir, causer)       // slot 114
return true
```

A hit during a reaction re-enters the reaction: an enemy can be hit while flinching, flying, lying or getting up, with
no invulnerability [C: the gate has no such test].

The STRONG-body rule, pawn vt+0x2728 = 0x140DF84F0 [I, whole function]:

```
if !pawn.vt+0x2720(): return true                              // base: always true
c1 = (info.kind == SMALL)                                      // pawn vt+0x2740 0x140DEC8F0
c2 = info.kind in {2, 3, 4, 7, 8, 9, 10, 11, 29}               // pawn vt+0x2748 0x140DEC8D0: mask 0x20000F9C ("big" kinds)
r = info.victimResult                                          // info+0x60, decided by the attacker's Classify (combat spec 8.2.6)
if r == 7 (STRONG_BODY_DAMAGE1):      if c1 || c2: pawn.vt+0x2738(...); return false
elif r == 8 || r == 9:                if c1: pawn.vt+0x2738(...); return false
                                      if c2: info.kind = SMALL                      // mov byte ptr [rdi+0Ah],1
return pawn.vt+0x2730(...)                                     // base: true
```

`pawn.vt+0x2738` is empty on the base enemy [I 0x14041F120]. `ATresEnemyGigasPawnBase` overrides it (slot 1255,
0x140DF3960 [C]): an ADDITIVE full-body shake (`M_751` … `M_754` by direction, slot `FullBodyAdd`), not more often than
`m_pro_AddReactionDisableTime` = 0.2 s [A] (timer pawn+0xC34). So a giant that ignores a hit still shudders.

With the classification of combat spec 8.2.6 (`left = slot32(reactionPower)`; `V = left > 0 ? (rp > 1 ? 8 : 7) : (rp > 0 ? 10 : 9)`)
the outcomes for a STRONG hurt shape are:

| Endurance after the hit | Reaction power of the row | Victim result | SMALL | Big kinds |
|---|---|---|---|---|
| > 0 (holds) | 0 or 1 | 7 | nothing | nothing |
| > 0 (holds) | ≥ 2 | 8 | nothing | a flinch (SMALL) |
| 0 (broken) | 0 | 9 | nothing | a flinch (SMALL) |
| 0 (broken) | ≥ 1 | 10 | full | full |

A DEFAULT hurt shape gives DAMAGE (2): every kind in full.

### 2.3 Setup (slot 114, 0x140457380) [C; constants I]

```
copy the previous info to +0xF0, the new info to +0x88;  append it to the list +0x180
+0x158 = dir;  +0x164 = normalize2D(dir)                         // zero when |dir.xy|^2 < 1e-8
flags(+0x1F0) &= ~5 (brake, landed);  +0x17C = info.effectTime
causer -> instigator pawn (a weapon is replaced by its owner)
if +0x164 == 0:  +0x164 = instigator valid ? normalize2D(forward of the instigator) : -normalize2D(forward of the victim)
StopMovement (pawn vt+0x16E8);  SetActorRotation(0, current yaw, 0)
slot 177 (0x140455C70): restore the walk speeds, the default GravityScale, gravity ON
sub-step = 0;  pawn+0xB00 = kind
(enemy with the hit's attribute as weak point and pawn kind byte 0x18..0x1B: kind = VERTICALROLL)
pawn.vt+0x1658 (GravityScale back to the class default)
if pawn.vt+0x20C0() (freeze):  handler = Frozen;  +0x1EC = info.badStatusTime
else:  attribute body effect;  handler = by kind (table of section 1);  SIDESPIN / VERTICALROLL need the clip, else kind = BLOW
if OnGround: +0x1E4 = 0, +0x1E8 = 0            // counters of the paint-attraction kind 0x18 only
+0x27C += 1
handler(dt)                                    // sub-step 0 runs inside Setup, in the frame of the hit
```

Not in Setup, unlike the player's (combat spec 9.3): no conversion of a lethal SMALL into a BLOW, no "recover delay", no
weapon sheathing. `info.m_DamageBrakeParam` and `info.m_DamageMoveLength` are **not read** by any AI handler [C: no
getter of slots 142 – 166 reads them]; the player's handlers do.

Tick (slot 97, 0x140440760) [C]: `if handler(dt): notify (pawn+0x448 vt+0x248), DoFinish; else +0x248 = max(+0x248 − dt, 0)`.
End (slot 98, 0x140434C50 → slot 177) restores speeds, GravityScale and gravity. The AI resumes when the state finishes.

### 2.4 A lethal hit [C]

`TakeDamage` → 0x1405A35D0 → request interface slot 129 (0x140DEF8A0): `SetState(0x3B)` (`UTresEnemyState_Die`) with the
hit's info, kind, attribute, direction, causer and impact point copied in. What the Die state does with them (whether the
body is blown away first) was **not read** [?]. `kh-react` answers `HitOutcome::Dying` and does nothing.

---

## 3. Parameters

### 3.1 The common enemy values [A over I]

Getter = state slot; `scale` = `info.m_DamagePowerScale` (state slot 176, 0x14043A850; the enemy override 0x140DD1E80 adds
a random term only for kind 0x27). Enemy value = `/Game/Blueprints/Enemy/EnemyCommonParam` class default object [A] over the
native constructors [I, sheet `sora-combat-damage-params` for the base class]. Player column for contrast (combat spec 9.3).

| Property (offset) | Getter | Enemy | Player | Use |
|---|---|---|---|---|
| `m_SmallDamagePower` (+0x40) | 142 0x14043D230 [I] | **550** × scale | 550 | SMALL horizontal speed |
| `m_SmallDamageBrake` (+0x44) | 143 0x14043D200 | **0.95** (native) | 0.9 | SMALL brake per frame |
| `m_SmallDamageStartFallingSpeed` (+0x48) | 145 0x14043D280 | **150** | 150 | gravity back at or below |
| `m_AirSmallDamageVelocityZ` (+0x4C) | 146 0x140439D50 | **200** (0 on the ground) | 0 | **lift of an air hit** |
| `m_BlowDamageVelocityZ` (+0x50) | 148 0x14043A570 | **0** | 300 | ground BLOW lift |
| `m_BlowDamagePower` (+0x54) | 147 0x14043A4C0 [I] | **1500** × scale | (not read) | ground BLOW speed |
| `m_BlowDamageBrake` (+0x58) | 149 0x14043A460 | **0.91** | 0.95 | |
| `m_AirBlowDamageVelocityZ` (+0x5C) | 148 | **300** | 300 | air BLOW lift |
| `m_AirBlowDamagePower` (+0x60) | 147 | **2000** (native) × scale | 2000 | air BLOW speed |
| `m_AirBlowDamageBrake` (+0x64) | 149 | **0.95** | 0.95 | |
| `m_AirBlowDamageStartBrakeTime` (+0x68) | 152 0x140439D00 | **0.1** (0 on the ground) | 0 | wait before the vertical brake |
| `m_BlowDamageStartFallingSpeed` (+0x6C) | 153 0x14043A530 | **250** | 700 | gravity back at or below |
| `m_TossDamagePower` (+0x70) | 155 0x14043DCB0 | **1500** (native) × scale | 1500 | |
| `m_TossDamageFlightDuration` (+0x74) | 156 0x14043DC80 | **0.2** | 0.2 | apex hover |
| `m_TossDamageFallGravityScale` (+0x78) | 157 0x14043DC50 | **1.0** (native) | 1.0 | |
| `m_LaunchDamagePower` (+0x7C) | 158 0x14043B8A0 | **2500** × scale | 2500 | replaced by info.effectTime when > 0 |
| `m_LaunchDamageFlightDuration` (+0x80) | 159 0x14043B870 | **0.2** | 0.2 | |
| `m_LaunchDamageFallGravityScale` (+0x84) | 160 0x14043B840 | **1.0** | 1.0 | |
| `m_BeatDamagePower` (+0x88) | 161 0x14043A410 | **1500** (native) × scale | 2500 | |
| `m_BeatDamageRebound` (+0x8C), `…ReboundGravityScale` | not read by the AI handler | (1500, 3) | 1000, 3 | **enemies do not bounce** |
| `m_ParabolaDamagePower` (+0x98) | 163 0x14043C570 | **1500** × scale | 1500 | |
| `m_ParabolaDamageAngle` (+0x9C) | 164 0x14043C4C0 | **45** (info.parabolaAngle when > 0) | 45 | |
| `m_ParabolaDamageBrake` (+0xA0), `…LimitBrake` (+0xA4) | 165, 166 | **0.98**, **300** | same | |
| `m_BlowDmgWallHitSpeed` (+0xFC) | enemy 151 0x140DD1C90 | **100** (native 50) | 50 hard-coded | wall hit needs more |
| `m_BlowDmgWallHitAngle` (+0x100) | enemy 150 0x140DD1C10 | **60°** (native 30) | 30 hard-coded | |
| `m_SmallDmgGravityScaleReinstateTime` (+0xF8) | no reader found | 0 | | [?] |
| turn rate | 144 / 154 0x14043A560 [I] | 0.5 | | facing: alpha = 0.5 × dt × 60 |

Handler constants [I]: GravityScale **3.0** while a TOSS / LAUNCH rises (`mov dword ptr [rcx+2F8h],40400000h` in
0x1404527F0 and 0x1404499F0); **0.25** during the LAUNCH hover (0x1404499F0); 150 cm/s (22500 squared) and −300 cm/s in
the enemy Blow; 0.5 s and 10 s in Parabola; 2500 cm and 0.7071 in Beat; turn factors 15 / 30 / 60.

Gravity: world gravity 980 cm/s² × the movement component's `GravityScale` [U]. No class default object of a generic
enemy's movement component sets `GravityScale` (7 enemy blueprints do: 0.4, 1.5 ×2, 4, 4.5 ×2, 5) [A]: 1.0 is assumed [U].
**This is the weakest number of the vertical motion**: the fall after a TOSS and every "gravity back" phase use it.

### 3.2 Per-enemy overrides [A]

Blueprint children of `EnemyCommonParam` (each sets only the listed properties):

| Asset | Override |
|---|---|
| `e_ex005` | `m_ReflectKnockBackPower` 600 |
| `e_ex015` / `e_ex016` / `e_ex073` | `m_AirReflectKnockBackVelocityZ` 500 / 300 / 600 |
| `e_ex026_s0` | `m_SmallDamageBrake` 0.96 |
| `e_ex322` | `m_BlowDamageVelocityZ` 400, `m_BlowDamagePower` 2500 |
| `e_ex358` | `m_AirReflectKnockBackPower` 500, `…Brake` 0.98 |
| `e_ex367` | `m_SmallDamageStartFallingSpeed` 50 |
| `e_ex702`, `e_ex703` | `m_SmallDamagePower` 0 (flinches in place) |
| `e_ex731` | `m_AirBlowDamageVelocityZ` 0, `m_AirBlowDamagePower` 2800, `m_AirBlowDamageBrake` 0.9, `m_BlowDamageStartFallingSpeed` 1000 |
| `e_fz903` (child of the native class) | `m_ParabolaDamagePower` 4000, `…Angle` 10, `…Brake` 0.85 |
| `e_he902`, `e_he903`, `e_he904` | every power 0 (reacts without moving) |

`Npc/FriendDamageCommonParam` (party members): `m_SmallDamagePower` 1500, brake 0.86, the rest close to the enemy's.

### 3.3 What Sora's rows carry [A: `sheets/port/sora-combat-attack-data.json`, already complete]

The attack-data sheet has every field a victim reads: `damage_kind`, `power_scale`, `parabola_angle`, `move_length`,
`brake`, `effect_time`, `knockback`, `reaction_power`, `armor_attack`, `revenge_point`, `bad_status` / `bad_time`.
**Nothing had to be extracted**; `tools/sora_combat_sheet.js` was not changed. `kh_core::HitInfo` carries them all
(`power_scale`, `parabola_angle`, `move_length`, `brake`, `effect_time`, `reaction_power`, `revenge_point`, `bad_*`),
`HitRecord::direction` is the knockback direction and `HitRecord::damage_kind` the kind.

Of the 87 rows: `parabola_angle`, `move_length` and `brake` are 0 on every row; `effect_time` is set only on the two MAGNET
rows (1.5). So for Sora the only row fields that shape an enemy's motion are **the kind and the power scale**:

| Kind | Rows (power scale 1 unless given) |
|---|---|
| SMALL | every basic ground and air hit: `KB_VERTICAL`, `KB_STAB`, `KB_TURN`, `KB_RISE`, `KB_ROLLDOWN`, `KB_QUICKSHAFT(_END)`, `KB_FLASHSTEP(_END)`, `KB_RADIALCUT`, `KB_AERIALSWEEP(_END)`, `KB_AERIALDIVE`; the leading hits of finishers (`KB_TRIPLE`, `KB_THRUST`, `KB_ROLLING`); `FF_SONIC_1`, `FF_SONIC_TURN` (0.5), `REVENGE_DIVE` (0.5), `LM_EXPLOSION` (0.1), `LM_LASTARCANUM*` (0.5 / 2) … |
| BLOW | `KB_BREAK` (ground finisher), `KB_THRUST_END`, `KB_ROLLING_END` (air finisher), `KB_DOWN_BOMB`, `KB_FINALHEAVEN`, `FF_SONIC_BOMB`, `BLOW_COUNTER_START`, `RDC_BACKRUSH_END`, `LM_CHANGE`, `LM_EXPLOSION2` (0.5), `LM_AERIALFINISH2` (0.75), `LM_UNIONDISCODE4` |
| SIDESPIN | `KB_TRIPLE_END`, `REVENGE_IMPACT`, `REVENGE_EX_END` (0.8), `LM_RIPPLEDRIVE` |
| VERTICALROLL | `KB_FLASH`, `KB_FINALHEAVEN_END`, `REVENGE_SLASH`, `REVENGE_DIVE_END` (1.5), `RDC_HOOKUPPER_END`, `DIVEFALL_ATTACK_END`, `FF_SONIC_TURN_END`, `FF_SONIC_SS_END`, `ATK_BARRIERCRACKER`, `LM_MAGNESPLASH2` |
| TOSS | **`KB_LAUNCHSPIN` (Rising Spiral, the launcher)**, `RUN_SLOPE`, `RUN_WALL_RISE`, `RUN_WALL_SIDE`, `FF_SONIC_2`, `LM_SONICRAVE_END`, `LM_STRIDEBREAK2` (0.85), `LM_HURRICANEPERIOD2` (0.75), `LM_UNIONDISCODE` (0.5), `LM_LASTARCANUM_END` (0.4) |
| BEAT | `KB_DOWN`, `KB_DOWN_FIN`, `KB_ROLLBEAT` (Groundbreaker), `DIVEFALL_ATTACK`, `RUN_WALL_DOWN`, `FF_SONIC_DOWN`, `RDC_WARPSLASH_END`, `LM_LASTARCANUM_END2` |
| MAGNET | `LM_MAGNESPLASH`, `LM_UNIONDISCODE2` (not ported) |
| LAUNCH, PARABOLA | none |

Reaction power: 0 on the basic hits, 1 on finishers and most specials, 5 on the counters and the big enders
(`REVENGE_*`, `RDC_*_END`, `BLOW_COUNTER_START`, `LM_STUNIMPACT`, `LM_SONICRAVE_END` …). Revenge points: 1.0 on the basic
ground hits, 0.2 – 0.6 on the ability replacements, 0.5 on finishers, 0 on every counter and Second Form ender.

The spins ignore the power scale (the motion is the clip's root motion).

---

## 4. The handlers

Helpers as in combat spec 9.3 (`OnGround`, `Gravity(b)`, `GravityScale(x)`, `SetVel3`, `SetVelXY`, `SetVelZ`,
`StopMovement`, `Turn(rot, alpha)`, `Play(clip, mode)`, `AnimEnd`, `Brake(b)` = `SetVelXY(Velocity × b^(dt × 60))`
0x140431E10). "landed" = flag bit 2. Every handler returns true to finish the state.

Clips are named per enemy by pawn functions; the common enemy names [C strings, A files]:

| Use | Names | Median length [A] |
|---|---|---|
| ground flinch by direction (front / back / right / left, two variants) | `200` … `206` | 1.0 s (82 enemies: 0.27 … 4.6) |
| air flinch | `210` … `216` | 0.77 s |
| BLOW start / loop / down (+ get-up) by direction | `240`+i, `_1`, `_2`, (`_3` on one enemy) | 0.1 / loop / **1.67 s** (0.67 … 5.1) |
| TOSS start / rise loop / fall start / fall loop / down / (get-up) | `m_250`, `_1`, `_2`, `_3`, `_4`, (`_5` on one enemy) | 0.1 / loop / 0.37 / loop / **1.7 s** |
| BEAT fall / slam / get-up | `m_250_3`, `m_259`, `m_259_1` | `m_259` exists on **3** enemies |
| SIDESPIN spin / down / get-up | `m_260`, `_1`, `_2` | 0.87 / 1.37 s (61 enemies) |
| VERTICALROLL front / back, down variants | `m_270` / `m_271`, `_1` / `_1B` / `_1C`, `_2` | 0.87 / 1.57 s (67 enemies) |
| wall hit / loop / down | `280` / `281`, `_1`, `_2` | 0.5 / loop / 1.8 s (46 enemies) |
| air-blow "break" fall | `DmgAirBlowBr{Front,Back,Right,Left}Loop` / `End`, fallbacks `m_280_1` / `m_281_1`, `_2` | [?] |
| landing on its feet, fall loop | `Land`, `JumpNormalDownLoop` | [?] not looked up |

Enemy clips carry **no `TresEnableChange` notify** [A: checked on `e_ex001`]: there is no early exit; a phase ends when
its clip ends. The down clips contain landing, lying and getting up in ONE clip.

### 4.1 Small (slot 116, 0x14044F650) [C; constants I]

```
sub 0: index = direction index of the knock (4-way: 0 attacker in front, 1 behind, 2 right, 3 left; pawn vt+0x23F0)
       knock = +0x164;  if index < 2: variant ^= 1
       power = SmallPower();  velZ = AirSmallVelZ() (0 on the ground);  sub = 1
sub 1: clip = Small(index, variant)                        // the ground set on the ground, the air set off it
       if !OnGround:  Gravity(off);  if !IsFlying: SetVel3(power x knock.x, power x knock.y, velZ)
       else (or Flying): SetVelXY(power x knock)
       Play(clip, 2);  flags |= brake;  sub = 4;  return                  // no brake in this tick
sub 4: Turn(facing, 0.5 x dt x 60)                         // facing: toward the attacker (index 1: away)
       if GravityOff():
           if !OnGround && !IsFlying: SetVelZ(Velocity.z x SmallBrake()^(dt x 60))
           if |Velocity.xy| <= SmallStartFallingSpeed(): Gravity(on)
       if OnGround && the clip playing is not Small(index, variant) (it landed during the air clip):
           if AnimEnd (or m_bEnableChange):  Play("Land", 2);  sub = 5
       elif AnimEnd:
           if an enemy whose DefaultLandMovementMode is Walking and it is not on the ground:
               Play("JumpNormalDownLoop", 3);  Gravity(on);  sub = 15
           else finish
sub 5:  if AnimEnd: finish
sub 15: if landed: Play("Land", 2);  sub = 5
every tick but sub 1: if brake flag: Brake(SmallBrake())
```

On the ground [D]: 550 cm/s, × 0.95 per frame: 175 cm by the end of a 1.0 s clip (183 cm in the limit); the enemy stays
on its feet and the AI is back when the clip ends. Hit again, the clip restarts with the other variant.

In the air [D]: Setup has already zeroed the velocity. From the next tick: 550 cm/s away and 200 cm/s up with gravity
OFF, both braked × 0.95 per frame; when the horizontal speed is ≤ 150 (26 frames) gravity returns. One hit: +51 cm at
0.52 s, 135 cm of drift, gravity back at about 0.47 s; left alone it then falls, plays its fall loop from the end of the
air clip, lands on its feet (`Land`) and is free. From 300 cm that is 1.37 s to the ground.

### 4.2 Blow (enemy override, slot 117, 0x140DD8980) [C; constants I]

```
sub 0: index (4-way), knock;  ground = OnGround
       power = (ground ? m_BlowDamagePower : m_AirBlowDamagePower) x scale;  brake = ground ? 0.91 : 0.95
       velZ = ground ? 0 : 300;  sub = 1
sub 1: if the instigator is gone: finish
       if !OnGround: Gravity(off)
       clip = Blow(index, 0);  SetMovementMode(Falling);  SetVel3(power x knock.x, power x knock.y, velZ);  Play(clip, 2)
       flags |= brake;  +0x1EC = ground ? 0 : 0.1 (start-brake time);  sub = 2;  return
sub 2: Turn(facing, 0.5 x dt x 60)
       if !landed:  if AnimEnd && clip is the start: Play(Blow(index, 1), loop)
                    +0x1EC -= dt;  if <= 0: sub = 3
       else:        wait for the start clip to end;  Play(Blow(index, 1), loop);  sub = 3
sub 3: start clip still playing: wait;  then Play(loop);  sub = 4 (falls into 4)
sub 4: if GravityOff():
           SetVelZ(Velocity.z x brake^(dt x 60));  if |Velocity.xy| <= 250: Gravity(on)
       elif !landed:
           was Flying:  if |Velocity.xy| <= 150: -> down
           else:        if |Velocity.xy| <= 150 && Velocity.z <= -300:
                            Play(DmgAirBlowBr<dir>Loop, loop);  sub = 6            // tumble -> fall pose
       else: down:  Play(Blow(index, 2), 2);  (was Flying: Gravity(off), SetVelZ(0));  sub = 6
sub 6: down clip:  if AnimEnd: Blow(index, 3) exists ? Play it, sub = 7 : finish
       fall loop:  if landed: Play(DmgAirBlowBr<dir>End, 2); then the wall get-up clip when the enemy has one, else finish at its end
sub 7: if AnimEnd: finish
sub 8 - 11: wall hit, 4.7
every tick but sub 1: if brake flag: Brake(brake)                // also through the down clip
```

The base-class version (0x140446FE0, used by friends) differs: no fall-pose branch; `Turn` in subs 3, 4, 6, 7 too.

On the ground [D]: gravity stays on and there is no lift, so the "flight" is a slide: the engine lands the body in the
next movement tick, the 0.1 s start clip plays out, then the down clip (1.67 s) while the brake runs: 278 cm × scale,
half of it in the first 7 frames, lying from 0.13 s, free at 1.8 s.

In the air [D, from 300 cm]: 2000 cm/s away, 300 cm/s up with gravity off; the vertical brake starts after 0.1 s; gravity
returns when the horizontal speed is ≤ 250 (0.7 s); apex +120 cm at 0.77 s; the fall pose from 1.08 s; lands 663 cm away
at 1.7 s; lying 1.67 s.

### 4.3 Toss (slot 120, 0x1404527F0) and Launch (slot 121, 0x1404499F0) [C; constants I]

Identical except where marked. No horizontal velocity is given and the facing is not changed.

```
sub 0:  power = m_TossDamagePower x scale | m_LaunchDamagePower x scale;  Launch: if info.effectTime > 0: power = effectTime
sub 1:  SetMovementMode(Falling);  Gravity(on);  Velocity.z = power;  GravityScale = 3.0;  Play("m_250", 0);  sub = 12
sub 12: if AnimEnd: Play("m_250_1", loop);  sub = 13
sub 13: if vertical speed <= 0:  Toss: Gravity(off) | Launch: GravityScale(0.25)
                                 +0x1EC = FlightDuration (0.2);  sub = 16
sub 16: +0x1EC -= dt;  when <= 0:  Gravity(on);  GravityScale = FallGravityScale (1.0);  Play("m_250_2", 0);  sub = 14
sub 14: if AnimEnd: Play("m_250_3", loop);  sub = 15
sub 15: if landed: StopMovement;  Play("m_250_4", 2);  sub = 6
sub 6:  if AnimEnd: "m_250_5" exists ? Play it, sub = 7 : finish
sub 7:  if AnimEnd: finish
```

No timer ends these handlers in the air (the player's does): the enemy always comes down and lies.

Toss [D]: 1500 cm/s against 2940 cm/s²: apex **382.7 cm after 0.51 s** (`1500² / (2 × 2940)`), 0.2 s hover (it drifts
down 4 cm at the −19 cm/s it had when the apex was seen), then 1 g: on the ground at **1.62 s**, lying for the 1.7 s clip,
free at 3.3 s. Launch: **1063 cm after 0.85 s**, 0.2 s at ¼ g, on the ground at 2.47 s.

For an attacker that means: after Rising Spiral the enemy is above 300 cm from 0.3 s to 1.05 s after the hit and hangs
around 380 cm from 0.45 to 0.75 s; the first air hit converts it into the juggle of 4.1.

### 4.4 Beat (slot 122, 0x1404463F0) [C; constants I]

```
sub 0:  power = m_BeatDamagePower x scale (1500)
sub 1:  onGround = on the ground;  SetMovementMode(Falling);  Velocity.z = -power
        if instigator valid && !onGround && a ground trace straight down (2500 cm) hits:
            t = time to reach it from Velocity.z with gravity (the quadratic)
            if t > 0: Velocity.xy = knock x (0.7071 x own radius + 0.7071 x instigator radius) / t
        Play("m_250_3", loop);  sub = 15
sub 15: if landed: StopMovement;  Play("m_259", 2);  sub = 6            // no such clip: finish
sub 6:  if AnimEnd: "m_259_1" exists ? Play it, sub = 7 : finish
```

There is no rebound (the player's handler bounces; the AI one never reads `m_BeatDamageRebound`). `m_259` exists on 3 of
the 117 enemies with reaction clips [A], so for almost every enemy a BEAT is a spike: from 300 cm it is on the ground
0.2 s later, 56 cm further from the attacker, and **free at once** [D]. On a standing enemy a BEAT does nothing visible
(it "lands" in the next tick).

### 4.5 Parabola (slot 127, 0x14044B990) [C; constants I]

```
sub 0: index, knock;  +0x1C0 = location
sub 1: if the instigator is gone: finish
       Play(Blow(index, 0), 2);  SetMovementMode(Falling);  Gravity(on)
       SetVel3(m_ParabolaDamagePower x scale x Vector(pitch = ParabolaAngle(), yaw = yaw(knock)));  flags |= brake;  sub = 3;  return
sub 3: Turn(facing, dt x 30);  if AnimEnd: Play(Blow(index, 1), loop);  +0x1EC = 0.5;  sub = 4
sub 4: Turn;  +0x1EC -= dt;  when <= 0: sub = 5        // (was Flying && kind 11: +0x1EC = 10 s instead)
sub 5: Turn;  if landed, or back at or below its start height: StopMovement;  Play(Blow(index, 2), 2);  sub = 6
sub 6: Turn;  if AnimEnd: finish (or the get-up clip)
every tick but sub 1: if brake flag && |Velocity.xy| > 300: Brake(0.98)
```

[D] 1061 cm/s away and up: apex 574 cm at 1.1 s, 968 cm away at 2.18 s. No row of Sora uses it.

### 4.6 SideSpin (slot 118, 0x14044EDC0) and VerticalRoll (slot 119, 0x140452E00) [C]

These two are **animation-driven**:

```
sub 0: knock = -(hit direction)              // toward the attacker
       VerticalRoll: attacker behind (|yaw difference| > 90) and the enemy has `m_271`: the back variant (keeps its facing)
sub 1: SetActorRotation(yaw(knock)) at once;  Gravity(off);  StopMovement;  SetMovementMode(Flying)
       Play("m_260" | m_270 / m_271, root motion on);  sub = 4
sub 4: (the clip's root motion moves the pawn)
       if AnimEnd (on the ground also: as soon as the root track starts to descend):
           StopMovement;  Gravity(on);  SetMovementMode(Falling);  Velocity = the last root delta x 60;  sub = 5
sub 5: if landed: StopMovement;  Play("m_260_1" | variant of m_270_1, 2);  sub = 6
sub 6: if AnimEnd: get-up clip if it exists, else finish
```

The root bone (`kh_sk`) of `m_260` travels 230 … 740 cm (median **470**) straight back over 0.67 … 1.0 s (median 0.87);
`m_270` 240 … 611 (median 450); `m_271` the same distance forward (the victim then faces away, so it is again away from
the attacker); no vertical root motion (the arc is in the body bones) [A, `build/sora_tmp/react/rootmotion.js`]. The
speed decays: `e_ex003`'s `m_260` covers 53, 52, 45, 37, 33, 30, 27 … cm per 1/30 s (0.89 per 30 Hz frame) [A].

So a spin finisher carries the enemy ~4.7 m away at the height it was hit at, and only then does it fall. An enemy
without the clip takes the hit as a BLOW (Setup). **`kh-react` approximates the root track** by a speed that decays
× 0.95 per 1/60 s over the median distance and length; a host with real clips should move the body by its own clip.

### 4.7 Wall hit (state event interface, 0x1404434C0) [C; thresholds A / I]

Called with the blocking hit of the movement. Only for kinds 2, 5, 10, 11 (and 0x10, 0x1D, 0x1E with other tests):

```
if |Velocity.xy| <= m_BlowDmgWallHitSpeed (100): return
if knock . hit.ImpactNormal > -cos(m_BlowDmgWallHitAngle (60 deg)): return
if state slot 178 (0x140DD3200: some enemy kinds): Setup again with kind SMALL;  return
index: 0 / 1 kept; a side index becomes 0 or 1 by the side the wall is on
facing = yaw(wall normal) for index 0 (back to the wall), yaw(-normal) for index 1
brake flag off;  StopMovement;  Gravity(off);  Play(Wall(index, 0), 2);  sub = 8
```

then in the Blow handler: `sub 8`: `Turn(facing, dt × 15)`; at the end of the clip `StopMovement`, default GravityScale,
gravity on; on the ground → `Wall(index, 2)` (down), else Falling + `Wall(index, 1)` loop until landed → `Wall(index, 2)`;
at its end the get-up clip if any, else finish. [D] a body blown at a wall 150 cm away hits it 0.1 s later, hangs 0.5 s,
drops and lies 1.8 s.

A wall met at a shallower angle or more slowly just blocks (stock movement: the velocity into it is lost) [U].

### 4.8 Freeze, stun, the other kinds [C, summary]

- **Freeze**: when pawn vt+0x20C0 says the hit freezes (bad status FREEZE with a time, not immune), the GATE starts state
  0x7B `UTresEnemyState_BadStatFreeze` with the time and no damage state runs; handler 123 of the damage state is the
  fallback. The state itself was not read [?].
- **Stun** (`LM_STUNIMPACT`): the bad status is applied by `TakeDamage` (0x1405A2350) and `UTresEnemyState_BadStatStun`
  exists; not read [?].
- MAGNET, SUCTION, the attraction kinds: handlers 124 – 140, not read.

`kh-react` holds a frozen / stunned victim still for the bad status's time (dropping if it is in the air) and reports
the phase; that is a stand-in, not the game's states.

---

## 5. What gates a reaction

### 5.1 Reaction endurance ("body strong value") [I for the functions named]

A struct at pawn+0x814: `{ value, max, timer }`, `max` = `m_MaxBodyStrongValue` of the body row (set by 0x1405B6130 [C]).
`Classify` asks it through actor-interface slot 32 → 0x140576400 [I, whole function]:

```
if max <= 0: value = 0; timer = 0; return 0
if value <= 0: if timer <= 0: timer = 10.0;  return value
if power <= 0: return value
left = value - power;  timer = 10.0;  value = left < 0 ? 0 : min(left, max);  return value
```

Tick 0x140598CD0 [C]: the timer runs down; at 0 a broken body (value < 1) is **refilled to max**, an unbroken one gains
1 point (and the timer restarts while it is below max).

With section 2.2 [D]: against a body of 5 Sora's basic hits never flinch it; each finisher (power 1) takes 1 without a
reaction; the fifth breaks it and lands in full; for the next 10 s every hit with power lands in full, big kinds without
power are flinches, basic SMALL hits still do nothing; a counter (power 5) breaks a fresh body of 5 at once.

Body rows [A, 202 rows of `DataTable/Enemy/Base`, difficulty variants left out]: `m_MaxBodyStrongValue` is 0 on 151 rows,
3 on 6, 5 on 25, 10 on 7, 15 on 4, 25 on 5, 40 on 1, 50 on 3. Whether a hurt shape is STRONG is the shape's
`ETresBodyCollReactionType` / the pawn's `m_DefaultBodyCollReactionType` (pawn+0x618), per enemy blueprint; not surveyed [?].

### 5.2 Armour [C]

`m_MaxArmorHP` (non-zero on 18 rows: 10 … 120). An armoured victim (actor-interface slot 63) takes the row's
`m_ArmorAttackPower` off it (slot 70, 0x1405A2160 → 0x140576360) and the hit is `V = NONE` (blocked, no damage) until it
breaks, which is `V = ARMOR_BREAK`: no HP, and the enemy's own break action, not the damage state (combat spec 8.2.6 / 8.4).
`kh_core::combat::classify` already implements the classification; `Targets::take_armor` is the host's.

### 5.3 The revenge value: the only combo limit [C]

Struct at pawn+0x7F8: `{ value, limit, hold, timeWithPoints, coolDownTime, coolDown; maxCount, count, flags }`; `limit` =
`m_RevengeLimit`, cool-down = `m_RevengeCoolDownTime`, max count = `m_MaxRevengeCount` (set by 0x1405B65A0 / 0x1405B6560).

On a hit (0x1405A3E70, from `TakeDamage`): when the pawn has no bad status, HP was lost and `info.m_RevengePoint != 0`:
`points = m_RevengePoint × m_RevengeCoefficient<attribute>` (1 on almost every row), then 0x140573EB0:

```
if coolDown > 0 || limit <= 0: nothing
v = clamp(value + (timeWithPoints >= 20 ? limit : points), 0, 100)
if value < limit && limit <= v: flag "revenge"                // -> notify interface: pawn+0x448+0x790 |= 0x20
if v > 0: hold = 3.0
value = v
```

Tick (0x140598D30): the cool-down first; then `hold -= dt`, and once it is over **and the pawn is not in the Damage
state** the value decays by 5 per second; `timeWithPoints` counts up to 20 while the value is above 0.

The flag is read by the enemy's AI, which starts one of its `m_RevengeAttacks` (pawn+0xB58): the enemy breaks out of the
combo with an attack of its own. How that action interrupts the damage state and when the value is reset were not read [?].

`m_RevengeLimit` [A]: 0 (no revenge) on 121 of the 202 rows — the ordinary heartless: **they can be juggled without
limit**; 6.5 on 29, 7 on 21, 8 on 17, 3 … 10 on the rest (bosses, the larger and later enemies). With Sora's rows: a
ground combo of three basic hits (1.0 each) and a finisher (0.5) is 3.5 points; a limit of 6.5 is crossed by the third
hit of the second combo; an air combo of Aerial Sweeps (0.4 a hit) takes 17 hits.

### 5.4 What does not exist

- **Hit-stop**: none (combat spec 8.5.3; the attack components run on the attacker's time dilation and nothing in the
  victim path writes a dilation).
- **Weight**: no mass, weight class or "blow resistance" scales a trajectory. `m_BodyPushPower` (PPL_10 … PPL_MAX) is the
  body-push level of pawn-to-pawn pushing only (combat spec 6.5).
- **Juggle decay**: no gravity scaling per air hit, no hit counter that forces a knock-down. The counters at +0x1E0 /
  +0x1E4 belong to the paint attraction (kind 0x18).
- **Down state**: no invulnerability and no damage scaling while lying or getting up; a hit on a lying enemy restarts
  the reaction of its kind (a SMALL puts it back on its feet in the flinch clip).
- **Bounce**: enemies do not bounce off the ground (BEAT has no rebound; TOSS lands into the lying clip).

---

## 6. Phases, for a host that animates [D]

`kh-react` reports one phase per clip family:

| Phase | KH3 clip | Motion owner |
|---|---|---|
| `FlinchGround` | `200`-set (variant alternates) | reaction: slide |
| `FlinchAir` | `210`-set | reaction: hang, gravity off |
| `Falling` | `JumpNormalDownLoop` | reaction: gravity |
| `LandFeet` | `Land` | reaction: standing |
| `BlownFlying` | `240`+i start + loop | reaction |
| `BlownFalling` | `DmgAirBlowBr*Loop`, the wall fall loop, after a spin | reaction: gravity |
| `SpunAway` | `m_260` / `m_270` | the clip's root motion |
| `LaunchedRising` / `LaunchedHover` / `LaunchedFalling` | `m_250`, `_1` / (same loop) / `_2`, `_3` | reaction |
| `SlammedDown` | `m_250_3` | reaction |
| `WallHit` | `280`-set | reaction: stuck |
| `Down` → `GettingUp` | `240_2`, `m_250_4`, `m_260_1`, `280_2`: ONE clip each | reaction: lying (BLOW: still sliding) |
| `Frozen`, `Stunned` | the bad-status states | stand-in |

The game has no "large flinch" clip: a big kind that a STRONG body lets through as a flinch plays the same `200` clip
(`HitOutcome::Started { downgraded: true }` tells the host, should it want a heavier animation). `GettingUp` is not a
game boundary: it is the last `ClipTimes::getup_tail` seconds (0.6 s stand-in) of a down clip.

---

## 7. Body classes and a mapping rule

KH3 has no body-class enum for reactions. The four classes of `kh_react::BodyClass` are the groups the 202 body rows fall
into by push level and endurance [A]:

| Class | Rows | Push level | `m_MaxBodyStrongValue` | Revenge | Examples | Behaviour |
|---|---|---|---|---|---|---|
| Light | 27 | PPL_10 … 40 | 0 on all | none | `e_ex001` (PPL_30), `e_ex014` / `e_ex017` (PPL_20), `e_ex009` (PPL_40) | every hit reacts; juggled freely |
| Medium | 100 | PPL_50 … 60 | 0 on 75; 3 on 5, 5 on 18, 10 on 2 | on 53 (6.5 on 21, 8 on 11, 7 on 9) | `e_ex003` … `e_ex005` (PPL_60, endurance 0), `e_ex006` (PPL_50, endurance 3) | as Light; many break out by revenge |
| Heavy | 34 | PPL_70 … 90 | 0 on 16; 5 on 7, 10 on 4, 15 on 3, 25 on 2, 3 and 40 on 1 each | on 12 (6.5 on 8) | `e_ex011` (PPL_80, endurance 10), `e_ex018` (PPL_80, 15), `e_ex002` (PPL_70, 0) | STRONG body: only hits with reaction power, after the break |
| Giant | 41 | PPL_MAX | 0 on 33; 25 on 3, 50 on 3, 10 and 15 on 1 each | on 16 (7 on 9, 8 on 6) | `e_ex021`, `e_ex027a` and the boss rows (enemy names were not looked up: ids only) | STRONG with a deep endurance, or no reaction at all (`m_bIsNoDamageResponse`); Gigas shudder additively |

All four fly the same arcs when they do react. `Body::of(class)` gives: Light / Medium — DEFAULT body, no endurance;
Heavy — STRONG, endurance 5; Giant — STRONG, endurance 25, revenge limit 7. Every field is public; set the real values
when the enemy is known.

**Suggested rule for a foreign game (Elden Ring)** — a suggestion, not KH3 data; the thresholds are to be tuned on the
running game:

1. An explicit table by character id wins.
2. Never reacts (`Body::never_reacts`): anything the host must not move (non-hostile, scripted, bound to the map,
   multi-part or riding bosses).
3. Otherwise by the enemy's poise maximum (`CSChrSuperArmorModule.sa_durability_max` / `CSChrToughnessModule.toughness_max`)
   and its capsule height: Light — low poise and shorter than about 1.3 m (rats, bats, dogs, slimes); Medium — ordinary
   humanoids (soldiers, nobles, skeletons); Heavy — large humanoids and anything with roughly two or more times a
   soldier's poise (`strong_max` 5 … 15, scaled from its poise); Giant — bosses, trolls, giants, dragons (`strong_max` 25 … 50
   or `never_reacts`).
4. `flying: true` for enemies that hover (they are not dropped when a reaction lets go).
5. `revenge_limit`: 0 for Light, 6.5 … 8 for Medium / Heavy elites, as the game does; on `take_revenge()` give the enemy
   back to its AI (with its own poise / super armour for a moment) and call `clear_revenge()`.

---

## 8. The implementation: `crates/kh-react`

Engine-free (glam + `kh-core` for `HitRecord`, the damage-kind enum and the `World` trait). Units and axes as `kh-core`:
cm, s, degrees, X forward, Y right, Z up; positions are the victim's FOOT point. 60 Hz step (`STEP`).

- `params.rs`: the generated constants (`ENEMY_REACTION_PARAMS`), `DamageParams::ENEMY` (section 3.1), `ClipTimes::MEDIAN`
  (the medians of section 4), `Body`, `BodyClass`.
- `react.rs`: `EnemyReaction` — the gate of 2.2, Setup, the handlers of 4.1 – 4.6 with the game's sub-step numbers, the
  wall test of 4.7, the endurance and revenge of section 5, and the body's movement (falling with gravity as the engine
  integrates it, walking on the floor, flying).
- `world.rs`: `ReactWorld` (one body sweep), `CapsuleIn` (adapter for a `kh_core::World`), `FlatWorld` (floor + walls,
  for tests).
- `tests/spec_numbers.rs`: 13 tests that reproduce the [D] numbers of this document.

What it is not: the 63 per-enemy damage-state subclasses, the kinds of 4.8, the Die state, clip-exact spins (4.6), the
per-enemy clip lengths (medians unless the host sets them), enemy `GravityScale` overrides, the base-class (friend)
variant of Blow, the special cases of Setup (the paint attraction, kind 0x19, the weak-point VERTICALROLL), the
"random damage action" of the gate. Timings that depend on the order of the engine's ticks are within one frame.

`kh-core` changes (marked "enemy-reaction implementer"): `HitRecord::attacker: HitAttacker { position, yaw, radius, valid }`
(the instigator as the victim's Setup / Beat read it), filled in `combat/hit.rs` and `magic/projectile.rs`;
`HitRecord::impulse` stays, documented as always zero (the game has no such number). The 64 combat tests
(`combat`, `combat2`, `combat3`, `combat4`) pass.

Test app (`crates/kh-app`): the training dummies own an `EnemyReaction` each (`fixtures.rs`: `apply_hit` feeds it,
`Dummies::step_reactions` steps it against the level, `update_visuals` moves and tilts the capsule; the label shows the
phase); `reset-dummies` puts them back; new script word `dummy-class N CLASS` (0 light, 1 medium, 2 heavy, 3 giant;
`dummy-body N TYPE`, which already existed, is the hurt shape's reaction type). `Targets::take_reaction_power` is
answered from the reaction; `Targets::reaction_state` is the kind really running.

---

## 9. Hand-over: Elden Ring

Everything the host must do, in the order of a frame. `crates/kh-er*` were not touched.

**Per enemy, once** (when it first becomes a target):

```rust
let mut body = kh_react::Body::of(class);        // section 7
body.radius = capsule_radius_cm;
body.flying = hovers;
body.clips.small_ground = /* length of the stagger animation you will play */;   // and the others you know
let reaction = kh_react::EnemyReaction::new(body);                                 // keep it as long as the enemy lives
```

**`Targets::body(id, part)`** → `reaction.target_body()` (plus the host's own guarding / armour / weak-point answers).
**`Targets::take_reaction_power(id, power)`** → `reaction.take_reaction_power(power)`.
**`Targets::reaction_state(id)`** → `reaction.reaction_state()`.

**Per hit** (inside `Targets::apply_hit(&HitRecord)`, after the host's HP bookkeeping):

```rust
let request = kh_react::ReactionRequest::from_hit(hit, hp_lost > 0.0, killed);
match reaction.hit(&request, kh_react::Pose { position: feet_cm, yaw: yaw_deg, grounded }) {
    HitOutcome::Started { kind, downgraded } => { /* suspend the enemy's AI and movement; see below */ }
    HitOutcome::NoReaction => { /* damage and effect only; a giant may play its own additive shudder */ }
    HitOutcome::HostState(result) => { /* guard / repel / armour break: the enemy's own animation */ }
    HitOutcome::Dying => { /* the game's death */ }
    HitOutcome::NotPorted => { /* MAGNET ...: ignore, or the old stand-in */ }
}
if reaction.take_revenge() { /* the enemy breaks out: give it back, then reaction.clear_revenge() */ }
```

`position` is the foot point in KH3 axes and centimetres (`kh-er` already converts for the targets seam: Elden Ring's
metres, Y up); `yaw` in the core's convention (forward = (cos, sin, 0)); `grounded` from the game. While a reaction is
already running the pose argument is ignored (the reaction's own position is the truth).

**Per 60 Hz step, for every enemy with a reaction** (active or not: the timers of section 5 run in it):

```rust
let out = reaction.step(&world);                 // world: impl kh_react::ReactWorld, below
if out.suspended {
    // write out.position (foot) and out.yaw to the character in BOTH physics phases, gravity disabled,
    // fall timer cleared, AI / own movement off; play the animation of out.phase (out.phase_time = seconds into it)
}
if out.finished {
    // last write; then gravity and AI back on. out.grounded says whether it stands or is in the air (a flyer).
}
```

`ReactWorld::sweep(start, end) -> Option<Contact { time, normal }>`: move the enemy's body shape from foot point `start`
to foot point `end` and report the first blocking contact with the surface normal (floors have `normal.z ≥ 0.7`). A
sphere cast of the enemy's capsule radius from `start + up × radius` is enough; a contact at the very start must be
reported when the move goes into the surface (that is how a standing body finds its floor).

Releasing: `reaction.release()` on death, despawn, leaving the target list or an origin shift (or `set_position` for the
shift); the puppet's safety rules of `IMPL_ER4_STATE.md` (never non-hostile, a time-out) stay the host's.

**Phases → Elden Ring animations** (which `dmgLevel` / animation id plays which is to be MEASURED per enemy class; the
names below are the roles):

| Phase | Elden Ring role | Notes |
|---|---|---|
| `FlinchGround` | light / medium stagger | restart it on every hit; `variant` alternates in KH3 |
| `FlinchAir` | an airborne-hit pose (or the fall pose with a hit shake) | the body hangs: gravity must stay off while suspended |
| `Falling`, `LandFeet` | fall, land | on its feet |
| `LaunchedRising` / `Hover` / `LaunchedFalling` | knock-up (fling) rise, hold, fall on the back | 383 cm: an Elden Ring knock-up animation is far shorter; loop or hold its airborne part |
| `BlownFlying` / `BlownFalling` / `SpunAway` | blow-away (knockback) airborne part | hold it for as long as the phase lasts |
| `SlammedDown` | fall | very short |
| `WallHit` | hold the blow-away pose | 0.5 s |
| `Down` | lying after a knock-down | `phase_time` from the landing |
| `GettingUp` | the get-up | set `clips.getup_tail` and the down clips' lengths to the Elden Ring animations' so the motion ends with them |
| `Frozen` / `Stunned` | hold / dizzy | stand-in timers |

The simplest correct first version: play ONE knock-down reaction at `Started` for the big kinds and a stagger for SMALL,
hold the airborne frame while `out.grounded` is false, play the get-up when `Down` begins, and let `kh-react` own the
position throughout.

**What makes it feel like KH3** (in order of weight): (1) the air hit — velocity zeroed, gravity off ~0.47 s, +200 cm/s:
without it there is no juggle; (2) Rising Spiral's 383 cm with the 0.2 s hover; (3) the ground slide of 175 cm per basic
hit (Sora's attacks home in after it); (4) the finisher throwing the enemy 2.8 m (ground) / 4.7 – 6.6 m (spin / air) and
the 1.4 – 1.8 s lying; (5) enemies that cannot be juggled (STRONG bodies) not moving at all until broken.

---

## 10. The scripted run in the test app

`build/sora_tmp/react/khgame_react.exe` (a copy of the release build), unattended, pad path (script verbs `tap a` =
attack, `tap b` = jump / launch), level-99 loadout:

```
khgame --mute --hide-debug-text --orbit-camera --camera-yaw 90 --camera-distance 15 --camera-height 3 --camera-pitch -8
       --frames 430 --shots "45:…,60:…,122:…,150:…,170:…,205:…,250:…,300:…,395:…"
       --script "warp -3.4 0 180; stop 0.5; tap a; stop 0.5; tap a; stop 0.35; tap b; stop 1.0; tap a; stop 0.4; tap a; stop 0.4; tap a; stop 0.5; tap a; stop 3"
```

Log (`build/sora_tmp/react/run2/run.log`), dummy 1: 0.85 s `KB_RADIALCUT` (SMALL) → `FlinchGround`; 1.23 / 1.47 s
`KB_FLASHSTEP(_END)`; **1.97 s `KB_LAUNCHSPIN` (TOSS) → `LaunchedRising` at 1500 cm/s**; 2.48 s `LaunchedHover` at 383 cm;
**2.65 s `KB_AERIALSWEEP` → `FlinchAir` at 380 cm** (−550 away, +200 up); further air hits at 2.85, 3.22, 3.38, 3.75,
3.93 s, the dummy rising to 598 cm; **4.43 s `KB_TRIPLE_END` (SIDESPIN) → `SpunAway`**, 470 cm in 0.88 s at that height;
5.32 s `BlownFalling`; 6.43 s `Down` 15.6 m from where it stood (3.6 m of it from the ground hits, 6 m from the air hits).

The 9-frame sheet (`build/sora_tmp/react/run2/sheet.png`, looked at): side view from 15 m. Frames 1 – 2: the dummies
upright, then the hit flash with the neighbours leaning (Radial Cut hits four). 3: dummy 1 leaning back, off the ground,
label `LaunchedRising`. 4: three dummies (1, the large one and the floating one, all caught by the launcher) lying
horizontal high above their shadows, labels `LaunchedHover`. 5 – 6: Sora beside dummy 1 in the air, hit sparks, label
`FlinchAir`, the capsule tilted, higher than in 4. 7: Sora's ring slash in the air; the large dummy `Falling`. 8: dummy 1
horizontal with the label `SpunAway`, away from Sora. 9: dummy 1 lying on the floor far down the lane (`Down`), Sora
landed, the large dummy upright again, the floating one still up (a flying body is not dropped).

Honest limits of the picture: capsules, not poses (a tilt stands for the animation); at this distance the figures are
small; the finisher the level-99 loadout chose is Triple Slash (a spin), so the BLOW arc itself is covered by the unit
tests, not by this run; the launcher also tossed the 4 m "large" dummy, because every dummy is a medium body until
`dummy-class 4 2` says otherwise — which is what the game's data does (no weight), but reads oddly on a prop that size.
The first sheet (`run/sheet.png`, the follow camera) shows the same run from behind Sora and is harder to read.

---

## 11. Confidence and open questions

| Part | Confidence |
|---|---|
| Parameter values of section 3.1 | high: class default object [A] + constructors [I]; the getters' offsets [I] for 142 and 147, [C] for the rest |
| STRONG-body rule (2.2), endurance arithmetic (5.1) | high [I] |
| Handler constants (3.0, 0.25, 150, −300, 0.5, 0.7071, 2500) | high [I] |
| Handler control flow (section 4) | medium [C]: read whole, not compiled; the structure matches the player's handlers, which are [I] |
| Order of ticks within a frame (movement before the state; when the direct velocity is consumed) | medium: taken from the player's analysis (combat spec 9.3); ± 1 frame |
| Gravity scale 1.0 of enemies | medium-low [U / A]: no override found on generic enemies, not read at run time |
| Clip lengths | medians [A]; an individual enemy differs by a factor of up to 3 either way |
| Spin distances | medians [A]; the speed profile is a fit to one clip |
| Revenge (5.3) | medium [C] for the arithmetic; what the AI does with the flag [?] |
| Die state, freeze / stun states, kinds ≥ 15, the 63 subclasses | not read |

Open questions / run-time probe requests (`docs/sora-runtime-probe.md`):

1. An enemy's `CharacterMovement.GravityScale` (component+0x2F8) at rest, and while a TOSS rises (expect 3.0) / hovers.
2. Enemy damage state +0x1CC / +0x1D0 / +0x1D4 (power, brake, vertical speed) on an air SMALL: expect 550 × scale, 0.95, 200.
3. Height gained per hit by a small heartless (`e_ex001`) during an air combo (expect about 40 – 50 cm per hit at the Kingdom Key's rhythm).
4. Pawn+0x7F8 (revenge value) and pawn+0x814 (endurance) on an enemy with endurance (`e_ex011`) while it is hit.
5. Which clip an enemy's anim set maps `Land` and `JumpNormalDownLoop` to, and their lengths (sheet row `clip.land` is unchecked).
6. What `UTresEnemyState_Die` does with the hit direction (is a killed enemy blown away?).
7. Whether `FUN_1405ADAA0` is exactly "on the ground" (used as such in Small, Beat, the spins and the wall hit).
