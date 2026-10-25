# Sora: melee combat specification (Kingdom Key, base form and Second Form) — revision 2

Research only. Source: `base/KH3.exe` (disassembly with iced-x86, decompiled C for orientation), the
cooked packages under `assets_converted/packages/KINGDOM HEARTS III/Content`, the converted clips of
`p_ex001_AnimSet`, the game tables under `sheets/entities`. Scope: everything Sora does in a fight
with the Kingdom Key (`w_so010`): the base combo, targeting and lock-on, hits and damage, being hit,
Second Form and the situation commands; excluding magic / shotlock / items and the movement
abilities themselves (dodge roll, air slide, guard: only how attacks chain into and out of them).

**Revision 2** merges the first pass (revision 1, kept as `build/sora_tmp/combat/spec_rev1.md`) with
six second-pass strands (A request path and attack state, B movement physics, C hit detection and
application, D targeting and lock-on, E Sora being hit, F Second Form and the command queue; their
working files are `build/sora_tmp/combat/P2_<A..F>_{section,rows,notes}`). Every correction the
strands made to revision 1 is folded into the text below; section 16 lists them, so that code
written against revision 1 can be re-checked. Disagreements with the other Sora specs and what
their owners should change: `docs/sora-spec-reconciliation-2.md`.

The numbers and tables of this document are sheets, written by `tools/sora_combat_sheet.js`
(code facts authored there and in `tools/lib/sora_combat_pass2.json`, data read from the game files
on every run):

| Sheet (`sheets/port/…`) | Rows | Content | Section |
|---|---|---|---|
| `sora-combat-params.json` | 294 | thresholds, speeds, rates, damage constants, gauge constants, targeting and camera constants (revision-1 rows + the pass-2 rows of all six strands) | all |
| `sora-combat-attacks.json` | 47 | one row per attack kind: clip, role, required ability, per-kind settings, clip timing (base kinds + Second Form 7A–7F) | 4.3, 10.5 |
| `sora-combat-hits.json` | 110 | one row per hit window (`TresAnimNotifyState_Attack`): time, shape group, attack-data row, hit sound, re-hit interval (base clips, Second Form finishers, finish commands) | 8.2, 10.6 |
| `sora-combat-collisions.json` | 27 | hit shapes of the keyblade (19) and the pawn (8) | 8.1 |
| `sora-combat-attack-data.json` | 75 | the rows of `p_ex001_AttackData` used here, all 30 fields | 8 |
| `sora-combat-rules.json` | 29 | the combo decision procedure as ordered rules | 4.2 |
| `sora-combat-reactions.json` | 16 | Sora's hit reactions: gate, damage kinds, animation tables (overview; detail in the damage-* sheets) | 9 |
| `sora-combat-notifies.json` | 25 | notify classes of the melee clips, the interface slot they call, their effect | 2 |
| `sora-combat-can-attack.json` | 15 | CanAttack 0x141426330 (request interface slot 44): the condition per current state id | 3.2 |
| `sora-combat-try-attack.json` | 8 | TryAttack 0x141437330 (slot 45): queue operation and writes per current state id | 3.3 |
| `sora-combat-attack-exits.json` | 9 | what can end a TresPlayerState_Attack, with the gate that decides it | 5.3 |
| `sora-combat-attack-end.json` | 14 | TresPlayerState_Attack::End 0x141335120 as ordered steps | 5.4 |
| `sora-combat-controller-order.json` | 13 | order of the combat calls in the controller input update 0x1413F1AB0 | 3.1.2 |
| `sora-combat-tick-delta.json` | 3 | displacement of one movement tick during an attack, per movement mode | 6.1.3 |
| `sora-combat-velocity-flags.json` | 6 | bits of the movement component flag byte +0xC18 (direct-velocity record and body-push switches) | 6.1.2 |
| `sora-combat-air-timeline.json` | 6 | movement-mode timeline of an air-mode attack | 6.6 |
| `sora-combat-push-weight.json` | 4 | body-push weight by push level (UTresBodyCollComponent 0x140637430) | 6.5 |
| `sora-combat-prim-flags.json` | 25 | flag bytes +0x984..+0x987 of UTresAtkCollPrimitive | 8.0 |
| `sora-combat-activation.json` | 29 | the 0x70-byte activation struct passed from the attack notify to Activate 0x140625500 | 8.2.1 |
| `sora-combat-reaction-rules.json` | 33 | Classify 0x14061FE20: victim result and attacker result by the body reaction type of the hit shape | 8.2.6 |
| `sora-combat-knockback-dir.json` | 6 | knockback direction by m_KnockbackType (0x14061EA30) | 8.7 |
| `sora-combat-damage-info.json` | 48 | TresDamageInfo as delivered inside FTresPointDamageEvent (+0xA8) | 8.7 |
| `sora-combat-shape-flags.json` | 9 | the keyblade shape groups with the flags their shape data produces (0x140642CA0) | 8.1 |
| `sora-combat-hit-effects.json` | 7 | hit effect and hit sound of the Kingdom Key per case (chooser 0x140628960 on w_so010_Wep m_HitEffects) | 8.6 |
| `sora-combat-attacker-events.json` | 8 | everything the attacker side does per hit, in order | 8.5 |
| `sora-combat-lockon-fields.json` | 32 | lock-on state on TresPlayerControllerSora | 7.1 |
| `sora-combat-lockon-slots.json` | 11 | virtual functions of TresLockonTargetComponent (vtable 0x146628970) the targeting code calls | 7.9 |
| `sora-combat-lockon-hud.json` | 4 | HUD calls of the lock-on code | 7.11 |
| `sora-combat-damage-params.json` | 46 | PlayerDamageCommonParam (TresDamageCommonParam): native default, asset value and the value the player uses | 9.3 |
| `sora-combat-damage-handlers.json` | 10 | TresPlayerState_Damage: damage kind -> handler, with the exits each handler allows | 9.3 |
| `sora-combat-damage-clips.json` | 32 | clips of the hit reactions with length, change point and the play mode the handler passes | 9.4 |
| `sora-combat-hurt-shapes.json` | 13 | the 13 TresBodyCollPrimitive templates of p_ex001_PawnBase (Sora's hurt, push and camera bodies) | 9.10 |
| `sora-combat-hp-rules.json` | 8 | ApplyHP 0x1405A2A00: the rules applied to new = HP - Floor(amount), in order | 9.7 |
| `sora-combat-hit-blocks.json` | 7 | everything that can stop a hit on Sora from damaging or staggering him | 9.6 |
| `sora-combat-form-clips.json` | 25 | clips of the formchange, the Second Form finishers and the finish commands with their gameplay notifies | 10.4 |
| `sora-combat-form-commands.json` | 47 | rows of Load/Common/TresCmdData (TresCommandKindData) used by attack, formchange, finish and grand-magic commands | 10.1 |
| `sora-combat-finish-levels.json` | 4 | finish level -> command -> per-command tick of TresPlayerState_StyleFinish | 10.6 |
| `sora-combat-form-abilities.json` | 6 | abilities added while wear form TRES_WEAR_FORM_05 (Second Form) is worn | 10.5 |
| `sora-combat-queue-classes.json` | 10 | where each kind of command goes in the HUD command part (classifier 0x141619E60) | 11.1 |
| `sora-combat-form-fields.json` | 25 | style-gauge and formchange fields of the player controller | 10.1 |

Asset list for the pack: `build/sora_tmp/combat/asset_requests.json` (119 animations, 30 notify classes, 126 effect assets, 75 sound assets). Working notes
and disassemblies: `build/sora_tmp/combat/`.

Tags (a tag in a heading covers the subsection):

- **[I]** verified at instruction level (address given)
- **[C]** decompiled C only
- **[U]** stock UE 4.17 by identity
- **[A]** asset or table value
- **[D]** derived by arithmetic from [I]/[A] values
- **[?]** not determined statically (section 13 says what to log)

Conventions as in `docs/sora-movement-spec.md`: cm, s, degrees; "slot n" = vtable index; "state" =
the current `TresPlayerState_*` object; `iface` = the request interface (pawn+0x450); `ctrl` = the
player controller. **Frame order: C (controller input update) → M (movement + pose tick + notifies)
→ pawn tick → S (state `MainProc`) → U (UI tick: the command menu, which issues the attack / magic /
item / situation commands).** Vtables: `TresPlayerState_Attack` 0x146900578 (143 slots), its
`UTresAnimInterface` sub-object (state+0x40) 0x1469009F0, its notify sub-object (state+0x48)
0x146900E38, `TresPlayerState_ComboLimit` 0x146910B80, `TresPlayerState_Damage` 0x1469154C8,
`TresPlayerState_StyleChange` 0x146947250, `TresPlayerState_StyleFinish` 0x146947A28, pawn
0x14696B930, request interface (pawn+0x450) 0x14685EB18, pawn notify interface (pawn+0x448)
0x14696E4B8, controller `TresPlayerControllerSora` 0x146969B98, `TresAtkCollComponent` 0x145D73008,
`TresAtkCollPrimitive` 0x145D75FB0, `TresLockonTargetComponent` 0x146628970, movement component
`TresAICharMovementComponent` (pawn+0x578) 0x145D073F0.

Helper names used throughout: `cur` / `queued` = current / next queued state (`FUN_1405ABE00` /
`FUN_1405AB6C0`), `SetState` = `FUN_1405B9FE0` (replaces the current state at the next `MainProc`),
`PushState` = `FUN_1405B4F80` (queues behind the current state; calls `Reset` on the instance),
`OnGround(pawn)` = `FUN_1405AD850` = `!(pawn+0xD80 & 1) && movement mode ∈ {Walking, NavWalking}`
[I], `has(id)` = pawn vt+0x14E0 (slot 668 0x1414358B0), `AbilityCount(id)` = pawn vt+0x1500 (slot 672
0x141424130: equipped copies), `MaxCombo()` = pawn vt+0x1D00 (slot 928 0x141433570), `Finishers()` =
pawn slot 1253 0x141433A60.

Pawn fields that the first pass addressed through the interface [I]: `iface+0x9C8` = **pawn+0xE18**
(combo index, shared with magic), `iface+0x9D8` = **pawn+0xE28** (hit-confirm byte, 3.2.1),
`iface+0x1A0` = **pawn+0x5F0** = `m_ChrUniqueID` (1 = Sora), `iface+0xA10` = **pawn+0xE60** (bit 17
command latch, bit 18 form launch flag, bit 19 "finish ended"), `iface+0x1014` = pawn+0x1464 (friend
unison request).

**Depth reached.** Sections 3–6 (request path, decision, attack state, movement), 7 (targeting; the
lock-on camera partly C), 8.2–8.3 and 8.5–8.7 (hit application, formula, gains, hit record), 9.1–9.7
(reactions) and 10.1–10.5, 10.7 (Second Form flow) are at instruction level where tagged [I]. Still
decompiled C: parts of the hit `Apply` outside its per-hit loop, the body-push solver, the lock-on
camera's framing solver, Suction / Eat handlers, the Die state, and the HUD side of the command queue
(11). The three finish-command bodies (Stun Impact, Sonic Rave, Last Arcanum; 10.6) were read whole in
the disassembly at the merge: 10.9. Section 13 lists what only the running game can settle.

---

## 0. Summary of the model

1. There is **no combo table in data**. The combo is code: one state class
   `TresPlayerState_Attack` (id 12) plays one attack per state instance; the attack to play is
   decided in `Begin` from the combo counter, the previous attack kind, the distance / height /
   count of targets and the equipped abilities (section 4). Data supplies per clip: the hit windows
   and their attack-data rows, the homing window, the input and cancel windows (anim notifies).
2. A press of the attack button is **not buffered**, and it is **not issued by the controller**: the
   HUD command menu's decide, in the UI tick of the frame, asks `CanAttack` and calls `TryAttack`
   (3.1). A request takes effect in the state tick of the next frame.
3. During an attack the next attack is accepted only after the swing's first hit window has opened,
   the `TresEnableInput` notify has fired **and a hit of this chain has landed** (hit-confirm byte
   pawn+0xE28; Combo Master removes the rule; 3.2.1). An accepted request queues the next Attack
   state, which starts when `TresEnableChange` fires. A whiffed first swing cannot be chained.
4. Base combo length is 4 (+1 per Combo Plus on the ground, per Air Combo Plus in the air); the last
   1–3 steps are finishers depending on abilities; every step has a ground and an air form
   (air kind = ground kind + 15). The jump button during a combo gives the two launch attacks
   (Rising Spiral 0x1C, Groundbreaker 0x1D), once each per chain (3.4).
5. Movement during an attack = the clip's root motion + a **homing velocity** ("traction", 800 cm/s
   toward the target while the `TresTraction` notify window is open, stopping 160 cm from the
   target's lock-on point) + for some kinds a root-motion scale `distance / nominal length`. The
   ordinary velocity code (braking, friction, stick) is switched off for the whole attack; in the
   air the attack runs in Flying mode without gravity until its window closes (section 6). Sora's
   capsule does not collide with enemies; a soft body push separates them (6.5).
6. The target is chosen from **Sora's facing** (not the stick, not the camera); an accepted attack
   pins its target until nothing is in range or the target dies; RB toggles a manual lock, RT steps
   through the six nearest (section 7).
7. Damage = `ceil((max(STR − DEF, 1) with floors and caps) × m_Power)`, then resistance, the
   difficulty factor and the ability multipliers; no random term (8.3). **There is no hit-stop**, no
   camera shake and no focus gain on a keyblade hit (8.5).
8. The victim's reaction is asked for by the attack row (`m_DamageKind`, knockback fields); the
   attacker does not choose animations for the victim. The guard / repel outcome is computed by the
   attacker's collision component from the victim's body reaction type before the hit is delivered
   (8.2.6). That is the host seam (section 12).
9. Style points (the formchange gauge) are granted only when the victim actually lost HP; a full
   gauge yields the Second Form command (10).

---

## 1. Objects [I for classes, offsets and vtables; A for blueprints]

| Object | Where | Role |
|---|---|---|
| `TresPlayerState_Attack` (size 640) | state id 12 `TSID_STATE_ATTACK`; subclasses `Combo*` (ids 75–90, 159–161) are the formchange move sets, `ComboLimit` (90) = Second Form; `StyleFinish` (33) = the finish commands | one attack; decision, facing, homing, chaining |
| request interface | pawn+0x450 | slot 44 `CanAttack` 0x141426330, slot 45 `TryAttack(target, type)` 0x141437330, slot 47 `CanLaunch` 0x14142B3F0, slot 48 `TryLaunch` 0x14143EAE0, slot 53 `TryMagic` 0x14143AEB0, slot 88 damage reaction gate 0x141439A50, slot 129 die 0x14143A190, slot 190 `TryStyleChange` 0x141445940, slot 192 `TryStyleFinish` 0x141447840 |
| controller | `TresPlayerControllerSora` | reads the buttons (0x1413F1AB0); `CanExecuteCommand` slot 682 0x141401F80, `ExecuteCommand` slot 680 0x14140EAA0 (writes the kind to ctrl+0x87C for the duration of the call), executor slot 678 0x14141B470; the target search (slot 714 0x141404E50) and the target pointers (section 7); the style gauge and the form bookkeeping (section 10) |
| HUD command part | `TresUIP_HudCommand` (cockpit `AHUD`+0x4F8 → +0x78) | the command menu: issues the attack press (3.1.1); owns the situation-command queue (section 11) |
| keyblade | `w_so010_Wep_C` → `w_so_KeybladeBase_C` | `TresAtkColl0` (`TresAtkCollComponent`): 19 shape groups (sheet collisions), hit effects per attribute; sound alias sets `enumset_hit_w_so010`, `enumset_swing_w_so010`, `setable_w_so010`; trail effect `ead_key_trail0` |
| pawn `TresAtkColl0` | `p_ex001_PawnBase` / `p_ex001_Pawn` | 8 groups (`StyleChange`, formchange and link shapes); not used by the base combo |
| pawn `TresBodyColl0` + 13 `TresBodyCollPrimitive` | `p_ex001_PawnBase` | Sora's hurt shape, push body and camera bodies (9.10) |
| attack-collision manager | `ATresAtkCollManager` | ticks every attack component: update, sweep, effects (8.2.2) |
| status object | `UTresActorInterface` vt+0x40 of an actor | strength, magic, defense, resistances, ability list (8.3; `docs/sora-stats-spec.md`) |
| attack table | `p_ex001_DataTableSet.m_AttackData` = `DataTable/Player/Attack/p_ex001_AttackData` (756 rows, struct `TresAttackDataTable`) | per hit: power, reaction, flags (sheet attack-data) |
| command table | `Load/Common/TresCmdData` (254 rows of `TresCommandKindData`, ctrl+0x8D8) | per command kind: type, category, queue lifetime, form level and duration (10.1) |

Player state classes with ids (`ETresStateID`, 175 values; file
`build/sora_tmp/combat/ETresStateID.txt`): IDLE 1, RUN 2, JUMP 3, GLIDE 4, FLOATING 5, ATTACK 12,
ATTACK_AQ 13, ATTACK_RI 14, GUARD 15, DODGEROLL 16, RISKDODGE 17, FIREMAGIC 18, DAMAGE 19,
AIR_SLIDE 20, STYLECHANGE 31, WEAPONCHANGE 32, STYLEFINISH 33–39, DIE 59, CATCH 61, BIND 62,
REFLECT 66, COMBO_DW…COMBO_LIMIT 75–90, SWIM 102, RESCUE 109, GAMEOVER 118, BADSTAT_STUN 122, FREEZE
123, BURN 124, SNEEZE 151, ATTACK_RO 156, ATTACK_KA 157, COMBO_LF/DF/TF 159–161. The attack state id
used for a request is pawn slot 1337 0x141432F00 (pawn vt+0x29C8) [I table 0x14143316C: ATTACK for
base Sora, 90 `COMBO_LIMIT` in Second Form, the `COMBO_*` id of the active form otherwise].

## 2. Notifies → state [I]

Notify classes call `UTresAnimInterface` on the mesh owner; the player pawn forwards to the current
state's interface sub-object (state+0x40). Base-state flag word state+0x84: bit 0 `m_bEnableChange`,
bit 1 `m_bReverseLeg`, bit 2 `m_bEnableTraction`, bit 4 `m_bFaceAt`, bit 5 `m_bEnableInput`.

| Notify (name in clips) | Interface offset | Attack state effect |
|---|---|---|
| `TresAnimNotifyState_Attack` ("TresAtk") begin 0x140561F40 / end 0x140564B10 | +0x200 / +0x208 | The pawn's handler first runs the pre-processor chain on the activation struct (8.2.1): Sora's 0x141443D70 writes the command kind, then the state's 0x141343800 renames rows (kind 0x0E: `KB_DOWN` → `KB_DOWN_FIN`), sets the finish flag from `finishEnd`, the aerial flag from air mode, the just-guard flag, **marks every swing after the first of a combo "no clash"**, and writes the four counter bytes (state+0x128, +0x12C, +0x130, +0x134; the damage formula's combo step is the third, state+0x130, **not** the combo count). The pawn then activates the shape group with the row on every equipped weapon and on its own component. Last, the state handler 0x1413459E0: **state+0x164 = 1** (first hit window opened), state+0x1F4 &= ~0x10, state+0x234 &= ~8, then slot 115 (counts the window, 5.5) |
| `TresAnimNotify_EnableInput` 0x14055F9A0 | +0x240 | state+0x84 \|= 0x20 (base 0x1415124D0). `Tick` also sets the bit when `m_bEnableChange` is set (5.2) |
| `TresAnimNotify_EnableChange` 0x14055F920 | +0x238 | handler 0x141342DF0: state+0x84 \|= 1; controller `FUN_1414142D0(ctrl, 0)` (ctrl+0x878 bit 9 = 0; pawn.vt+0x2840(0)). In the same frame's `Tick`: a queued next state starts, the cancel bit is raised, the hover of an air-mode attack ends (5.2) |
| `TresAnimNotifyState_Traction` 0x140564520 / 0x1405666F0 | +0x258 / +0x260 | state+0x84 bit 2 on / off: the homing window (6.3) |
| `TresAnimNotify_FaceAt` 0x14055FED0 | +0x298 | state+0x84 \|= 0x10 (6.2) |
| `TresAnimNotify_Shot` 0x140561B70 | +0x228 | handler 0x141344580 (strings SHOCKWAVE_S, DOWNIMPACT, BARRIERCRACKER): spawns the attack's projectile / shockwave; body not read [C] |
| `TresAnimNotifyState_AttackEnableMapHitReflect` 0x1405620F0 | — | opens the map-hit-reflect window of the active primitives (prim+0x987 bit 1; 8.2.5 step 9, 8.5.1) |

All notifies fire in M (pose tick); the state sees the flags in S of the same frame, the command menu
in U of the same frame.

---

## 3. From the button to the request

### 3.1 The press


#### 3.1.1 The normal press goes through the command menu, one phase later than the controller [I]

The controller update 0x1413F1AB0 reads the attack key (variable 0x1487D7760, `WasInputKeyJustPressed` 0x144445000 →
`attackPressed`) but uses it only in two special branches:

```
0x1413F2524: if !ctrl.slot677()                       // slot 677 0x14128C050: ctrl+0x8C8 > 0 (command cool-down, 3.1.3)
             && (ctrl.slot890() || ctrl.slot879())    // 0x14128BAC0: byte ctrl+0xC08 (attraction active); 0x1411AF940: byte ctrl+0xBB8 (link active)
             && attackPressed:
                 if iface.TryAttack(ctrl.slot722(), 0): return               // call 0x1413F257D
0x1413F2495: if ctrl+0xA10 != 0 (restricted mode): only in states 0x29 / 0x2A:
                 state 0x2A: guardPressed → iface.vt+0x240(1, 0); jumpPressed → iface.vt+0x150(0)
                 attackPressed → iface.TryAttack(null, 0)                    // call 0x1413F250A
             return
```

So the direct call is the attack of a **link or an attraction**; base-form Sora never takes it (both bytes 0). The
ordinary press is consumed by the HUD command window:

```
UI manager tick (group DuringPhysics, after the controller, movement, pawn and state ticks of the frame: docs/sora-ui-spec.md "Update order")
 → command menu update 0x14162FF80 → state "MainSel" update 0x1416220D0
0x141622513: if pad button 27 (decide = variable 0x1487D7760) state == pressed (0x141583B80):
    row = hud+0x6F0
    if !FUN_141620D10(hud, row) && row == 0                                  // cursor on the Attack row
       && rowEnabled (byte hud+0x1EC + row*0x20) && !(hud+0x739 || hud+0x73A[row]):
        kind = 0x1B (ATTACK)
        if hud+0xA59: kind = 0x15 (SNEEZE)
        elif hud+0x9A8 ∈ {0xAA, 0xAF..0xB2, 0xF3}: kind = hud+0x9A8          // context commands shown on the Attack row
        if ctrl.slot682(kind, 0):  ctrl.slot680(kind, 0)                     // 0x141622661 / 0x1416226E1
slot 682 `CanExecuteCommand` 0x141401F80:
    if cmdTable[kind].type != 0x10 && !ctrl.slot681(kind): false             // table = *(ctrl+0x8D8), rows of 0x48 bytes, type at +0x19
    kind 0x1B: return iface.CanAttack()                                      // 0x14140203B
slot 681 0x1413EC9F0 (common gate of every command):
    pawn valid && ctrl+0xA10 == 0
    type 0x10: return cur id ∈ {0x21, 0x26}
    !ctrl.slot677()            (ctrl+0x8C8 <= 0)
    && !(pawn+0xE60 bit 17)    (pawn vt+0x2788)
    && pawn.slot1336()         (0x141427690: !(pawn+0x868 & 2), character kind ∉ {0x0C, 0x11}, cur and queued id ∉ {61, 62})
    && cur id ∉ {0x1F, 0x21..0x27, 0x3B, 0x4A, 0x68, 0x69, 0x6A, 0x6C, 0x6D, 0x70, 0x96}        // table 0x1413ECB48
slot 680 `ExecuteCommand` 0x14140EAA0: ctrl+0x87C = kind; ctrl+0x890 = param
    if !ctrl.slot678(): return false                                         // executor 0x14141B470
    for each actor of the world's list that IsA TresCharPawnBase, has the tag (name 0x146960B18) and is valid: (actor+0x448).vt+0x1E8(kind)
    game-instance counters 0x140E9E2B0(kind, 1), 0x140EB2700(kind);  ctrl+0x87C = 0;  return true
slot 678 executor 0x14141B470: if !ctrl.slot681(ctrl+0x87C): false
    kinds 0x15, 0x1B (0x14141B5E7):
        if iface.TryAttack(ctrl.slot722() /* current target, section 7 */, 0):
            FUN_1414158F0(ctrl, kind, 0)        // pawn.vt+0x1D98(kind, 0): pawn+0xE2C = kind, +0xE30 = 0; ctrl+0x884 = 0; ctrl+0x880 = kind
            return true
        → fail tail 0x14141BD38 (context retry for table type 0x0C only)
```

Consequences:

- The attack command **does not** write the 0.1 s reaction block `ctrl+0x8F4` and does not start the command cool-down:
  those writes are in other branches of the executor (grand magic 0x14141B82D / 0x14141B844 with 0.1 s, form and other
  commands 0x14141B959, 0x14141B995, 0x14141B9DA, 0x14141BB7F, 0x14141BCED). Ordinary magic (branch 0x14141B631) does
  not either. So a hit taken right after an attack press is not shielded by the 0.1 s reaction block of 9.6.
- `CanAttack` runs twice per accepted press (slot 682, then inside `TryAttack`); both see the same state.
- No buffering: unchanged. The byte "next attack queued" (state+0x143) has exactly three writers: `TryAttack`
  0x141437ADA, `TryMagic` 0x14143B100, `TryLaunch` 0x14143EBE6.

#### 3.1.2 Order of the combat calls in the controller update [I] (0x1413F1AB0)

```
keys (pressed = one frame): attack 0x1487D7760, jump 0x1487D7778, guard 0x149C70748, triangle 0x149C70760, LB 0x149C70778, LT 0x149C707A8, View 0x149C706B8
keys (held): RB 0x149C70790, RT 0x149C707C0, LB, LT, jump, guard
input lock (ctrl.vt+0x888): everything zero this frame
stick (movement spec 2.3)
lock-on: slot 712 0x1413F0700 (RB; forced off in states 0x5F, 0x60, 0x67, 0x90, 0x91, 0x96)
if pawn.vt+0xD70() == 1: slot 800 0x1414165E0 (aim / shotlock, RB); slot 713 0x1413F0D00 (target change, RT)
slot 714 target search 0x141404E50; slot 717 0x1414095E0; slot 721; target = slot 722 0x1413FED90
[restricted mode ctrl+0xA10 != 0: 3.1.1; return]
slot 915 0x1413F3910
direct attack of link / attraction mode (3.1.1)
slot 829 0x14141C4F0; slot 841 0x1413F9990; slot 900 0x1413FCC10
if ctrl+0xA08 & 2: return
if LB held && form kind (ctrl+0xB38) != 0x8B: guardPressed = trianglePressed = jumpPressed = false      // shortcut modifier
if ctrl+0x878 bit 6: guard / triangle / jump presses and the jump hold are dropped                      // 0x1413F25E8
else: masks from the HUD object (0x1415839C0 on hud+0x4F8)
if ctrl.slot674(trianglePressed) (situation command 0x141404460) && RT held: return
guardPressed: TryFlow (iface+0x240) → TryAirSlide (+0x228) → stick active ? TryDodge (+0x1D0) : TryGuard (+0x1B8); each returns on success
else guard held: iface+0x1C0
jumpPressed: TryRiskDodge (+0x1E8) → TryAttack(target, 1) = launch (+0x168, call 0x1413F27F5) → JumpRequest(0) (+0x150)
else jump held: TryGlide (+0x218) → JumpHeld (+0x158)
LB pressed: iface+0x328 (returns on success)
stick active: MoveInput (+0x118), TryRun (+0x140);  ctrl+0x878 bit 5: iface+0x128
iface+0x6F8 (state input event) when any of stick / jump / LT / RT is active;  View pressed: pawn.vt+0x2A28()
```

The key layout itself is `docs/sora-actions-spec.md` II.7 (not repeated). Frame phases: **C** (this function) → M → pawn
tick → S (state `MainProc`) → **U** (UI tick: the attack / magic / item / situation decide).

#### 3.1.3 Command cool-down and the one-frame latch [I]

- `ctrl+0x8C8`: written by controller slot 676 0x141415A50 `(t)`: `if pawn.MyReactionCoolDownTime (+0xC78) == 1.0: t = 1.0`
  (native default 1.0; **`p_ex001_PawnBase` sets 1.5** [A], so for Sora `t` is the argument); `pawn.vt+0x2780(1)` (pawn+0xE60
  bit 17 = 1); `ctrl+0x8C8 = t`. Counted down by controller slot 635 0x1413F9790 (`-= dt`, floor 0). No attack path
  calls slot 676.
- pawn+0xE60 bit 17 is cleared **every frame** by the player pawn tick 0x141448AA0 (0x141448B50: `vt+0x2780(0)`). It is
  therefore a latch from the U phase of one frame to the pawn tick of the next: it makes `CanAttack` (final test),
  `CanLaunch`, `CanMagic` and slot 681 refuse during C of the frame after a command that armed the cool-down.
- pawn+0xE34 `command cool-down` of the pawn (`MyCommandCoolDownTime` pawn+0xC68 = **0.6 s**, ctor 0x141423D9E, not
  overridden [A]): set by the Attack `End` (5.4) when an **air** attack ends without a follow-up; counted down by the
  pawn tick 0x141449CD0 (0x14144A016); `CanAttack` and `CanLaunch` refuse while it is > 0 (pawn vt+0x1E10).

#### 3.1.4 Latencies [D]

Press in frame N (key edge seen by C(N) and by U(N)). Normal path: request in U(N) → from Idle / Run `SetState` takes
effect in S(N+1) (`End` of the old state, `Begin` + first `Tick` of the Attack); the clip clock first advances in M(N+2).
In a combo: the request in U(N) sets +0x143; the swap happens in S of the first later frame whose M fired
`TresEnableChange` (or in S(N+1) if it had fired already). This is **one frame later than section 3.5 of revision 1**
(which assumed the request in C). The acceptance test itself (`CanAttack`) sees the notify flags of M(N).
Jump-button requests (launch, risk dodge) and guard / dodge are issued in C(N) and take effect in S(N).

### 3.2 `CanAttack` (iface slot 44 0x141426330) [I, whole function]

```
ctrl = pawn.vt+0x948();  if !ctrl: false
if pawn.slot989(): false          // 0x1414365D0: pawn+0x1130 & 8, or pawn+0xE60 & 0x8001, or pawn.vt+0x1FE8(0)
if !pawn.slot1336(): false        // 3.1.1
if pawn+0xE34 > 0: false          // command cool-down (3.1.3)
if has(COMBO_MASTER 30): pawn+0xE28 = 1                                      // 0x141426401
if queued id ∈ {19, 32, 59, 61, 62}: false                                   // mask 0x6800000100080000
switch cur id:                                                               // tables 0x141426EFC / 0x141426F60
  1 IDLE:      !(Idle+0x9C (i32) > 0)                 // never written (actions spec II.0.1): always true
  2, 4, 5, 11, 26, 34..39, 98:  true                  // RUN, GLIDE, FLOATING, SWAYING, AF_ROLLER_COASTER, STYLEFINISH_*, AI_MOVE
  3 JUMP:      !(Jump+0x98 == 2 && Velocity.z > 0)    // pawn vt+0x1568
  7 CLIMBING:  sub >= 3 && queued is not an Attack
  8 WALLRUN:   queued is not an Attack && sub == 1
  9, 10, 21, 23, 24:  queued is not an Attack         // HOPPING, SLOPESLIDE, SUPER_SLIDE, WALL_KICK, POLE_TURN
  22 RAILSLIDE: cur.vt+0x398() && queued is not an Attack
  12, 13, 14, 75..90, 100, 156, 157, 159, 160, 161, 171 (attack classes):    // 0x141426587
      queued is not an Attack
      && cur+0x143 == 0                               // nothing requested yet during this attack
      && cur+0x164 != 0                               // the first hit window of the swing has opened
      && cur+0x84 & 0x20                              // TresEnableInput (or TresEnableChange, Tick copies it: 5.2)
      && (cur.combo < MaxCombo()  ||  cur+0x234 & 0x200000)                  // signed compare
      && T
  15 GUARD (0x141426832), cur must be a Guard:
      (queued is not an Attack && pawn.slot605(-1) && Guard+0xDC != 0 && Guard+0x84 & 0x20)   // slot 605: pawn+0x598 vt+0x448
      || ctrl+0x8A0 > 0                               // counter window open (5.6.1); no test of the queue or of the clip
      || (queued == null && Guard+0xDC != 0 && Guard+0x84 & 0x20)
  16 DODGEROLL: m_bEnableChange
  17 RISKDODGE: RiskDodge+0xD8 & 1 && !(RiskDodge+0xC8 & 4)
  18 FIREMAGIC: queued is not an Attack && !(Magic+0xD8 & 0x10) && (i32)Magic+0xD0 > 0 && Magic+0xB4 < MaxCombo() && T
  19 DAMAGE:    queued == null && Damage+0x90 == 0 && sub (+0x88) ∈ {1, 2, 7, 8, 9, 10} && m_bEnableChange   // table 0x14142700C
  20 AIR_SLIDE: flags +0xA8: bit 7 → false; bit 6 → true; else m_bEnableChange
  32 WEAPONCHANGE: sub != 0 && pawn+0xE28 != 0
  42 ATHLETICFLOW: ctrl.vt+0x15F0() && queued id <= 0 && state+0x138 & 0x10
  74, 97:      the state's own predicate (vt+0x388 / sub-object vt+8)
  102 SWIM:    returns Swim.vt+0x398() directly (skips the final test)
  111 USE_ITEM: m_bEnableChange;  122 BADSTAT_STUN: queued == null && m_bEnableChange
  142 COVER:   +0xA0 == 0 && !m_bEnableChange && sub != 10;  162 SLIDE_TURN_RO: +0xA1 != 0
  other:       false
T (0x141426729):  pawn+0xE28 != 0  &&  !(pawn+0xE48 bit 5)                   // hit confirm; combo not over
final (0x141426EDF): !(pawn+0xE60 bit 17)                                    // 3.1.3
```

#### 3.2.1 The hit-confirm byte pawn+0xE28 [I]

A chained attack (and an attack out of a cast) needs pawn+0xE28 != 0. Writers:

| Value | Where | When |
|---|---|---|
| 0 | pawn constructor 0x141423AF0 | start |
| 0 | pawn slot 981 0x141432780 (0x1414327C4) | Attack `End` when the next state is not an attack / cast / weapon change (5.4); the same function is the reset used by other states |
| 1 | Attack on-hit handlers 0x141341B10 (0x141341E45) and 0x141343B00 (0x141343BC4) | a contact that is not repelled (victim result not REFLECT / GUARD_REFLECT / GUARD); also pawn+0xE24 += 1, state+0x13C \|= 4, state+0x234 \|= 0x2000 |
| 1 | `CanAttack` 0x141426401, `CanLaunch` 0x14142B53B, pawn 0x1414426E0 | `has(COMBO_MASTER 30)` |
| 1 | magic `OnShot` 0x14137C9CA | the spell left |
| 1 | Attack slots 137 / 138 (0x14134CFFA, 0x14134CAAE), `PlayAttack` 0x1413386C2 | unison shots; one formchange kind |

So **without Combo Master the second attack of a chain is accepted only after some hit of this chain has landed**; the
byte stays 1 for the rest of the chain (later whiffs still chain). With no target in reach a press during the first
swing is lost and the next press, after the swing has ended, starts a new combo at step 0. Combo Master (ability 30,
victory bonus `Vbonus_036`, and a form ability of every formchange [A]) removes the rule. `CanMagic` has no such test:
a cast can always follow a swing (14).

### 3.3 `TryAttack(target, type)` (iface slot 45 0x141437330) [C read in full; every write and compare below checked in the disassembly]

```
if type == 1: return TryLaunch(target)                                       // iface vt+0x180
if !CanAttack(): false
id = pawn.vt+0x29C8()                       // attack state id (ATTACK 12 in base form)
ctrl.slot731(target)                        // 0x1414130A0: attack target record (section 7)
by cur id:
 1 IDLE, 2 RUN (0x1414375F1; JUMP falls in here when neither of its two cases applies):
     st = SetState(id)
     if pawn+0x10E4 > 0 (vt+0x1770; formchange carry timer, never armed in base form):
         st.combo = pawn+0xE18 + 1;  st+0x12C = pawn+0xE1C;  st+0x138 = pawn+0xE20
     W: if ctrl+0x8A0 > 0:                                                   // 0x14143767B, counter window open
            if AbilityCount(CRITICAL_COUNTER 104) > 0 && ctrl+0x878 bit 8: st+0x23C |= 1     // just-guard attack
            FUN_1414145C0(ctrl, 0.0, null, 0, 0)                             // close the window
     st.bAir(+0x142) = !OnGround(pawn);  st.target(+0x118) = target;  pawn+0xE18 = st.combo
 3 JUMP: Jump+0xA0 == 2:        st = SetState(id); st.target = target; if m_ChrUniqueID == 1: st.forcedKind(+0x1E8) = 0x37; pawn+0xE18 = st.combo
         Jump+0x114 bit 14:     (dive fall) FUN_141414350(ctrl, 0xB, 0, 0); b = ctrl+0x96C & 1; ctrl.vt+0x1830(0); Jump+0x114 &= ~0x4000
                                st = SetState(id); target; forcedKind = 0x35; pawn+0xE18 = st.combo; st+0x234 bit 14 = b
 4 GLIDE, 5 FLOATING: as IDLE with st.bAir = 1
 11, 74, 97, 102, 111, 122: st = SetState(id); bAir = !OnGround; target; pawn+0xE18 = st.combo           // no window test
 12, 13, 14, 75..90, 156, 157, 159..161, 171 (0x141437ADA):
     cur+0x143 = 1
     next = PushState(id)                                                    // Reset → everything not copied is zero
     next.combo = cur.combo + 1;  next.bAir = cur.bAir;  next.target = target
     next.reach(+0x154) = cur.reach;  next+0x1F0 = cur+0x1F0;  next+0x1F4 bit 0 = cur+0x1F4 bit 0
     pawn+0xE18 = next.combo;  if pawn+0x1464 (iface+0x1014) != 0: next+0x1EC |= 0x80        // friend unison
 15 GUARD (0x141437CA2):
     if the passed target's owner is a valid character pawn:
         tc = ctrl.slot685()                 // 0x14128A4B0: weak pointer ctrl+0x8B0, the component stored with the counter window
         if !valid(tc): tc = thatPawn.vt+0x2108(0)
         if valid(tc): ctrl.slot731(tc); target = tc
     st = SetState(id);  st.target = target;  pawn+0xE18 = st.combo
     if ctrl+0x8A0 > 0:                                                      // 0x141437DB8; otherwise a plain attack
         if AbilityCount(104) > 0 && ctrl+0x878 bit 8: st+0x23C |= 1
         by form kind ctrl+0xB38 (table 0x1414389DC): {0x86, 0x87, 0x8E, 0x8F, 0x92, 0x93, 0xDD..0xE0}: forcedKind = 0x25
                                                      {0x89, 0x91}: none
                                                      else: forcedKind = has(REVENGEDIVE 6) ? 0x24 : has(REVENGEIMPACT 5) ? 0x23 : 0x21
         FUN_1414145C0(ctrl, 0.0, null, 0, 0)
     (bAir is not written here; PreDecide recomputes it)
 16 DODGEROLL: st = SetState(id); then W and the rest of IDLE (no carry test)
 17 RISKDODGE (0x141437EFA): f = RiskDodge+0xD8;  RiskDodge.vt+0x3A8() (`CounterUsed`);  next = PushState(id)
     next.bAir = !OnGround;  target = (RiskDodge+0xD0 ? that pawn.vt+0x2108(0) : target);  next.target = target
     next.forcedKind = RiskDodge+0xE0 (0x28..0x2B);  pawn+0xE18 = next.combo
     if AbilityCount(104) > 0 && f & 2: next+0x23C |= 1
 18 FIREMAGIC (0x141437BEA): Magic+0xD8 |= 0x10;  next = PushState(id);  next.combo = Magic+0xB4 + 1
     next.bAir = Magic+0xD8 & 1;  next.target = target;  pawn+0xE18 = next.combo;  unison bit as above
 20 AIR_SLIDE: AirSlide.vt+0x390();  next = PushState(id);  W;  next.bAir = !OnGround
     target = (AirSlide+0x98 ? that pawn.vt+0x2108(0) : target);  next.forcedKind = AirSlide+0xB0;  pawn+0xE18 = next.combo
 32 WEAPONCHANGE: st = SetState(id); st.combo = pawn+0xE18 + 1; st+0x12C = pawn+0xE1C; st+0x138 = pawn+0xE20; bAir = 1; target
 8 WALLRUN (0x141437792) [C, third pass]: st = SetState(id); st.target = target; st+0x1EC |= 8; st+0x1E4 = WallRun+0xA8 (class: 0 up, 1 / 2 along);
     pawn+0xE18 = 0                                                          // the wall-run attack: Decide rule 4 (4.2)
 21 SUPER_SLIDE: st = SetState(id); target; forcedKind = 0x36; pawn+0xE18 = st.combo
 22 RAIL_SLIDE: pawn.vt+0x21C8() false and m_ChrUniqueID == 1 (Sora): st = SetState(id); target; forcedKind = 0x36
                (vt+0x21C8 true: forcedKind 0x37 when RailSlide+0xD0 == 2 else 0, bAir = 1, +0x234 bit 12: not Sora's branch [?])
 23 WALL_KICK: only when nothing is queued: next = PushState(id); target; forcedKind = 0x36; WallKick+0xA7 = 1
 24 POLE_TURN: st = SetState(id); target; PoleTurn+0xB0 bit 0 (a bar swing): forcedKind = 0x39 and Turn(PoleTurn+0xE4 rotation, 1.0);
                else (a pole spin): forcedKind = 0x38
 9 HOPPING [C; fourth pass]: st = SetState(id); st+0x1EC |= 0x10; st.target = target; pawn+0xE18 = st.combo
 10 SLOPESLIDE [C; fourth pass]: the same with st+0x1EC |= 0x20       // both bits are consumed by the head of PlayAttack (4.3)
 7 CLIMBING (st+0x1EC &= ~0x40), 34..39, 42, 95: not ported (no such state in the port)
```

Forced kinds 0x21 / 0x23 / 0x24 / 0x25 (guard counters) and 0x28–0x2B (risk-dodge counters) are rows of sheet
`sora-combat-attacks`; `PlayAttack` takes `forcedKind` instead of the decision (4.3).

### 3.4 Launch attacks (jump button during a combo) [I]

`CanLaunch` (iface slot 47 0x14142B3F0), whole function:

```
if pawn.m_ChrUniqueID (pawn+0x5F0, iface+0x1A0) != 1: false     // TRES_CHR_UID_SORA; written by the ATresPlayerPawnSora constructor 0x1413E761B → true for Sora
if !pawn.slot1336() || pawn+0xE34 > 0 || queued != null || pawn.slot902() (in water) || !ctrl: false
if !(pawn+0xE60 bit 18):
    if cur id ∉ {159, 160, 161} && cmdTable[form kind (ctrl+0xB38)].+0x28 > 0: false      // ctrl slot 847 0x141401160
    if has(COMBO_MASTER 30): pawn+0xE28 = 1
    if pawn+0xE50 != 1 && pawn+0xE54 != 1: false              // unused Rising Spiral / Groundbreaker stock
by cur id (table 0x14142B848):
  12, 90, 159, 160, 161:  cur is an Attack; queued is not an Attack; cur+0x143 == 0; cur+0x164 != 0; cur+0x84 & 0x20;
                          cur.bFinish(+0x140) == 0;  pawn+0xE28 != 0;  !(cur+0x1EC & 0x80)
  76, 77, 79, 85, 87, 88: only with pawn+0xE60 bit 18; !(pawn+0xE48 bit 5); then as above without the bFinish test
  18 FIREMAGIC:           queued is not an Attack; !(Magic+0xD8 & 0x10); (i32)Magic+0xD0 > 0; !(Magic+0xD8 & 0x40); pawn+0xE28 != 0
  other: false
final: !(pawn+0xE60 bit 17)
```

`TryLaunch` (iface slot 48 0x14143EAE0):

```
if !CanLaunch(): false;  ctrl.slot731(target);  id = pawn.vt+0x29C8()
attack states: cur+0x143 = 1 (0x14143EBE6);  next = PushState(id);  next.combo = 1;  next.bAir = cur.bAir;  next.target = target;  next.reach = cur.reach
FIREMAGIC:     Magic+0xD8 |= 0x10;  next = PushState(id);  next.combo = 1;  next.target = target;  next.bAir = Magic+0xD8 & 1
both: next+0x1EC |= 4;  next+0x1F4 bit 1 = pawn+0xE60 bit 18;  pawn+0xE18 = 1 (vt+0x1D50)
```

`Decide`, launch branch (0x14134B035–0x14134B0FB), confirmed:

```
kind = (pawn+0xE50 != 1) ? 0x1D : ((pawn+0xE54 == 1 && bAir && dz < 100) ? 0x1D : 0x1C)
if !bAir && kind == 0x1C: DirZ = 1000; fixedDirZ = 1
kind 0x1C: pawn+0xE50 = 2;  kind 0x1D: pawn+0xE54 = 2
state.combo = 0;  pawn+0xE18 = 0;  pawn.slot1303() (unison reset)
```

Availability [A]: `SLASH_UPPER` (10, row `A00010`, "Rising Spiral", 3 AP, max 1; victory bonus `Vbonus_028`) and
`AIR_ROLL_BEAT` (11, `A00011`, "Groundbreaker", 3 AP, max 1; `Vbonus_041`) are ordinary equippable abilities of Sora
(`sheets/entities/Load/Tres/TresAbilityData.json`, `TresVBonusData.json`; both equipped in `sora-loadout-99`). The stocks
are loaded from the equipped counts when a combo starts (`Decide` with combo == 0) and by pawn slot 981 at the end of a
chain, so each can be used **once per chain**; a launch resets the combo count to 0 (the chain continues with a fresh
count, `first = true`), and is refused on a finisher step (`bFinish`) and before a hit has landed (pawn+0xE28).
Kinds 0x1C / 0x1D are **reachable for base Sora**.

Pawn slot 1304 0x141430A60 (called by `Decide` one step before the last) [I for the early exit, C for the rest]: the
friend "combo finish" roll. It returns at once unless `(AbilityCount(190) + … + AbilityCount(194)) × MyFriendComboFinishAddRate
(pawn+0x148C) × 10 != 0` (0x141430AC8–0x141430B51), the pawn is player-controlled (`vt+0xD70() == 1`) and `ctrl+0x920 ==
0`; then it needs party member 0xB or 0xC within `MyFriendComboFinishRange` and rolls `rand() % 1000` to arm the unison
request (pawn+0x1464). Solo Sora: no effect. With the level-99 loadout the abilities are equipped, but no party member
exists in the port: no effect either; the host may ignore it.

### 3.5 Latencies

See 3.1.4 (revision 1 had them one frame short for the normal press path).

---

## 4. The combo decision [I] (rules: sheet `sora-combat-rules`)

`Begin` 0x141333210 calls slot 122 `Decide` 0x14134ACA0 (which first calls slot 121 `PreDecide`
0x14134C420), stores the result in state+0x1B0, then slot 120 `PlayAttack` 0x1413378B0.
`ComboLimit` (Second Form) overrides only `Decide` (0x141362AB0): its differences are the table of 10.5.

### 4.1 Inputs

```
PreDecide (0x14134C420):
  targetOnGround(+0x1DC) = true; dist(+0x1CC) = 0; dz(+0x1D0) = 0
  selfPos(+0x1A0) = root location
  TargetMetrics()                                   // slot 123 0x141330D30
  bAir(+0x142) = true
  if !(pawn+0xD80 & 1):
      if GroundProbe(pawn, 0, 800) hits && probeDist <= 30: bAir = false          // probe: movement spec 4.5
      if !bAir: FUN_14063C870(pawn, …): if its pitch result < -35: bAir = true     // [C] floor / slope test
  numEnemies(+0x1C4) = controller.slot726()         // controller+0x95C, or (>0 ? 1 : 0) when controller+0x96C bit 1
  lastKind(+0x1AC) = pawn+0xE10
TargetMetrics (0x141330D30), with tc = state.target (+0x118):
  targetPawn(+0x110) = tc's owner as a character pawn
  p = tc.slot369()                                  // lock-on position, vt+0xB88
  if targetPawn valid: targetOnGround = OnGround(targetPawn)
                       p += targetPawn.measuredVelocity * 8 * (1/30)        // 0x141330E25: pawn vt+0xE28 = movement+0x9A0 (location delta of the
                                                                            // target's last movement tick / dt); the f64 at 0x14871E340 is 0.0333…:
                                                                            // the decision aims at where the target will be in 0.2667 s
  state+0x1EC |= 2
  selfPos.z = pawn.slot220(…).z                     // adjusted actor location
  targetHigh(+0x1E0) = (p.z - selfPos.z) > pawn+0xBA8 (160)
  dz = p.z - selfPos.z
  dist = max(|p.xy - selfPos.xy| - 50, 0)
  DirZ(+0x1D4) = 0; if !(state+0x23C & 0x10): if |dz| > 30: DirZ = clamp(dz * 5, -1200, 1200)
```

With no target `dist = dz = 0`, `targetHigh = false`: the decision then always takes the
"near, single" branches (reach 0). The lead applies to the decision inputs (`dist`, `dz`, `DirZ`) only; the
per-tick traction of 6.3 uses the un-led lock-on position.

`MaxCombo()` = pawn slot 928 0x141433570 [C; the Second Form constants I]: base Sora → `4 + AbilityCount(onGround ?
COMBO_PLUS 28 : AIRCOMBO_PLUS 29)` (4 without the ability term while swimming or without a controller; 5 for pawn type
6, 3 for type 0x0A); in a formchange a constant by command kind (**Second Form 6**, no Combo Plus term; for kind 0x88 it
depends on ctrl.vt+0x1B28()).
`Finishers()` = pawn slot 1253 0x141433A60 [C; Second Form I]: base Sora → `1 + (TRIPPLE_SLASH 13 or CHARGE_THRUST 14)
+ (MAGICFLUSH 15 or FINAL_HEAVEN 110)`; **Second Form 3**. `has(id)` = pawn slot 668 0x1414358B0,
`AbilityCount(id)` = slot 672 0x141424130 (equipped copies; a few movement abilities are gated by
story flags).

### 4.2 Procedure (0x14134ACA0; kinds in hex)

```
last = lastKind; first = false
if combo == 0:
    reach = (dist > 400) ? 1 : (numEnemies > 1 ? 2 : reach)          // reach stays 0 otherwise
    first = true; last = 0; pawn.lastKind = 0
    pawn+0xE50 = AbilityCount(SLASH_UPPER); pawn+0xE54 = AbilityCount(AIR_ROLL_BEAT)
elif last ∈ {1C, 1D}: same reach rule; first = true
max = MaxCombo(); fin = Finishers(); m3 = combo % 3
// finisher slots
A_is_special = has(TRIPPLE_SLASH) || has(CHARGE_THRUST)
B = A_is_special ? 4 : 0;  C = 0
if has(MAGICFLUSH):  f = (has(FINAL_HEAVEN) && numEnemies < 2) ? 0C : 7;   if B == 0: B = f else C = f
elif has(FINAL_HEAVEN):                                                    if B == 0: B = 0C else C = 0C

if state+0x1EC & 8 (wall-run request) && pawn.vt+0x28B0() (weapon type ok [?]: taken as true for the Kingdom Key):   // [C, third pass]
       side = state+0x1C8, set by TargetMetrics 0x141330D30 from the class c = state+0x1E4, his forward f and the XY direction t to the target:
              c == 0: |f·t| >= 0.643 → 0, else (f × t >= 0) ? 2 : 1;   c == 1: (f·t < 0) ? 2 : 1;   c == 2: (f·t >= 0) ? 2 : 1;   no target: 0
       dz > 400 && dist <= 400: kind = 31
       elif dz < −250 && dist <= 400: kind = 34
       else side 1 → 32; side 2 → 33; side 3 → 34 with fixedDirZ, DirZ = −1000; else 31 with fixedDirZ, DirZ = 1000
       store, return
if state+0x1EC & 8: bAir = true                                                             // weapon type not ok
if state+0x1EC & 4 (launch): kind = (pawn+0xE50 != 1 || (pawn+0xE54 == 1 && bAir && dz < 100)) ? 1D : 1C
       if !bAir && kind == 1C: DirZ = 1000, fixedDirZ(+0x1DD) = 1
       used stock = 2; combo = 0; pawn+0xE18 = 0; pawn.slot1303(); store, return
if controller+0x920: finish flags; kind = 1E (sneeze); pawn.finishCount++; store, return
if state+0x1EC & 0x80: finish flags; kind = 3F..43 by pawn+0x1464; …; store, return

if combo + 1 == max - 1: pawn.slot1304()            // 0x141430A60: the friend "combo finish" unison roll; no effect for solo Sora (3.4)
if combo < max - fin || state+0x234 & 0x200000:     // NORMAL STEP
    if targetHigh && last ∉ {8, 17, 1F} && dist <= 400:
        kind = 8; fixedDirZ = 1; DirZ = pawn+0xBAC (3.5) * 200
    elif bAir && dz < -100:
        kind = (has(AIR_DOWN) && dz <= -250 && targetOnGround && last ∉ {0D, 0E} && dist <= 400) ? 0D : 0F
    else:
        switch last:
          1, 9, 18:          kind = (reach == 1) ? 3 : 2
          2, 0A, 11, 19:     kind = (reach == 1) ? 1 : 3
          3, 0B, 12, 1A:     kind = (reach == 1) ? 1 : 2
          8, 1F:             kind = 1
          0F, 10, 20:        kind = 8
          17:                kind = 3
          default: if first: kind = (reach == 1) ? 2 : (reach == 2 ? 3 : 1)
                   else m3 == 0: kind = reach + 1
                        m3 == 1: kind = (reach != 0) ? 1 : 2
                        m3 == 2: kind = (reach == 2) ? 2 : 3
        if bAir || (combo > 0 && targetHigh): kind += 0F
    pawn.finishCount(+0xE14) = 0
else:                                               // FINISHER STEP
    n = pawn.finishCount; bFinish(+0x140) = 1
    if n >= fin - 1 || combo >= max - 1: pawn+0xE48 bit 5 = 1; finishEnd(+0x141) = 1
    if bAir && dz < -100 && has(AIR_DOWN) && dz <= -250 && targetOnGround && last ∉ {0D, 0E} && dist <= 400:
        pawn.finishCount = n + 1; kind = 0E; store, return
    if n == 0:  kind = !A_is_special ? 4
                     : !has(CHARGE_THRUST) ? 5
                     : (!has(TRIPPLE_SLASH) || dist > 400 || numEnemies == 1) ? 6 : 5
    elif n == 1: kind = B
    elif n == 2: kind = C
    else:        kind = 1
    if bAir || targetHigh: kind += 0F
    pawn.finishCount = n + 1
// ability upgrades
1 → 9 with QUICK_SHAFT (107);  2 → 0A with FLASH_STEP (108);  3 → 0B with RADIAL_CUT (109)
10 → 18, 11 → 19, 12 → 1A with the same three;  8 → 1F with AERIAL_SWEEP (111);  0F → 20 with AERIAL_DIVE (112)
store kind in state+0x1B0
```

Consequences [D]:

- No abilities: max 4, fin 1 → ground, near, single target: 1, 2, 3, then finisher 4
  (`1301`, `1302`, `1303`, `1304`). With a far target (dist > 400 at combo start) the first attack
  is the approach kind 2; with several targets within 400 cm kind 3 comes first. `reach` is
  evaluated only at combo start and carried through the chain.
- Finisher abilities do not lengthen the combo: they replace the last normal steps (max 4, fin 3 →
  1 normal step and 3 finishers). Combo Plus adds normal steps.
- Air: kind + 0F (`1311`…). In the air the normal cycle is 10 → 17 (clip `1308`) → 12 → 10/11 …
- The second and later attacks of a chain exist only if the request was accepted: without Combo Master that needs a
  landed hit of this chain (3.2.1). The launch branch (jump button, 3.4) restarts the count at 0.
- Second Form: 10.5 (combo 6, finisher kinds 7A–7F).
- `kind 0` can result for finisher slot B / C when the ability set leaves the slot empty
  (fin counts MAGICFLUSH-or-FINAL_HEAVEN once): `PlayAttack` then plays nothing [? not reached with
  consistent ability sets].

### 4.3 Kind → clip, settings (0x1413378B0) [I]

Sheet `sora-combat-attacks` (41 kinds reachable with the Kingdom Key in base form; the switch has
224 cases, the rest are the other characters' and forms' moves). `PlayAttack`:

```
if forcedKind(+0x1E8) != 0: slot 124 (slide-attack variants); upgrade 0F/10/11/12 by the abilities as above; kind = forcedKind
if state+0x1EC & 0x50: kind = 10, bAir = true;  elif & 0x20: kind = (dz > 30) ? 10 : 2F, bAir = true
pawn.lastKind(+0xE10) = kind
defaults: m_bFaceAt = 1; state+0x234 = (… & ~6) | 1; stop_dist(+0x24C) = 160; z_gain(+0x14C) = 3.0;
          +0x21C = 0; +0x224 = +0x228 = 0; airMode(+0x1DF) = 0; +0x198 = 0
per kind (sheet): animation name, root_len(+0x1D8), bits of +0x234 (1 = XY homing channel, 2 = root-scale channel,
          8 = no vertical homing), keepFace (+0x1F4 bit 4), airMode, stop_dist, z_gain, fixed DirZ
tail:
  if !airMode: gravity on (pawn slot 741 (1)); DirZ = 0
  else: state+0x234 |= 4 | 0x10; GravityScale = 0.1; gravity off; SetMovementMode(5 = Flying)        // 0x14133A8BA–0x14133A8F7; timeline: 6.6
        +0x18C = 0.6                                  // arming value of the whiff cool-downs (5.4); not a hover time
  startScale = 1.0 (slide kinds: clamp(dist / range, 0.1, max))
  PlayAnim(name, mode 2 (one-shot, root motion), group +0x198, root scale (startScale, 1.0), …)       // state+0x40 vt+0x18; 0x14133AAB7
  state+0x1FC = state+0x17C = actor yaw
  if state+0x21C == 0: install the per-tick delegate state+0x90 run by slot 128: thunk 0x140430C70 = slot 116, the traction
                       update (0x14133AC47); alternatives: 0x140430C94 → slot 119 (0x14133AC3E), 0x140430C7C → slot 117, the warp
                       countdown (0x141337B41)
  pawn.vt+0x16D8(1)    // 0x14133AC7B: movement +0xA3C bit 2 = 1: CalcVelocity is skipped for the whole attack (6.1.1); End clears it
  [I 0x14133AC8A, fourth pass] if pawn bad status (+0x85D) == 13 SNEEZE && !(state+0x234 & 0x200000) && combo >= MaxCombo() - 2:
      FUN_14140DF30(controller, 1)      // NOT a camera: controller+0x920 = 1 and the HUD's Attack command becomes "Sneeze"
                                        // (0x141583FF0): the sneeze latch of 9.9.5; the next Decide takes rule 6
```

Clip data per kind (length, hit windows, `TresEnableInput` / `TresEnableChange` / `TresTraction` /
`TresFaceAt` times, root travel): sheets `sora-combat-attacks` and `sora-combat-hits` [A]. Example
[A]: `1301` 0.7667 s; TresEnableInput 0.1333; TresTraction 0.1333–0.3083; hit window 0.1667–0.4333
(group `KB_Stick`, row `KB_VERTICAL`, hit sound HIT1); TresEnableChange 0.3667; root travel 207 cm.

Fourth pass (2026-10-12) additions [I for each kind's case, C for the head]: the head's `+0x1EC & 0x50` / `& 0x20` test is the
HOPPING / SLOPESLIDE request (3.3): kind 0x10 out of a hop, 0x2F (or 0x10 when dz > 30) out of a slope slide, forced kind
cleared, bAir set; for kind 0x10 with one of the bits 4 - 6 and for the kinds 0x21 - 0x24, 0x27 - 0x2B, 0x2D - 0x30, 0x35 -
0x39: `pawn.vt+0x1388(1)` (equipment vt+0x508 [?]) and `state+0x234 |= 0x400000` (the target is re-acquired with the long
range: 7.10). Kind 0x2F `M_1143` (1.2 s, window 0.2333 - 0.7667 with row `RUN_SLOPE` 1.5 TOSS): root scale XY of the play
call = clamp(dist / 815, 0.1, 2.5), state+0x18C = 0.3, air mode (row `kind_2f`). Two per-kind bits that the sheet did not
carry: **state+0x234 bit 3** (the Z channel writes 0 while the traction window is open: 6.3) and **state+0x23C bit 4**
(`TargetMetrics` skips its DirZ write, 0x141331132) are both set by 0x1C Rising Spiral (`1319`, 0x14133817C) and 0x37
Sonic Cleave (`1182`), which also sets DirZ −1000, the dive flag, stop distance 250, `+0x190 |= 1` and `+0x148 = 5.0` (the
last two have no reader in the Attack class: byte scan of 0x141330000–0x141350000). So Sonic Cleave descends by its clip's
root motion (500 cm) with vz = 0 through its window 0–1.0 s, the dive's ground probe is never reached, and DirZ acts only
after the window (phase 2). Kind 0x38 Buzz Saw asks for the attack camera's mode 5 and 0x28 Hook Upper for mode 3 with the
target (7.12.1); `End` switches it off for kinds 0x28–0x2B, 0x36–0x39.

Third pass (2026-10-12) additions to the kind table [C 0x1413378B0; rows `kind_31` … `kind_34`, `kind_39`, `kind_22` of sheet
`sora-combat-attacks`]: 0x31 `M_1144` (defaults); 0x32 `M_1147` and 0x33 `M_1146` (root-scale channel added, root length 595);
0x34 `M_1145` (`m_bFaceAt` off, z gain 4.5, stop distance 60); 0x39 `1184` (Somersault: `m_bFaceAt` off, facing off, target
cleared, state+0x234 &= ~7 | 0x100, +0x21C = 3, air mode); 0x1E `BadStatSneeze` / `BadStatAirSneeze` (9.9.5). After the play
call `PlayAttack` calls `SetAnimTime` (anim interface vt+0xF0) only for kinds 0x5F (0.2 s) and 0x80 (0.3333 s, when the
previous clip is past 0.5): both belong to other keyblades' forms - nothing for the Kingdom Key.

---

## 5. Attack state lifecycle [I unless tagged]

### 5.1 Fields, `Reset`, `Begin`

Fields (state+): 0x90 per-tick delegate (traction / warp countdown, installed by `PlayAttack`), 0x110
`m_pTargetPawn`, 0x118 `m_HomingTargetComponent` (the request's target), 0x120
`m_OldTargetComponent`, 0x128 combo count, 0x12C / 0x138 **hit-window counters** of the chain (5.5;
copied to 0x130 / 0x134 at Begin; saved in pawn+0xE1C / +0xE20 between states), 0x13C bit flags (bit
2 "this attack landed a hit"; bits 0 / 1 are the arguments of slot 115), 0x140 bFinish, 0x141
finishEnd, 0x142 bAir, 0x143 next attack / cast queued, 0x14C z gain, 0x150 homing speed override,
0x154 reach, 0x164 first hit window opened, 0x168 weapon type at Begin, 0x16C athletic kinds, 0x180
target offset vector, 0x18C **arming value of the whiff cool-downs** (0.6 for an air-mode attack, 0
after a hit; 5.4), 0x198 play group, 0x19C voice variant, 0x1A0 self position, 0x1AC last kind,
0x1B0 kind, 0x1C4 numEnemies, 0x1C8 side class, 0x1CC dist, 0x1D0 dz, 0x1D4 DirZ, 0x1D8 root length,
0x1DC targetOnGround, 0x1DD fixed DirZ, 0x1DE dive, 0x1DF airMode, 0x1E0 targetHigh, 0x1E1 warp
start, 0x1E8 forced kind, 0x1EC request flags (bit 0/1 target position valid, 2 launch, 3 wall run,
4–6 athletic, 7 unison, 8), 0x1F4 facing flags (bit 0 copied along the chain, bit 1 form launch, bit
3 facing on, 4 keep, 5 pitch too), 0x1F8 facing timer, 0x1FC facing yaw, 0x200 target position,
0x218 facing pitch, 0x21C callback mode, 0x220 / 0x224 / 0x228 channel phases, 0x234 flags (bit 0 XY
homing channel, 1 root-scale channel, 2 air mode / Z channel, 3 no vertical homing, 4 hover gravity
set, 5 dive landed, **7 cancel window**, 8 cancel only at `TresEnableChange`, 12 carry the entry
speed, **13 a hit landed**, 14 dive-fall, 15 rush suppression, 16 / 17 form scale / form homing
rules, 18 not a first air attack, **19 chase armed**, 21 endless combo (Berserk Charge), 22
re-acquire the candidate, **23 reaction command shown**, 24), 0x238, 0x23C flags (bit 0 just-guard
attack, bit 1 guarding attack, 2 / 3 / 5 channel re-arm), 0x240, 0x24C stop distance, 0x254 cancel
timer, 0x258 warp delay, 0x25C, 0x264 bit 0 + 0x268 recoil record, 0x270 `m_pReflectCauser`, 0x278
`m_pRevengePawn`. `Reset` 0x14133D230 zeroes them (+0x22C = 11, +0x250 = 6000, +0x260 = 0.2).

```
Begin (0x141333210):
  pawn slot 616 (1, 1, 1.0)                 // Draw(weapon): (pawn+0x598).vt+0x4A8(b, immediate, rate), put-away timer 10.0
  weaponType(+0x168) = equipment slot 136(-1); pawn+0x6E4 = 1 (stance value)
  +0x130 = +0x12C; +0x134 = +0x138 (0x1413332E9); pawn+0xE1C = 0; pawn+0xE20 = 0
  if target invalid or not targetable (component vt+0xA80): target = controller.slot722() (current target, 7.6)
  if has(CHARGE_BERSERK 178) && controller style count == 0 && pawn+0x7C0 bit 1: state+0x234 |= 0x200000   // endless combo
  if !(state+0x234 & 0x1000): pawn.StopMovement()     // 0x141333408; pawn vt+0x16E8: Velocity = 0, Acceleration = 0, direct record cleared
  kind = Decide(); if state+0x234 & 0x80000 (chase armed: slot 113): kind = 0x22, warp start, +0x22C = 13
  PlayAttack(); sub-step = 1
```

### 5.2 `Tick` (slot 97 0x141341340) [I]

```
if pawn invalid: return
if slot129(): return                                   // recoil hand-off, 5.6
slot114(dt, dt*12); slot130(); slot125() (empty); slot126(dt) (facing); slot128(dt); slot131(); slot132() (empty)
if !(+0x258 > 0):                                      // warp delay not running
    if AnimEnded() (iface40 vt+0xC8)
       || (m_bEnableChange && +0x143 != 0)
       || +0x238 != 0
       || current clip name ∈ {names at 0x1468FBC90, 0x1468FBCA8, 0x1468FBCC0}       // RunLoopB, WalkLoopB, RunLoopW
        → DoChain (vt+0x270); return
if +0x25C > 0 && !warpStart(+0x1E1): +0x25C -= dt; if <= 0: +0x25C = -1
if (m_bEnableChange || +0x25C == -1) && pawn.vt+0x2060(): pawn.vt+0x2068(0)          // pawn+0x630 bit 2 off
if +0x143 == 0 && state id != 0x59 && pawn.vt+0x1A80(0) (in water):
    pawn+0xE48 bit 5 = 1;  if +0x234 & 0x80 && PushState(SWIM 102): DoChain; return
+0x158 = 0
if +0x164:                                             // first hit window has opened
    if m_bEnableChange && !m_bEnableInput: m_bEnableInput = 1
    if +0x234 bit 24 && m_bEnableInput: clear bit 24
    if !(+0x234 & 0x80) && (state id == 0x59 || !pawn.vt+0x1A80(0)):
        if !finishEnd(+0x141) && !(+0x234 bit 8):
            +0x254 += dt;  if +0x254 > 0.133333 (0x146903A28): +0x234 |= 0x80
        if m_bEnableChange: +0x234 |= 0x80
if (+0x1EC & 3) == 3: update the stored target position (pawn of the target .vt+0xE28 …); +0x1EC &= ~3
```

Slot 128 0x14134D510 [I]: runs the delegate stored at state+0x90 (`FUN_141336430`: bound → `Execute(dt)`), then
`if m_bEnableChange: if +0x234 & 0x10: { clear bit 4; GravityScale = class default (pawn vt+0x1658); gravity on (vt+0x1728(1));
SetMovementMode(3 Falling, false) } elif lastKind ∈ 0x3F..0x43 && !FUN_1405AD7C0(pawn): SetMovementMode(3, false)`.
The delegate is installed by `PlayAttack`: thunk 0x140430C70 = `this.vt+0x3A0` = **slot 116, the traction update
0x141349CC0** (0x14133AC47; the alternatives are 0x140430C94 → slot 119 0x141349C80 at 0x14133AC3E and 0x140430C7C →
slot 117 0x14134AA80, the warp countdown, at 0x141337B41). So traction runs every tick from the first Tick (in S, after
the facing update), and **the hover of an air attack ends at `TresEnableChange`**, not at the clip end.

Slot 117 0x14134AA80 [C]: `+0x258 -= dt`; at 0: for `warpStart` teleport (slot 136 with 150 / 100), then
`iface40.vt+0x1A0(pawn+0x8E0, +0x22C + 1, 0)` and `PlayAttack()`. Slot 131 0x141341870 [C]: only for kinds with +0x16C
(athletic); nothing in the base combo. Slot 113 0x1413325A0 [I]: `if m_pRevengePawn && its iface.vt+0x2C8() ∈ {2, 3, 4}:
+0x234 |= 0x80000`; returns bit 19. Slot 115: 5.5.

### 5.3 What ends an attack [I]

| Cause | Condition | Mechanism |
|---|---|---|
| clip end | `AnimEnded` | `DoChain` to the queued state or the default state (Idle; Idle's first tick calls `Fall` when airborne) |
| next attack / cast / launch | request accepted (3.2, 3.4, 14) → +0x143; then `TresEnableChange` | `DoChain` to the pushed state |
| guard | guard gate 0x1414294C0 @0x1414296EF: **Attack+0x234 bit 7** | `SetState`: replaces the attack at the next `MainProc` |
| dodge roll | dodge gate 0x141427CC0 @0x141428190: `OnGround` && bit 7 | `SetState` |
| air slide | air-slide gate 0x141425780 @0x141425D3E: availability bit && bit 7 | `SetState` |
| jump | jump gate 0x14142A3D0: attack ids → 0x14142AA45 = **false** | never; the jump press during an attack only reaches `TryLaunch` |
| run / stick | `CanRun` 0x14142CE00: attack ids → 0x14142D180 = **false** | never: the stick does nothing during an attack |
| swim | in water with bit 7 | `PushState(SWIM)` + `DoChain` |
| damage | section 9 (queue cleared, `SetState(DAMAGE)`) | `End` runs with next id 19 |
| recoil | 5.6 | REFLECT (66) or the guard-hit entry |

Bit 7 of state+0x234 ("cancel window") is raised by Tick: **0.1333 s after the first hit window opened** (timer +0x254,
only outside water), or at `TresEnableChange`; for the last finisher of a combo (`finishEnd`) and for kinds with
+0x234 bit 8 only at `TresEnableChange`. Revision 1 called this the "swim exit" timer and named
`m_bEnableChange` as the dodge / guard condition: both corrected (`docs/sora-actions-spec.md` II.0.2 point 1 is right).

### 5.4 `End` (slot 98 0x141335120) [I, whole function]

```
pawn must be a valid ATresPlayerPawnBase, else return
ctrl = controller;  kind = pawn+0xE10 (last attack kind)
if warpStart(+0x1E1): iface40.vt+0x1A0(pawn+0x8E0, +0x22C + 1, 0)
+0x234 &= ~0x1000000
slot139(0)                               // 0x14134CC10: weapon effect group of the attack off (only when (pawn+0x460).vt+0xA8() > 0)
slot142(0)                               // 0x14134CAD0: if +0x23C & 2: restore the reaction of collision part "Body"; clear the bit
if +0x21C == 3: [attached kinds 0x39 / 0xC2: detach, StopMovement, re-place the actor at +0x240: out of scope]
elif kind == 0x35 (dive fall): if +0x234 & 0x4000: (pawn+0x440).vt+0x208("Range5000"); ctrl.vt+0x1830(0); ctrl.vt+0x1820(0)
pawn.vt+0x1388(0, 1, 0)                  // pawn+0x598 vt+0x508
if +0x1F4 & 0x20: Turn((0, yaw, roll) of the actor rotation, 1.0); clear the bit      // level the pitch
if ctrl && kind ∈ {0x28..0x2B, 0x36..0x39, 0x3F..0x43, 0xD0}: FUN_14140DF20(ctrl, 0, 0, 0, 0)
pawn.vt+0x13B8(7)
pawn.vt+0x16D8(0)                        // movement +0xA3C bit 2 = 0 (skip-CalcVelocity off; PlayAttack set it)
MaxWalkSpeed = default (vt+0x1628);  MaxAcceleration = class default (vt+0x1648, movement +0x39C)
pawn+0x630 bit 2 = 0 (vt+0x2068(0))
if +0x234 & 0x800: FUN_1405B6100(pawn+0x460)
if +0x1E2: (pawn+0x448).vt+0x180(); +0x1E2 = 0
if queued id != 12: pawn.slot1303()      // 0x141431A80: clear the unison request pawn+0x1464
gravity on (pawn.vt+0x1728(1))
if !pawn.slot902() (not in water):
    if OnGround(pawn):
        if +0x16C:            SetMovementMode(3 Falling, bStop = false)
        elif +0x234 & 4:      SetMovementMode(3 Falling, bStop = true)       // air-mode attack that ended on the floor
        else:                 SetMovementMode(1 Walking, bStop = true)       // Velocity = 0, Acceleration = 0
    else:
        SetMovementMode(3 Falling, bStop = false)                            // velocity kept
        if +0x234 & 0x2000 (a hit landed): +0x18C = 0
        else: pawn+0xE38 = MyAirSlideCoolDownTime (pawn+0xC6C, 0.6);  pawn+0xE3C = MyAirGuardCoolDownTime (pawn+0xC70, 0.3333)
GravityScale = class default (vt+0x1658)
by queued id (table 0x141335CC4):
  attack classes {12, 13, 14, 75..90, 156, 157, 159, 160, 161}, next is an Attack:
      if slot113(): next+0x234 |= 0x80000;  if target: next.target = target
                    if AbilityCount(104) > 0 && +0x23C & 1: next+0x23C |= 1
      if !(+0x13C & 5): +0x12C = +0x138 = 0
      next+0x12C = +0x12C;  next+0x138 = +0x138
  18 FIREMAGIC, next is a FireMagic: same reset;  next+0xB8 = +0x12C;  next+0xC4 = +0x138
  32 WEAPONCHANGE: ctrl: FUN_14140DF20(ctrl, 0, 0, 0, 0); then as 162
  162: same reset;  pawn+0xE1C = +0x12C;  pawn+0xE20 = +0x138
  every other id, and no queued state:
      pawn.slot981(+0x18C > 0)           // 0x141432780: pawn+0xE14 = 0, +0xE18 = 0, +0xE24 = 0, +0xE28 = 0, +0xE48 bit 5 = 0,
                                         //   +0xE50 = AbilityCount(10), +0xE54 = AbilityCount(11), +0xE10 = 0;
                                         //   if arg: pawn+0xE34 = MyCommandCoolDownTime (0.6);  ctrl.slot731(null);  FUN_14140DF30(ctrl, 0)
      if (+0x13C & 5) && !(+0x18C > 0): keep, else +0x12C = +0x138 = 0
      pawn+0xE1C = +0x12C;  pawn+0xE20 = +0x138
      ctrl: FUN_14140DF20(ctrl, 0, 0, 0, 0)
tail:
  unless queued is a Guard with Guard+0xDC != 0 (the guard-hit entry):
      slot141(0)                         // 0x14134C660: if +0x234 & 0x800000: FUN_141414350(ctrl, 0, 0.0, 0); clear (reaction command off)
      ctrl: FUN_1414142D0(ctrl, 0)       // ctrl+0x878 bit 9 = 0; pawn.vt+0x2840(0)
  pawn.vt+0x22E8()                       // cancel a pending driven move
  if finishEnd(+0x141) && game object (0x141522B10) && !(obj+0x2EB): pawn+0xE60 bit 19 = 1
```

Reading it by case [D]:

- **Chained ground attack**: no `StopMovement` call anywhere on this path — the velocity is zeroed by
  `SetMovementMode(1, bStop)` (same zeroing, actions spec II.2). The next `Begin` zeroes it again unless +0x234 bit 12.
- **Air attack** (`airMode`, +0x234 bit 2; PlayAttack set +0x18C = 0.6): still airborne → Falling, velocity kept,
  gravity on, default gravity scale. If it **whiffed** (no hit landed: bit 13 clear) *and* is not followed by another
  attack / cast, three cool-downs start: commands 0.6 s (pawn+0xE34: no attack, launch or command-cool-down-gated action),
  air slide 0.6 s (pawn+0xE38), air guard 0.3333 s (pawn+0xE3C). A hit clears +0x18C, so none starts. Ground attacks
  never have +0x18C > 0.
- **Interrupted** (damage, guard, dodge): same function; the "every other id" branch resets the pawn's combo memory.
- `+0x18C` is also what `MyCommandCoolDownTime` hangs on in the cast's End (magic spec 5.2).

### 5.5 The counters +0x12C / +0x130 / +0x134 / +0x138 [I]

Slot 115 0x141332790 `(rowName, r8b, r9b, count)`: `unless (r9b && +0x13C & 2): { if pawn.slot932(pawn.slot933(rowName)):
+0x12C += count;  +0x138 += count }`; `if r9b: +0x13C |= 2`; `if r8b: +0x13C |= 1`. Callers in the Attack class pass
`r8b = r9b = 0`: the `TresAtk` begin handler 0x1413459E0 (`count = max(m_ComboCountUpParam, 1)`, skipped when
`m_bIgnoreComboCountUp`: activation+0x6C and +0x5C bit 7) and the count-up handler 0x1413426A0.

- pawn slot 933 0x1405AD700 = the attack row's `m_bIsMagicAttack` (row+0x50 bit 0, looked up in the pawn's attack table).
- pawn slot 932 0x1405AD380(isMagic) = `!(has(isMagic ? ENDLESS_MAGIC 82 : CHARGE_BERSERK 178) && pawn+0x7C0 bit 1)`;
  pawn+0x7C0 bit 1 = "MP charge running" (pawn vt+0x17E0).
- So: **+0x138** = hit windows opened so far in this chain (each weighted by its count parameter); **+0x12C** = the same,
  not counted while MP is charging with Berserk Charge (the "combo step" that feeds COMBO_UP / AIRCOMBO_UP: damage info
  +0x54 = state+0x130). `Begin` copies `+0x130 = +0x12C`, `+0x134 = +0x138` (0x1413332E9): the values **before** this
  attack's own windows.
- Carry rule (5.4): the two counters pass to the next attack / cast only if **this attack landed a hit** (+0x13C bit 2,
  set by the on-hit handlers; bit 0 has no writer in the Attack class); otherwise the next step starts from 0. With
  Berserk Charge the combo step therefore freezes during MP charge while the combo itself becomes endless (+0x234 bit 21).

Third pass (2026-10-12): slot 115 (0x141332790) adds to +0x12C only when `pawn.vt+0x1D20(rowIsMagic)` says so (0x1405AD380, verdict
same; `rowIsMagic` = pawn.vt+0x1D28: bit 0 of the row's flag byte +0x50): a physical row not while the MP charge runs with
CHARGE_BERSERK (178), a magic row not with ENDLESS_MAGIC (82). +0x138 is always counted.

### 5.6 Guard, risk-dodge and recoil hand-offs

#### 5.6.1 The two controller timers [I] (agrees with `docs/sora-actions-spec.md` II.1; corrects combat 3.2 / 3.3 / 13)

| Field | Meaning | Writer | Countdown |
|---|---|---|---|
| ctrl+0x89C | **just-guard timer**, 0.1333 s (pawn+0xCD4) | `FUN_141414B70` (Guard `Begin`, and Attack slot 142 for guarding attacks) with CRITICAL_COUNTER (104) on the ground | controller slot 636 **0x1413F85D0**: `-= dt`, floor 0 (the actions spec had not located it) |
| ctrl+0x8A0 | **counter window**, 1.0 s (2.0 for some form modes) | `FUN_1414145C0(ctrl, time, attacker, success, critical)` on a successful guard; closed by the same function with time 0 | controller slot 637 0x1413F8130: set to 0 when the stored attacker fails 0x1413F0040; `-= dt`; at 0 the stored pointers are cleared |
| ctrl+0x878 bit 8 | "the guard was a just guard" | `FUN_1414145C0` when `critical` and AbilityCount(104) > 0 | cleared when the window closes |
| ctrl+0x8A4 | counter command id (3, 4 with REVENGEIMPACT, 5 with REVENGEDIVE) | `FUN_1414145C0` | |

#### 5.6.2 Counters

- **Guard counter**: `CanAttack` GUARD branch accepts an attack whenever the counter window is open (`ctrl+0x8A0 > 0`),
  whatever the guard clip's notifies; `TryAttack` replaces the Guard by an Attack with `forcedKind` 0x24 (REVENGEDIVE) /
  0x23 (REVENGEIMPACT) / 0x21, target = the guarded attacker's lock-on component, `+0x23C bit 0` (just-guard attack:
  damage rule of 8.3) when bit 8 is set, and closes the window. Without an open window the same branch gives a plain
  attack out of the guard-hit clip after its `TresEnableInput` (`Guard+0xDC != 0`). The window also upgrades an attack
  from **Idle, Run, Dodge Roll, Air Slide, Glide and Floating** (block W of 3.3): it gets the just-guard bit and closes
  the window, but no forced kind — the counter kinds exist only out of the Guard state.
- **Risk-dodge counter**: `CanAttack` RISKDODGE branch (`+0xD8 & 1` offered, `+0xC8 & 4` not used yet); `TryAttack` calls
  the state's `CounterUsed`, pushes an Attack with `forcedKind = RiskDodge+0xE0` (0x28–0x2B), target = the dodged
  attacker's component, just-guard bit from `RiskDodge+0xD8 bit 1`. The attack starts when the Risk Dodge chains.
- **Air-slide attack**: `forcedKind = AirSlide+0xB0`, target from `AirSlide+0x98` (aerial recovery: the attacker).
- **Blow counter** (out of an Aerial Recovery, kind 0x27): 9.5.

#### 5.6.3 Recoil (slot 129 0x141331AF0) [I constants, C structure]

The on-hit handler stores a recoil record when the victim's result is REFLECT / GUARD_REFLECT / GUARD (8.5.1):
state+0x264 bit 0, +0x268 = result, +0x270 = `m_pReflectCauser`. The next Tick:

```
if +0x264 & 1:
    if +0x268 == 4 (GUARD_REFLECT): if iface.vt+0x450(0) (counter-allowed gate 0x141429CF0) && iface.vt+0x460(m_pReflectCauser) (0x141444C30):
                                        if has(GUARD_REGENE 74): pawn.vt+0x1878(0, 3, 0, 0, 0)
                                        DoChain; return true
    else: st = FUN_1405B9DC0(iface, 66 REFLECT); if st is a Reflect: st+0x88 |= 1; st+0xB8 = m_pReflectCauser; st+0x98 = 400.0; DoChain; return true
```

Agrees with the Reflect section of the actions spec (weapon recoil, travel 400).

The gate and the two entries, read in the third pass (2026-10-12) [C; the state list is the jump table of the function]:

```
CounterAllowed(bAuto)  iface slot 138 0x141429CF0 (verdict differs):
    current state ∈ {12..16, 19, 20, 75..85, 87..90, 156, 157, 159..161}: go on;   137: false
    any other state: only if bAuto && (iface+0x10).vt+0x2D0() (the automatic-guard test) [?]
    w = pawn.vt+0x12C8(-1) (equipment vt+0x428 [?]);  w ∈ {1, 2, 4, 8..19}: true
    TresCmdData[ctrl.form_cmd (+0xB38)].Param1 == 0 (no form): true;   else false
GuardReactionOrNot(…, causer)  iface slot 139 0x14143CA90 (verdict same):       // TakeDamage, victim result GUARD_REFLECT (8.4)
    if !CounterAllowed(0): false;   ClearQueue;   return GuardReaction(causer) != null
GuardReaction(causer)  iface slot 140 0x141444C30 (verdict same):
    st = PushState(GUARD 15);  FUN_1414145C0(ctrl, 1.0, causer's pawn (a weapon → its owner), 0, 0)      // the 1.0 s counter window
    st+0xE8 |= 2 (success);  st+0x110 = causer;  st+0xDC = 1 (a hit entry: GuardHit);  pawn.vt+0x1048(0, 0, 0)
```

So a hit that Sora's body answers with GUARD_REFLECT (and his own swing that an enemy answers so) becomes a successful
guard-hit with the counter window whenever he is attacking, guarding, rolling, air-sliding or flinching; out of any other
state the gate refuses and the weapon recoil (state 66) plays. The weapon test (fourth pass): equipment vt+0x428 0x141780500
is `return [equipment+0x220]` [I], the keyblade's kind written once when it is equipped (0x14177C6C0) - 1 for the Kingdom
Key (the switch of `AwardAtMax`, 10.2) - so the test passes for the Kingdom Key in and out of Second Form. Port: `combat/guard.rs` `reaction_allowed`, `states/damage.rs` `take_hit`,
`states/attack.rs` `recoil`, `states/guard.rs` `guard_reaction` (actions side).

#### 5.6.4 Final Blow: the chase attack, kind 0x22 (ability REVENGE_EX 7) [C; third pass 2026-10-12]

```
on contact, not repelled (0x141341F5F–0x14134208A), when the victim is the state's target pawn (+0x110) and
    kind ∈ {0x21, 0x23, 0x24}  (0x25 only in the forms 0xDD..0xE0):
        if has(REVENGE_EX 7): m_pRevengePawn (+0x278) = victim
                              bit 19 of +0x234 = slot113()
                              if bit 19: slot141(0xC)      // 0x14134C660: FUN_141414350(ctrl, 12 REVENGESLASH2, 1.0, 0); +0x234 bit 23
slot 113 (0x1413325A0) [I]: m_pRevengePawn && its request interface vt+0x2C8() ∈ {2, 3, 4}: +0x234 |= 0x80000; return bit 19
    vt+0x2C8 = iface slot 89: for a player pawn 0x1414339F0 = the running Damage state's vt+0x388 [C]; the three values are
    read here as the damage kinds BLOW, SIDESPIN, VERTICALROLL (the counter rows are VERTICALROLL / SIDESPIN) [D]
End (5.4): next is an Attack: if slot113(): next+0x234 |= 0x80000
Begin of the next attack (5.1): bit 19: kind = 0x22; warp start (+0x1E1 = 1); +0x22C = 13
PlayAttack with the warp pending: the countdown delegate slot 117 (0x14134AA80): +0x258 −= dt; at 0:
    if +0x1E1: +0x240 = actor location; slot136(&location, 150.0, 100.0); SetActorLocation(location, sweep); +0x1E1 = 0
    anim iface vt+0x1A0(pawn.vt+0x2458(), +0x22C + 1, 0);  PlayAttack()          // "ChaseAttack" = clip 1290, root length 350, air mode
slot 136 (0x141337320, verdict not-compiled) for kind 0x22: T = target component vt+0xB88() (its lock point);
    needs the sweep pawn.vt+0x2750(own location → T, …, 60.0) to answer; dir = normalize2D(T − own);
    point = T − dir × 150; destination = point with z − 100; a second sweep (T-level point → destination) clamps it
    (other kinds: a random yaw offset of ±90° instead of the straight line)
```

The countdown value: `Reset` gives +0x260 = 0.2 and 6.7 says "after a 0.2 s timer"; the copy into +0x258 was not located
[?: taken as 0.2]. Sheet rows `chase.*`. Port: `combat/hit.rs` (`on_contact`, `chase_ready`), `states/attack.rs` (Begin,
Tick, End), host seam `Targets::reaction_state(id)` (the damage kind of the reaction the target is in, 0 = none); test
`final_blow_chases_the_target_of_a_guard_counter`.

---

## 6. Movement during an attack [I unless tagged]

Component = `TresAICharMovementComponent` (pawn+0x578, vtable 0x145D073F0). Fields: Velocity +0x214, mode +0x314,
GravityScale +0x2F8, flags +0xA3C (bit 1 gravity disabled, bit 2 skip `CalcVelocity`), root-motion velocity +0xA24
(used when +0x458 bit 2, set in the constructor), body-push displacement +0xBA8 / velocity +0xBB4, direct-velocity
record +0xC0C..+0xC14 with flags +0xC18.

### 6.1 What moves Sora during an attack

#### 6.1.1 Every attack runs with `CalcVelocity` switched off [I]

- `Begin` 0x141333408: unless state+0x234 bit 12: `StopMovement` (pawn vt+0x16E8: Velocity = 0, Acceleration = 0,
  direct record cleared).
- `PlayAttack` 0x14133AC7B (and 0x141337B71 on the warp-start path): `pawn.vt+0x16D8(1)` → component vt+0xA40
  0x14057E740 → **+0xA3C bit 2 = 1**. `End` 0x14133568C clears it.
- `CalcVelocity` 0x1405795F0: returns at once (no tail) when `!vt+0x5B8()`, `dt < 1e-6` or the owner is a simulated
  proxy; then `test [+0xA3C],4; jne 0x14057A286` (0x140579658): with the flag the whole body — braking, friction,
  input acceleration, speed clamp — is skipped and only the tail runs:
  `if +0xC18 & 1: Velocity.xy = (+0xC0C, +0xC10); clear bit 0` · `if +0xC18 & 2: Velocity.z = +0xC14; clear bit 1` ·
  `if +0x594 & 0x10000: vt+0x8C0(dt)`.

So during an Attack state **Velocity is exactly what the state last wrote** (0 after `Begin`), plus gravity in Falling.
The stick has no effect in any mode (the Acceleration computed by the tick head is never used: both `CalcVelocity`
calls of `PhysFalling` are skipped, so its air-control term is 0). `MaxWalkSpeed`, friction and braking play no role.

#### 6.1.2 The direct-velocity record and the other bits of +0xC18 [I]

| Bit | Meaning | Writers | Readers |
|---|---|---|---|
| 0 / 1 | direct XY / Z pending | setters vt+0x9E8 0x140592DD0 (XYZ), vt+0x9F8 0x140592E10 (XY), vt+0xA08 0x140592E40 (Z); cleared by vt+0xA18 0x14057CFE0 (both, record zeroed), 0x14057D010 (XY), 0x14057D030 (Z) | **only** the `CalcVelocity` tail 0x14057A286 |
| 2 | "ignore body push" | vt+0xC28 0x1405943A0 | vt+0xC30 0x1405891D0 (6.5) |
| 3 | "land only on a downward step" | vt+0xC48 0x140594950 | `PhysFalling` 0x14058D6F2, 0x14058D92D |
| 4 | "body push in XY only" | vt+0xC40 0x140593D50 | vt+0xC38 0x140588790 (`m_bBodyCollPushVectorConstraintXY` \|\| bit 4) |
| 5 | (movement tick) | — | 0x14058CE56, 0x140595805, 0x140598360 |

`PhysWalking`, `PhysFalling` and `PhysFlying` do not read the record themselves. A setter writes `Velocity` at once
*and* arms the pending copy, so with the body of `CalcVelocity` skipped the pending copy re-writes the same value: during
an attack the record is redundant, the plain `Velocity` governs. The Attack state uses pawn vt+0x16C0 (→ vt+0x9F8, XY) and
vt+0x16D0 (→ vt+0xA08, Z); bits 2–5 are not touched by the Attack state.

#### 6.1.3 Displacement of one movement tick [I for the formulas' terms, D for the composition]

`RM` = root-motion velocity of the tick = world root translation extracted by the pose tick ÷ dt, after the root scale
(6.4); `P` = body-push term (6.5), zero unless push bodies overlap; `V` = Velocity.

| Mode | Delta of the tick | Source |
|---|---|---|
| Walking (ground attack) | `((V.x + RM.x + P.x), (V.y + RM.y + P.y), 0) × dt` along the floor (ramp projection, step-up, slide: movement spec 3.4); `V.z` is zeroed (`MaintainHorizontalGroundVelocity`) | `PhysWalking` 0x1405902E5–0x14059033F |
| Flying (air-mode attack until its window closes) | `(V + RM) × dt + Pdisp`; no gravity, no floor search, no landing; a blocked move slides; Velocity is kept through a slide | `PhysFlying` (actions spec II.2) |
| Falling (non-air-mode attack in the air; air-mode attack after the window / after `TresEnableChange`) | `V.z += g × t` first, with `g = −980 × GravityScale` (0 while gravity is disabled); delta = `((Vold + V)/2 + RM + P) × t`; a walkable hit lands | `PhysFalling` (movement spec 4.4), 0x14058D707 |

Timing: the state's writes happen in S(n) and are used by M(n+1); the root motion used by M(n+1) is extracted by the pose
tick of M(n+1) itself.

Ground attack with the XY channel, 60 Hz [D from 6.3]: commanded speed after the window opens `v ← v + (800 − v) × 0.2`
per tick: 160, 288, 390.4, 472.3, 537.9, 590.3, 632.2, 665.8, 692.6, 714.1, … cm/s (travel 85.7 cm in the first ten
ticks); target inside the stop distance or window closed: `v ← v × 0.5` per tick (remaining travel `v/60` cm in total).
Without a target the channel never commands a speed: the ground combo moves by root motion only. The homing speed is
added to the root motion, it does not replace it.

### 6.2 Facing (slot 126 0x14134D0E0)

```
if +0x1F4 & 4: clear it; return
alpha = dt * 12
if m_bFaceAt (state+0x84 bit 4): clear it; slot 127 (facing on, timer 0.2); alpha = 1.0; TargetMetrics()
if !(+0x1F4 & 8):                                   // facing off
    if +0x1F4 & 0x20: Turn((pitch +0x218 or 0 after EnableChange, yaw +0x1FC), alpha)
    return
if target valid: p = target lock-on position (+0x200); d = p - selfPos + offset(+0x180)
                 yaw(+0x1FC) = yaw of d.xy (pitch +0x218 too with bit 0x20)
Turn((pitch, yaw +0x1FC, 0), alpha)                 // pawn slot 438: slerp, movement spec 3.1
if !(+0x1F4 & 0x10): timer -= dt; if timer <= 0: facing off
```

Every attack starts with `m_bFaceAt = 1`: on its first tick Sora **snaps** to the target and keeps
turning toward it at `12·dt` for 0.2 s (whole attack for `keep_face` kinds); each `TresFaceAt`
notify snaps again. With no target the yaw stays the actor yaw at `PlayAttack`: the stick does not
steer an attack [I: no stick read in the state].

### 6.3 Traction update (slot 116 0x141349CC0) [I, whole function]

Called every state tick through the slot-128 delegate (5.2). `Approach(cur, target, rate, dt)` = slot 135
0x1413364D0: `cur == target → target`; `d = target − cur`; `f = rate × dt × 60`; `if f < 1: d ×= f`; `|d| < 0.0003 →
target`; else `cur + d`.

```
vel = pawn.vt+0x2B8() (Velocity);  speedXY = |vel.xy|;  vz0 = vel.z
self = slot134()                                  // 0x141336940: actor location with z from pawn.vt+0x6E0
if target (state+0x118) valid: d = target.vt+0xB88() − self + offset(+0x180)
                               dzc = clamp(d.z, −500, 500);  dxy = min(|d.xy|, 800)
else dzc = dxy = 0
firstAir = (combo == 0 && bAir) && !(+0x234 bit 18);   k = pawn+0xBB0 (MyAttackFirstAirMoveRate 1.5)
window = m_bEnableTraction (state+0x84 bit 2, TresTraction notify state)

XY channel (+0x234 bit 0):
  target = 0;  rate = 0.2;  inside = false;  behind = false
  if +0x234 bit 17 && target valid:                               // form kinds
      behind = dot(forward(yaw +0x1FC), normalize2D(lockOn − self)) < 0;  if behind && +0x1EC bit 0: pawn.vt+0x140(...)
      k = 1.0
  phase +0x220:  0 && window → 1;   1 && !window → 0 (so a second window re-opens the channel)
  if phase == 1 (window open):
      if bit 17:   target = behind ? 0 : 800
      elif bit 15: inside = true
      elif bit 12: speedXY = pawn.vt+0xE38();  clear bit 12;  +0x150 = 2000          // slide kinds: carry the entry speed
      else:        target = +0x150;  if target == 0: if dxy > stop_dist(+0x24C): target = 800 else inside = true
      if firstAir: target ×= k;  rate = 0.2 × k
  if target > 0:                          rate as above
  elif inside:                            rate = 0.5
  elif phase was 0 and stays 0 && behind: rate = 0.5
  else:                                   rate = (airMode(+0x1DF) && !firstAir) ? 0.1 : 0.5
  v = Approach(speedXY, target, rate, dt)
  pawn.vt+0x16C0(dir(yaw +0x1FC) × v)                             // direct XY, every tick of the attack

Root-scale channel (+0x234 bit 1):  phase +0x224:  0 && window → 1;   2 && window && +0x23C bit 2 → 1 (acts next tick)
  phase 1, window open:
      bit 15:  SetRootScale(0.1)
      bit 16:  dxy == 0 → SetRootScale(1.0), flag;   dxy > stop_dist → SetRootScale(root_len > 0 ? dxy/root_len : 0);   else SetRootScale(0.1), flag
      else:    target valid → SetRootScale((root_len > 0 ? dxy/root_len : 0) × (firstAir ? k : 1));   no target → SetRootScale(1.0)
  phase 1, window closed:  unless bit 15 or 16: SetRootScale(1.0);   phase = 2
  SetRootScale(s) = FUN_1405A1D30(pawn+0x440, s, 1.0, null)       // horizontal s, vertical 1.0 (6.4)

Z channel (+0x234 bit 2, set by PlayAttack for air-mode kinds):  phase +0x228:  0 && window → 1
  phase 1, window open:
      if +0x234 bit 3: vz = 0
      elif dive (+0x1DE): g = probe 0x14063A650(pawn, 0, 800) distance (800 when nothing; 0 when +0x1EC bit 8 && a hit landed)
              if pawn.vt+0x1A80(0) || g < 1.0: +0x234 |= 0x20; +0x23C &= ~0x20; shot handler (iface40 vt+0x228); traction notify bit off;
                                               StopMovement; +0x234 &= ~4; DirZ = 0; vz = 0
              else vz = min(vz0, DirZ) + DirZ × 0.25
      elif fixedDirZ (+0x1DD): vz = DirZ   (0 for kind 0x24 while the scale channel raised its flag)
      else: if sign(DirZ) == sign(dzc) != 0: DirZ = vz = dzc × z_gain(+0x14C)   else vz = vz0
      pawn.vt+0x16D0(vz)                                          // direct Z, every tick of the window
  phase 1, window closed:
      if +0x16D: DirZ = 0; offset.z = 0
      if !inWater: gravity on (pawn.vt+0x1728(1));  SetMovementMode(3 Falling, false)
      phase = 2
      if +0x23C bit 5 && !(+0x234 bit 5): land sequence as above
  phase 2:
      if DirZ != 0: DirZ −= dt × DirZ × 10  (0 on a sign change);  pawn.vt+0x16D0(DirZ)     // no write at all once DirZ == 0
      if +0x23C bit 3 && window: phase = 1;  DirZ = (|dzc| > 30) ? clamp(dzc × 5, −1200, 1200) : 0

tail: if +0x1EC bit 0: pawn.vt+0x140(target position …)          // unison only
```

Differences from the revision-1 6.3: the XY phase returns to 0 when the window closes; the "inside the stop distance" rate
is 0.5 also in air mode; bit 17 / bit 12 behaviours; the root scale has **no clamp** and is 0 when the target is exactly
above / below (`dxy == 0`, non-bit-16 kinds); phase 2 of the Z channel stops writing when `DirZ` reaches exactly 0;
`vz = vz0` (a re-write of the current Z speed) when the signs differ.

`TargetMetrics` lead (0x141330E25) [I]: `p += targetPawn.measuredVelocity × 8 × (1/30)` — the f64 constant at
0x14871E340 is 0.0333…, converted to f32; `measuredVelocity` = pawn vt+0xE28 = component+0x9A0 (location delta of the
target's last movement tick ÷ dt). So the decision inputs `dist` / `dz` / `DirZ` aim at where the target will be in
**0.2667 s**; the per-tick traction above uses the un-led lock-on position.

Slot 114 0x1413417F0(dt) [I]: `if +0x1F4 bit 5 (pitch facing): pawn.vt+0xDC8(dt × 60)`; `elif +0x21C != 3:
pawn.vt+0xDC8(dt × 12); pawn.vt+0xDB8(dt × 12)` — the same pitch / roll levelling toward zero the cast state does (magic
spec 1.3); vt+0xDC8 = `vt+0x21B0(zero rotator, alpha)`.

### 6.4 Root-motion scale [I]

- Path of the per-tick write: `FUN_1405A1D30(pawn+0x440, sXY, sZ, montage)` → mesh sub-object (+0x1118) vt+0x150
  0x1405C5370 → anim instance vt+0x428 **0x14056B420**:

```
montage = arg or instance+0x438 (the current one);  if none: false
mi = instance.vt+0x258(montage);  if !mi || !mi.Montage || !mi+0xA0 (playing) || !(mi+0x140 > 0): false
mi+0x174 = sXY;  mi+0x178 = sZ;  true
```

- Path of the start value: the `rootScale` pair of the play call (`PlayAttack` 0x14133AAB7: `(startScale, 1.0)`) is stored
  in the same two fields by the montage-instance play 0x143F4D3F0 (actions spec II.2).
- Use: montage advance 0x143F4EA20 @0x143F4EB47 multiplies the root translation extracted for the tick by
  `(s.x, s.x, s.y, 0)` — forward / side by `sXY`, up by `sZ` — before it goes to the movement component. Rotation is not
  scaled.
- **No clamp** anywhere on this path (neither in the traction channel nor in the setter nor at the multiplication). The
  only clamp is `PlayAttack`'s start scale of the slide kinds: `clamp(dist / range, 0.1, max)`; plain kinds start at the
  default (1, 1).
- Persistence: the pair belongs to the montage instance. It applies to every later tick's root delta until changed; the
  traction channel overwrites it each tick while its window is open and writes 1.0 when the window closes (kinds with
  +0x234 bit 15 / 16 keep their last value to the end of the clip); a new play call creates a new instance with its own
  start pair. A scale written in S(n) first acts on the root motion of M(n+1).
- The `group` argument of the play call is state+0x198 (values 0x106, 0x834, 0x898, 0xD48, 0x71E… are written for
  formchange kinds only; 0 in the base combo), the voice variant passed to vt+0x1F0 is state+0x19C; neither affects motion.

Effect for the Kingdom Key [D]: kinds with `root_len` (sheet `sora-combat-attacks`) scale their horizontal root travel to
`dxy / root_len` during the traction window, i.e. the clip's own step is stretched or shrunk so that the part of it played
inside the window would cover the clamped distance (≤ 800 cm) if the whole clip were played at that scale; with no target
the scale stays 1. Vertical root motion is never scaled by an attack.

### 6.5 Pawn–pawn collision while homing [A profile, I movement side, C solver]

1. **The capsule never blocks on another pawn** [A, `Config/DefaultEngine.ini` 1397 and 1346]: profile `TresPlayerPawn`
   (object type Pawn) has `Pawn`, `TresEnemyPawn`, `TresSpecialPawn` = Ignore, and `TresEnemyPawn` ignores `Pawn` in
   turn. The move sweeps of 6.1.3 pass through enemy capsules. (Agrees with movement spec 6.)
2. The encroach check (component slot 404 0x14057AE30, run by `PerformMovement` before the physics) [C] is a
   depenetration against **blocking** overlaps of the capsule only (`bComputeEncroachCheck`, pawn not attached, pawn
   collision on, `!m_bDisableObjTypeCollision`): pawns are not among them. Pawn slots 616 / 752 named in the pass-2 work list
   are not collision: slot 616 0x141448060 → 0x1405BA440 is `Draw` of the weapon (`(pawn+0x598).vt+0x4A8(b, immediate,
   rate)` + put-away timer 10.0) and slot 752 writes the stance value pawn+0x6E4 [I]; slot 731 is the skip flag of 6.1.1.
3. **Separation is a soft push between "push bodies"** of the body-collision components:
   - Sora's push body [A]: primitive #1 of `TresBodyColl0` in `p_ex001_PawnBase`: capsule radius 50, half height 90 at
     mesh (0, 0, 90), `m_bEnablePush`, no damage (9.10). Push bodies answer channel `TresBodyCollision` (0x18)
     (0x1406341C0) and count only while the owner has collision on, `!m_bDisableObjTypeCollision` and pawn+0x3CC == −1
     (0x140643E70).
   - Solver `UTresBodyCollComponent` 0x140637430(dt) [C]: push vector (comp+0x498) = 0; for each of my push bodies an
     overlap query lists the other push bodies (`m_OverlapList`); for each pair
     `push += w × penetrationVector(mine, other)` (0x144524AB0: the vector that separates the two shapes) with

     | my level vs other's | w |
     |---|---|
     | my level 9 (`PPL_MAX`), or I am in the "unpushable" state (component vt+0xC30 of my movement) | 0 |
     | lower than the other's | 1.0 |
     | equal | 0.5 |
     | higher | 0 |

     Level = comp+0x4A4 = the character's `m_BodyPushPower` (`TresChrBaseParamDataTable` +0x2C); **Sora: `PPL_50` (4)**
     [A `p_ex001_BaseParamData`], which is also the native default (0x14062F52E).
   - Movement side [I]: component slot 414 0x14057ABD0 (called from the movement tick, 0x1405958E5):
     `bodyColl.vt+0x718(owner)` (notifies each overlapped pawn pair through the pawn's notify interface slot 8);
     unless vt+0xC30(): `+0xBA8 = bodyColl push vector`; `if vt+0xC38(): +0xBA8.z = 0`; `+0xBB4 = +0xBA8 / dt`.
     Walking and Falling add `+0xBB4` as a velocity, Flying adds `+0xBA8` as a displacement (6.1.3); `PerformMovement`
     zeroes both after the physics (0x14058CEAF).
4. Consequence for an attack [D]: Sora neither stops at nor slides around an enemy capsule. While his push body
   (r 50) overlaps an enemy's, each tick moves him out by the full overlap (enemy heavier), half of it (same level; the
   enemy takes the other half in its own tick) or not at all (enemy lighter: Sora walks into it and the enemy is pushed
   away). The push is added to the same sweep as the homing velocity and the root motion, so world geometry still
   blocks it. What normally keeps Sora off the target during a swing is not collision but the traction rule: the homing
   speed target becomes 0 once the clamped distance to the **lock-on point** is ≤ `stop_dist` (160; 250 for the air-down
   kinds), and the root-scale kinds shrink their step to the distance.

Fourth pass (P3 H5) [C, the whole solver; weights are the immediates 0.5 / 1.0 / 0.0]: the penetration vector is the full
minimum translation: 0x144524AB0 hands both primitives' world transforms to 0x144524150 (SQEX_PhysXCollision.cpp: the PhysX
penetration query), which returns direction and depth; `push += direction × depth × w`, and the normalised direction is kept in
the pair's entry for the contact notification (slot 8, 7.10). Additions to the weight table: an **other** body whose owner is
unpushable (0x1405AD300) weighs 1.0 whatever the levels; an other that is not a character pawn weighs 0.5. A game-object flag
(0x141522B30 +0x84C) selects, for player pawns, a sequential variant in which each pair is measured with his transform
already moved by the push so far [?: who sets it]. Caller and phase: movement slot 414 from the movement tick, before the
physics, as given above. Enemy levels: 15 base-parameter tables are sheets (players, a few bosses); the enemies' own are
blueprint data that was not tabulated - not needed for a test level without enemies.

For the port: the host needs, per enemy, a push shape and a level (or a fixed "immovable / equal / light" class); the
default for an unknown enemy is "equal" (0.5). Without it Sora passes through enemy bodies, exactly as the capsule does.

### 6.6 Air attacks: mode timeline [I for each step, D for the composition]

Air-mode kinds (`air_mode` in sheet `sora-combat-attacks`: 8, 0x0D–0x1D, 0x1F, 0x20, 0x22; not 1–7, 9–0x0C, 0x21, 0x23):

| When | What | Where |
|---|---|---|
| S(N) `Begin` | `StopMovement` → V = 0. `PlayAttack` tail: state+0x234 \|= 4 \| 0x10; `GravityScale = 0.1` (vt+0x1650); gravity disabled (vt+0x1728(0)); `SetMovementMode(5 Flying, false)`; state+0x18C = 0.6; skip flag on | 0x14133A8BA–0x14133A8F7 |
| until the traction window | Flying, V = 0: Sora hangs and moves by the clip's root motion only (no vertical drift at all) | 6.1.3 |
| window open | XY homing (first attack of an air combo: target 800 × 1.5 = 1200 cm/s, rate 0.3; else 800, rate 0.2); vertical speed `dzc × z_gain` (3.0; 4.5 for kinds 0x14 / 0x15) re-computed every tick when the target is above / below by the sign rule, else the current Z speed (0) is re-written | 6.3 |
| window closes (Z channel phase 1 → 2) | gravity enabled, **Falling**; GravityScale still 0.1 → g = −98 cm/s²; XY speed decays ×0.9 per tick (×0.5 for the first air attack); while `DirZ != 0` the Z speed is re-written each tick with a value decaying ×(1 − 10·dt) → the fall cannot build up: Sora keeps hanging, sinking ≈ `½·g·dt²` per tick (0.014 cm) plus the decaying `DirZ` (total `DirZ × 0.1 s`). With `DirZ == 0` (no target, or target within ±30 cm of his height at `Begin` and never re-aimed) nothing writes Z: free fall at −98 cm/s² | 0x14134A902–0x14134A951, 0x14134A64F–0x14134A6A1 |
| `TresEnableChange` (slot 128, 5.2) | state+0x234 bit 4 cleared; GravityScale = class default (1.5 → g = −1470); gravity on; Falling. If the traction window is still open the Z channel keeps writing | 0x14134D52E–0x14134D590 |
| `End` | gravity on, GravityScale default, skip flag off; airborne → Falling with the velocity kept; on the floor → `SetMovementMode(Falling, bStop)` for an air-mode attack (it lands in the next movement tick) or Walking with V = 0 | 5.4 |

Landing and ledges [U/I by the movement spec, D here]:

- **Flying never lands** (no floor test in `PhysFlying`): an air-mode attack that descends onto the floor during its
  window slides along it and stays in Flying. From the moment the mode is Falling (window closed or `TresEnableChange`)
  a walkable hit lands normally (`ProcessLanded` → Walking, `Velocity.Z = 0`, movement spec 4.6); the Attack state has no
  landing branch — the clip plays on, now with Walking physics (homing XY and root motion on the floor), and the
  state's `bAir` (state+0x142) is not updated until the next attack's `PreDecide`.
- A **ground** attack whose root motion / homing carries Sora over an edge: `PhysWalking` loses the floor, the edge
  checks of movement spec 5.4 apply unchanged, then `StartFalling` → Falling with gravity (gravity is enabled for
  non-air-mode attacks, scale 1.5): he falls while the clip continues; `End` keeps Falling.
- A non-air-mode attack started in the air (ground kinds are never chosen with `bAir`; the forced counter kinds 0x21,
  0x23 can be): Falling with full gravity throughout.

### 6.7 Warps

Kind 0x22 (chase attack) and `+0x1E1`: slot 117 0x14134AA80 after a 0.2 s timer teleports to a point
150 cm / 100 cm from the target (slot 136 0x141337320: random angle ±90° unless special, sweep
checks) [C]. Flash Step / Quick Shaft are ordinary root-motion clips (680 / 360 cm). Slot 117 [C]: `+0x258 -= dt`; at 0: for `warpStart`
the teleport (slot 136 with 150 / 100), then `iface40.vt+0x1A0(pawn+0x8E0, +0x22C + 1, 0)` and `PlayAttack()`; while the
delay runs `Tick` does not test the chain conditions (5.2).

---

## 7. Targeting and lock-on

Everything here is the controller (`TresPlayerControllerSora`, vtable 0x146969B98; "slot n" below is a controller slot
unless another object is named). One function does the search, the soft target, the manual lock and the switching:
slot 714 `UpdateLockon(dt)` 0x141404E50 (8493 bytes, verdict `same-partial`; **read completely in the disassembly**,
`build/sora_tmp/combat/dis/141404E50.dis`). Function identities of the callees are by address; their verdicts are in the
table at the end.

### 7.1 State on the controller [I]

| Field | Type | Init (0x1414162A0) | Accessors | Meaning |
|---|---|---|---|---|
| +0x934 / +0x938 | u8 / f32 | 0 / 0 | slot 711 get, slot 712 update | RB tap flag (one frame) / RB hold time |
| +0x93C, +0x93D, +0x93E / +0x940 | u8 ×3 / f32 | 0 | slots 727 (+0x93C), 728 (+0x93E), 713 update | RT hold flag, RT hold latch, RT tap flag (one frame) / RT hold time |
| +0x944 (weak ptr) | `TresCharPawnBase` | null | — | pawn of the locked target (null for a soft target; see 7.6) |
| +0x94C (weak ptr) | `TresCharPawnBase` | null | slot 732 clear | pawn of the attack target |
| +0x95C | i32 | 0 | slot 726 | number of targets within 400 cm (the "crowd" input of the combo decision); slot 726 returns `min(count, 1)` when +0x96C bit 1 (`BP_SetNearTargetSingle`) |
| +0x960 | u8 | 0 | slot 720 | **manual lock active** |
| +0x964 | i32 | 0 | — | index of the last switch in the candidate list |
| +0x968 | u8 | 0 | slot 729 | shooter-mode aiming active (RT hold; keyblade forms 0x80, 0x83, 0x8E, 0x8F only; not the Kingdom Key) |
| +0x969 | u8 | 0 | setter 0x141413F70 | dive-fall targeting (candidates must be below Sora, 7.4) |
| +0x970 | f32 | 900 | slots 735 / 736, reset 738 0x141410FD0 | `MyAutoLockonLimit` (pawn+0xC88): range of the soft target |
| +0x974 | f32 | 150 | slot 737 | `MyAutoLockonNear` (pawn+0xC8C): floor of the range-adjusted distance |
| +0x978 | f32 | 3500 | slots 739 / 740, reset 741 0x1414111D0 | `MyManualLockonLimit` (pawn+0xC90): range in which RB locks |
| +0x97C | f32 | 4200 | slot 739 | `MyManualLockonOff` (pawn+0xC94): a lock is dropped beyond it |
| +0x980 / +0x984 / +0x988 | f32 | 5000 / 15000 / 3750 | slots 742–750 | shotlock limit, athletic-flow limit and range (other specs) |
| +0x98D, +0x991 | u8 | 1, 1 | slots 756, 757 set; slot 755 = both | manual lock permitted (`BP_SetDisableManualLockon(b)` → slot 782 → slot 756(!b)) |
| +0x98E | u8 | 0 | slot 758 get, 759 0x141413970 set | auto lock-on disabled (`BP_SetDisableAutoLockon`) |
| +0x98F | u8 | 0 | slot 760 get, 761 0x141413C20 set | gimmick lock-on disabled (`BP_SetDisableGimmickLockon`): owners tagged "Gimmick" are skipped |
| +0x990 | u8 | 0 | slot 762 get, 763 set | "soft target only" mode: no markers, no manual lock (7.5 step 3) |
| +0x994 (weak ptr) | `TresLockonTargetComponent` | null | slot 722 0x1413FED90; slot 721 0x1413FEE00 = its owner as `TresCharPawnBase` | **current target** (soft or locked) |
| +0x99C (weak ptr) | component | null | slot 731 0x1414130A0 set; slot 734 get | **attack target**: the target of the last accepted attack request; pins the current target (7.6) |
| +0x9AC (weak ptr) | component | null | slot 723 0x1413FFC90 | best candidate of this frame's search (within 3500) |
| +0x9B4, +0x9B5 | u8 | 0 | set 0x14141A810, cleared 0x1413FB730 | shotlock aim search active: slot 718 0x141407920 then replaces the whole function |
| +0x9C4 | u8 | 0 | set by slot 790 0x141419C00, cleared by slot 791 0x1413FA990 | multi-lock mode (`m_CameraMultiLock`, +0x9D8); only reachable in forms 0x80 / 0x8E |
| +0x9C8 / +0x9CC | f32 / i32 | 2400 / 6 | — | range / maximum length of the switch list |
| +0x9D0 | f32 | 0 | — | timer that clears the markers of list 2 (+0x9F8) |
| +0x9E8 (TArray, 20-byte entries) | `{f32 dist; weak ptr (unused, null); weak ptr component}` | empty | — | switch list: candidates within 2400, rebuilt every frame while locked |
| +0xA10 | i32 | — | written by slot 800 0x1414165E0 | shotlock state; != 0 freezes the target update (7.5 step 1) |
| +0x878 bit 6 | flag | cleared | 0x14140E610 (caller in the UI module, 0x141615760) | **input lock** set by the UI (also zeroes the stick and the RB state in the input update, 0x1413F237D); freezes the target update. *Not* "manual lock active" as revision 1 said |

Pawn constructor 0x141423210 [I]: `MyAutoLockonLimit` 900, `MyAutoLockonNear` 150, `MyManualLockonLimit` 3500,
`MyManualLockonOff` 4200, `MyShootLockonLimit` 5000, `MyAthleticLockonLimit` 15000, `MyAthleticLockonRange` 3750,
`MyDiveFallLockonRangeXY` 1000, `MyDiveFallLockonLimitZ` 3500. Neither `p_ex001_PawnBase` nor `p_ex001_Pawn` overrides
any of them [A]. The controller's init writes the same numbers as constants; the reset slots copy the pawn's values.
+0x9C8 (2400) and +0x9CC (6) have no reflected source.

### 7.2 The two buttons [I]

`sheets/port/sora-input-layout.json`: lock-on = RB (`Gamepad_RightShoulder`, keyboard R), target change = RT
(`Gamepad_RightTrigger`, keyboard E). Both are **taps**: the flag is raised on the frame of the release when the button
was down for no more than 0.26666668 s (16 frames at 60 Hz).

```
RB: slot 712 (held, dt)  0x1413F0700                      // called every frame from the input update (0x1413F241F)
    tap(+0x934) = 0
    if held: t(+0x938) += dt
    else if t > 0: tap = (t <= 0.26666668); t = 0
RT: slot 713 (held, dt)  0x1413F0D00                      // called only when pawn.vt+0xD70() == 1 (pawn+0x5F0)
    if ctrl+0xBB8 || +0xC08 || +0xC09 || +0xC18: return   // command-menu / shortcut states: nothing changes
    tap(+0x93E) = 0
    if held: t(+0x940) += dt; if t > 0.26666668 && latch(+0x93D) == 0: hold(+0x93C) = 1, latch = 1
    else:    if t > 0: tap = (t <= 0.26666668);  t = 0; hold = latch = 0
```

RB held is forced to 0 by the input update in states 95, 96, 103, 144, 145, 150 and under the input lock
(0x1413F23B0–0x1413F240F). A hold of RB longer than the tap time is the shotlock (slot 800, magic spec).

### 7.3 Frame order [I] (input update 0x1413F1AB0, part C of the frame)

```
0x1413F241F  slot 712 (RB tap timer)
0x1413F2443  slot 800 (shotlock), 0x1413F2459 slot 713 (RT tap timer)        // both only with pawn+0x5F0 == 1
0x1413F2468  slot 714 UpdateLockon(dt)                                       // this section
0x1413F2477  slot 717 0x1414095E0 (dt)                                       // per-frame target bookkeeping, not read [?]
0x1413F2483  slot 721, 0x1413F248F slot 722 → the current target; the command executor reads slot 722 again in phase U of the same frame for TryAttack / TryMagic (3.1.1)
... attack / guard / jump requests (section 3.1)
```

So an attack requested on the frame an enemy comes into range already homes on it, and the lock state seen by the
states (S) is this frame's.

### 7.4 Search loop [I] (0x141404E50, 0x141405073–0x1414059BD)

Early outs before anything is touched: slot 718 returns true (shotlock aim search, +0x9B4); the controller's world is
invalid; `UTresGameInstance`+0x84C != 0 (slot 643 0x140F27C60) [? which mode]; the pawn is not a valid
`TresPlayerPawnBase`; the game mode (target registry) or the HUD object is missing.

```
team     = pawn.m_TeamNo (pawn+0x514)                               // pawn slot 434
origin   = pawn.slot1058()                                          // 0x141448550: root location + (0, 0, 100)
                                                                    //   (ride states: the accompany pawn's position)
fwd      = Vector(Rotator(pawn root ComponentToWorld rotation)); fwd.z = 0; normalise XY if |fwd.xy|² > 1e-8
origin  += fwd * 30                                                 // searchDir = Sora's facing. Not the stick, not the camera.
bestScore = bestDist = ctrl+0x978 + 5000;  best = none;  bestPri = 0;  count(+0x95C) = 0
listMode = ctrl+0x9C4 || locked(+0x960) || ctrl+0x968
if listMode: old = copy of the switch list; switch list emptied

for tc in registry (game mode vt+0x9D0: array of weak pointers, registration order):
    skip if !tc.Targetable()            // 7.9
    skip if tc.Team() == team
    skip if ctrl+0x98F && tc.owner has tag "Gimmick"
    p  = tc.Location()                  // comp vt+0xB88 = component world location
    d  = p - origin;   d3 = |d|
    if d3 < 400: count++                                            // crowd count: raw 3D distance, before every later test
    dist = d3
    if tc.m_AutoLockOnRange > 0:                                    // comp vt+0xAF0 = comp+0x888
        dist = d3 - (tc.m_AutoLockOnRange - pawn.MyAutoLockonLimit) // big targets advertise a longer range than 900
        if dist < ctrl+0x974 (150): dist = d3 * 0.01 + ctrl+0x974
    if !(tc.owner has tag "Enemy") && dist > ctrl+0x970 * 0.5 (450): continue
    pri = tc.m_LockonPriority (u8, LOW 0 / HIGH 1)
    if pri < bestPri: continue
    if ctrl+0x969 (dive fall):
        if |d.xy| > pawn.MyDiveFallLockonRangeXY (1000) || d.z > 0 || d.z < -pawn.MyDiveFallLockonLimitZ (3500): continue
        factor = 1.0
    else:
        dir = (|d.xy|² < 1e-8) ? (0,0,0) : normalize2D(d)           // z = 0
        factor = 1.8 - dot(fwd, dir)                                // 0.8 dead ahead, 1.8 abeam, 2.8 behind
    score = factor * dist
    take = score < bestScore || (pri > bestPri && dist < ctrl+0x970 (900))
    if take:
        if !LineOfSight(tc): continue                               // 7.4.1; a blocked candidate is not listed either
        best = tc; bestScore = score; bestDist = dist; bestPri = pri
    if listMode && dist < ctrl+0x9C8 (2400): switch list += {dist, null, tc}
          // candidates that lost on score are listed without a line-of-sight test

candidate(+0x9AC) = (bestDist < ctrl+0x978 (3500)) ? best : null
if attackTarget(+0x99C) is valid && !attackTarget.Targetable(): attackTarget = null; attackPawn(+0x94C) = null
```

Comparison senses as coded: `d3 < 400` (0x1414056A0 `jae` skips), `dist < 150` (`jae` skips), `dist > 450` rejects
(`ja`), `pri < bestPri` rejects (`jb`), `score < bestScore` (`jb` takes), `dist < 900` for the priority override
(`jae` rejects), `dist < 2400` for the list (`jae` rejects), `bestDist < 3500` (`cmovb`).

Consequences [D]: a target directly behind Sora at distance x scores like one dead ahead at 3.5 x; a HIGH-priority
target within 900 cm beats every LOW one regardless of angle; once a HIGH one has been taken, LOW ones are ignored for
the rest of the pass (so the result depends on registration order only among equal scores). There is no camera or
screen-space term anywhere in the choice.

#### 7.4.1 Line of sight [I for the constants, C for the control flow] (0x1413F0750, `no-verdict`)

```
from = pawn.slot1058()                       // root + 100 up, without the 30 cm push
to   = tc.Location()
params: trace tag "Lockon", simple collision, ignore Sora and the target's owner with all its attach parents
responses: everything Ignore, then WorldStatic (0), WorldDynamic (1), TresGimmick (18) = Block
visible = !LineTraceTestByChannel(world, from, to, channel 15 (ECC_GameTraceChannel2 = "TresProjectile"), params, responses)
```

Pawns never block the line (object types 2 and the Tres pawn channels are ignored): an enemy standing in front of
another does not hide it. Channel names: `DefaultEngine.ini` lines 590–606 [A].

### 7.5 After the loop: which branch runs [I] (0x141405B6C–0x1414060DA)

In this order; `cur` = the component in +0x994 at entry, `lockPawn` = the pawn in +0x944 at entry; every path that does
not return early ends with `+0x944 = lockPawn; +0x994 = cur`.

1. `ctrl+0xA10 != 0` (shotlock aiming): return. Target and markers stay as they are.
2. `ctrl+0x878 bit 6` (input lock): return (in shooter mode slot 715 runs first).
3. `ctrl+0x990` (soft-only mode): `if attackTarget invalid: cur = best`; `if cur valid && bestDist > 900: cur = null,
   attackTarget = attackPawn = null, lockPawn = null`; write; return. No markers, no lock.
4. Timer +0x9D0 (if running): at 0 the markers of list 2 are removed and the list is emptied.
5. Shooter mode (+0x968): slot 715 0x141406F80 (not base Sora); return.
6. RT held past the tap time (slot 727) and the pawn's request interface slot 191 0x141447640 returns a ShooterMode state
   (needs form kind 0x80 / 0x83 / 0x8E / 0x8F with a full gauge, 0x14142D9D0 [C]): enter shooter mode, drop the lock.
   **Never with the Kingdom Key**: with it a long RT press does nothing here.
7. `ctrl+0x98E` (auto lock-on disabled): `if locked: Unlock(cur)`; `cur`'s marker removed, `cur = null`; slot 791(1);
   write; return. No soft target, no lock.
8. `locked (+0x960)`: 7.7.
9. `ctrl+0x9C4` (multi-lock mode): every listed candidate gets a type-0 marker; RB tap locks the nearest (list[0] after
   the sort) and leaves the mode (slot 791(1)). Not reachable with the Kingdom Key.
10. Otherwise: the normal path, 7.6.

### 7.6 Normal path: soft target, pinning, locking on [I] (0x141406BEF–0x141406ED7)

```
if !valid(attackTarget(+0x99C)) && cur != best:
    if valid(cur): Hud.Remove(cur)
    cur = best                                                      // may be null
if valid(cur):
    if ManualLockPermitted() (slot 755) && RBtap (slot 711) && bestDist < ctrl+0x978 (3500):         // LOCK ON
        locked(+0x960) = 1
        Hud.Remove(cur); Hud.Add(cur, type 1 (lock cursor))
        Camera.SetLockon(cur)                                       // 7.12
        +0x944 = lockPawn (unchanged; it is refreshed next frame by 7.7); switchIndex(+0x964) = 0; +0x994 = cur
        return
    if bestDist > ctrl+0x970 (900):                                 // nothing in soft range
        Hud.Remove(cur); cur = null; attackTarget = attackPawn = null; lockPawn = null
    else:
        Hud.Add(cur, type 0 (auto cursor))                          // idempotent: Add returns early when the marker exists
+0x944 = lockPawn;  +0x994 = cur
```

Reading it as behaviour:

- **Soft target** = the best-scored candidate, shown with the auto cursor, as long as the best candidate's adjusted
  distance is ≤ 900 cm. It is re-evaluated every frame from Sora's facing: turning the character re-targets, turning the
  camera does not.
- **Pinning (the only hysteresis)**: `TryAttack` stores its target with slot 731 (+0x99C; 0x1414373B3). While that
  component is alive and targetable, `cur` is **not** replaced by a better-scored candidate. The pin is released only
  when the target stops being targetable, when no candidate at all is within 900 (`bestDist > 900`: note that this is
  the *best* candidate's distance, not the pinned one's), or by an unlock. The attack state's `End` does not clear it.
  So a combo stays on its victim, and so does the next combo, until the area is empty or the victim dies.
- **No target**: `cur = null`; `TryAttack` then gets a null component and the attack decides with `dist = dz = 0`
  (section 4.1) and keeps the actor yaw.
- **Locking on** takes the current soft target if there is one, otherwise the best candidate up to 3500 cm (this frame's
  `cur = best` assignment makes them the same unless a pin is active — with a pin the pinned target is locked).
  Nothing within 3500: the tap does nothing (`cur` is then null, or the pin survives without lock).
- There is no timer anywhere in the soft-target logic: it switches on the frame the scores cross.

### 7.7 While locked [I] (0x1414060DA–0x1414067B2)

```
keep = valid(cur) && ManualLockPermitted() && cur.Targetable() && cur.slot340() (= Targetable again in this class)
       && cur.Team() != team
if keep:
    d = |origin - cur.Location()|                                   // origin of 7.4 (with the 30 cm push)
    if cur.m_AutoLockOnRange > 0: d = max(d - cur.m_AutoLockOnRange, ctrl+0x974 (150))    // no own-range term here
    keep = !RBtap && d <= ctrl+0x97C (4200)
if !keep:
    Unlock(cur)                                                     // 7.8
    if !ctrl+0xBB8 && !+0xC08 && !+0xC09 && !+0xC18 && form kind (+0xB38) ∈ {0x80, 0x8E}: slot 790 (multi-lock)   // not Kingdom Key
    return
if !pawn.slot902() (0x141436BB0: the diving state, the one that selects "DiveIdle") && RTtap (slot 728) && list.num > 0:   // SWITCH
    sort list ascending by dist (0x1413E60E0); if list.num > ctrl+0x9CC (6): truncate to 6 (0x1414156A0)
    repeat at most list.num times: switchIndex++; if switchIndex >= list.num: switchIndex = 0; stop at a valid entry
    if list[switchIndex] != cur:                                    // 0x140F1D6E0
        +0x994 = list[switchIndex]
        Hud.Remove(cur); Hud.Add(new, type 1); Camera.SetLockon(new)
        if pawn+0x15B4 == 11: mark the tutorial "Tutorial_ManualLockon_TargetChange" done (0x140E611B0)
t = the locked target after the switch (cur when nothing changed)
+0x944 = weak(t.owner if it is a TresCharPawnBase else null);  +0x994 = t
return
```

- **RB is a toggle**: a tap while locked unlocks (no hold-to-lock mode exists in this function).
- **Loss**: target dead / lock permission withdrawn / team changed, manual lock disabled by script, or farther than
  4200 cm (range-adjusted). After a loss the next frame runs the normal path: a soft target reappears if something is
  within 900. There is no automatic re-lock onto another enemy (base forms).
- **Switching** cycles through the candidates within 2400 cm of Sora (range-adjusted distance from the 7.4 origin),
  **nearest first, at most 6**, in ascending distance, by a running index — not by stick direction and not by screen
  position. The list is rebuilt and re-sorted every frame, so the index is positional: the order follows the current
  distances, and the locked target itself is in the list (when the index lands on it nothing happens for that tap).
  A candidate enters the list without a line-of-sight test unless it was the best-scored one (7.4).
- While locked the soft-target logic does not run at all; the lock ignores the 900 limit, priority and facing.

### 7.8 Unlock and the other entry points

`Unlock(cur)` = slot 752 0x1413FA580 [C, `differs`; writes confirmed by the field scan]: removes the markers of `cur`, of
+0x994 and of every entry of both lists, empties the lists, `Camera.SetLockon(null)`, +0x994 = +0x99C = +0x94C =
+0x944 = null, +0x960 = 0, +0x964 = 0. Note that it also drops the soft target and the pin; they are rebuilt next frame.

Other writers of the lock (not driven by RB) [C]:

| Function | What |
|---|---|
| slot 753 0x1414110A0 `(comp)` | forced lock: +0x960 = 1, lock cursor, camera, +0x994 = comp, +0x944 = its pawn, +0x964 = 0. Used by the attack-camera code 0x1414122A0 to restore the lock |
| slot 751 0x141415020 `(comp)` | lock onto a given component, replacing attack target and current target first |
| slot 754 0x1413ECEA0 | another forced lock (830 bytes, not read [?]) |
| slot 787 0x1413EA240 `BP_EndManualLockon` | `if locked: Unlock(null)` [I] |
| 0x1413FB4A0 | clears +0x968, +0x98C and the lock (leaving shooter mode) |
| slot 791 0x1413FA990 `(b)` | only in multi-lock mode: clears markers and lists; with `b`: +0x9C4 = 0, +0x9CC = 6, +0x9D0 = 0, camera unlock, `m_CameraMultiLock` ended with a 0.5 s blend |

### 7.9 `TresLockonTargetComponent` [I] (vtable 0x146628970, size 0x8C0, super `TresGimmickComponentBase`)

A scene component; an actor may carry several. Registered with the game mode on BeginPlay (slot 93 0x140F960B0 →
`ATresGameMode` vt+0xA78) and removed on EndPlay (slot 96 0x140F9D100 → vt+0xA80); the search reads the array through
game mode vt+0x9D0 (the pawn caches the game mode, pawn slot 284 0x1405CF480).

| Slot (vt offset) | Address | Returns / does |
|---|---|---|
| 336 (+0xA80) `Targetable` | 0x140F96570 | `comp+0x868 bit 0` (component enabled, base class) `&& m_bPermissionLockOn (comp+0x870 bit 0) && owner (comp+0x1B8) valid && owner.vt+0x698()` [? the owner-side test: alive / lock-on enabled, `TresPawnBase::IsEnableLockOn`] |
| 337 (+0xA88) | 0x140F5A7C0 | set `m_bPermissionLockOn` |
| 338 / 339 (+0xA90 / +0xA98) | 0x140846E10 / 0x140F5A300 | get / set `m_LockonPriority` (comp+0x874) |
| 340 (+0xAA0) | 0x140AD0F70 | jumps to slot 336 |
| 342 / 344 / 356 | 0x140F51A50 / 0x140F51A30 / 0x140F51A10 | shoot-flow / move-flow / cover-shot permission (comp+0x878 bits 0, 1; comp+0x8A0 bit 0), each with the enable bit |
| 350 / 351 (+0xAF0 / +0xAF8) | 0x140F50950 / 0x140F50C30 | `m_AutoLockOnRange` (comp+0x888) / `m_ShootLockOnRange` (comp+0x88C); setters clamp at 0 |
| 354 (+0xB10) | 0x140F28770 | `m_bRushSuppression` (comp+0x898 bit 0) |
| 355 (+0xB18) | 0x140F27A90 | `m_CloseRange` (comp+0x89C): read by the athletic-flow state 0x1413321B0 / 0x141348BA0 and 0x14141F210, **not by the melee attack state** |
| 362 (+0xB50) `LookAtPosition` | 0x140F95CA0 | `owner && m_ViewTargetIndex (comp+0x884) >= 0 ? owner.vt+0x710(index) : Location()`: the camera's aim point |
| 363 / 364 | 0x140F50CF0 / 0x140F5AE20 | get / set `m_ViewTargetIndex` |
| 369 (+0xB88) `Location` | 0x140ACF250 | component world location (ComponentToWorld translation, comp+0x2B0): **the point searched, scored and homed on** |
| team | 0x140F75F30 | `comp+0x868 bit 1 ? (owner is a TresPawnBase: its m_TeamNo; a TresGimmickActor: vt+0x838) : comp+0x86C` |

Reflected properties (registration 0x14270C1B0): `m_bPermissionLockOn` +0x870, `m_LockonPriority` +0x874
(`ETresLockonPriority`), `m_bPermissionShootFlow` / `m_bPermissionMoveFlow` +0x878, `m_MoveFlowType` +0x87C,
`m_bMoveFlowDestination` +0x880, `m_ViewTargetIndex` +0x884, `m_AutoLockOnRange` +0x888, `m_ShootLockOnRange` +0x88C,
`m_LookAtTargetBoneName` +0x890, `m_bRushSuppression` +0x898, `m_CloseRange` +0x89C, `m_bPermissionCoverShot` +0x8A0,
delegate `OnTargetLock` +0x8A8. Constructor defaults and per-enemy values were not dumped [?]; the component's world
location comes from its attachment (pawn property `MyLockOnSetBoneName`, pawn+0x4B0).

`m_AutoLockOnRange` is **not a radius**: it is the range at which this target may be auto-targeted, and it shortens the
measured distance by `range − 900` (7.4) or by `range` (7.7). Revision 1's `radius()` was this field.

### 7.10 What an attack uses [I]

- `TryAttack(target, type)` is called with slot 722 (the current target of this frame) and records it with slot 731
  (+0x99C, +0x94C): the pin of 7.6.
- `Begin` falls back to slot 722 when the passed component is gone; slot 130 drops a target that stops being targetable;
  with state+0x234 bit 22 it re-acquires slot 723 (the best candidate, up to 3500 cm).
- The point used by `TargetMetrics`, facing and traction is `Location()` (vt+0xB88), not the camera's look-at position.
- **Rush suppression**: 0x141343160, a handler in the vtable of the state's second interface sub-object (state+0x48;
  receives a pawn): when that pawn is the owner of the attack's target component, `state+0x234 bit 15 =
  target.m_bRushSuppression`. Bit 15 is the "0.1 root scale" case of the traction's root-scale channel (6.3) and the
  debug line "Rush Suppression.". Caller (fourth pass) [I call site 0x140634100]: the body-collision component's contact
  dispatcher 0x140633EE0, which tells every pair of its overlap list (6.5) to the pawn through the notify interface's slot 8
  with (direction, the other pawn, the component). So the bit follows the target's flag **while Sora's push body touches the
  target's**; without contact it keeps its last value for the rest of the attack. Host: `Targets::rush_suppression`.
- The crowd count for the combo decision is +0x95C (7.4): targets of other teams within 400 cm of the search origin,
  line of sight and priority ignored.

### 7.11 HUD calls [C]

`hud = ATresHUD + 0x4F8` (0x141401180); its lock-on part is the object at hud+0x68.

| Call | When | Effect |
|---|---|---|
| `Add(target, 0)` 0x1415759C0 → 0x141668C60 | every frame for the soft target (7.6); for every listed candidate in multi-lock mode | auto cursor (type 0). Returns at once when a marker for this component exists |
| `Add(target, 1)` | lock on, switch, forced locks | lock cursor (type 1); UI sound id 7 (0x14176F6B0) |
| `Remove(target)` 0x14157A7A0 (type argument 8, id −1) → 0x14166C730 | target change, loss, unlock, before every `Add(…, 1)` | removes that component's cursors |
| inside `Add` for types 0 and 1 | first marker of a component | `0x14158A860(hud+0x1C8, owner as TresCharPawnBase, 0, −1)`: registers the **enemy HP gauge** target. So the gauge shows the soft target too, not only a locked one |

Cursor types of the same function: 2 / 3 shotlock marks (random offset), 4 (sound 0x13), 6, 7; the widgets are
`HudTarget` of `docs/sora-ui-spec.md` section 7. The controller's "is locked" getter is slot 720; the target pawn for
other code is slot 721.

### 7.12 Lock-on camera

This subsection supersedes the lock-on stub of `docs/sora-camera-spec.md` section 5 (first line of its table); that file
is not edited here (`docs/sora-spec-reconciliation-2.md` has the sentence for its owner). Symbols of the free camera
(`rotTarget`, `autoPower`, `stickWait`, `rotLerpStep`, look-point collision) are those of the camera spec.

**Entry** [C + key constants I]. `Camera.SetLockon(comp)` = `0x1405F0140(manager, comp)`: with a valid `comp` and no
`m_LockonCamera` (manager+0x28B8) the manager picks the top camera of its stack that supports lock-on (camera slot 250:
`m_CameraLocClass` (camera+0x410) is a `TresCameraBase` class) — in the field the normal camera — and calls its slot 251
`SetLockonTarget(comp, blend)`; `m_LockonTarget` (manager+0x28C0) = comp. With null: slot 251 (null), both manager
fields cleared.

`TresCameraNormal` slot 251 0x14060AB70: stores `m_LockonTargetCmp` (camera+0x408); ends an existing `m_CameraLoc` with
the transition {time 0.5 s, function 3, exponent 2.0}; with a target it spawns an actor of `m_CameraLocClass` — native
`TresCameraNormalLoc` (vtable 0x145D5D3E0, constructor 0x1405FC7E0, size 0xAD0, super `TresCameraNormal`, no reflected
properties; `BP_TresCameraNormal_ex001` does not override the class [A]) — gives it the same followed actor and
controller, copies the parameters below (0x14060A930), and starts it with the same 0.5 s transition (ease exponent 2).
So locking and unlocking are **0.5 s blends between two camera actors**; the normal camera keeps existing underneath
and the lock-on camera is destroyed on unlock or on a switch of target (a switch spawns a fresh one: 0.5 s blend from
the old lock-on camera to the new).

**Parameters** copied from the normal camera into the lock-on camera [I for the defaults, A for the blueprint]:

| Lock-on camera field | Source on the normal camera | Value for Sora |
|---|---|---|
| `m_Distance` (+0x800), also the minimum of the framing distance | the normal camera class default's `m_Distance` (0x1405EC6A0) | 400 |
| FOV, target info (socket `Zup_Cam`, offset (0,0,48)), collision switch | same as normal | 100°, as in camera spec 2.3.1 |
| `m_Rotation` (+0x828) = offset added to the look-at rotation | `m_LockonRotation` (+0x928) | (pitch −20, 0, 0) (ctor 0x1405FC450) |
| pitch limits (+0x880 / +0x884) | `m_LockonLimitPitchMin` / `Max` (+0x910 / +0x914) | −70 / +20 |
| yaw limit (+0x888), relative to the look-at yaw | `m_LockonLimitYawMax` (+0x918) | 120 |
| stick-speed range (+0xA24 / +0xA28 / +0xA2C) | `m_RotationSpeedMaxDistance` / `MinDistance` / `MaxRate` (+0x91C / +0x920 / +0x924) | 2500 / 500 / 0.45 (blueprint; native 1000 / 200 / 0.5) |
| `m_bMoveRotation` (flag 0x80 of +0x8F0) | `m_bLockonMoveRotation` (+0x934) | true |
| `m_TargetMaxSpeed`, lerp rates (+0x430..+0x43C, +0x44C, +0x450) | same fields | 800, as normal |
| target offset Z (+0x680) | `m_LockonTargetOffsetZ` (+0x944) when `m_bLockonTargetOffset` (+0x940 bit 1) | not used (flag clear) |
| read from the parent each frame | `m_LockonAutoRotationLowDist` / `HighDist` (+0x938 / +0x93C), `m_bLockonAutoRotationDist3` (+0x940 bit 0), parent flag 0x01 of +0x8F0, parent +0x948 bit 0 | 300 / 3000 (blueprint; native 0 / 0), false, clear, set |

Own constructor values of `TresCameraNormalLoc` [I]: distance smoothing rate (+0x448) **8.0** (normal camera 4.0),
start distances +0x9E0 = +0x9E4 = 400, `m_Rotation.pitch` −15 and limits −70 / 20 / 80 before the copy overwrites them.

**Per frame**: `CalcCamera` (slot 179 0x1406023F0) runs `0x14060EDB0(this, dt)` instead of the normal update
(0x1406033C0): a rotation step 0x140611170, then a position step 0x1406100D0.

Rotation step [C; constants I]:

```
stick: read as in the normal camera (camera spec 2.5: dead zones, sensitivity, 120 °/s), then scaled (slot 266 0x140607390):
    d  = |playerPoint - target.LookAtPosition()|                     // playerPoint = slot 265, the Zup_Cam target point
    hi = max(+0xA24, +0xA28) = 2500; lo = +0xA28 = 500
    scale = hi > lo ? ((hi - lo) - (clamp(d, lo, hi) - lo)) / (hi - lo) * (1 - 0.45) + 0.45 : 0.45
                                                                     // 1.0 within 500 cm of the target, 0.45 beyond 2500
rotTarget += stickDelta * scale;  normalise;  flag 0x02 = stick active
followed = AutoFollow(rotTarget, base, dt, stickActive, -0.8)        // the normal camera's 0x1405FFA60 with the changes below
if parent+0x948 bit 0 && !stickActive && stickWait (+0x8E4) <= 0:
    s = project(target.LookAtPosition()) with the view-projection saved from the local player
    if |s.x| > 1 || |s.y| > 1 || s.w <= 0:  rotTarget = base         // target left the screen: face it at once
rotTarget = Clamp(rotTarget)                                         // 0x140611760 [C]: with L = LookAt(a point above Sora's root → the
                                                                     //   midpoint between that point and the target):
                                                                     //   pitch into [m_LimitPitchMin (-70), L.pitch >= 0 ? min(L.pitch, m_LimitPitchMax (20)) : 0],
                                                                     //   yaw into L.yaw ± m_LimitYawMax (120) (skipped when the limit is 0 or collision is off)
rotCur = rotLerpStep(component rotation, rotTarget, m_fRotationLerpPer (+0x454 = 8), dt)      // camera spec 2.2

base (slot 299 0x140606190, verdict differs):
    look = LookAt(playerPoint → target.LookAtPosition())             // slot 312 0x140613300:
           pitch = atan2(dz, |dxy|) in degrees, yaw = atan2(dy, dx) in degrees + 0.08;
           if |dxy|² < 1: pitch = 0, yaw = the followed actor's yaw
    if m_bWorldBaseRotation: base = (m_Rotation.pitch, look.yaw, roll)                       // not Sora
    elif look.pitch >= -10 || (+0xA70 bit 1): base = look + m_Rotation (pitch -20);  if !(+0xA70 bit 1) && base.pitch < -10: base.pitch = -10
    else: base = look                                                // target more than 10° below: look straight at it
```

`AutoFollow` in lock-on [I] (0x1405FFA60, the `vt+0x7E8` branch at 0x1405FFC8E–0x1405FFEBA and 0x14060018F–0x1406001B8):
the velocity `v` that drives the follow rate (camera spec 2.6: `rate = |v.xy| / m_TargetMaxSpeed × autoPower`, `v = 0`
in the Idle state) is modified first:

```
if parent flag 0x01 (+0x8F0 bit 0; setter 0x1406058B0, default clear): v = (m_TargetMaxSpeed, 0, 0); rate *= 6
elif High (3000) > 0 && Low (300) > 0 && High > Low:
    d = |target.LookAtPosition() - playerPoint| in XY (3D with m_bLockonAutoRotationDist3)
    k = d < Low ? 0 : d > High ? 1 : (d - Low) / (High - Low)
    v = v + ((m_TargetMaxSpeed, 0, 0) - v) * k
    rate = rate * (1 + 5 k)
```

So within 300 cm of the target the lock-on camera turns toward `base` only as fast as the normal camera follows a
moving Sora (and not at all while he stands in Idle); the farther the target, the more it turns by itself, up to
`autoPower (≤ 1.2) × 6 = 7.2 /s` at 3000 cm. The 1.5 s stick wait, the `autoPower` ramp (1.2 /s) and the "speed > 1"
test of the normal camera all still apply — with the blended `v`. The yaw of `base` now points **at the target**, not
behind Sora, and the "running toward the camera" exception of 2.6 compares against it unchanged.

Position step [C; constants 0.24, 3000, 6.0, 2.0, 0.5, 0.6 read in the disassembly; solver, lead and obstruction blend read in the third pass, below]:

```
P = playerPoint (+ the camera position effect offset), T = target.LookAtPosition() (the last known one, +0xA74, when the target is gone)
look-point collision as in the normal camera (camera spec 3.1)
lead: P and T are each shifted along their owner's measured velocity by 0.24 × (length of their clamped screen position),
      only when that moves them away from each other on screen; smoothed with rate +0x450
if T jumped more than 3000 cm/s × dt since last frame: T is approached with rate 6.0 instead
minDist = manager+0x2918 (SetLockonNearLimit) if > 0, else m_Distance (400) + distance offset;
          while it rises it is approached with rate 2.0 when the stick is active
(look, dist) = Frame2(P, playerHeight, T, targetSize, camera rotation, FOV, minDist)        // 0x1405DEFA0 → 0x1405E0590 ×4:
          starts from the midpoint (P + T) / 2 at distance |T - P| × 0.5 / sin(FOV / 2) and iterates three times so that
          both points stay in frame (the fitting rule: "Framing solver" below)
dist = max(dist, minDist)
lookSmooth = lerpStepVec(lookSmooth, look, rate +0x450, dt) with the lag limit +0x44C       // as camera spec 2.3.2
distSmooth (+0x9E4) = lerpStep(distSmooth, dist, 8.0, dt)
camPos = lookSmooth - Forward(rotCur) × distSmooth
water plane, an obstruction blend (0x14061BA10; 0.6 s ease when it releases), final camera collision 0x14061CB40 (camera spec 3.3)
```

**Framing solver** (third pass 2026-10-12) [C 0x1405DEFA0 / 0x1405E0590; the tail of the fit I 0x1405E0F93–0x1405E107B]:

```
Frame2(A = T with size a, B = P with size b, rot, fov):         // a = camera slot 285 (0x1406245C0), b = slot 286: both the constant 50.0 [I]
    cam = (A + B) / 2 − Forward(rot) × |A − B| × 0.5 / sin(fov / 2)
    if |A − B|xy² < 1: Fit(cam, flag 0); return                 // the look point is kept
    (look, d) = Fit(cam);   repeat 3 times: cam = look − Forward(rot) × d;  (look, d) = Fit(cam)
Fit(cam):                                                       // everything relative to cam
    far, near = the point farther from / nearer to cam (with their sizes)
    Q = the point of the ray cam → far that has near's depth along Forward(rot)          // (far itself when it is behind the camera)
    u = normalize(near − Q);   E1 = Q − u × size(far);   E2 = near + u × size(near)
    look = (E1 + E2) / 2;   half = |E2 − E1| / 2
    n = normalize(screen(E2) − screen(E1)) with the view-projection saved from the previous frame (camera+0xA30)
    w = 1 − cos(|n.y| × π / 2)                                  // 0: the segment lies across the screen, 1: it stands upright
    a0 = π / 2 − fov / 2;   aspect = viewport height / width (0.5625 without a viewport)
    angle = a0 + (atan2(tan(a0) × half, aspect × half) − a0) × w
    d = tan(angle) × half × (1.1 − 0.1 × w)                     // [I: 1.1 = 0x3F8CCCCD]
```

So across the screen the distance is 1.1 × half / tan(fov / 2), upright it is half / tan(fov / 2) / aspect. Because the far
point is brought onto the near point's depth plane, a target straight ahead of Sora gives a short segment and the result
stays under the minimum: **the usual lock-on distance is `m_Distance` (400)**; it grows when the pair is seen from the
side (after the stick orbited) or the target is high above.

**Lead** [C 0x1406100D0 lines 215–410; 0.24 I]: with s(X) = the screen position of X by the saved view-projection, each
axis clamped to [−1, 1], and the zero vector for a point behind the camera:

```
vP = player pawn.vt+0xE28() (its measured velocity);   leadP = (|s(P) − s(T)| < |s(P + vP) − s(T)|) ? vP : 0
leadP = leadP × 0.24 × |s(P)|;   +0x9E8 = lerpStepVec(+0x9E8, leadP, rate +0x450, dt);   P += +0x9E8
vT = (camera+0xA00 = T's displacement since the last frame) / dt;   leadT = (|s(T + vT) − s(P)| <= |s(T) − s(P)|) ? 0 : vT
leadT = leadT × 0.24 × |s(T)|;   +0x9F4 = lerpStepVec(+0x9F4, leadT, rate +0x450, dt);   T += +0x9F4
```

A point is led only while its motion carries it away from the other one on screen, and the more the nearer it is to the
frame's edge (nothing at the centre).

**Obstruction blend** [C 0x14061BA10 and its caller]: the camera collision 0x14061CB40 is run from the player point (z
averaged with the look height) to the camera position; the camera class's own test (slot 283; the base class answers
"no") is or-ed in. Blocked: the camera takes the pulled-in position, it is saved (+0xA80) and the interpolator at +0xA98 is
started with (type 2, from, to, 0.6 s) [the curve of type 2: ?]. Free again while the interpolator runs: its start is
moved by the camera's own displacement since the last frame (+0xA8C), its end is the free position, and the camera shows
its value: the release takes 0.6 s. Then the final collision from the look point as in the field camera.

Port: `crates/kh-camera/src/lockon.rs` (`frame2`, `fit`, the lead and the blend in `step`; the 0.6 s curve is a
smoothstep), tests `crates/kh-camera/tests/lockon.rs`.

At start (slot 242 0x14060BD30): `rotTarget = base`, component rotation = it, look point = the midpoint (P + T) / 2,
+0x9E0 = `m_Distance`. `Reset` (R3; slot 244 0x140609DF0) = the normal reset (rotTarget = base, 0.5 s fast rate) plus
`m_Distance` restored: R3 while locked re-centres on the target.

For a port: the lock-on camera is a second camera state with its own `rotTarget` / look point / distance, entered and
left through a 0.5 s ease (exponent 2) blend of position, rotation and FOV with the free camera; it looks at a point
between Sora and the target, from at least 400 cm, yaw toward the target, pitch = the elevation of the target minus 20
(never below −10 when the target is level or above), pitch between −70 and the target's elevation (at most +20, at
least 0), yaw limit ±120 around the line to the target; the stick still orbits (slowed to 0.45 × at range), and after 1.5 s without stick it drifts back, faster the
farther the target; an off-screen target snaps the wanted rotation back at once.

The soft target does **not** touch the camera: no call into the camera manager is made for it.

Other camera hooks next to the targeting code [C]: `m_CameraMultiLock` (controller+0x9D8, class `TresCameraMultiLock`)
is spawned by slot 790 for the multi-lock mode; `m_pAttackCameraTarget` (controller+0xAD8) belongs to the attack-camera
function 0x1414122A0 (thunk 0x14140DF20; modes 1–7 at controller+0xAB0, cameras at +0xAB8 / +0xAD0): the finisher /
formchange "combo camera" called from `PlayAttack` and `End`; when it switches off in modes 1–3 or 6 with a manual
lock active it re-locks the camera through slot 753. Read in the fourth pass: 7.12.1.

#### 7.12.1 The attack camera (fourth pass 2026-10-12) [C 0x1414122A0, verdict same-partial; constants are immediates]

`AttackCamera(ctrl, bOn, mode, targetComponent, pPosition)`; mode = controller+0xAB0, target pawn = controller+0xAD8.

```
off: if mode == 0: return.   cam = the player's normal camera (0x1405ECEB0(0x141435620(ctrl)))
     0x1405D4730(cam, mode == 6 ? 1.0 : mode == 7 ? 0.1 : -1):    // look-point rate (+0x450) and lag limit (+0x44C) back to the class
                                                                  // defaults (8 / 100): at once, or over that time (interpolators type 5)
     0x140600BD0(cam): lock-on yaw limit (+0x918) back to the class default (120)
     cam+0x804 (distance scale) = controller+0xAE0 ? 1.35 : 1.0   // +0xAE0: no writer for Sora with the Kingdom Key found [?]
     the cameras of modes 1 / 2 (+0xAB8) and 8 (+0xAD0) end with the transition {0.5 s, function 3, exponent 2}
     mode ∈ {1, 2, 3, 6}: if ctrl.slot720() (manual lock) and the lock target is alive: unless controller+0x880 == 0xDC: ctrl.slot753(target)
                          else Camera.SetLockon(null)
     mode = 0; controller+0xAC8 = 0
on:  mode 0: return.   pPosition != null: controller+0xAC8 = pPosition
     3: Camera.SetLockon(manager, target); +0xAD8 = target's pawn                  // the lock-on camera of 7.12 on the attack's target
     4: 0x1405C63A0(cam, 2.0, 2000): rate +0x450 = 2.0, lag limit +0x44C = 2000 (also on the lock-on camera cam+0x418);
        0x14060A7D0(cam, 0): lock-on yaw limit 0 (= no yaw clamp); distance scale = 1.0
     5: distance scale = 1.6
     6: 3, then 4          7: as 4 with (0.2, 3000)
     1 / 2: a `TresCameraNormalLoc`-class camera of its own on the target (distance 750 / 600, pitch −20 / a yaw drawn at random, …),
     8: a `TresCameraShooterMode` actor: other keyblades' finishers and shooter mode, not read further
     controller+0xAB0 = mode
```

Interpolator curve by type (0x1411A0E70, dispatch [I]): 2 sin(t π/2), 3 1 − cos(t π/2), 4 (1 − cos(π t)) / 2, 5 1 − (1 − t)²,
6 t², else linear. The obstruction release of 7.12 is type 2.

Callers for Sora with the Kingdom Key [C]: `PlayAttack` kind 0x38 (off, on 5) and 0x28 (off, on 3 with state+0x118);
`Attack::End` kinds 0x28–0x2B, 0x36–0x39 (off); Sonic Rave (10.6: off, on 3 at the first dash; off at `LMEX_FINISH` and at the
end); the StyleFinish end event (off); `BadStatBurn` (9.9.4: off, on 4; off when its end clip starts and at End); `PoleTurn`
Begin 0x1413A41F0 (off, on 5) / End (off); `RiskDodge` Begin 0x1413A5730 (off, on 3 with the attacker) / End (off).
`FUN_14140DF30` (called from `PlayAttack`'s tail) is **not** a camera function: it is the sneeze latch (9.9.5).

Port: `kh-core/src/combat/camera.rs` (`request`, `off`; `EventKind::AttackCamera { mode, restore }` and the existing
`CameraLock`), `kh-camera/src/rig.rs` `CameraRig::set_attack_mode`, `lockon.rs` `interp_curve`.

### 7.13 Other readers of the lock [I]

- Idle state: `Begin` 0x141386050 stores `locked = controller.slot720()` in Idle+0x91 and slot 721's pawn in Idle+0xB0;
  `Tick` 0x1413944D0 recomputes `controller.vt+0x1908() | slot720()` each tick and, on a change, replays the idle name
  from pawn 0x141433F80 (`same`): `IdleN` (weapon away or not battle), `IdleB`, `IdleLockon` when the flag is set,
  `DiveIdle` in water. So only a **manual** lock (or the shotlock getter) gives `IdleLockon`; a soft target does not.
- Attack-camera code 0x1414122A0, magic / shotlock / athletic flow (their specs) read slots 720–723.

### 7.14 Host interface for targets (part of section 12)

```
trait LockonTarget {                         // one per TresLockonTargetComponent; an enemy may expose several
    fn id(&self) -> TargetId;
    fn targetable(&self) -> bool;            // enabled && lock-on permitted && owner alive / lockable        (7.9, slot 336)
    fn team(&self) -> i32;                   // compared with Sora's team number
    fn owner_is_enemy(&self) -> bool;        // actor tag "Enemy"; others are only considered within 450 cm
    fn owner_is_gimmick(&self) -> bool;      // actor tag "Gimmick" (skipped when the host disables gimmick lock-on)
    fn priority(&self) -> u8;                // LOW 0 / HIGH 1
    fn location(&self) -> Vec3;              // component world location: searched, scored, homed on
    fn look_at_position(&self) -> Vec3;      // camera aim point; = location() unless the owner supplies a view target
    fn auto_lock_on_range(&self) -> f32;     // 0 = use Sora's 900; > 0 shortens the measured distance (7.4, 7.7)
    fn rush_suppression(&self) -> bool;      // 7.10
    fn owner(&self) -> ActorId;              // pawn for the HP gauge, the attack state's target pawn, LOS ignore list
    fn owner_velocity(&self) -> Vec3;        // TargetMetrics lead (section 4.1) and the camera lead
    fn owner_on_ground(&self) -> bool;       // gates the air-down attack
    fn reaction_state(&self) -> u8;          // chase attack only
}
trait LockonHost {
    fn targets(&self) -> &[&dyn LockonTarget];                       // registration order
    fn line_blocked(&self, from: Vec3, to: Vec3, ignore: [ActorId; 2]) -> bool;
                 // static world, dynamic world objects and gimmicks block; pawns never do
    // events out
    fn cursor_add(&mut self, t: TargetId, kind: CursorKind /* Auto = 0, Lock = 1 */);     // idempotent per (target, kind)
    fn cursor_remove(&mut self, t: TargetId);
    fn enemy_gauge_target(&mut self, owner: ActorId);                // first cursor of a target
    fn ui_sound(&mut self, id: u8);                                  // 7 on a lock cursor
    fn camera_lock(&mut self, t: Option<TargetId>);                  // 7.12; None = unlock
}
inputs per frame: dt, rb_held, rt_held, Sora root location + facing yaw, team, in_water,
                  flags: input_lock, manual_lock_permitted, auto_lock_disabled, gimmick_lock_disabled, soft_only
outputs per frame: current_target (soft or locked), locked: bool, candidate (≤ 3500), crowd_count (< 400 cm)
```

---

## 8. Hits and damage

### 8.0 Objects, offsets and flag bytes [I]

| Object | Offsets |
|---|---|
| `UTresAtkCollComponent` (vtable 0x145D73008) `comp` | +0x1B8 owning actor (the weapon); +0x868 team number (ctor 0xFF); +0x870 `m_IgnoreActors`; +0x880 / +0x888 / +0x890 weak actors (attack owner / instigator / velocity source); +0x898 weak **owner** = attach-root actor; +0x8A0 weak `AttachParent` mesh; +0x8A8 weak "owner attached mesh"; +0x8B0 `m_pOwnerBaseParam`; +0x8B8 `m_pOwnerDataTableSet`; +0x8C0 `CollisionShapesSrc`; +0x8D0 `m_HitEffects`; +0x8E0 `m_fHitEffectRandomOffsetRadius`; +0x8E8 `m_CollAutoActivate`; +0x8F8 `m_AtkColls` (primitives, count +0x900); +0x908 map group name → activation record (0x68-byte entries); +0x958 hit-effect set index (ctor −1); +0x95C bit 0 team check on (ctor 1), bit 2 dirty, bit 3 map-height limit; +0x960 height limit; +0x964 bit 0 registered; +0x968 bit 0 keep-ignore-lists-off-in-reflect-window, bit 1 nearest-only; +0x970 `m_Mgr` |
| `UTresAtkCollPrimitive` (vtable 0x145D75FB0) `prim` | +0x868 group name; +0x870 centre; +0x87C previous centre; +0x888 size (radius, y, half-length); +0x894 previous size; +0x8A0 scale; +0x8B0 rotation quaternion; +0x8C4 / +0x8D0 relative location 1 / 2; +0x8E0 relative rotation; +0x8F0 `m_Size`, +0x8FC `m_IncSize`, +0x908 `m_IncMaxSize`; +0x938 / +0x944 end points, +0x950 / +0x95C previous end points; +0x968 / +0x969 attach type 1 / 2; +0x970 / +0x978 socket 1 / 2; +0x980 power; +0x984–0x987 flag bytes (table below); +0x988 override colour, +0x994 override alpha; +0x998 shape type; +0x999 rotation attach type; +0x9A0 / +0x9A8 default row name / pointer; +0x9B0 collision profile name; +0x9B8 bad-status rate; +0x9BC damage exec rate; +0x9C8 `TresDamageInfo` (0x68); +0xA30 current effect group; +0xA38 shape effect group; +0xA4C time since activation; +0xA58 / +0xA60 size / scale curve; +0xA68 current hit sound; +0xA78 default hit sound |
| activation record `rec` (map entry +8) | +0x00 flags (bit 0 active, bit 1 clashed, bit 2 ending, bit 3 first tick); +0x04 re-hit interval; +0x08 re-hit timer; +0x0C count interval; +0x10 count timer; +0x18 **hit list** (weak actors, count +0x20); +0x28 ignore list (count +0x30); +0x38 held list (count +0x40); +0x48 clash list (count +0x50) |

Primitive flag bytes [I: init 0x140642CA0, row 0x140645900, setters named]:

| Byte | Bit | Meaning | Written by |
|---|---|---|---|
| +0x984 | 0 | active | `Enable` 0x1406468A0 |
| | 1 | updated this tick | 0x14062CA80 |
| | 2 | positions initialised (cleared on activation) | 0x1406468A0 / 0x1406489A6 |
| | 3 | `m_bDisableSweep` | 0x140642DC1 |
| | 4 | "no bonus" argument of the damage formula | slot 335 0x1406469B0 |
| | 5 | "victim may refuse" (see 8.2.6, first test) | activation S+0x5C bit 6; 0x1406277E0 |
| | 6 | `m_bIgnoreParentScale` | 0x140642DEF |
| | 7 | = `m_bIsPhysAttackCollision` (copy of +0x985 bit 0) | slot 334 0x1406457E0 |
| +0x985 | 0 | `m_bIsPhysAttackCollision` | 0x140642D7E |
| | 1 | `m_MapHitType` ≥ 2 (HITMAP, IGNORE_GROUND): the sweep includes WorldStatic | 0x140642D07 |
| | 2 | `m_MapHitType` == 3 (IGNORE_GROUND) | 0x140642D38 |
| | 3 | `m_MapHitType` == 1 (PHYSOBJ_NOHIT): the sweep ignores PhysicsBody | 0x140642D21 |
| | 4 | `m_bDisableTakeDamage` | 0x140642D50 |
| | 5 | `m_bEnablePawnRootCollision` | 0x140642D69 |
| | 6 / 7 | `m_bAbsoluteOffset1` / `2` | 0x140642E86 / 0x140643070 |
| +0x986 | 0 / 1 | `m_bDisableLocation1Attach` / `2` (position frozen after the first tick) | 0x140642E30 / 0x140642E51 |
| | 2 / 3 | size / scale curve loops | 0x140642ED6 / 0x140642F01 |
| | 4 | row `m_TeamCheckType` != NORMAL | 0x140645900 |
| | 5 | row `m_TeamCheckType` == SAME_ZERO | 0x140645900 |
| | 6 | `m_bDisableCharHit` | 0x140642D98 |
| | 7 | **no clash**: set at init, cleared by a row with `m_ReflectLevel` > 0, set again by activation flag S+0x5C bit 1 | 0x140642DAB, 0x140645900, 0x140625500 |
| +0x987 | 0 | has a spawned shape actor | several |
| | 1 | map-hit-reflect window open | 0x140625C00 / 0x1406250E0 |
| | 2 / 3 | hit-effect colour / alpha override | 0x140646970 / 0x140646950 |

### 8.1 Shapes [A, flags I]

Sheet `sora-combat-collisions` (19 keyblade groups of `w_so_KeybladeBase` `TresAtkColl0`; `w_so010_Wep` adds none and
sets only `m_HitEffects`). Facts that matter for the base combo, with the flags they produce (sheet
`sora-combat-shape-flags`):

- `KB_Stick`: CAPSULE2 between weapon-mesh sockets `ATK_00` and `ATK_01`, radius 50, `m_MapHitType` IGNORE_GROUND —
  **the only keyblade group whose sweep includes WorldStatic**; `m_bIsPhysAttackCollision`.
- `KB_StickM` (80), `KB_StickL` (120): same sockets, NOHIT. `KB_SphereSS` 250, `KB_SphereS` 350: sphere on `ATK_00`,
  phys. `SphereM` 550, `KB_FixRangeS` 350 (location frozen at activation): not phys.
- `FrontImpactS`: sphere on Sora's body mesh socket `vfx_cure` + (200, 0, 0), radius `100 + 700 t` capped at 350
  (reached at 0.357 s). `KB_FixPillar`: CAPSULE1 at `vfx_cure` + (200, 0, 0), frozen, rotation LOCAL, radius
  `50 + 2000 t` capped at 300 (0.125 s), half-length 800.
- All: profile name `TresProjectile` (stored at prim+0x9B0, not used by the sweep, 8.2.3), `m_bDisableSweep` false,
  no default row, effect group `None`, no hit sound of their own, `m_bDisableTakeDamage` / `m_bDisableCharHit` false.
- The component has no `m_CollAutoActivate`, `m_fHitEffectRandomOffsetRadius` 0 [A: property absent, ctor 0x140616C90
  zero].
- Weapon constructor (`ATresWeaponBase` 0x141777570) [I 0x1417778A0]: map-height limit on, `comp+0x960 = 50`.
  Component constructor (0x140616C90) [I]: team check on, team number 0xFF, effect set index −1.

Size over time (0x140649E40) [I]: `size = max(m_Size + m_IncSize × t, 0)` per component, then
`min(size, m_IncMaxSize)` for every component whose `m_IncMaxSize` > 0; a size curve replaces it. `t` = prim+0xA4C,
reset on activation, advanced by the owner-scaled dt (8.2.2).

### 8.2 Detection and application

#### 8.2.1 Activation [I] (notify begin 0x140561F40 → interface +0x200; end 0x140564B10 → +0x208)

`TresAnimNotifyState_Attack::NotifyBegin` builds a 0x70-byte activation struct `S` and calls the mesh owner's
`UTresAnimInterface` slot 64 (+0x200):

| S offset | Content | From |
|---|---|---|
| +0x00, +0x10 | group name | `CollisionGrpName` (notify+0x48) |
| +0x08 | bit 0 = 1 | |
| +0x18 | attack-data row name | `m_AttackDataIDName` (+0x50) |
| +0x20 | re-hit interval, s | `AttackInterval` (+0x58) |
| +0x24 bit 0, +0x28 | hit-effect group override | `m_bOverrideHitEffect` (+0x5C), `m_HitEffectGrpName` (+0x60) |
| +0x30 | hit sound (weak object) | `m_HitSEAsset` (+0x68) |
| +0x38 | weapon hit-sound alias (u32) | `m_WeaponHitSEAliasID` (+0x70) |
| +0x3C bits 0 / 1, +0x40 (16 bytes) | effect colour / alpha override | notify+0x74, +0x78 |
| +0x50, +0x54 | command kind, shoot-flow kind (−1 at the notify) | filled by the pawn, below |
| +0x58..+0x5B | four counter bytes (0) | filled by the state, below |
| +0x5C bit 7 | | `m_bIgnoreComboCountUp` (+0x98) |
| +0x5D bit 1, +0x60 | take over another group's hit lists | `m_bTakeOverHitList` (+0x88), `m_TakeOverHitListGrpName` (+0x90) |
| +0x68 bits 0 / 1, +0x69, +0x6A | optional calc type / survive rule | 0 at the notify |
| +0x6C | | `m_ComboCountUpParam` (+0x9C) |

Pawn interface slot 64, base 0x1405B3FE0 (player override 0x14054A370 adds AI bookkeeping only) [I]:

```
S' = copy(S)
(pawn+0x448 notify iface).slot13(S')              // pre-processor chain:
    Sora 0x141443D70: S'.cmdKind(+0x50) = pawn.slot959() if > 0; S'.shootFlow(+0x54) = pawn.slot960() if > 0
    → state notify sub-object (state+0x48, vtable 0x146900E38) slot 13 = 0x141343800:
        row renames (kind 0x0E: KB_DOWN → KB_DOWN_FIN, …)
        if state.finishEnd(+0x141): S'+0x5C |= 1                     // → info.finish
        if state.combo(+0x128) >= 1: S'+0x5C |= 2                    // → reflect level 0, no clash (first swing only clashes)
        S'+0x58 = (u8)state+0x128; S'+0x59 = (u8)state+0x12C; S'+0x5A = (u8)state+0x130; S'+0x5B = (u8)state+0x134
        S'+0x5C bit 3 = state+0x234 bit 2 (air mode)                 // → info.aerial
        S'+0x5D bit 0 = state+0x23C bit 0 (just-guard attack)        // → info.justGuard
for each equipped weapon (pawn+0x598 → list +0x40 / +0x48): Activate(weapon.MyAtkColl (+0x578), S')    // 0x141787280
if pawn.MyAtkColl (+0x588): Activate(it, S')
current state's anim interface slot 64 (S')       // Attack state 0x1413459E0: section 2
```

Both components receive every activation; a component without a primitive of that group does nothing.

`Activate(comp, S)` 0x140625500 [C, constants and field writes I]:

```
row = comp.m_pOwnerDataTableSet ? FindRow(set.m_AttackData, S.rowName) : null      // 0x140580BB0: exact FName, no fallback name
for each prim with prim.group == S.group, or S.group == "@ALL" (FName 0x1487CAB40):
    if row: prim.SetRow(S.rowName, row, S.cmdKind, S.shootFlow)                     // slot 327 0x140645900, below
    else:   prim.SetDefaultRow(S.cmdKind, S.shootFlow)                              // slot 328 0x140645BB0: the shape's
                                                                                    // m_DefaultAttackDataIDName row, else the static default row
    if comp.m_pOwnerBaseParam: prim.info+0x40 = baseParam+0x7D (byte)
    prim.Enable(true)                           // 0x1406468A0: on a rising edge time = 0, size reset, "initialised" cleared
    prim.info+0x52..+0x55 = clamp(S+0x58..+0x5B, 0, 99)
    prim.info.finish     = S+0x5C bit 0;   prim.info.aerial = S+0x5C bit 3
    prim.info.fullMpBurst = S+0x5C bit 5;  prim+0x984 bit 5 = S+0x5C bit 6
    prim.info.justGuard  = S+0x5D bit 0
    if S+0x5C bit 1: prim.info.reflectLevel = 0; prim+0x986 |= 0x80
    if S+0x5C bit 2: prim.badStatusRate = 0
    prim.effectGroup(+0xA30) = S.bOverrideHitEffect ? S.hitEffectGroup : shape m_EffectGrpName        // slot 332
    prim.hitSound(+0xA68) = valid(S.hitSound) ? S.hitSound
                          : (S.alias > 0 && comp.owner valid && comp's actor is an ATresWeaponBase
                             && (e = weapon.slot466(S.alias)) != null) ? *e          // weapon.m_SoundAssets_Hit[alias], 0x14178BEB0
                          : prim default hit sound (+0xA78)                           // slot 333
    colour / alpha overrides; S+0x68 bit 0: info+0x65 = S+0x69; bit 1: info+0x66 = S+0x6A
if nothing matched: return
rec = record of S.group
rec.interval = S.interval; rec.timer = 0
rec.countInterval = (S.interval > 0 && S+0x6C <= 0) ? S.interval : 0; rec.countTimer = 0
rec.flags = (rec.flags & ~0xE) | 1 | (rec.countInterval > 0 ? 8 : 0)
clear rec.hitList, rec.ignoreList, rec.heldList, rec.clashList            // 0x140626910
if S.bTakeOver: copy the three lists +0x18, +0x28, +0x48 of the record named S.takeOverGroup
comp+0x95C |= 4
```

Row → primitive (slot 327 0x140645900) [I, whole function]: `prim.power(+0x980) = m_Power`;
`prim.badStatusRate(+0x9B8) = clamp(m_BadStatusGenRate, 0, 100)`; `prim.execRate(+0x9BC) = clamp(m_DamageExecRate, 0,
100)`; prim+0x986 bit 4 = `m_TeamCheckType != NORMAL`, bit 5 = `== SAME_ZERO`, bit 7 = `(int8)m_ReflectLevel <= 0`;
the `TresDamageInfo` at prim+0x9C8 is filled as in 8.7; command kind and shoot-flow kind are written only when
≤ 0xFD / ≤ 0x2C. Static default row (constructor 0x140570870) [I]: power 1.0, SMALL, PHYSICAL, power scale 1.0, exec
rate 100, everything else 0 / false / NORMAL. It is what a window gets when its row name is not in the table **and**
the shape has no default row: there is no lookup of a row called `DEFAULT`.

Deactivation (`End` → interface slot 65 0x1405B2D80 → `Deactivate(comp, group, keep = false)` 0x140625000) [I]:
`prim.Enable(false)`; effect group and hit sound back to the shape's; `rec.flags &= ~0xF`, the four timers zeroed.
The hit lists stay until the next activation of the group (which clears them).

Data check [A, `S5_anims_all.json`]: none of the 169 attack windows of Sora's clips uses take-over, 26 set
`AttackInterval` (0.1–0.5 s), 5 override the effect group; all 95 rows used have `m_DamageExecRate` 100,
`m_TeamCheckType` NORMAL, `m_ReflectReaction` DEFAULT; `m_ReflectLevel` is 1 on 16 rows (`KB_VERTICAL`, `KB_STAB`,
`KB_TURN`, `KB_RISE`, `KB_ROLLDOWN`, `KB_DOWN`, `KB_LAUNCHSPIN`, `KB_ROLLBEAT`, `KB_QUICKSHAFT(_END)`, `KB_RADIALCUT`,
`KB_AERIALSWEEP(_END)`, `KB_AERIALDIVE`, `FF_SONIC_1 / 2`), 4 on `KB_FLASHSTEP`, 0 elsewhere.

#### 8.2.2 Tick order [C for the driver, I for the timers]

All attack components are ticked by one actor, `ATresAtkCollManager` (constructor 0x14062EC80: tick-group byte
actor+0x48 = 8 [I; the group's name in this engine fork is ?]), `Tick` 0x1406472A0:

```
pass 1, every registered component: Update(comp, dt × owner.slot204() × owner.CustomTimeDilation)        // 0x140626310
    0x140620320: a record with flag bit 1 (clashed): interval <= 0 → deactivate its group now; else flag |= 4
    0x14062CA80: every active prim: prim.Update(owner, attachMesh, ownerMesh, dt) (slot 344 0x140647950), 8.2.3
    0x14062CC90: every record: n = TimerTick(rec, dt); if n > 0: owner notify slot 22 ({comp, group, rowName, countInterval, n})
pass 2, every component, skipped while game flag 0x140EAB080 is set: 0x140624940 → 0x140620AA0:
    for every prim with flag bit 0 and whose record has flag bit 2 clear: Sweep(comp, world, owner, prim)   // 0x14062A920
    owner notify slot 48 (comp); comp+0x95C &= ~4
pass 3: 0x140640F70: queued guard / clash effects are spawned
```

`owner` = comp+0x898 = the attach-root actor of the component (0x140626390 → 0x144034830): for the keyblade in the
hand, **Sora's pawn**. So the dt of the hit shapes follows Sora's own time dilation. Relative to the pawn's frame
order (C → M → S) the manager tick is a separate actor tick. Fourth pass: its tick group 8 is `TG_SQEX_PostUpdateTransform`
in this fork's `ETickingGroup` (reflection: Early 0, LevelScript 1, PrePrePhysics 2, PrePhysics 3, Start / During /
EndPhysics 4–6, PostPhysics 7, SQEX_PostUpdateTransform 8, SQEX_PostUpdateManager 9, SQEX_LastEffectUpdate 10, …), and no
constructor of the pawn chain writes the pawn's group (stock `TG_PrePhysics` [U]): the manager ticks **after the pawn's
whole tick, in the same frame**. A window opened by a notify of M(N) sweeps in frame N, after `MainProc`; the state sees
the hit in its next Tick. (Port: `sora.rs`, `combat::hit::tick` after `states::main_proc`.)

`TimerTick` 0x14062CFC0 [I, whole function]:

```
n = 0
if !(flags & 1): flags &= ~8; return 0
if countInterval > 0:
    if flags & 2: countInterval = countTimer = 0
    else: if !(flags & 8): countTimer += dt
          while countTimer >= countInterval: countTimer -= countInterval; n++
if interval > 0:
    if hitList.count <= 0 && !(flags & 2): timer = 0                 // the clock starts at the first hit
    else: timer += dt
          if timer >= interval:
              hitList.count = 0; timer = 0                            // every victim may be hit again
              if flags & 2: flags &= ~2; clashList.count = 0
              flags &= ~4
flags &= ~8
return n
```

So `AttackInterval` is not a per-victim cooldown: the timer starts when the window scores its first hit, and when it
elapses the whole hit list is emptied at once. With interval 0 a window hits each actor once.

#### 8.2.3 Shape update and sweep [I unless tagged]

`prim.Update` (0x140647950), each tick while active:

```
time += dt
prevCentre = centre; prevEnds = ends; prevSize = size
size = Size(time) (8.1); scale = Scale(time)
T1 = SocketTransform(attachType1, socket1) [, T2 for CAPSULE2]            // 0x14064BA60 [C]
    NORMAL: comp.AttachParent mesh (the weapon mesh) GetSocketTransform(socket, world); OWNER_ATTACHED_MESH: comp+0x8A8
    mesh (the wielder's body mesh), else the weapon mesh; WORLD: identity; no mesh: the owner's root transform
unless (DisableLocationNAttach && initialised):
    endN = TN.translation + (AbsoluteOffsetN ? relLocN × TN.scale : TN.rotation ∘ (relLocN × TN.scale))
SPHERE / CAPSULE1: centre = end1; rotation = (rotAttach == LOCAL) ? relRot : T1.rotation × relRot
                   size × = min(scale.x, scale.y, scale.z) unless IgnoreParentScale
CAPSULE2: centre = (end1 + end2) / 2; radius = size.x × min scale; halfLength = max(|end2 − end1| / 2, 0.5);
          rotation = rotation from the axis end1 → end2, × relRot
if !initialised: prevCentre = centre; prevEnds = ends; prevSize = size; initialised = true      // 0x1406489A6
```

The first tick of an activation therefore tests the shape in place (zero-length sweep); later ticks sweep from the
previous centre to the current one with the **current** size and rotation (the rotation is not swept).

`Sweep` (0x14062A920):

```
start = (prim+0x984 & 8 /*DisableSweep*/) ? centre : prevCentre;  end = centre
ignored: the prim, the component's actor (the weapon), the owner (Sora), comp.m_IgnoreActors,
         rec.hitList and rec.ignoreList — unless (comp+0x968 bit 0 && reflect window open)
trace channel 0x17 = ECC_GameTraceChannel10 "TresAtkCollision" [A ini line 621]
response set: every channel Overlap, then Ignore for
    4 Camera, 0x0E TresEquip, 0x0F TresProjectile, 0x10 TresPrize, 0x11 TresTrigger, 0x13 TresReactor, 0x14 TresSound,
    0x17 TresAtkCollision;
    2 Pawn, 0x1C TresEnemyPawn, 0x1D TresSpecialPawn, 0x1E TresWorldUnique   unless m_bEnablePawnRootCollision;
    0 WorldStatic                                                            unless m_MapHitType >= HITMAP;
    5 PhysicsBody                                                            if m_MapHitType == PHYSOBJ_NOHIT;
    0x1A TresAtkBody (other attack shapes)                                   if "no clash" (prim+0x986 bit 7)
shape: degenerate (radius <= 0.0001, slot 336): line trace; SPHERE: PxSphere(size.x);
       CAPSULE1 / CAPSULE2: PxCapsule(radius size.x, half segment length size.z, pose rotation);
       ring / box / convex / laser types: not used by the keyblade
hits = multi-sweep (PhysX scene sweep, up to 512 touches) [C 0x144526380, SQEX_PhysXCollision.cpp]
Apply(comp, hits, owner, prim, rec, false, false)                            // 0x140618940
```

Because already-hit actors are ignored by the query itself, a victim in the hit list costs nothing until the list is
emptied. Pawn root capsules are not hit (no keyblade shape enables it): what a swing touches on a character are its
hurt-shape components (`TresBodyCollPrimitive`; that their object channel is TresBodyCollision 0x18 and that they
answer the TresAtkCollision trace is inferred from the channel names, not read [?]).

#### 8.2.4 `Apply`: ordering, clash pass [C, constants I]

```
if !(comp+0x964 & 1) or prim null: return
instigator = comp+0x888 actor ?: comp+0x880 actor ?: owner          // damage causer, ability source of the formula
attackOwner = comp+0x880 actor ?: owner                              // whose Strength / Magic the formula reads
ctrl = controller of instigator (0x143E9C570)
held = copy of rec.heldList; rec.heldList.count = 0
if hits.count >= 2: sort (0x1406217F0 + 0x140614B80): attack-shape components first, then by hit Time ascending,
                    ties by the distance of ImpactPoint from lerp(ref1, ref2, Time); ref points by knockback type:
                    ATK_COLLISION / VELOCITY: prevCentre → centre; INSTIGATOR_PAWN: the instigator pawn's location;
                    NORMAL: comp+0x890 actor ?: instigator ?: owner location
```

Clash pass (only when the prim may clash — reflect level > 0 and first swing of the combo, 8.2.1 — and a record
exists), for every hit whose component is another `UTresAtkCollPrimitive` P2 of object type 0x1A with clash enabled
and whose record is not ending [I 0x14061920E–0x1406192F2]:

```
skip: P2's actor == owner; same team (team test of 8.2.5); same projectile id
res = (P2.reflectLevel − prim.reflectLevel > 0) ? 3 : 4              // ETresReactionResultType REFLECT / GUARD_REFLECT
my m_ReflectReaction: STAGGER → 3; GUARD → 4; NOREACTION → 0; DEFAULT → as computed
if actor already in my hit / clash list, or the owner is in P2's: skip
rec.flags |= 2; add the actor and its owner chain to rec.clashList
manager: queue the clash effect (unless owner notify slot 97 vetoes; Sora: never)
owner notify slot 15 (hit, {comp, prim, res, info})                 // Attack state: res 3..5 → weapon recoil record (5.6.3)
```

Afterwards every hit on an attack-shape component is dropped from the array. A record with flag bit 1 stops sweeping
from the next tick (`rec.flags |= 4` in pass 1) until its interval elapses, or is deactivated at once when it has no
interval.

#### 8.2.5 `Apply`: per hit [I 0x140619650–0x14061A2E3]

`A` = hit actor (may be null), `C` = hit component, `p6` / `p7` = the two flag arguments (false in the sweep path).

```
 1  C valid, not an attack shape, C.objectType != 0x1A; else next
 2  A in rec.clashList or rec.heldList: next.   A == owner: next
 3  A != null: needs A.bActorEnableCollision (+0xA6 bit 1) && !A.m_bDisableObjTypeCollision (+0x3B4 bit 4)
               && A+0x3CC == −1; else next
 4  isMap = IsMapHit(A, C)                                            // 0x140643CC0, below
 5  if !mapHitsOn(prim+0x985 bit 1) && !p6 && !reflectWindow && isMap && rec:
        if prim is not phys: if 0x14042E040(A) returns an empty list (components of A it collects; kind not read [?]):
                                 rec.ignoreList += A; rec.hitList += A
        next
 6  pawnV = A as ATresPawnBase; gimmickV = A as ATresGimmickActor; victimNotify = A's UTresNotifyInterface
 7  if m_bDisableCharHit && pawnV: next
 8  team: if comp team check on && !(row team type == NO_CHECK):
        if pawnV && (pawnV+0x460).slot47(C, instigator, comp, prim): next       // 0x1405D0370 [C]: row NORMAL → true when
                                                                                // comp.teamNo == victim.GetTeamNo() (inverted for RECOVER rows)
        if gimmickV && (gimmickV+0x3F8).slot47(…): next
 9  map gate, entered when (IGNORE_GROUND && !p6) or (reflectWindow && !mapHitsOn && !p6 && isMap),
    and then only for A == null or (A.m_ObjType ∈ {NONE, MAP, BG} && isMap):
        r = ownerNotify ? ownerNotify.slot24(hit, prim) : 0          // Sora → Attack state 0x141342760 [C, read whole in the third pass]:
                                                                     //   n = dot(Up, ImpactNormal); low = ImpactPoint.z <= root.z + 45
                                                                     //   if no recoil record yet && prim+0x987 bit 1 (reflect window) && (n >= 0.656059 || n <= −0.8660254):
                                                                     //       trace "ReflectCheck" from root + Up × 50 to the primitive (CAPSULE2: its nearer end);
                                                                     //       first blocking hit h, m = dot(Up, h.ImpactNormal):
                                                                     //       return (m <= −0.8660254 || m >= 0.656059 || (phys && low && h.ImpactPoint.z <= root.z + 45)) ? 1 : 2
                                                                     //   if n < 0.656059: return (phys (prim+0x984 bit 7) && low) ? 1 : 2
                                                                     //   return 1
                                                                     // 1 = not a wall worth hitting (a floor up to 49°, or lower than 45 cm for
                                                                     // a physical shape), 2 = hit it
        r == 1: next;   r != 0: go on
        r == 0: if dot((0,0,1), hit.ImpactNormal) >= 0.6427876 (cos 50°): next
                if owner is a pawn && comp height limit on && prim is phys:
                    if ownerRoot.z + comp+0x960 (50) >= hit.ImpactPoint.z: next
10  bHit = true; reflectMap = false
    if rec && comp registered:
        if A in held: rec.heldList += A; bHit = false
        elif A in rec.hitList: if reflectWindow && !p6 && isMap: reflectMap = true   else bHit = false
11  if A's actor interface slot 7 object exists && 0x1405897E0(it, info.damageKind): rec.hitList += A; next      // immune to the kind
12  rate = prim.execRate: rate <= 0 → rec.hitList += A; next
    rate < 100: roll = (xorshift128() >> 8) × 5.9604645e-6            // uniform [0, 100); 0x142CFF460, constant 0x145D73AC0
                roll > rate → rec.hitList += A; next
13  if !bHit: next
14  if owner && nearest-only (comp+0x968 bit 1): if hit.Time > best: next; best = min(best, hit.Time)   (best starts at 2.0)
15  if owner == null: next
16  info = copy of prim.info
    info.magicDraw = AbilityCount(attackOwner, MAGIC_DRAW 60) > 0 && info.magic
    sameTeam = pawnV && row team type == SAME_ZERO && !(prim+0x984 & 0x10) && comp.teamNo == pawnV.GetTeamNo()
    info.critical: stub 0x14041F340 returns false — never set
    info.victimResult(+0x60) = DAMAGE (2); info.attackerResult(+0x61) = NONE (0)
    ownerNotify.slot17(hit, &info)                                    // Sora: empty
17  if m_bDisableTakeDamage: rec.hitList += A; victimResult = NOREACTION, attackerResult = NOREACTION (1);
                             clear bad status; go to 21
18  if info.badStatusKind != 0 && prim.badStatusRate < 100: roll as in 12; if roll > rate: clear bad status
    noReact = false
    reg = Classify(comp, prim, A, C, p7, &info, &noReact)             // 0x14061FE20, 8.2.6
    amount = DamageFormula(prim.power, instigator, attackOwner, A, &info, sameTeam, prim+0x984 bit 4)   // 8.3
    result = info.attackerResult
    dir = KnockbackDir(info.knockbackType, …)                         // 8.7
    event = FTresPointDamageEvent{damage type class, amount, dir, hit, info with knockbackDir = dir}     // 8.7
19  if victimNotify && !victimNotify.slot25(amount, ctrl, hit, &info, &dir, instigator): next           // victim's veto
    if rec && reg && comp registered: rec.hitList += A
20  if result != 0:
        if !reflectMap: HitEffects(comp, ctrl, instigator, A, pawnV, prim, hit, &info, result, noReact)  // 8.6
        if A != null:
            if amount != 0 && !noReact: A.TakeDamage(amount, event, ctrl, instigator)                    // vt+0x570, 8.4
            else victimNotify?.slot28(ctrl, hit, &info, &dir, instigator)                                // hit without damage
21  if result != 0 && ownerNotify: ownerNotify.slot15(hit, {comp, prim, result, info})                   // 8.5
```

Order facts worth copying: the exec-rate roll (12) is made before the already-hit test (13), so a prim with rate
< 100 consumes one random number per frame per overlapping actor (irrelevant for Sora: all rows 100, no roll); a
failed roll puts the victim in the hit list (no second chance until the interval); the damage formula runs before
the victim's veto.

`IsMapHit(A, C)` 0x140643CC0 [C]: `A == null` → true; `A.m_ObjType` NONE, MAP or BG → true unless C's object channel
is PhysicsBody (5) or Destructible (7); GIMMICK → true for channels 0, 1, 3, 0x12 when C does not simulate physics
(`C.slot120(0)` false); every other object type (PLAYER, ENEMY, FRIEND, …) → false.

#### 8.2.6 Guard / reaction classification (`Classify` 0x14061FE20) [I, whole function, jump table 0x1406202F4]

Inputs asked of the victim: `bc` = C's `UTresBodyCollInterface`; `ai` = A's `UTresActorInterface`;
`w = ai.slot12(info.attribute)`, `x = ai.slot13(info.attribute)` (the attribute is the victim's weak point /
nullified — names inferred), `g = info.rapidFire && ai.slot15()`. Results are `ETresReactionResultType` values:
0 NONE, 1 NOREACTION, 2 DAMAGE, 3 REFLECT, 4 GUARD_REFLECT, 5 GUARD, 6 ARMOR_BREAK, 7–10 STRONG_BODY_DAMAGE1–4.
`V` = victim result (info+0x60, preset DAMAGE), `Att` = attacker result (info+0x61); returns `reg` (put the victim in
the hit list).

```
no hurt-shape interface, or bc.slot2() false (a wall, a prop):
    if !p7 && reflect window open:  Att = REFLECT;  if A != null: Att = NOREACTION;
                                    if A.m_DefaultAttackHitResponse ∈ {GUARD 1, REFLECT 2}: Att = REFLECT
    else Att = NOREACTION
    return true                                         // V stays DAMAGE; A == null gets no TakeDamage (step 20)
if prim+0x984 bit 5 && ai && ai.slot16(0x10): clear bad status; V = Att = NONE; return true
if ai && ai.slot63() (armoured):
    ap = info.armorAttackPower
    if ap > 0 || w: info.armorDamage = 1; left = ai.slot70(ap, w, &info+0x5C)
                    if left <= 0: V = ARMOR_BREAK; Att = NOREACTION; return true
    else: V = NONE; Att = NOREACTION; noReact = true; return true
t = bc.slot7()                                          // ETresBodyCollReactionType
if (t == DEFAULT 0 || t == WEAK_GUARD 4 || (t == STRONG 2 && !w && !x)) && info.kind != WITHOUT && ai && ai.slot89(): t = GUARD 3
by t:
 1 NO_REACTION:     x → BLOCK;  w → V = DAMAGE;  else info.noReactBodyCorrection = 1, V = NOREACTION.   Att = NOREACTION
 2 STRONG:          w → V = DAMAGE;  x → V = NONE, noReact;  g → V = NOREACTION;
                    else rp = info.reactionPower; left = ai.slot32(rp);
                         V = left > 0 ? (rp > 1 ? 8 : 7) : (rp > 0 ? 10 : 9).                             Att = NOREACTION
 3 GUARD, 5 GUARD_NOREACT:
                    w, or (info.kind ∈ {CATCH, BIND} || info.ignoreGuard) → V = (t == 5 ? NOREACTION : DAMAGE), Att = NOREACTION
                    x → BLOCK;  else GUARDED
 4 WEAK_GUARD:      w → as "default";  x → BLOCK;
                    (kind ∈ {CATCH, BIND} || ignoreGuard) or !info.weakGuard → V = (g && !w) ? NOREACTION : DAMAGE, Att = NOREACTION
                    else GUARDED
 6 INVINCIBLE, 7 INVINCIBLE_NOREACT:  w → V = (t == 7 ? NOREACTION : DAMAGE), Att = NOREACTION;  else clear bad status, BLOCK
 8 GHOST, 9 GHOST_NOREACT:            w → V = (t == 9 ? NOREACTION : DAMAGE), Att = NOREACTION;
                                      else clear bad status, V = Att = NONE, return FALSE (not even recorded)
 10 REFLECT1:       w → V = DAMAGE, Att = NOREACTION
                    x → V = NONE, noReact, Att = NOREACTION
                    else V = g ? NOREACTION : DAMAGE; Att = NOREACTION
                         if the prim may clash: clear bad status; V = Att = GUARD_REFLECT (4);
                             d = 2 − info.reflectLevel: d > 0 → Att = REFLECT;  d < 0 → V = REFLECT
                             my m_ReflectReaction: STAGGER → Att = REFLECT; GUARD → Att = GUARD_REFLECT; NOREACTION → Att = NONE
 11 GHOST_INVINCIBLE: clear bad status; V = Att = NONE
 default (0, >11):  x → BLOCK;  else V = (g && !w) ? NOREACTION : DAMAGE; Att = NOREACTION
BLOCK:   V = NONE; Att = NOREACTION; noReact = true
GUARDED: if "no clash" (prim+0x986 bit 7) or p7: V = GUARD, Att = NOREACTION
         else V = GUARD, Att = REFLECT; my m_ReflectReaction: GUARD → Att = GUARD_REFLECT; NOREACTION → Att = NOREACTION
         clear bad status
return true (except GHOST)
```

For the Kingdom Key [D from 8.2.1]: every swing after the first of a combo, and every finisher, is "no clash", so a
guarding enemy just takes `V = GUARD` with no recoil for Sora; only the first swing (rows with reflect level 1) can be
repelled (`Att = REFLECT`) by a guard or a REFLECT1 body, which the Attack state turns into the recoil of 5.6.3.

### 8.3 Damage formula [I] (0x140632240, verdict "differs": read in the disassembly)

The port's one implementation is `crates/kh-core/src/status/damage.rs` (`compute_damage`), written from
the listing `build/sora_tmp/stats/dis_140632240.txt`; `docs/sora-stats-spec.md` section 7 describes the
same function. This subsection was corrected against that listing in revision 2 (two statements of
revision 1 were wrong: the difficulty factor and the RECOVER return value; see the end).

`amount = F(power, abilitySource, attackOwner, victim, info, flagOnly, noBonus)`;
`Ceil(v)` = `−(cvtss2si(−0.5 − 2v) >> 1)`, `Floor(v)` = `cvtss2si(2v − 0.5) >> 1` (UE idioms; `cvtss2si`
rounds to nearest, ties to even), every step in f32.

```
A = info.magic ? MagicPower(owner) : Strength(owner)        // 0x1405836E0 / 0x140580E70 (ints); 0 without a status object
D = Defense(victim)                                         // 0x140581830; 0 without a status object
lvl = battle level (game work +0x7E09 through 0x140EA85D0; 1 without a game instance);  flagG = code-menu option 0x140EAB160
x = max(A, 0);  go = true                                    // 0x1406323F1
if info.kind == RECOVER (0x14064E770): go = false            // x keeps the attack stat
elif flagOnly: x = 0; go = false
if !(power > 0): x = 0; go = false                           // 0x140632425
if !go: return nz(x)                                         // RECOVER with power > 0 returns max(A, 0); everything else here 0.1
if info.calcType (+0x65) == 1: return nz(Ceil(max(power, 0) * MaxHP(victim)))        // percentage attacks
if info.calcType == 2: return nz(Ceil(max(power, 0) * victim.slot23()))
if !noBonus && Count(source, JOIN_FORCE 100) > 0 && victim ∈ the attacker team's shared target list (world object +0x44E0): x += 5
x = max(x - D, 1)
if victim has status:
    lo = (float)status+0xA4 (m_DamageMin); hi = max((float)status+0xA8 (m_DamageMax), lo)
    type = status+0x7C                                       // 0 player, 3 / 4 friends, 1 / 5 enemy kinds
    if lo <= 0: lo = (type ∈ {1, 5}) ? 1 + Ceil(lvl * 0.1) : (lvl > 70 ? 13 : 1)
    x = max(x, lo)
    if hi > 0 && x > hi: t = Floor((hi + A - D) * 0.5); if lo > 0: t = max(t, lo); x = min(x, t)      // the raw stat A here
    if lvl > 70 && type ∈ {1, 5}: h2 = hi * (flagG ? 6 : 2); if x > h2 && h2 > lo: x = h2
    if type ∈ {0, 3, 4} && x > 80 && !(lo >= 80): x = 80     // 0x1406326B5
x = Ceil((x - 1e-6) * (power - 1e-6))                        // 0x1406326DF
if victim has status:
    cut = status+0x114; if cut > 0 && info.flag51 bit 6: x = Ceil((100 - min(cut, 100)) * 0.01 * x)
    x = Ceil(clamp(100 - Resist(victim, info.attribute), 0, 200) * 0.01 * x)             // 0x1405850B0
    if info.rapidFire (flag50 bit 4): x = Ceil(clamp(100 - status+0xDC, 0, 100) * 0.01 * x)
    if status+0x89 bit 0: x = Ceil(x * 0.25)
x = Ceil(DifficultyFactor(source) * x)                       // 0x140642820, call 0x140632865: reached WITH and WITHOUT a victim status
if source has status (abilities of the instigator):
    if !noBonus && step = info+0x54 (i8) > 0:                // = Attack state +0x130: hit windows of the chain before this attack (5.5)
        physical: n = Count(info.aerial ? AIRCOMBO_UP 32 : COMBO_UP 31); if n > 0: x += Ceil(step * n * 0.05 * x)
        magic:    n = Count(MAGIC_COMBO_UP 57);                           if n > 0: x += Ceil(n * step * 0.1 * x)
    if info.counter:            n = Count(COUNTER_UP 69);        if n > 0: x += Ceil(n * 0.5 * x)
    if info.counter && info.justGuard && Count(CRITICAL_COUNTER 104) > 0: x *= 2
    if attribute ∈ 1..5:        n = Count(32 + attribute);       if n > 0: x += Ceil(n * 0.2 * x)       // FIRE_UP … AERO_UP
    n = Count(HEARTLESS_BUSTER 101); if status(victim)+0x7D == 1 && n > 0: x = Ceil((n * 0.3 + 1) * x)
    n = Count(NOBODY_BUSTER 102);    if status(victim)+0x7D == 2 && n > 0: x = Ceil((n * 0.3 + 1) * x)
    if sourceInterface.slot19() (vt+0x98): x = Ceil(x * 1.5)                                            // [?] a state of the instigator
    if !noBonus && info.fullMpBurst: n = Count(FULLMP_BURST 77); if n > 0: x += Ceil(n * 0.5 * x)
if Count(victim, CRITICAL_HALF 51) > 0 && HPPercent(victim) <= 25: x = Ceil(x * (Count > 1 ? 1/3 : 0.5))
return nz(x)                                                 // nz: 0 → 0.1
```

`DifficultyFactor` (0x140642820; sheet `sora-difficulty`, `docs/sora-stats-spec.md` section 3) [I]: an
attacker on the player side (actor kind 1 or 10, or a kind 3 / 5 / 11 actor owned by one): **×1.25 on
Beginner, ×1 on Standard, Proud and Critical**; any other attacker: ×0.5 / ×1 / ×1.5 / ×2. No random
number and no critical-hit roll. Difficulty also enters through the stats: the stat getters switch to
alternative base values (status+0x164 / +0x168 / +0x16C) for types 0 / 3 / 4 when `FUN_140EAB0E0(game,
0)`, the enemy base tables exist in variants (`*_BaseDataTable_s0 … _s5`), and `lvl` moves the floors [C].

`Strength(owner)` (0x140580E70) [C] = base (status+0x5C; from the level-up table and bonuses) +
boost byte (save object +0x980) (×3 when `flagG`) + armor and accessory strength + weapon strength
(×1.25 rounded up with ability 171) with a level-scaled floor for ability 179 (OVERCOME) and +1 for
ability 178 in a condition. Defense analogous (0x140581830) with DEFENDER (50) bonuses at low HP. The
level-99 test configuration (`sheets/port/sora-loadout-99.json`): **Strength 66** (level row 62 +
Kingdom Key 4), Magic 67, Defense 59.

For a port with foreign enemies: `amount = Ceil(max(STR − DEF, 1) × m_Power)` × resistance × difficulty,
then the ability terms that exist on the port's Sora; the type-dependent floors and caps apply only to
victims that carry a KH3 status object (`Defender::NONE` in `damage.rs`: Defense 0, no floors).
Level-99 numbers: section 15.

Corrected in revision 2 [I]: (a) revision 1 put the difficulty factor in the `else` of "victim has
status" and concluded "no difficulty multiplier inside the formula": the call at 0x140632865 is
reached from both paths (the no-status path jumps from 0x140632711 to 0x140632855 → 0x14063285D);
(b) revision 1 returned 0.1 for a RECOVER hit: the function keeps `max(A, 0)` in that case
(0x140632400–0x140632422) and returns it when `power > 0` — the heal amount is the caster's stat, the
victim side adds `Ceil(amount)` to HP (8.4). `docs/sora-stats-spec.md` section 7 step 1 has the same
error for RECOVER (`docs/sora-spec-reconciliation-2.md`); (c) the sixth argument (`flagOnly`) makes
every non-RECOVER call return 0.1; (d) the 80 cap of player / friend victims is skipped when
`lo >= 80`.

### 8.4 Victim side of `TakeDamage` (0x1405BB850) [C for the order, I where an address is given]

```
if HP (pawn+0x5FC, vt+0x17B0) < 1: return 0
amount = APawn::TakeDamage(...)                                     // stock [U]; 0 -> only an armor callback, return
info   = TresDamageInfo of the event (0x14064DD70)                  // absent (plain engine damage): apply = react = true
(hit, dir) = event.GetBestHitInfo(pawn, instigator)                 // event vt+0x18 [U]: dir = the event's shot direction
if info.kind == KILL (14):    amount = HP
if info.kind == RECOVER (13) [I 0x14064E770]: notify; SetHP(HP + Ceil(amount)); return
type  = info+0x60                       // byte: the VICTIM RESULT of 8.2.6 (ETresReactionResultType), preset DAMAGE (2)
apply = !IsNoDamage()   (pawn vt+0x2080)         // section 9.6
react = !IsNoReaction() (pawn vt+0x2070)
event = type != 0
if type == 1 (NOREACTION): react = false
if type in 3..6 (REFLECT, GUARD_REFLECT, GUARD, ARMOR_BREAK): apply = false
if info.kind == WITHOUT (12) [I 0x14064E7C0]: type = 1; react = false; apply = true; event = true
if info+0x50 bit 6: apply = false; event = false                    // bit 6 = armor damage, set by Classify on an armoured victim (8.7)
bReact = info+0x50 bit 2 (killer attack), else instigator-controller.vt+0x618()     // only used by the armor / revenge bookkeeping
if apply:
    amount = pawn.vt+0x20E8(amount)                                 // player: identity [I 0x14054B190]
    notify (pawn+0x448 vt+0xD8), ReceiveTresTakeDamage, delegate
    ApplyHP(pawn, causer, info, amount, bReact, force = 0)          // 0x1405A2A00, section 9.7  <-- the HP subtract
    pawn.vt+0xD40(Floor(amount))                                    // slot 424 0x141424360: damage statistics only
                                                                    //   (game instance 0x140E9E770), NOT the HP subtract
if event: pawn.vt+0x20D8(...)                                       // AI "fear" hook of the controller component
if HP < 1:
    if !Survive() (pawn vt+0x11F8, slot 575 0x14142F640): Die path 0x1405A35D0; return          // section 9.8
    survived = true; info.badStatus = none (0x14064CFD0)
if apply && !(type == 1 && vt+0x20B8()): revenge / armor bookkeeping 0x1405A3E70                 // player vt+0x20B8 = false
if !Freeze(info) (vt+0x20C0): attribute body effect 0x1405B77B0(pawn, 0, info.attribute)
if react:
    type 3 (REFLECT): iface vt+0x448(3, causer, hit)
    type 4 (GUARD_REFLECT): iface vt+0x458(...) else vt+0x448(4, ...)
    type 5 (GUARD): iface vt+0x470(...)               // the guarded-hit path of docs/sora-actions-spec.md II.1
    else:   bad status (0x1405A2350: clear status 12; if info.badStatus != 0 && !Immune(kind): apply (kind 7 is left to
            the gate), statistics); if iface slot 88 (reaction gate, 9.1)(info, amount, hit, dir, causer): BP event vt+0x20F0
else:       bad status only (0x1405B7C50), with iface slot 90 as its reaction
if survived: pawn.vt+0x1888(1)                                      // auto-revive item, section 9.8
```

Revision 1 placed the HP subtraction in slot 424 (0x141424360 → 0x1405C38B0); that call only reports the
damage number to the game instance. HP changes in `ApplyHP` through pawn slot 776 (0x1405B9700) [I].

The values of `info+0x60` are victim results, decided by the attacking side's `Classify` (8.2.6) from the body reaction
type of the hurt shape that was hit (Sora's shapes: 9.10) before `TakeDamage` is called; a hit that `Classify` blocked
(`noReact`) or whose amount is 0 does not reach `TakeDamage` at all (victim notify slot 28 instead, 8.2.5 step 20).

### 8.5 What the attacker gets from a hit

#### 8.5.1 On contact (owner notify slot 15) [I chain, C for the state handler]

Sora's notify interface is pawn+0x448 (vtable 0x14696E4B8). Slot 15 0x14140B8A0 → 0x141442F70 → base 0x140549A80:

```
pawn+0x6C8 = hit.ImpactPoint                                        // 0x1405B2710
current state's notify sub-object slot 15: Attack state 0x141341B10 [C]:
    kinds 0x0A / 0x19 with row KB_FLASHSTEP_END: result = NOREACTION
    result ∈ {REFLECT, GUARD_REFLECT, GUARD}:
        map hit: only if combo < 1 && no target && state.slot140(hit, info): recoil record (state+0x264 |= 1, +0x268 = 3, +0x270 = null)
        else: state+0x268 = result; state+0x264 |= 1; state.m_pReflectCauser(+0x270) = A
    else: pawn+0xE24 += 1; pawn+0xE28 = 1; state+0x13C |= 4; state+0x234 |= 0x2000
          kinds 0x21 / 0x23 / 0x24 / 0x25 hitting the target pawn with REVENGE_EX (7): arm the chase (state+0x278, +0x234 bit 19)
pawn slot 1287 0x1414471A0 [I]: if info.justGuard && pawn+0x1540: spawn that effect at the impact
```

A wall hit that is not repelled takes the last branch too: it counts as a landed hit for pawn+0xE24 / +0xE28 [C].

The map-hit recoil, state slot 140 (0x14133D4D0) [C, third pass; constants from the function]: with v = the impact point in
the actor's frame minus (0, 0, 50): `|yaw(v)| < 80 && |pitch(v)| < 80 && (pitch(v) < 40 || dot(Up, ImpactNormal) > −0.8660254)`.
A map hit gets attacker result REFLECT from `Classify` only while the clip's map-hit-reflect window is open
(`TresAnimNotifyState_AttackEnableMapHitReflect`: clips 1301, 1311, `M_1359`, `M_1363`, each 0.2333–0.3333 s [A]) and the
surface has no actor (BSP) or an actor whose `AActor::m_DefaultAttackHitResponse` is GUARD (1) or REFLECT (2) (8.2.6);
during that window the level is hit again even when it is already in the hit list (step 10 `reflectMap`: no second
effect). So: **the first swing of a combo, without a target, whose blade is in a wall in front of him between 0.2333 and
0.3333 s of the clip is repelled** (state 66, no causer). Host seam: `World::attack_hit_response(component)`. Port:
`combat/hit.rs` `attack_map_hit`, `map_recoil`, `PawnCombat::map_reflect`.

#### 8.5.2 On damage (causer notify slot 31) [I]

The victim's HP code (`ATresCharPawnBase` 0x1405BB850 → 0x1405A2A00, 8.4) ends by calling the damage causer's
`UTresNotifyInterface` slot 31 with `(victim, hpLost, &info)`. Sora: 0x141442DE0:

```
ctrl = pawn.slot297()                                               // player controller
pts = (float)info.m_FormPoint;  kind = info.commandKind
if hpLost > 0:
    ctrl.AddStylePoint(pts, 1, kind)                                // slot 848 0x1413E9000, section 10.2
    if kind == 0xFD (SHOOTFLOW) && info.shootFlowKind != 0: ctrl.slot808()
    if ctrl+0xA08 bit 1: hud = 0x141401180(ctrl): 0x14158A860(hud, victim, 0, victim.HP)      // target HP gauge
elif victim && 0x1405AD4A0(victim): ctrl.AddStylePoint(pts, 1, kind)                          // a class of victims that never loses HP [C]
if pawn.MyFocusPointAddHit (+0xD00) > 0: ctrl.AddFocus(MyFocusPointAddHit, pawn, 0)           // slot 817 0x1413E8CA0
base 0x1405B26B0 → state notify slot 31 (Attack state: empty)
```

- **Focus**: `AddFocus(n)` = `pawn.SetFocus(pawn.GetFocus() + n)` unless ctrl+0xA08 bit 0 [I]. `MyFocusPointAddHit` is
  0 in the constructor (0x141423D45) and not set by `p_ex001_PawnBase` / `p_ex001_Pawn` [A] → **Sora gains no focus
  from hits**.
- **Style points**: `AddStylePoint(pts, mode, kind)` = controller slot 848 0x1413E9000 is specified in full in 10.2.
  For a base-form keyblade hit: `pts = m_FormPoint × rate(level) × MyStylePointAddRate` (+20 % with Form Up), × the chain
  factor (1 for the first hit since the last gauge tick, then ½, ¼ …), added to the **pending** value ctrl+0xB44, which the
  gauge tick moves into the gauge ctrl+0xB40 (clamped to `MyStylePointMax` there). The call is made **only when the victim
  lost HP** (or belongs to the class of 0x1405AD4A0). Slot 899 0x1413E8D70 (called with the points) adds to ctrl+0xC10, the
  friend-gauge points (section 11.6), not to the style gauge.
- Nothing else: no MP, no EXP, no munny on a non-lethal hit in this path.

#### 8.5.3 Hit-stop [I]

There is none in the shipped game.

- The only hit-stop entry of the player pawn is slot 1374 0x141444090 (`kind` 0 → `MyHitStopTimeSmall` pawn+0x1574 =
  0.02 s, 1 → `MyHitStopTimeBig` +0x1578 = 0.05 s, passed to the game mode's slot 359). **No instruction in the image
  calls or jumps through vtable offset 0x2AF0** (exhaustive scan of the code section for `call / jmp [reg+0x2AF0]`,
  with and without REX), and the function has no direct callers (references are the 11 player-pawn vtables only).
  Slot 1375 0x141431C90 is the exec `DebugSetHitStopTime`.
- `ATresGameMode` slot 359 0x1415246C0 does nothing unless gameMode+0x4738 is set; that byte is zeroed in the
  constructor (0x14151981D) and written only by the exec `DebugEnableHitStop` (slot 358 0x141520180). When enabled it
  pauses the game through the game state (request id 0x59E) for the given time (tick 0x141520690).
- The attack components run on `dt × owner.slot204() × owner.CustomTimeDilation`; nothing in the hit path writes
  either.

The port must not add a hit-stop for Kingdom Key hits. (What reads as impact weight in play is the victim's reaction
animation and the hit effect.)

#### 8.5.4 Camera shake, rumble, HUD [I for the chain read, A for the clips]

None of the functions of 8.2.5, 8.5.1, 8.5.2 starts a camera shake or a force-feedback effect, and Sora's combat
clips contain no shake / force-feedback notify class (47 notify classes in `S5_anims_all.json`; 73
`TresAnimNotifyState_BpEvent` windows are the only unresolved carriers [?]). HUD effects of a hit: the style-point
accumulator (slot 899), the target HP gauge (0x14158A860), and on the victim's side `0x141584130(hud, victim, 0,
damage)` in 0x1405A2A00 [C].

### 8.6 Hit effect and hit sound

`HitEffects` 0x1406294F0 (called only from step 20, i.e. `result != 0` and not a repeated reflect-window map hit) [C,
argument list I 0x14061A158–0x14061A1A2]:

```
V = info.victimResult
if V == NONE && A.m_ObjType == ENEMY && noReact:            // blocked / armoured enemy
    e = pawnV ? (pawnV+0x460).slot79(ctrl, instigator, &info, hit) : null
    manager: e ? spawn e : spawn the default "no damage" effect (0x140646D60: `m_CmnInvincibleHitEffect` = `vs_com_mtk0` [A]), at ImpactPoint along ImpactNormal;  return
if V ∈ {GUARD_REFLECT, GUARD}:
    e = pawnV ? (pawnV+0x460).slot78(…) : null             // the guarding character's own guard effect
    manager: e ? spawn e : spawn the common guard effect for V (0x140646E20: V == REFLECT → `m_CmnStaggerReflectEffect` = `vs_com_yor00`,
             else `m_CmnGuardReflectEffect` = `vs_com_haj00` [A TresGameModeLoad]; the clash pass queues the same pair by its result,
             spawned in pass 3 0x140640F70; the guard-break effect of the chooser is `m_CmnIgnoreGuardHitEffect` = `vs_com_mukou`);  return
if instigator's actor interface slot 50 (victim, &info, hit): skip the chooser               // Sora 0x14143B430: calls slot 49, returns false
else: Chooser(comp, prim.effectGroup(+0xA30), comp+0x958, info.attribute, prim.hitSound(+0xA68), hit,
              result, prim is phys (+0x984 bit 7), info.critical, prim+0x987 bit 2, bit 3, &prim+0x988)
      if info.ignoreGuard && kind ∉ {CATCH, BIND} && the victim's actor interface slot 51 () == 0:
          manager: spawn the guard-break effect at the impact
if pawnV: d = pawnV.slot467(info.damageKind): the victim's own damage effect / sound for that kind (attached or at its root)
```

`Chooser` 0x140628960 [C; the twelve arguments confirmed I]: (component, effect group name, effect-set index,
attribute, hit sound, hit, attacker result, phys flag, critical flag, colour-override flag, alpha-override flag,
colour).

```
E = comp.m_HitEffects entry with m_GrpName == group (none → no effect, no entry sound)
U = (index valid for E.m_HitEffSet) ? E.m_HitEffSet[index] : E                 // same layout
isMap = 0x140643BD0(A, C)                                                       // A null; ObjType NONE / MAP / BG unless channel 7; GIMMICK by channel [C]
pos = hit.ImpactPoint (+ a random offset of length m_fHitEffectRandomOffsetRadius when > 0: three xorshift rolls → rotator)
rot = rotator of hit.ImpactNormal
effect = (isMap && U.m_bChangeMapHit) ? U.m_MapHitEffect
       : (attribute < 8 && U.m_PawnHitEffectPerAttr[attribute]) ?: U.m_PawnHitEffect
spawn effect at pos / rot (attached to C when the victim's actor interface slot 49 says so); colour / alpha overrides
if phys && !isMap && U.m_DirectHitEffect: spawn it too
if result != REFLECT (3):
    s = (!isMap || C simulates physics) ? hit sound : none
    c = (isMap && U.m_bChangeSEMapHit) ? U.m_MapHitSEAsset
      : (attribute < 8 && U.m_PawnHitSEAssetPerAttr[attribute]) ?: U.m_PawnHitSEAsset
    if c: s = c
    if s: PlaySoundAtLocation(s, pos, rotation 0, volume 1, pitch 1)
if isMap && U.m_bEnableDecal: decal from U.m_DecalData (random roll angle)
if critical: manager critical effect                                            // never: the flag is never set (8.2.5 step 16)
```

Kingdom Key [A `w_so010_Wep` `m_HitEffects`, entry `None`; D]:

| Hit | Effect | Sound |
|---|---|---|
| character / prop, PHYSICAL row | `vw_so010_hit0` + (phys shapes) `vw_so010_str0` | the window's hit sound: notify `m_HitSEAsset`, else `enumset_hit_w_so010[m_WeaponHitSEAliasID]` (HIT1–7 → `se02001_001`–`007`), else none |
| character, NOTYPE row (`KB_FLASH`, …) | `VS_non_HIT0` + `vw_so010_str0` if phys | same |
| character, elemental row | `vs_fir_hit0` / `vs_blz_hit0` / `vs_thd_hit0` / `vs_wat_hit0` / `vs_win_hit0` / `vs_dak_hit1` | same |
| wall (`KB_Stick` only) | `VW_SO010_WHT0` (`m_bChangeMapHit`) | **none** (`m_bChangeSEMapHit` false, no entry sounds, and the window's sound is dropped on a non-simulating map component) |
| guarded / blocked | the victim's or the common guard effect instead | none from the chooser (the guard effect carries its own, audio spec 5.5) |
| repelled first swing (`Att` = REFLECT) | chooser effect as above when `V` is not GUARD / NONE-blocked | none |

Sound sheets: `docs/sora-audio-spec.md` 5.4 has the precedence right for character hits; add the two conditions above
(no sound with attacker result REFLECT; the window's sound is not used on map hits). `sheets/port/sora-sound-rules`
route `notify.attack_hit_se` needs no other change (`docs/sora-spec-reconciliation-2.md`).

### 8.7 The hit record handed to a victim

`A.TakeDamage(float amount, FTresPointDamageEvent& event, AController* ctrl, AActor* causer)` — vtable +0x570, call
0x14061A1D5; `ctrl` = controller of the instigator, `causer` = the instigator actor (8.2.4). Built at
0x14061A035–0x14061A0AF [I]:

| Offset | Field | Value |
|---|---|---|
| +0x00 | vtable | 0x145D6FD28 (`FTresPointDamageEvent`; base `FPointDamageEvent` 0x145D6D868) |
| +0x08 | `DamageTypeClass` | manager's class for the row's `m_PhysForceLv` (0x140642B20), kept only if a `UTresDamageType`; else null |
| +0x10 | `Damage` f32 | `amount` (8.3; never 0 when damage is dealt: the formula returns 0.1 for 0) |
| +0x14 | `ShotDirection` vec3 | knockback direction, below |
| +0x20 | `HitInfo` `FHitResult` (0x88) | the sweep hit: +0x04 Time, +0x0C Location, +0x18 ImpactPoint, +0x24 Normal, +0x30 ImpactNormal, +0x64 Actor, +0x6C Component, +0x74 BoneName |
| +0xA8 | `TresDamageInfo` (0x68) | copy of the prim's, edited in steps 16–18; its `m_KnockbackDir` = the same direction |

`TresDamageInfo` as delivered (source: row field unless said) [I 0x140645900 + steps above]:

| Offset | Field | Source |
|---|---|---|
| +0x00 | `m_AttackDataIDName` | row name |
| +0x08 | `m_CommandKind` | pawn slot 959 at activation (if > 0 and ≤ 0xFD) |
| +0x09 | `m_ShootFlowKind` | pawn slot 960 (if > 0 and ≤ 0x2C) |
| +0x0A / +0x0B | `m_DamageKind` / `m_DamageAttribute` | row |
| +0x0C | `m_AttackSrcPower` | `m_Power` |
| +0x10 / +0x14 / +0x18 / +0x1C | power scale / parabola angle / move length / brake | row |
| +0x20 | `m_DamageEffectTime` | row |
| +0x24 | `m_KnockbackType` | row |
| +0x28 | `m_KnockbackDir` | computed per hit |
| +0x34 / +0x38 / +0x3C | bad status kind / time / param | row; cleared (kind 0, time 0, param 0) by a failed `m_BadStatusGenRate` roll, by a guard, by `m_bDisableTakeDamage` |
| +0x40 | byte | 0, then owner base-param byte +0x7D |
| +0x44 / +0x48 / +0x4C | `m_ReactionPower` / `m_ArmorAttackPower` / `m_FormPoint` | row |
| +0x50 | bit 0 magic, 2 killer, 3 ignore guard (`m_bIsIvalidGuard`), 4 rapid fire | row |
| | bit 1 finish | Attack state `finishEnd` |
| | bit 5 critical | never set |
| | bit 6 armor damage | set by `Classify` on an armoured victim |
| | bit 7 aerial | Attack state air mode |
| +0x51 | bit 0 attraction, 1 counter, 3 weak-guard attack | row |
| | bit 2 just guard | Attack state just-guard attack |
| | bit 4 full-MP burst | activation flag (magic) |
| | bit 5 magic draw | attacker has MAGIC_DRAW and the row is magic |
| | bit 6 no-react body correction | `Classify`, body type NO_REACTION |
| +0x52 / +0x53 / +0x54 / +0x55 | four counters, each clamp 0..99 | Attack state +0x128 (combo count) / +0x12C / +0x130 / +0x134. **The damage formula's "combo step" is +0x54 = state+0x130**, the value state+0x12C had at `Begin` |
| +0x58 | `m_RevengePoint` f32 | row |
| +0x5C | i32 | armour value left, written by the victim's slot 70 |
| +0x60 | victim result (`ETresReactionResultType`) | `Classify` |
| +0x61 | attacker result | `Classify` |
| +0x62 / +0x63 | `m_ReflectReaction` / `m_ReflectLevel` (i8) | row; level forced to 0 after the first swing of a combo |
| +0x64 | `m_PhysForceLv` | row |
| +0x65 / +0x66 | damage calc type / survive rule | 0 for the keyblade (activation S+0x69 / +0x6A otherwise) |

Knockback direction (`KnockbackDir` 0x14061EA30; dispatch I 0x14061EB4C–0x14061EB8C, bodies C; all three-dimensional,
not flattened; `norm(v)` = v / |v|, the zero vector when |v|² < 1e-8; "victim" = A's root location, or C's location
when A is null):

| `m_KnockbackType` | Direction |
|---|---|
| 0 NORMAL | `norm(victim − O)`, O = root location of comp+0x890 actor ?: instigator ?: owner |
| 1 VELOCITY | `GetVelocity()` of comp+0x890 actor ?: owner — not normalised |
| 2 ATK_COLLISION | `norm(victim − P)`, P = shape centre at the hit time: `lerp(prevCentre, centre, clamp(hit.Time, 0, 1))` (ring / laser shapes: centre) |
| 3 INSTIGATOR_PAWN | `norm(victim − Q)`, Q = root location of the pawn of the instigator's controller; without one, as NORMAL |
| 4 IMPACT_LOCATION | `norm(victim − hit.ImpactPoint)` |
| 5 SPECIFIC | (1, 0, 0), then the notify interface of comp+0x890 actor ?: instigator ?: owner, slot 29 `(dir, victim)` may overwrite it (Sora's: forwards to the state) |

The initial value before any case is (1, 0, 0) (0x14871E3B0). 18 `kb_*` rows are INSTIGATOR_PAWN, the other 77 rows
used are NORMAL [A]; for Sora both give **the unit vector from Sora's root to the victim's root, including its
vertical component**.

What comes back: `TakeDamage`'s return value is ignored. The attacker learns the outcome through `info+0x61` (decided
before the call, 8.2.6) and, when HP was lost, through notify slot 31 (8.5.2). The veto (victim notify slot 25) is the
victim's only way to make a hit not happen.

### 8.8 Shots of the attack state (`TresAnimNotify_Shot`, the dive's landing) [C 0x141344580, verdict same-partial; third pass 2026-10-12]

The Attack state's shot handler is anim-interface slot 69
(vt+0x228). Callers: the notify `TresAnimNotify_Shot` (0x140561B70), the dive's landing in the traction update
(0x14134A7E0 / 0x14134A8D5, each after `state+0x234 |= 0x20`), and the on-contact handler for kind 0x37.

```
state+0x1F4 &= ~0x10 … ;  needs a player pawn that may shoot (pawn.vt+0x1330()) and a player controller
index = notify parameter (0 / 1; else 0);  team = pawn.vt+0xD90();  kind = pawn.vt+0x1DC8()
switch kind:
    0x0E: if (state+0x234 & 0x60) != 0x60: state+0x234 |= 0x40; return          // needs the landing bit (5) AND one earlier call (bit 6)
          name = "DOWNIMPACT"
    0x25: name = "BARRIERCRACKER"                                                // formchange kinds only
    0x37: if (state+0x234 & 0x60) != 0x60: state+0x234 |= 0x40; return
          if state+0x170 (shots so far) > 0: return
          name = "SHOCKWAVE_S"
    0x3F..0x41 / 0x42: state slots 137 / 138 (unison shots);  0x76, 0x77, 0xB9, 0xD0: other forms
    default: return                                                              // 0x0D, Magic Flash 7, Final Heaven 0x0C: no shot
location = pawn.vt+0x1410(index)   // 0x1405AA790 → equipment vt+0x580 0x1417812E0 [C]: weapon m_WeaponEquips[index] (index 0 when
                                   // out of range): its mesh (weapon+0x570) socket `MuzzleAttachPoint` (weapon+0x5A0, class default
                                   // "ATK_01", the keyblade's tip); no weapon: the actor location.   0x37: + Forward × 100
shot = pawn.vt+0x1450(name, location, location, Forward(actor), team, 0)         // an actor of p_ex001_ProjectileSet
shot.vt+0x1130(state+0x128); shot.vt+0x1140(state+0x12C, +0x130, +0x134); shot.vt+0x1120(state+0x141, 0); shot.vt+0x1180(state+0x23C & 1)
state+0x170 += 1
```

| Shot | Blueprint [A] | Life | Shape (its own `TresAtkColl0`) | Row | Particle |
|---|---|---|---|---|---|
| `DOWNIMPACT` | `/Game/Blueprints/Projectile/TresProj_DownImpact` | 0.5 s, not destroyed by a hit, no movement | SPHERE, radius 50 + 800 /s up to 200, no map hits, not physical, no interval | `KB_DOWN_BOMB` (0.4, BLOW) | `vp_hit_zmn0` 75 cm under its root |
| `SHOCKWAVE_S` | `/Game/Blueprints/Projectile/TresProj_SonicJumpEx` | 0.5 s, not destroyed by a hit; speed 100 with homing from 0.1 s (not ported) | SPHERE, radius 100 + 800 /s up to 350 | `FF_SONIC_BOMB` (1.5, BLOW) | `vp_snc_hit0` |

Timing [D]: clip 1318 (kinds 0x0D / 0x0E) has its Shot notify at 0.6667 s; the finisher air-down's shockwave appears at the
later of "landed" and that notify. Kind 0x0D never spawns it. Sonic Cleave (0x37, clip 1182, notify at 0.8667 s): at its
first landed hit with combo 0 the on-contact handler (0x14134208A) sets both bits and calls the handler, else as the dive.
Fourth pass: with state+0x234 bit 3 (4.3) Sonic Cleave never reaches the dive's landing inside its traction window, and the
window-end shot of 6.3 needs state+0x23C bit 5, which no Kingdom Key kind sets: a cleave that hits nothing spawns no shot.
Port: `states/attack.rs` `shot_handler`, `combat/hit.rs` `Shot` / `spawn_shot` / `shot_origin` (socket `ATK_01` of the playing clip).

---

## 9. Sora being hit

### 9.1 Reaction gate (iface slot 88, 0x141439A50) [I: whole function read]

Arguments: `(info, amount, hit, dir, causer)`. Returns true when the hit was "taken as a reaction" (also when it
was swallowed on purpose).

```
if !iface.slot87() (0x1414277F0): return false        // current state == DIE (59), or next queued state is CATCH (61) / BIND (62)
kind = info+0x0A; attr = info+0x0B; carried = 0
switch current state id:
    DAMAGE (19):            cur.+0x124 = attr; carried = cur.+0x140        // hit counter survives the re-entry (juggle)
    AF_ROLLER_COASTER (26): return true                                    // swallowed
    SHOOTERMODE (40) && kind == SMALL: ShooterMode.iface2.vt+0x80(1, attr, ...); return true
    FR_BAYMAXRIDE (137), PLANEBOSS (148): return cur.vt+0x388(same args)   // the state's own reaction
    RAILSLIDE_MI (138):     return cur.vt+0x3E8(same args)
    HOLD_CAMERA (144):      if HasAbility(98 BATTLE_GRAPHER) && kind == SMALL && info.badStatus == 0: return true
if ctrl:
    if ctrl+0x8F4 > 0: return false                    // reaction-block timer, section 9.6      [I 0x141439D9A]
    if ctrl+0x878 bit 8: FUN_1414142D0(ctrl, 0); clear bit 8               // drops a pending just-guard flag
if pawn+0xE60 bit 17 (vt+0x2788): return false                             // command-lock restriction
if info.badStatus == FREEZE (7) && info.time(+0x38) > 0 && !Immune(7):     // pawn vt+0x20C0 0x1405ADF20
    st = (cur is BADSTAT_FREEZE 123) ? cur : ClearQueue+SetState(123);  if st: st+0x88 = info.time; return true
if info.badStatus == BURN (0x11) && info.time > 0 && !Immune(0x11) && kind == NONE:     // vt+0x20C8 0x1405ADEF0
    if cur != BADSTAT_BURN 124: ClearQueue+SetState(124);  return true
if kind in {0, 1, 5, 6, 9} or kind >= 11:              // NONE, SMALL, BILLIARD, FLOAT, BEAT, PARABOLA_NO_RECOVERY and above  [I 0x141439ED9]
    if iface.slot195() (0x141436AA0): return false     // no flinch: JUMP with Jump+0xF8 != 0, states 21..25
                                                       //   (SUPER_SLIDE, RAIL_SLIDE, WALL_KICK, POLE_TURN, ENEMY_FLOW), RAILSLIDE_MI
    if cur is an Attack state && attack kind (pawn vt+0x1DC8) in 0x36..0x39: return false
ClearQueue (0x1405A7FF0, 0x1405A8010)
st = PushState(DAMAGE 19) (0x1405B4F80)
if st: st+0x140 = carried; st.Setup(info, amount, hit, dir, causer)        // slot 114
return true
```

Kinds 2, 3, 4, 7, 8, 10 (BLOW, SIDESPIN, VERTICALROLL, TOSS, LAUNCH, PARABOLA) always break through. A hit during
DAMAGE re-enters DAMAGE: there is no invincibility while Sora is being juggled, lying down or getting up.

### 9.2 State fields [I for the offsets used below]

| Offset | Type | Meaning |
|---|---|---|
| +0x78 | i32 | sub-step (`m_nSubStep`) |
| +0x84 bit 0 | | `m_bEnableChange` (notify `TresEnableChange`, or forced by a handler) |
| +0x88 | i32 | **damage kind** (copy of info+0x0A after the dead-conversion); slot 113 0x1413514C0 returns it |
| +0x8D | u8 | stick active this frame: written 1 by `MoveInput` (0x141440B9E), cleared every Tick |
| +0x8E | u8 | "recoverable": Aerial Recovery may be requested |
| +0x8F | u8 | wall hit armed |
| +0x90 | u8 | a bad status (freeze / stun / burn) is on the pawn: no attack out of the state, state ends at the first exit point |
| +0x98 … +0xFF | TresDamageInfo | copy of the hit's info (+0xA2 kind, +0xA3 attribute, +0xA8 power scale, +0xAC parabola angle, +0xB0 move length, +0xB4 brake, +0xB8 effect time, +0xCC bad status, +0xD0 its time) |
| +0x100 / +0x124 | u8 | attribute for the body effect (current / to switch off in End; Reset writes 8) |
| +0x104, +0x110 | vec3 | hit direction raw / normalised in XY (zero when shorter than 1e-4) |
| +0x11C, +0x1C4 | weak ptr | causer / instigator pawn (a weapon is replaced by its owner) |
| +0x128 | f32 | info.effectTime |
| +0x12C | f32 | time since the hit (Tick adds dt) |
| +0x130 | f32 | handler timer (Tick subtracts dt, floor 0) |
| +0x138 | f32 | recover delay: **0.2** (0.46666 when +0x1C0 bit 3) [I 0x141366025] |
| +0x13C bit 0 | | the hit killed (HP < 1 at Setup) |
| +0x140 | i32 | consecutive-hit counter (carried by the gate); +0x1A0 = its low bit before the increment = clip variant |
| +0x148 | FName | clip being played |
| +0x150 | vec3 | actor position at Setup |
| +0x15C | vec3 | knock direction |
| +0x168 | rot | facing held during the flight |
| +0x174, +0x178, +0x17C | f32 | power (XY speed), brake, vertical speed |
| +0x190 | i32 | direction index (9.3) |
| +0x1A4 | f32 | freeze time (handler 122) |
| +0x1C0 | u32 | bit 0 was Flying at Begin, bit 1 brake every tick, bit 2 **landed** (set by event interface slot 75 0x141356DA0 [I]; the meaning "landed" is inferred from its use [C]), bit 3 long recover delay (no writer found in 0x140400000–0x141700000: never set) |
| +0x1D0 | delegate | the kind's handler |
| +0x210 | u8 | EAT clean-up needed |

### 9.3 Lifecycle

**Reset** (slot 95 0x141352FF0) [C]: clears flags, timers, +0x140, +0x90; +0x124 = 8; +0x1B0 = 1.0.

**Setup** (slot 114 0x141365930) [I for the parts quoted with addresses, C otherwise]; runs inside the gate, i.e. in the
frame of the hit:

```
copy info -> state+0x98
+0x13C bit0 = 0
if HP < 1: +0x13C |= 1; if info.kind == SMALL: info.kind = BLOW; info.powerScale = 1.0; info.brake = 0     // [I 0x1413659CC]
+0x88 = info.kind; +0x100 = info.attribute; +0x104 = dir
+0x110 = normalize2D(dir) (z = 0)                       // exactly zero when |dir.xy|^2 < 1e-8
causer weak ptr; +0x1C0 &= ~6 (brake, landed)
+0x1A0 = +0x140 & 1; +0x128 = info.effectTime; +0x140 += 1; +0x8D = +0x8E = 0                              // [I 0x141365B52]
instigator = causer (weapon -> its owner pawn)
if +0x110 == 0: +0x110 = normalize2D(forward of the instigator)         // hits without a direction push along the attacker's facing
StopMovement (pawn vt+0x16E8); SetActorRotation(0, current yaw, 0)
FUN_14135FF70: restore MaxWalkSpeed / (+0x390) / GravityScale from the class defaults, gravity on
+0x150 = actor location; sub-step = 0
+0x138 = (+0x1C0 bit 3) ? 0.46666 : 0.2
if info.badStatus == FREEZE && info.time > 0 && !Immune(7):             // only reached when the gate could not start state 123
    handler = slot 122; +0x174 = SmallPower(); +0x1A4 = info.time
else:
    attribute body effect 0x1405B77B0(pawn, 0, attribute)
    if pawn bad status (pawn+0x85D) in {7 freeze, 0x11 burn}: +0x90 = 1;  12 (stun): effect 0xC58, +0x90 = 1
    if +0x90: FUN_141413910(ctrl, 1, 1)                                 // HUD
    handler = table below                                              // [I jump table 0x1413662A8 / 0x1413662CC]
    if kind != SMALL: Sheathe(1, 1.0) (pawn vt+0x1358)
handler(dt = world DeltaTimeSeconds (state vt+0x140 object +0x948))    // sub-step 0 runs at once [C]
```

| Damage kind | Handler | Slot | Address |
|---|---|---|---|
| 2 BLOW, 21 SHIELD_BLOW | Blow | 117 | 0x14135B300 |
| 7 TOSS, 22 SHIELD_TOSS | Toss | 118 | 0x14135EEA0 |
| 8 LAUNCH | Launch | 119 | 0x14135C410 |
| 10 PARABOLA | Parabola | 120 | 0x14135CAB0 |
| 9 BEAT | Beat | 121 | 0x14135A870 |
| 19 SUCTION | Suction | 123 | 0x14135E260 |
| 20 EAT | Eat | 124 | 0x14135BDA0 |
| 41 PUDDING_EAT | PuddingEat | 125 | 0x14135D680 |
| every other value (0 NONE, 1 SMALL, 3–6, 11–18, 23–40: SIDESPIN, VERTICALROLL, PARABOLA_NO_RECOVERY, SWIRL_UP, the binds …) | Small | 116 | 0x14135D740 |
| (freeze fallback) | Frozen | 122 | 0x14135B150 |

**Begin** (slot 96 0x14134F750) [C]: `pawn.SetLastAnimPose(1)` (vt+0x1780); comp vt+0xBD0; +0x1C0 bit 0 = `IsFlying`;
`pawn.vt+0x16D8(0)`; +0x210 = 0.

**Tick** (slot 97 0x1413553F0) [I]:

```
if handler(dt) returns true (slot 115 wrapper 0x141350F00; no handler bound = true):
    (pawn+0x448).vt+0x248(info)                                // "reaction ended" notification
    if +0x90 && pawn bad status == BURN (0x11): PushState(BADSTAT_BURN 124)
    DoFinish
else:
    if +0x130 > 0: +0x130 = max(+0x130 - dt, 0)
    +0x12C += dt; +0x8D = 0; if +0x1C0 bit 3: +0x8E = 0
```

**End** (slot 98 0x141350C20) [C]: `FUN_141413910(ctrl, 0, 0)` unless +0x90; `FUN_14135FF70` (speeds, GravityScale, gravity
on); `pawn.vt+0xDC8(1.0)` (pitch / roll level); `pawn.vt+0x16D8`; attribute body effect off (+0x124); if the next state is
an Attack state: `next+0x234 |= 0x40000`; if the weapon was drawn before (pawn vt+0x1360) and the next state is not an
attack: `Draw(1, 1, 1.0)`; EAT clean-up (0x141350060) when +0x210.

**Helpers used by the handlers** [I]:

| Call | Is |
|---|---|
| `OnGround` 0x1405AD850 | `!(pawn+0xD80 & 1)` and movement mode Walking / NavWalking |
| `IsFlying` 0x1405AD7C0, `IsSwimming` 0x1405AD820 | movement mode 5 / 4 |
| `GravityOff()` pawn vt+0xB10 | comp+0xA3C bit 1 |
| `Gravity(b)` pawn vt+0x1728 | comp+0xA3C bit 1 = !b |
| `GravityScale(x)` pawn vt+0x1650 | comp+0x2F8 = x (0x1405B9690); vt+0x1658 restores the class default |
| `SetVel3(v)` pawn vt+0x16B0 | comp slot 317 0x140592DD0: record (+0xC0C…+0xC14) = v, flags +0xC18 \|= 3, `Velocity` = v |
| `SetVelXY(v)` pawn vt+0x16C0 | comp slot 319 0x140592E10: record.xy, flags \|= 1, `Velocity.xy` |
| `SetVelZ(z)` pawn vt+0x16D0 | comp slot 321 0x140592E40: record.z, flags \|= 2, `Velocity.z` |
| `StopMovement` pawn vt+0x16E8 | 0x140530DD0: `Velocity` = 0, record = 0, flags &= ~3 |
| `Turn(rot, alpha)` pawn vt+0xDB0 | 0x1405DDD90, as in the other specs; every handler uses alpha = dt × 30 |
| `Play(name, blendIn, mode)` | state+0x40 vt+0x18 `(name, 0, blendIn, mode, effectGroup, 1.0, …)`; −1 = default blend; returns the length (≤ 0 = clip missing → the handler finishes) |
| `AnimEnd` | state+0x40 vt+0xC8 |
| `Brake(b)` 0x140431E10 | `v = GetVelocity() × b^(dt × 60)`; `SetVelXY(v)` |

The direct-velocity record is consumed **once** at the end of the next `CalcVelocity` [I 0x14057A286: bit 0 → `Velocity.xy` =
record.xy, clear bit 0; bit 1 → `Velocity.z` = record.z, clear bit 1]. So while a handler rewrites it every tick (brake flag,
+0x1C0 bit 1) the horizontal speed is exactly `v × b^(dt × 60)` per tick whatever friction did; when it is written once, the
ordinary `CalcVelocity` braking acts from the second movement tick on (air, no input: 2048 cm/s², movement spec 4.4).

**Direction index** (pawn vt+0x23F0 0x1405A94E0) [I]: `Δ = Normalize180(yaw(−d) − actorYaw)` with d = +0x110:

| Δ | Index | Attacker is |
|---|---|---|
| −45 ≤ Δ ≤ 45 | 0 | in front |
| 45 < Δ ≤ 135 | 2 | to the right |
| −135 ≤ Δ < −45 | 3 | to the left |
| otherwise | 1 | behind |

Two-way variant (vt+0x23F8 0x1405A9790) [I]: `−90 ≤ Δ ≤ 90` → 0, else 1. With d == 0 the index is 0 and the knock direction is
`−forward`. The facing held during a flight is `yaw(−knock)` for index 0, 2, 3 and `yaw(knock)` for index 1 (Sora is turned
front or back to the attacker; the side clips are played on that facing).

**Parameters.** Slot 126 (0x1413513D0) returns the class default object of the game mode's `m_PlayerDamageCommonParam`
[C] = `/Game/Blueprints/Player/PlayerDamageCommonParam` (a blueprint child of `TresEnemyCommonParam` : `TresDamageCommonParam`).
Values = native constructor 0x14064C130 [I] overlaid with the asset [A] (sheet `sora-combat-damage-params`). Getters (all [I]; `scale` = info.powerScale state+0xA8):

| Slot | Address | Returns | Player value |
|---|---|---|---|
| 127 | 0x1413518D0 | `m_SmallDamagePower` (+0x40) × scale | 550 × scale |
| 128 | 0x141351860 | info.brake if > 0, else `m_SmallDamageBrake` (+0x44) | 0.9 |
| 129 | 0x141351950 | `m_SmallDamageStartFallingSpeed` (+0x48) | 150 |
| 130 | 0x141351010 | OnGround ? 0 : `m_AirSmallDamageVelocityZ` (+0x4C) | 0 |
| 131 | 0x141351270 | **`m_AirBlowDamagePower` (+0x60)** × scale — ground and air alike; `m_BlowDamagePower` (+0x54, 1500) is not read by the player state | 2000 × scale |
| 132 | 0x141351340 | OnGround ? `m_BlowDamageVelocityZ` (+0x50) : `m_AirBlowDamageVelocityZ` (+0x5C) | 300 / 300 |
| 133 | 0x1413511D0 | info.brake if > 0, else OnGround ? `m_BlowDamageBrake` (+0x58) : `m_AirBlowDamageBrake` (+0x64) | 0.95 / 0.95 |
| 134 | 0x14043A5D0 | wall-hit angle, hard-coded | 30° |
| 135 | 0x14043A5E0 | wall-hit minimum speed, hard-coded (the asset's `m_BlowDmgWallHitSpeed` 350 / `Angle` 60 are not read) | 50 |
| 136 | 0x141350F90 | OnGround ? 0 : `m_AirBlowDamageStartBrakeTime` (+0x68) | 0 |
| 137 | 0x1413512E0 | `m_BlowDamageStartFallingSpeed` (+0x6C) | 700 |
| 138 / 139 / 140 | 0x141351A40 / 0x1413519E0 / 0x141351980 | `m_TossDamagePower` × scale / `FlightDuration` / `FallGravityScale` | 1500 × scale / 0.2 / 1.0 |
| 141 / 142 / 143 | 0x141351590 / 0x141351530 / 0x1413514D0 | `m_LaunchDamage…` same three | 2500 × scale / 0.2 / 1.0 |
| 144 | 0x1413517E0 | `m_ParabolaDamagePower` (+0x98) × scale | 1500 × scale |
| 145 | 0x141351690 | info.parabolaAngle if > 0, else `m_ParabolaDamageAngle` | 45° |
| 146 | 0x141351710 | info.brake if > 0, else `m_ParabolaDamageBrake` | 0.98 |
| 147 | 0x141351780 | `m_ParabolaDamageLimitBrake` | 300 |
| 148 / 149 / 150 | 0x141351090 / 0x141351110 / 0x141351170 | `m_BeatDamagePower` × scale / `Rebound` / `ReboundGravityScale` | 2500 × scale / 1000 / 3.0 |

**Move-length start** (0x14135FD80, called at the end of every sub-step 0) [C]: only when info.moveLength (+0xB0) > 0:
`u = normalize(power × knock, with z replaced by +0x17C when that is non-zero)`; if u.z > 0: gravity off, mode Falling;
`SetVel3(u × moveLength × 60)`. Rows that give no move length (the usual case) skip it.

**Common exits** (the "tail" of the Small handler; the get-up sub-steps of the others use the same two tests) [I 0x14135E188]:

```
if m_bEnableChange:
    if nothing queued && (IsSwimming || in water (pawn vt+0x1A80)): if PushState(SWIM 102): StopMovement; finish
    if +0x8D (stick) || +0x90 (bad status): StopMovement; finish
```

### 9.4 Handlers

Clip tables on the pawn [C, strings]: `Small(index, variant)` = pawn vt+0x1CB0 0x141435200: on the ground `0201` / `0202` /
`0203` / `0204` for index 0–3, with `0205` / `0206` instead of `0201` / `0202` when variant = 1 (every second consecutive
hit); in the air `0207`–`0210`. `Blow(index, phase)` = vt+0x1CB8 0x1414333A0: `0211`+3×index+phase (phase 0 start, 1 loop,
2 down and get-up). `Wall(index)` = vt+0x1CC8 0x141433F10: `M_0223`–`M_0226`. `LandClip(pose, weaponOut)` = vt+0x1C98,
fallback `Land`.

#### Small (slot 116 0x14135D740) [I]

```
sub 0: index / knock as in 9.3;  power = SmallPower();  velZ = AirSmallVelZ();  MoveLengthStart;  sub = 1;  -> tail
sub 1: if DefaultLandMovementMode == Walking && IsFlying: Gravity(on); SetMovementMode(Falling)
       clip = Small(index, variant)
       if !OnGround:
           Gravity(off)
           if !IsFlying:
               SetVel3(power × knock.x, power × knock.y, velZ);  Play(clip, -1, 2);  sub = 4;  return        // no brake flag
       // on the ground (or Flying):
       SetVelXY(power × knock);  Play(clip, -1, 2);  +0x1C0 |= 2 (brake);  sub = 4;  return
sub 4: Turn(facing(index), dt × 30)
       if GravityOff():
           if !OnGround && !IsFlying && !IsSwimming: SetVelZ(Velocity.z × SmallBrake()^(dt × 60))
           if |Velocity.xy| <= SmallStartFallingSpeed(): Gravity(on)
       if OnGround && clip != Small(index, variant):                    // landed while an air clip plays
           if m_bEnableChange || AnimEnd:
               Play(LandClip(land pose, weapon out) or "Land", -1, 2);  m_bEnableChange = 1;  sub = 5
       elif AnimEnd: finish                                            // (enemies on Walking default go to a fall loop instead)
       -> tail
sub 5: if AnimEnd: finish;  -> tail
tail:  common exits;  if +0x1C0 bit 1: Brake(SmallBrake())
```

Numbers [D from I / A values, 60 Hz]: ground hit: 550 × scale cm/s, × 0.9 per frame → 91.7 × scale cm of slide in total (half
of it in the first 7 frames); clips `0201`–`0206` 1.1667 s with `TresEnableChange` at 0.4667 s: from 0.4667 s Sora can attack,
jump, dodge (the request gates), and a held stick ends the state; untouched it ends at 1.1667 s. Air hit: gravity off, 550 ×
scale horizontally with the ordinary 2048 cm/s² air braking until the speed is ≤ 150 (0.195 s, 68 cm at scale 1), then
gravity; clips `0207`–`0210` 0.6 s, `TresEnableChange` 0.4667 s; landing during the clip switches to the landing clip at the
change point.

#### Blow (slot 117 0x14135B300) [I]

```
sub 0: index (4-way; 2-way when the hit killed or pawn+0x5F0 in {2, 3, 7}); knock
       power = BlowPower(); brake = BlowBrake(); velZ = BlowVelZ(); MoveLengthStart; sub = 1
sub 1: if the instigator is gone: finish
       Gravity(off);  clip = Blow(index, 0);  SetMovementMode(Falling)
       SetVel3(power × knock.x, power × knock.y, velZ);  Play(clip, -1, 2)
       +0x1C0 |= 2;  +0x130 = AirBlowStartBrakeTime() (0);  sub = 2;  return (no brake this tick)
sub 2: +0x168 = facing(index);  sub = 3;  (falls into 3)
sub 3: Turn(+0x168, dt × 30)
       if clip == Blow(index, 0):                                   // the START clip is still the one playing
           if !AnimEnd: break                                       // [C 0x14135B300 case 3: `if (!IsEnd) break;`] nothing else this tick
           clip = Blow(index, 1); Play(clip, -1, 3 loop)
       if +0x130 <= 0: +0x130 = 0.6;  if pawn+0x15B4 != 0x12: +0x8F = 1 (wall hit armed);  sub = 4
sub 4: Turn;  if +0x12C > +0x138: +0x8E = 1                         // recoverable from 0.2 s after the hit
       if GravityOff():
           SetVelZ(Velocity.z × brake^(dt × 60));  if |Velocity.xy| <= BlowStartFallingSpeed() (700): Gravity(on)
       elif +0x130 <= 0:                                            // at least 0.6 s of flight
           if !landed (+0x1C0 bit 2): if actor.z < startZ(+0x158) - 50: sub = 6; finish     // dropped off an edge: free fall
           else: if +0x1C0 bit 0: Gravity(off); SetVelZ(0)
                 if floating (pawn+0xD80 bit 0): sub = 6; finish
                 clip = Blow(index, 2); Play(clip, -1, 2); sub = 5; +0x8E = 0
sub 5: Turn;  if AnimEnd: finish;  if +0x90 || (m_bEnableChange && +0x8D): StopMovement; finish
sub 6: finish
sub 10 (wall hit): SetActorRotation(+0x168);  if AnimEnd: finish
every tick (subs 3, 4, 5): if +0x1C0 bit 1: Brake(brake)
```

Numbers [D, scale 1]: 2000 cm/s horizontally × 0.95 per frame (asymptotic distance 667 cm). Order of sub-step 3 (corrected
2026-10-12): the start clip (0.1 s) plays out first - during it the horizontal brake runs but the Z speed stays 300 with
gravity off - then the loop starts and sub-step 4 brakes Z (× 0.95 per frame) until the horizontal speed is ≤ 700 (frame 21
after the hit), then gravity: the rise is **92 cm** (30 cm during the start clip + 62 cm braked; measured in the port at
60 Hz), not the 66 cm of revision 2, which braked Z from the first frame. The landing is only acted on from
0.6 s after the loop began; clips: start 0.1 s, loop 0.3333 s, down / get-up `0213` / `0216` 0.8333 s, `0219` / `0222` 0.9333 s,
all with `TresEnableChange` at 0.6 s. The horizontal brake keeps running through the get-up.

**Wall hit** (event interface slot 9 0x141356930, called with the blocking hit) [C; thresholds I]: needs +0x8F; horizontal
speed ≥ 50; `knock · hitNormal ≤ −cos(30°)`. Then: facing from the wall normal (index 1 faces the wall), brake flag off,
`StopMovement`, gravity off, `Play(Wall(index), -1, 2)` (0.7 s), +0x8E = +0x8F = 0, sub = 10; at the end of the clip the state
finishes and End restores gravity. Armed in Blow sub 3 → 4 and Parabola sub 3 → 4.

#### Toss (slot 118 0x14135EEA0) and Launch (slot 119 0x14135C410) [I]

Identical except where marked.

```
sub 0: power = TossPower() | LaunchPower();  Launch: if info.effectTime (+0x128) > 0: power = effectTime
       +0x174 = +0x17C = power;  MoveLengthStart;  sub = 1
sub 1: Gravity(on);  SetMovementMode(Falling);  SetVelZ(power);  GravityScale(3.0)
       Play("M_0227", -1, 0);  sub = 12
sub 12: if AnimEnd: Play("M_0228", -1, 1 loop);  sub = 13
sub 13: recoverable if +0x12C > +0x138;  if Velocity.z <= 0: GravityScale(0.25);  +0x130 = FlightDuration (0.2);  sub = 14
sub 14: recoverable;  +0x130 -= dt  (Tick subtracts dt as well: the 0.2 lasts 0.1 s);  when <= 0:
        Play("M_0237", -1, 1 loop);  Gravity(on);  GravityScale = FallGravityScale (1.0);  +0x130 = 1.0 | Launch 1.2;  sub = 15
sub 15: recoverable
        if landed: StopMovement
                   if !floating: if a rail is under Sora (probe pawn vt+0xEA0, iface slot 75): finish
                                 else Play("0213", -1, 0);  sub = 6;  +0x8E = 0
                   else finish
        elif +0x130 <= 0: StopMovement; finish                     // still airborne: the state ends in the air
sub 6: if AnimEnd: finish;  if +0x90 || (m_bEnableChange && +0x8D): StopMovement; finish
```

No horizontal velocity is given: the hit throws straight up from the stop that Setup made. Numbers [D; gravity = 980 ×
GravityScale]: Toss 1500 × scale: apex after 0.51 s at 383 cm, 0.1 s of near-hover, falls 383 cm in 0.88 s → lands and plays
`0213` (change point 0.6 s). Launch 2500 × scale: apex after 0.85 s at 1063 cm; the 1.2 s fall timer ends before the ground
(1.47 s needed) → the state finishes in the air and Sora drops in the ordinary fall. Aerial Recovery is possible from 0.2 s
after the hit once sub 13 is reached (≈ 0.1 s clip + loop start).

#### Parabola (slot 120 0x14135CAB0) [I]

```
sub 0: index (4-way; 2-way when dead or pawn+0x5F0 in {2, 3, 7});  power = ParabolaPower();  brake = ParabolaBrake()
       dir = Vector(pitch = ParabolaAngle(), yaw = yaw(knock));  +0x17C = power × dir.z;  MoveLengthStart;  sub = 1
sub 1: if the instigator is gone: finish
       Gravity(on);  clip = Blow(index, 0);  SetMovementMode(Falling);  SetVel3(power × dir);  Play(clip, -1, 2)
       +0x1C0 |= 2;  sub = 3;  return
sub 3: +0x168 = facing(index);  Turn;  if AnimEnd: clip = Blow(index, 1); Play(loop);  +0x130 = 0.5;  sub = 4;  +0x8F = 1
sub 4: recoverable if +0x12C > +0x138;  Turn;  if +0x130 <= 0: m_bEnableChange = 1;  +0x130 = 2.0;  sub = 5
sub 5: Turn
       if !OnGround: if +0x130 <= 0: StopMovement; brake off; +0x8F = 0; finish
       elif floating: same finish
       elif rail under Sora: finish
       else: clip = Blow(index, 2); Play(clip, -1, 2);  sub = 6;  +0x8E = 0
sub 6: Turn;  if AnimEnd: finish;  if +0x90 || (m_bEnableChange && +0x8D): StopMovement; finish
sub 10: wall hit as in Blow
every tick: if +0x1C0 bit 1: if |Velocity.xy| <= LimitBrake() (300): +0x8F = 0   else Brake(brake)
```

A ballistic throw: 1500 × scale at 45° (1061 horizontally, 1061 up at scale 1) under normal gravity, horizontal part × 0.98
per frame while faster than 300. `m_bEnableChange` is forced 0.6 s after the start (0.1 s clip + 0.5 s), so from then Sora can
attack / air-slide out in the air even without Aerial Recovery (gates in 9.5).

#### Beat (slot 121 0x14135A870) [I for constants, C for the landing-point arithmetic]

```
sub 0: power = BeatPower();  +0x17C = -power;  MoveLengthStart;  sub = 1
sub 1: Gravity(on);  SetMovementMode(Falling);  Velocity.z = -power
       if the instigator is alive and a ground trace straight down (range 2500) hits:
           t = time to reach the ground from the quadratic with the component's gravity
           if t > 0: Velocity.xy = knock × (0.7071 × ownRadius + 0.7071 × instigatorRadius) / t      // slides Sora out from under the attacker
       Play("M_0230", -1, 1 loop);  sub = 15
sub 15: if landed: StopMovement;  sub = 5;  +0x8E = 0;  Play("M_0227", blendIn 0.13, 0, effect group 0xC7C)
sub 5: if AnimEnd: power = BeatRebound() (1000);  sub = 12;  Play("M_0228", -1, 1 loop)
sub 12: SetMovementMode(Falling);  SetVelZ(power);  GravityScale(BeatReboundGravityScale() = 3.0);  sub = 13
sub 13: if Velocity.z <= 0: sub = 6; finish                    // ends at the top of the bounce (170 cm, 0.34 s)
```

Slammed down at 2500 × scale, bounces 170 cm, and the state ends at the top of the bounce. None of its clips has
`TresEnableChange`: no action until then.

#### Frozen (slot 122 0x14135B150) [C]

Used only if state 123 could not be started. `0x1405B77B0(pawn, 1, 8)` (sets pawn+0x868 bit 1 = "stopped", freeze body
effect), gravity on, keeps the horizontal velocity with the brake flag (`SmallBrake`), waits +0x1A4 seconds, releases.

#### Suction (123), Eat (124), PuddingEat (125) [C, summary]

Enemy-specific grabs: Suction pulls Sora toward the attacker in Flying mode with the Small clip, holds up to 5.0 s, then
throws him with the Blow clips (jump gate: sub-step 4); Eat hides the weapon, disables the camera collision and sets the
invincible / HP-limit flags until End; PuddingEat plays `PuddingEat`. Not needed without those enemies.

### 9.5 Acting out of the Damage state [I]

| Request | Gate (address) | Condition |
|---|---|---|
| attack | `CanAttack` 0x14142674F (jump table 0x14142700C) | nothing queued, +0x90 == 0, **kind** (state+0x88) ∈ {1, 2, 7, 8, 9, 10}, `m_bEnableChange` |
| jump | jump gate 0x14142AA57 (table 0x14142B01C) | nothing queued; kind 1, 9: `m_bEnableChange`; kind 2: (+0x8E && `AIR_RECOVERY`) or `m_bEnableChange`; kinds 7, 8, 10: +0x8E && `AIR_RECOVERY` (1); kind 19: sub-step 4; then `!(pawn+0xE60 bit 17)` |
| Aerial Recovery | `JumpRequest` 0x14143E022 | +0x8E != 0, kind ∈ {2, 7, 8, 10} (mask 0x584), controller present, `HasAbility(AIR_RECOVERY 1)` → `ClearQueue` + `SetState(AIR_SLIDE 20)`, AirSlide+0xA8 = (flags & ~8) \| 0x20 \| (in water ? 8 : 0), AirSlide target = the instigator (Damage+0x1C4) |
| dodge roll, air slide, magic, item | actions spec 3 / 6.1, magic spec | `m_bEnableChange` with kind ∈ {1, 2, 9} (their text says "types"; see corrections) |
| stick | `MoveInput` 0x141440B9E | writes +0x8D = 1; ends the state at the handler's exit points once `m_bEnableChange` |

Revision 1 said "sub-step ∈ {1, 2, 7, 8, 9, 10}" for `CanAttack`: the tested field is state+0x88, the damage kind.

"Recoverable" (+0x8E) is 1 from **0.2 s after the hit** (+0x12C > +0x138) while the handler is in an airborne sub-step
(Blow 4; Toss / Launch 13–15; Parabola 4–5) and returns to 0 at the down / get-up clip and at a wall hit. This agrees with
`docs/sora-actions-spec.md` 6.6 (jump press → `AIR_SLIDE` with flag 0x20, kinds 2, 7, 8, 10, ability 1): no disagreement.
Additions to 6.6: the 0.2 s delay and the sub-step windows above; kind 2 also lets a plain jump through at
`m_bEnableChange` (get-up clip, 0.6 s) without the ability.

**Blow counter** [I unless tagged]: `AirSlide::Begin` 0x141332930, recovery variant, unless pawn vt+0x1EE8(): with
`BLOW_COUNTER` (2): AirSlide+0xAC = 2 (`ETresCommandKind` BLOWCOUNTER), +0xB0 = 0x27 (attack kind) [C]. `AirSlide` slot 113
0x14134C5E0, called with 1 in Tick sub 0: `FUN_141414350(ctrl, kind = +0xAC, 2.0 s, 0)` → the reaction command is offered
for **2.0 s** [I 0x14134C608]; flag 0x40 set, which makes `CanAttack` true at once in AIR_SLIDE (3.2). `TryAttack` from
AIR_SLIDE pushes the Attack state with forcedKind = AirSlide+0xB0 = 0x27 (`BlowCounter` 1294, rows `blow_counter*`).
Reaction command store on the controller [C]: +0x8BC kind, +0x8B8 remaining time (ticked in 0x1413F8130), +0x8C0 target,
+0x898 shown kind; HUD `FUN_14158C180(hud, 0 show / 1 hide / 3, kind)`. The other reaction commands of the range 1–5
(REFLECTGUARD 1, REVENGESLASH 3, REVENGEIMPACT 4, REVENGEDIVE 5) are not produced by the Damage / AirSlide path: callers of
`FUN_141414350` are AirSlide (2, and 13 ANTIRAID for the form variant), Attack slot 113 0x14134C660, Jump, RiskDodge and
`TryAttack`; the guard counters use the counter window ctrl+0x8A0 (5.6.1).

### 9.6 What makes a hit not count [I]

| Source | Where set | Where tested | Effect |
|---|---|---|---|
| body-collision reaction type (pawn+0x619 and `MyBodyColl`; `FUN_1405B8B00(pawn, type)`, restore `FUN_1405B60D0`) | by state code only: Air Slide / Aerial Dodge / Aerial Recovery type 11 for the whole state, Guard type 3, … (actions spec 2.4). **No clip of Sora carries a body-collision notify** (class census of the 428 clips: none exists) | hit detection (8.2.6: body type → victim result) → `info+0x60` → `TakeDamage` | victim results 3–6: no damage; 3 / 4 / 5: guard handlers; 1: damage without reaction |
| `IsNoDamage` pawn vt+0x2080 (player 0x141435AC0 → 0x1405AD200) | pawn+0x15B4 == 0x0B [? which mode]; `m_bNoHpDamage` (pawn+0x630 bit 4, setter vt+0x2088); **pawn+0x638 > 0** (timer, counted down in 0x1405BC470; set to 1.0 by the auto-revive); pawn+0x518 bit 0; current or next state CINEMATIC (67); game-instance flag vt+0x278 unless pawn+0x630 bit 8 | `TakeDamage` | no HP loss; the reaction still happens |
| `IsNoReaction` pawn vt+0x2070 (0x1405AD1A0) | `m_bIsNoDamageResponse` (pawn+0x630 bit 3, setter vt+0x2078); +0x518 bit 0; cinematic; the same game flag | `TakeDamage` | HP loss without reaction |
| `IsInvincible` pawn vt+0x2060 (player 0x141435A90 → 0x1405AD120) | **pawn+0xE44 > 0** (timer; setter pawn vt+0x1E48 0x141112510, called only by the friend-link states, ShootFlow and StyleFinish); `m_bIsInvincible` (pawn+0x630 bit 2, setter vt+0x2068); pawn+0x634 bit 2; +0x518 bit 0; cinematic; game flag | callers of vt+0x2060 (not read; the hit application of 8.2.5 does not call it) | [?] presumably the hit is not registered |
| ctrl+0x8F4 > 0 | **0.1 s**, written when a command is executed (executor 0x14141B844; 0x14140EA49, 0x14140EE79, 0x14140EF09, 0x14140F1CF, 0x14140F440, 0x14140F6AE, 0x141421DC5; setter 0x141413960); counted down in 0x1413F80F0 | reaction gate | damage applies, no flinch for 0.1 s |
| pawn+0xE60 bit 17 | script / volumes (command lock) | reaction gate | damage applies, no flinch |
| iface slot 195 (flowmotion states, rising part of a flow jump), attack kinds 0x36–0x39 | — | reaction gate, light kinds only | damage applies, no flinch |
| state+0x138 | Setup | handlers | not an invincibility: the 0.2 s delay before Aerial Recovery |

There is **no invincibility after a hit, while down, or while getting up**: the Damage state never changes the reaction type,
and the gate re-enters DAMAGE for every hit. The protection the game gives instead is the HP rule below.

### 9.7 HP application, Second Chance and Withstand Combo (0x1405A2A00) [I: 0x1405A2A00–0x1405A2CE8]

`ApplyHP(pawn, causer, info, amount, bReact, force)`:

```
if amount <= 0 && !force: return
hp = HP;  if hp < 1: return
dmg = Floor(amount)                                 // cvtss2si(2a - 0.5) >> 1
new = hp - dmg
if !force:
    if new < 1:
        if pawn+0x630 bit 5 (m_bIsHpLimitOne): new = 1
        elif pawn+0x630 bit 6 (m_bIsHpLimitOneExceptPlayerAttack) && !bReact: new = 1
        else switch info+0x66:                      // unreflected byte: "cannot kill" class of the attack
                 1: new = 1;   2: if objType (pawn+0x3B0) == 1: new = 1
                 3: if objType in {1, 10}: new = 1;   4: if objType == 2: new = 1
    if HasAbility(LAST_LEAVE 44) && hp >= 2 && new <= 0: new = 1                          // Second Chance
    if HasAbility(COMBO_LEAVE 45):                                                        // Withstand Combo
        inDamage = current state id == 19 || next queued state id == 19                   // state machine pawn+0x5A0
        if (pawn+0x848 > 0 || inDamage) && new <= 0: new = 1
    pawn+0x848 = 0.6                                                                      // on every damage that reaches here
if new < 1 && player pawn && pawn+0x518 bit 0: new = 1
SetHP(new)   // pawn slot 776 0x1405B9700: clamp to [0, MaxHP (vt+0x1820)], pawn+0x5FC, (pawn+0x448).vt+0x220(old)
HUD damage number (0x141584130);  pawn.vt+0x1198(lost, info)      // slot 563: Damage Syphon etc., magic spec
```

Timer pawn+0x848, pawn tick 0x1405AF5B0 [I 0x1405AF814]: `if AbilityCount(45) > 0: t = (t > 0) ? max(t − dt, 0) : 0  else t = 0`.

So: Second Chance keeps 1 HP on any hit taken at 2 HP or more. Withstand Combo keeps 1 HP when the lethal hit arrives while
Sora is already in (or queued for) the Damage state, or within **0.6 s** of the previous damage — at 1 HP too. A fresh hit
at 1 HP outside that window kills. Both are tested with `HasAbility` (equipped or not, no level). The order of the
abilities in the code does not matter (both only raise `new` to 1).

`0x1405BC2C0` is the same arithmetic as a prediction ("would this hit kill?") for other callers; it does not write.

### 9.8 Death [C; constants I]

```
Survive() (pawn slot 575 0x14142F640) [I]:
    !(current or next state == DIE)  &&  pawn+0x5F0 == 1  &&  !GameFlag(0x0C) (0x140EAB0E0)  &&  ItemCount(0xA01) > 0
    -> the hit is processed as a normal reaction with the bad status removed, then pawn.vt+0x1888(1) → 0x1405BCF10 →
       item effect 0x1405BE820(0xA01): HP = Ceil(MaxHP × 0.5) (full HP and the MP refill branch with item 0x905 [?]),
       pawn+0x638 = 1.0 (no damage for 1 s) [I 0x1405BEDBA], effect type 6
else Die path 0x1405A35D0:
    pawn.vt+0xD28();  causer notify vt+0x288 / prize vt+0x290;  attribute effect;  clear bad statuses
    iface slot 129 (0x14143A190):
        gate slot 128 (0x141427950): current state != DIE
        ctrl.vt+0x1968();  ClearQueue;  st = PushState(DIE 59)
        st+0x88 = info;  st+0xF0 = hit direction;  st+0xFC bit 0 = (info.commandKind == 1)
        st+0x100 = (|yaw from Sora to the causer − facing| > 90) ? 1 : 0;  st+0x104 = causer position;  st+0x110 = causer
        game instance 0x140EBA4D0(record{state id before, killer kind, in water, …}, 1, 0)        // starts the game-over flow
```

`TresPlayerState_Die` (vtable 0x1469190C0: Reset 0x14136E440, Begin 0x141368650, Tick 0x1413733D0, End 0x14136B780):
Begin restores speeds / gravity, `StopMovement`, levels the actor, sheathes, tells the controller (input off, camera
distance 250). Tick sub 0: knock = away from the causer in XY (or −forward); mode Falling; `SetVel3(knock × 2000, z = 125)`
[I 0x141373926, 0x141373948]; faces front / back by +0x100; `Play(Blow(+0x100, 0), blendIn 0, 2)`; death camera
(`TresCameraAnim`, blend 2.0); sub = 1; while flag +0x120 bit 0: horizontal velocity × 0.95^(dt × 60) [I 0x141373E68].
Sub 1: at the clip's end `Play(Blow(+0x100, 1), -1, 2)`, timer 0.5 s. Sub 2: after 0.5 s brake off, `StopMovement`. Sub 3:
waits. RESCUE (109) and GAMEOVER (118) are started by the game-over flow of the game instance, not by this state: not read
[?]. For the port: emit `PlayerDied` at the Die push and let the host decide (reload / revive).

### 9.9 Bad statuses on Sora (rewritten 2026-10-12) [C unless tagged; [I] where an address is given for a constant or branch]

Port: `crates/kh-core/src/combat/badstat.rs`, `states/badstat_{stun,freeze,burn}.rs`, the sneeze rule in
`states/attack.rs`; tests `crates/kh-core/tests/combat3.rs`.

#### 9.9.1 The status on the pawn

One status at a time: byte pawn+0x85D (`ETresBadStatusType`), timer pawn+0x860 (seconds), HP cap pawn+0x61C, "frozen"
pawn+0x868 bit 1, the attached status effect id pawn+0x908 (rows of the attach-data asset `m_CmnBadStatesEffect` =
`ead_com_abn0`, effect interface group 2). Panacea's immunity is pawn+0x868 bit 0 with its timer pawn+0x864.

```
Immune(kind)   pawn vt+0xF50 0x1405ADBF0 (verdict same-partial):
    kind == 0x13 TIME_BREAK: false
    pawn+0x634 bit 0 (a form: 10.5) || (pawn+0x460).vt+0xA0() || (pawn+0x460).vt+0x98(): true         // [?] the two interface tests: Panacea / script immunity
    swimming (0x1405AD820) && (pawn+0x460).vt+0xB0() && kind in {7, 8, 12..17}: true                   // not ported
    kind == 0x11 BURN && pawn vt+0x1990 (movement +0xB70 bit 1 || +0xB48 > 0: in water [?]): true      // not ported
    else the base parameter's table 0x140589610: the *_GUARD abilities (BURN 63, CLOUD 64, SNEEZE 65, FREEZE 66,
         DISCHARGE 67, STUN 68; `docs/sora-stats-spec.md`)

Apply(kind, time, info)   0x1405B7C50:
    kind == 0x13 TIME_BREAK: pawn vt+0x20E0 → pawn slot 1052 → ctrl slot 876 0x1413E9D30(time) (10.7); return
    pawn+0x634 bit 0 || (pawn+0x460).vt+0xA0() || .vt+0x98(): return
    param = info ? info.badStatusParam (+0x3C) : 0
    kind in {1, 2, 3, 4, 5, 9, 10, 11}: kind = 0            // DEATH, CATCH, the binds, MAGNET, DRAIN, ASPIR are not pawn statuses
    if kind == current:
        14 HONEY: rate = max(CustomTimeDilation − max(param, 0.01), 0.1); pawn vt+0xF88(rate, 1 − rate); timer = max(timer, time); return
        18 HP_BREAK: +0x61C = max(HP, 0)
        0: return
        timer = time; return                                // the same status again restarts its time
    Clear(0);  +0x85D = kind;  if kind != 0: timer = time;  (pawn+0x448).vt+0x238(old, kind)
    8 STOP:       effect 0xC67; pawn vt+0xF90(0.1)            // the actor's own time dilation [I 0x3DCCCCCD]
    12 STUN:      effect 0xC58
    13 SNEEZE:    effect 0xBCC
    14 HONEY:     the time dilation as above
    15 CLOUD:     spawn game-mode asset +0x1D8 (`m_BadStatCloudClass`, a `TresAdhereObjBase`), attached to the root, scale 10
    16 DISCHARGE: spawn game-mode asset +0x1E0 (`m_BadStatDischargeClass`) on socket `center`; effect 0xBF4
    18 HP_BREAK:  +0x61C = max(HP, 0)
    6 ENERGYBURST, 7 FREEZE, 17 BURN: nothing here (7 and 17 belong to their states)

Clear(only)   0x1405A5C40:   if only == 0 || only == current:
    14: vt+0xF88(1.0, 0);  8: vt+0xF90(1.0);  effect group 2 off;  +0x85D = 0;  adhere actor destroyed;  timer = 0;  +0x61C = 0

Tick(dt)   0x1405AFF90, from the pawn tick 0x1405BC470:
    Panacea: if bit: timer864 −= dt; when ≤ 0 (or the interface tests say so): bit off, effect off
    if +0x85D == 0: return
    if (pawn+0x460).vt+0x98(): Clear(0)
    else switch +0x85D:
        7 FREEZE:  not counted here (its state counts its own time)
        8, 14:     step = the actor's undilated delta (0x143EAE830)       // real seconds
        12 STUN:   only while the current state is BADSTAT_STUN (122); else no countdown
        17 BURN:   step = 0 unless the current state is BADSTAT_BURN (124); in water (vt+0x1990): Clear(0)
        others:    step = dt
        if step > 0 && timer > 0: timer −= step; if timer ≤ 0: Clear(0)
    15 / 16: the adhere actor's own "finished" tests end the status                                   // not read

Shorten(t)   pawn vt+0x20D0 0x1405BB7E0:   if +0x85D != 0: timer −= t; if timer ≤ 0: Clear(0)
MaxHP()      0x1405AB200:                  +0x85D == 18 && +0x61C ≥ 1 ? +0x61C : the normal maximum
```

Where a hit applies it (`TakeDamage` 0x1405BB850, section 8.4): an auto-revive removes the status from the hit; with a
reaction, `0x1405A2350`: **`Clear(12)` first (any damaging hit cures a stun)**, then, if the hit carries a status and
`!Immune(kind)`: `Apply` for every kind except FREEZE (left to the gate, 9.1); without a reaction (`IsNoReaction`,
kind WITHOUT): `Apply` alone, then request-interface slot 90 (0x14143EEB0): FREEZE with a time and no immunity → state
123 with the hit's time (a running state 123 only gets the new time); BURN likewise → state 124 (already in it: the burn
effect 0xC08 is re-attached). The die path clears the status. A form start cures and blocks (0x1405B9230(pawn, 1, 1)).

The Damage state's `Setup` (9.3) sets state+0x90 when the pawn's status is FREEZE / BURN / STUN: no attack out of the state,
every get-up sub-step finishes at `m_bEnableChange` without the stick, and at its finish a BURN pushes state 124.

HUD: every bad-status state calls `0x141413910(ctrl, 1, 1)` at Begin and `(ctrl, 0, 0)` at End (HUD 0x141584140 /
0x1415846F0: the command menu greyed and closed to input). Port: `PawnCombat::bad.hud_locked`, event
`BadStatus { from, to, time }`.

#### 9.9.2 STUN: `TresPlayerState_BadStatStun` (122, vtable 0x1469098E0; size 0x90)

Entered by the pawn's anim-interface (pawn+0x440, vtable 0x14685E3F0) slot 78 0x1405B2A50: forwards to the current
state's anim sub-object (vt+0x2B0), then `if !pawn.vt+0x1BD8() && movement.IsMovingOnGround() && +0x85D == 12 && current
state != 122: SetState(122)` (0x141514920). **The caller of slot 78 was not found [?]**; the port runs the test in every
pawn tick, so a stun taken on the ground replaces the flinch at the next `MainProc` and a stun taken in the air starts
when he lands (probe request).

```
Begin (0x14134E7E0 → 0x141508930, base verdict differs):
    SetActorRotation(0, yaw (pawn vt+0xDA0), 0);  movement vt+0xA40(1) (movement +0xA3C bit 2: input off [?]);  StopMovement (vt+0x16E8)
    Play(BadStatStunStart, start 0, blend −1, mode 2);  effect group 2 row 0xC58;  HUD lock
Tick (0x141353140 → 0x14150EE70):
    player part: if pawn.vt+0x1A80(0) (movement vt+0x5A0: in water [?]) && PushState(SWIM 102): finish          // not ported
    sub 0: if AnimEnd: Play(BadStatStunLoop, mode 3 loop); sub = 1
    sub 1: if +0x85D != 12: Play(BadStatStunEnd, mode 2); m_bEnableChange = 1; sub = 2; return
           if state+0x88 > 0: pawn.vt+0x20D0(state+0x88); state+0x88 = 0                       // [I 0x14150EF85–0x14150EFAD]
    sub 2: if AnimEnd: finish
End (0x141350300 → 0x14150B820): HUD unlock; movement vt+0xA40(0); movement vt+0xBD0 (rail-slide work reset)
```

Escape: request-interface slot 37 (0x141440570) adds **0.3 s** to state+0x88 [I 0x1414405DD] when the current state is 122;
the controller's input update (0x1413F1AB0) calls it when controller+0x878 bit 5 is set: with the stick active
(`vt+0x888`), on the frame the stick leaves the centre (previous magnitude ctrl+0x854 < 0.1) or its yaw (ctrl+0x85C)
differs from the previous frame's by ≥ 90° - or when `WasInputKeyJustPressed` 0x144445000 answers for the key object at
0x149C6F9F8 [?: which key; not ported]. So every flick of the stick shortens the stun by 0.3 s.

Clips [A]: `BadStatStunStart` = 0241 (1.3667 s, voice at 0), `BadStatStunLoop` = 0242 (2.0 s), `BadStatStunEnd` = 0243
(1.0667 s, `TresEnableChange` 0.70 s). Gates out of the end clip: `CanAttack` row 122 "nothing queued && m_bEnableChange"
(3.2), the same for magic and items. Duration: the status time counted from the first tick of state 122.

#### 9.9.3 FREEZE: `TresPlayerState_BadStatFreeze` (123, vtable 0x146909558; size 0x90)

```
Reset (0x14150DDA0, verdict same): sub = 0; state+0x88 = 1.0
Begin (0x14134E7A0 → 0x1415087B0):
    movement input off (vt+0x16D8(1));  StopMovement;  SetActorRotation(0, yaw, 0)
    MaxWalkSpeed / MaxFlySpeed / MaxAcceleration = class defaults (vt+0x1578, +0x1628, +0x1648)
    pawn vt+0x1728(1): gravity on (movement +0xA3C bit 1 = 0);  if movement.IsFlying: SetMovementMode(Falling, stop)
    SetFrozen(1) = 0x1405B77B0(pawn, 1, 8):  HONEY / STOP dilation back to 1;  animation paused (pawn vt+0xC78(1) → mesh vt+0xDF8,
        equipment vt+0x658);  attached effects paused (0.1 s);  effect row 0xC1C (the ice) on body and weapon;  +0x85D = 7;  +0x868 bit 1
    sub = 1;  HUD lock
Tick (0x14150EDF0, verdict same): state+0x88 −= dt
    if +0x85D != 7: state+0x88 = 0; finish
    if state+0x88 ≤ 0: finish
End (0x1413502D0 → 0x14150B7B0): HUD unlock;  input on;  SetFrozen(0): animation resumes, 0xC1C removed, effect row 0xC1F (the ice
    breaking; 0xC1E for attribute 1, 0xC1D otherwise);  +0x85D = 0;  rail reset;  a flying-by-default pawn → Flying
```

The gate (9.1) writes the hit's `m_BadStatusEffectTime` into state+0x88; a second freezing hit rewrites it. Nothing shortens
it. Any other flinching hit runs the gate's `ClearQueue` + DAMAGE, and the freeze state's End breaks the ice.

#### 9.9.4 BURN: `TresPlayerState_BadStatBurn` (124, vtable 0x146908D50; size 0xA8)

Parameters: the game mode's common-param class (`TresDamageCommonParam` +0xB8 … +0xC4 through state vt+0x398 0x14043A7C0):
`m_BadStatBurnMoveSpeed` **300**, `m_BadStatBurnGravityScale` **2**, `m_BadStatBurnJumpHeightMax` / `Min` **200 / 200** [A:
`EnemyCommonParam` and `PlayerDamageCommonParam` agree; native 480 / 5 / 350 / 50, I 0x14150CA9F, 0x14150CA0F].

```
Begin (0x14134E6E0 → 0x141508620, base verdict same):
    SetActorRotation(0, yaw, 0);  StopMovement;  MaxWalkSpeed = 300 (vt+0x1570);  GravityScale = 2 (vt+0x1650)
    pawn vt+0x1340(0, 0, 1.0) (weapon hidden);  pawn vt+0x2078(1) (m_bIsNoDamageResponse: hits take HP without a flinch, 9.6)
    Play(BadStatBurnStart, mode 2);  effect group 2 row 0xC08
    player part: HUD lock;  0x1414122A0(ctrl, 0, 0, 0, 0), 0x1414122A0(ctrl, 1, 4, 0, 0) (the attack-camera function of 7.12: mode 4 on; not ported);  state+0xA0 bit 0
Tick (0x1413530B0 → 0x14150E960) [I for the branch order]:
    sub 0 / 1: if AnimEnd: Play(BadStatBurnLoop, mode 3 loop); sub = 2
    sub 2: if !OnGround (0x1405AD850):                                   // in the air: driven forward
               if state+0x8C bit 0: SetActorRotation(0, state+0x94, 0)        // a direction given from outside [?: writer not found]
               AddMovementInput(forward of the actor, 1.0) (pawn vt+0x16A0);  bit 0 = 0;  return
           if +0x85D != 0x11:                                             // the status is gone: recover
               StopMovement;  jump latch cleared (vt+0x1098);  Play(BadStatBurnEnd, mode 2);  m_bEnableChange = 1;  sub = 3
               pawn vt+0x2078(0);  return
           if Velocity.z (pawn vt+0x1568) ≤ 0:                             // landed, still burning
               StopMovement;  jump latch cleared
               if current clip != BadStatBurnStart:
                   if !(state+0x8C bit 0): SetActorRotation(0, rand24 × (360 / 2^24) − 180, 0)      // [I 0x14150EB7C: 2.1457672e-05]
                   Play(BadStatBurnStart, mode 2);  sub = 0;  return
    sub 3: if AnimEnd: finish            (player part: the attack camera is switched off when sub == 3)
    other: finish
`TresJumpStart` of the start clip → 0x141512610, only in sub 0:
    jump latch (pawn vt+0x1090);  h = min + rand × (max − min);  JumpZVelocity = movement vt+0xC50(h)       // 0x140578010:
                                                                    // sqrt(2 × |GravityZ × m_JumpUpGravityScale| × h)
    sub = 1;  if hops (state+0x98) > 0: pawn vt+0x1870(0.05, 1, 3, 1, 0, 0);  hops += 1
End (0x141350220 → 0x14150B720): HUD unlock, attack camera off;  jump latch cleared;  MaxWalkSpeed, GravityScale = class defaults;
    weapon shown (vt+0x1340(1, 1, 1.0));  pawn vt+0x2078(0)
```

`pawn vt+0x1870(rate, mode, …)` = 0x1405A3970 (verdict differs; the calls read in C): nothing when invincible / no-damage;
`amount = Ceil(rate × MaxHP)` for mode 1 (mode 2: × current HP); `ApplyHP(…, force = 0)` (9.7: Second Chance and Withstand
Combo apply); at 0 HP the die path. So **every hop after the first costs Ceil(5 % of max HP)**.

Clips [A]: `BadStatBurnStart` = 0257 (0.1667 s, `TresJumpStart` 0.1333 s), `BadStatBurnLoop` = 0258 (0.8 s), `BadStatBurnEnd` =
0259 (1.2 s, `TresEnableChange` 0.9 s). A hop [D]: launch speed sqrt(2 × g × 200) with g = the rise gravity of the movement spec
(4.3) at GravityScale 2; apex 200 cm; forward at up to 300 cm/s. Duration: the status time counted while state 124 runs; he stops
at the first landing after it ran out. Started by the gate for a BURN hit of damage kind NONE, by slot 90 for a hit without
a reaction, and by the Damage state's finish when the flinch came first.

#### 9.9.5 SNEEZE (13), and the statuses without a state

- SNEEZE: no state of its own for Sora (`TSID_STATE_BADSTAT_SNEEZE` 151 is not a `TresPlayerState` class). **Corrected by the
  fourth pass** [I 0x14133AC8A–0x14133ACB7, 0x14140DF30 same]: controller+0x920 is a latch, not the status. `PlayAttack`'s
  tail sets it (and turns the HUD's Attack command into kind 0x15 `SNEEZE`, 0x141583FF0) when the status is on him, the
  combo is not the endless one and `combo >= MaxCombo() − 2`; the cast's start does the same with its combo index
  (0x1413794B0); it is cleared when the chain ends (pawn slot 981 0x141432780) and when the carry timer pawn+0x10E4 runs out
  (0x14144A530). So a sneezing Sora attacks normally and **the sneeze takes the place of the combo's finisher**: with the
  latch set the Attack state's `Decide` takes its sneeze branch (4.2 rule 6): `state+0x234 |= 0x100; +0x140 = 1; pawn finish-end bit (vt+0x2740(1)); +0x141 = 1;
  finishCount += 1; kind = 0x1E` [C 0x14134ACA0]. `PlayAttack` case 0x1E: `BadStatSneeze` (0239, 1.1 s) on the ground,
  `BadStatAirSneeze` (0240) with the air-mode flag in the air; XY traction channel off. The clip has no hit window and no
  `TresEnableChange`: it plays out and the combo is over. (Magic: every spell but Cure executes the same command:
  `docs/sora-ui-spec.md` 5.3 item 5; magic side.)
- STOP (8): effect 0xC67, his own time × 0.1 for the status time in REAL seconds.
- HONEY (14): his own time slowed by `max(m_BadStatusParam, 0.01)` per application, not below 0.1; real seconds.
- CLOUD (15), DISCHARGE (16): an adhere actor of the game mode follows him; `TresDamageCommonParam` has
  `m_BadStatCloudInputLight` 1 / `…LightInterval` 5 / `…InputHeavy` 0.5 / `…HeavyInterval` 1 for the cloud [A]. The actors
  (`TresAdhereObjBase` subclasses, blueprint side) were not read [?]: the port keeps the status and its time and tells the
  host.
- HP_BREAK (18): max HP = the HP he had when it landed, for the status time.
- TIME_BREAK (19): not a status; the form's time left drops by the time (10.7).
- ENERGYBURST (6): stored and counted down; no pawn-side effect found.

Which enemy rows carry which status is data of the enemies' attack tables (`m_BadStatusKind`, `m_BadStatusGenRate`,
`m_BadStatusEffectTime`, `m_BadStatusParam`); the roll against the rate is step 18 of 8.2.5 and a guard clears it (8.2.6).

Open [?] (probe requests in `docs/sora-runtime-probe.md` 8b): the caller of anim-interface slot 78 (when the stun state
starts); the key of the second flick source; movement +0xA3C bit 2; the adhere actors; the writer of Burn state+0x8C /
+0x94; the attack camera's mode 4.

### 9.10 Hurt shapes [A; native defaults I 0x14062F560]

`TresBodyColl0` (`TresBodyCollComponent`, attached to the mesh) with 13 `TresBodyCollPrimitive` templates in
`p_ex001_PawnBase` (`p_ex001_Pawn` overrides none). Native defaults: shape SPHERE, size (50, 0, 0), reaction DEFAULT, damage
on, push on, camera off. Sheet `sora-combat-hurt-shapes`:

| # | Group name | Shape | Size (x, y, z) | Socket | Location | Rotation (P, Y, R) | Damage | Push | Camera | Reaction |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | None | CAPSULE | 50, 0, 90 | — (mesh) | 0, 0, 90 | 0 | no | yes | no | DEFAULT |
| 2 | None | CAPSULE | 50, 0, 90 | `Center` | 0, 15, 0 | 0, 0, 90 | **yes** | no | no | DEFAULT |
| 3 | Camera | CAPSULE | 30, 0, 60 | `hara1` | −10, 0, 0 | 90, 0, 0 | no | no | yes | |
| 4 | Guard | CAPSULE | 51, 0, 91 | — | 0, 2, 90 | 0 | yes | no | no | GUARD (3) |
| 5 | Camera | SPHERE | 23 | `atama` | 10, −2, 0 | 0 | no | no | yes | |
| 6 | BaymaxRide | SPHERE | 120 | — | 0 | 0 | yes | no | no | DEFAULT |
| 7 / 8 | Camera | CAPSULE | 10, 0, 23 | `R_ashi1` / `L_ashi1` | 8, 10, 0 | 0, 0, 90 | no | no | yes | |
| 9 / 10 | Camera | CAPSULE | 20, 0, 35 | `R_sune` / `L_sune` | 15, 0, 0 | 90, 0, 0 | no | no | yes | |
| 11 / 12 | Camera | CAPSULE | 12, 0, 25 | `R_ude2` / `L_ude2` | 18, 0, 0 | 90, 0, 0 | no | no | yes | |
| 13 | GuardBody | CAPSULE | 50, 0, 90 | — | 0, 0, 90 | 0 | yes | no | no | WEAK_GUARD (4) |

Sora's hurt shape in normal play is **#2**: one capsule on the `Center` bone (it follows the animation). #4, #13 and #6 have
`MyIgnoreNameNoneGroup` and belong to named groups (`Guard`, `GuardBody`, `BaymaxRide`) that are switched on by the guard
states / the Baymax ride [C: group switching not read]. #1 is the push body; the `Camera` bodies only fade the camera. How
`MyShapeSize` maps to radius / half height (x = radius, z = half height assumed from the attack shapes) is [?].

---

## 10. Formchange with the Kingdom Key: Second Form

### 10.0 Summary [D]

1. Every damaging hit adds the attack row's `m_FormPoint` × rates to a gauge on the controller (max 100). After 20 s
   without gain it loses 5 points per second.
2. When the gauge is full it is emptied and — unless a grand-magic command was rolled instead (magic spec part I
   section 7) — the command `CHANGE_L1` (148) enters the situation-command list for 30 s.
3. Triangle on that command clears the state queue and pushes STYLECHANGE (31): the world slows to 0.1, Sora hangs
   in place, plays `StyleChange` (0.83 s) → `StyleChangeLoop` (one tick) → `StyleChangeEnd` (1.4 s; a 550 cm blast
   `LM_CHANGE` at 0.37–0.97 s; other states can take over from 0.97 s). The form starts counting at the press: 30 s.
4. In the form: attack state id 90 (`ComboLimit`): combo length 6, the last 3 steps are the Second Form finishers
   (ground Explosion → Stride Break → Ripple Drive, air Hurricane Period → Aerial Finish → Magnet Splash); gauge gain
   × 0.8; six abilities are added; bad statuses are cured and blocked.
5. When STYLECHANGE ends, the finish command `LM_STUNIMPACT` (171) appears in the finish slot. Each further full
   gauge raises it (172 `LM_SONICRAVE`, 173 `LM_LASTARCANUM`) and adds 3 s of form time (never above 30).
6. The form ends when its time runs out (not during an attack, guard, dodge or cast: at the first frame outside them)
   or when a finish command's state ends.

### 10.1 Objects and fields

| Object | Where | Role |
|---|---|---|
| gauge and form bookkeeping | ctrl+0xB34 … +0xBB0 (sheet `sora-combat-form-fields`) [I] | gauge +0xB40, pending +0xB44, chain +0xB48, decay timers +0xB4C / +0xB50, form command +0xB38, form level +0xB34, form time left +0xB54 / full +0xB58, finish level +0xB64 / command +0xB68, flags +0xB5C / +0xB5D / +0xB60 / +0xB78 |
| gauge parameters | pawn properties +0xCA8 … +0xCF0 (`MyStylePoint*`, `CriticalMode*`, `MyStyleFinishOffsetTime*`) [I ctor 0x141423210 + A blueprint] | rows `form.gauge.*`, `form.finish.offset_time*` |
| command rows | `Load/Common/TresCmdData` (254 rows of `TresCommandKindData`, 0x48 bytes; ctrl+0x8D8 → pointer → rows indexed by kind) [A; offsets I] | +0x18 `Type`, +0x19 `Cate`, +0x1B `Attr`, +0x24 `Param0` (queue lifetime, s), +0x28 `Param1` (form level), +0x30 `Param3` (form duration, s). Row 148 `CHANGE_L1`: Type 4, Cate 14, Param0 30, Param1 1, Param3 30 |
| `TresPlayerState_StyleChange` | state id 31, size 168, vtable 0x146947250: Reset 0x1413D0C50, Begin 0x1413CBA70, Tick 0x1413D3AC0, End 0x1413CE760, slot 113 0x1413D08A0 | the transformation |
| `TresPlayerState_ComboLimit` | state id 90, vtable 0x146910B80; overrides only slot 122 `Decide` 0x141362AB0 | the form's attack state |
| `TresPlayerState_StyleFinish` | state id 33, size 736, super `TresPlayerState_Attack`, vtable 0x146947A28: Reset 0x1413D0CA0, Begin 0x1413CBF70, Tick 0x1413D4550, End 0x1413CEBD0; per-command ticks slots 143–161, extra-command helpers slots 162 / 163 | finish commands |
| form abilities | `Blueprints/Player/p_ex001/p_ex001_FormAbility` (`TresFormAbilitySet`: wear form → ability list) [A] | 10.5 |
| equipment variants | `Blueprints/Player/p_ex001/w_so010_Equip` [A]: weapon form 01, grow level 01 = wear form NORMAL, grow level 02 = wear form `TRES_WEAR_FORM_05`; same `RWeapon` unit at `R_wep_skt`, no replaced anim / magic set | the Keyblade keeps blueprint, shape and sockets in Second Form (as `docs/sora-vfx-spec.md` 2.8) |

Form command kinds [I, jump tables]: Second Form family = 0x94 … 0x98 (`CHANGE_L1` … `CHANGE_L5`); the Kingdom Key
gives 0x94. Anti form = 0x8B, marked by ctrl+0x888 == 0x8B. Everything below takes the 0x94 branch.

### 10.2 The gauge [I]

**Gain** — `AddStylePoint(amount, type, cmd)` = ctrl slot 848 0x1413E9000 (xmm1 = amount, r8d = type, r9d = cmd; the
decompiled signature is wrong). Caller on a hit: the player's damage notify 0x141442DE0 with
`((float)info.m_FormPoint, 1, info.m_CommandKind)` for every hit with damage > 0 (8.5.2; call verified at
0x141442E56).

```
if pawn invalid || pawn state id == 138 (RAILSLIDE_MI): return
m = 1.0
if type != 3:
    if ctrl+0x888 == 0x8B (anti form):                      // not Sora + Kingdom Key in a test level
        pending += (type == 2 ? amount : 0); decStart = pawn.MyStylePointDecStartTime; decInterval = 0; return
    if type == 0 && !pawn.MyStylePointAddWithoutHit (+0xCDC, default false): amount = 0
    else:
        cmd = cmd != 0 ? cmd : 27 (ATTACK)
        lv = TresCmdData[ctrl.form_cmd (+0xB38)].Param1          // slots 846 / 847: 0 no form, 1 in Second Form
        if pawn.vt+0x8D0() (critical mode):
            amount *= (lv == 1 ? 0.8 : lv == 2 ? 0.55 : 0.5) * CriticalModeStylePointAddRate (1.0)
            if AbilityCount(0x69 CRITICAL_CHARGE) > 0 && pawn.vt+0x17E0() (MP charge running): m = 1.3
        else:
            amount *= (lv == 1 ? 0.8 : lv == 2 ? 0.55 : 1.0) * MyStylePointAddRate (1.0)
// by command (jump table 0x1413E9384, cmd - 0x20):
if cmd ∈ {32, 36, 40, 44, 48, 110..124, 126, 231}: amount = 0           // grand-magic casts add nothing
if cmd == 125 (S_CURAGAN) && !has(0x67 POWER_CURE): amount = 0
if has(0x47 FORM_UP): m += 0.2
if pending (+0xB44) <= 0: chain (+0xB48) = (ctrl.form_cmd == 0x9A) ? 2.0 : 1.0
else:                     chain *= 0.5
amount = amount * m * chain
if amount > 0:
    ctrl.slot899(amount)                                    // 0x1413E8D70: friend points +0xC10 (not style)
    pending += amount; decStart (+0xB4C) = pawn.MyStylePointDecStartTime (20); decInterval (+0xB50) = 0
    if cmd != 0: e = TresCmdData[cmd].Attr; if e < 8: points[e] (+0xB98) += amount; last[e] (+0xBA8) = max(last[e], cmd)
```

The chain factor makes simultaneous hits count 1, ½, ¼ …: it is reset only when the gauge tick has consumed the
pending value. `type`: 1 = hit, 0 = "without hit" (debug property), 2 = raw add in anti form, 3 = raw add without the
rates (the periodic add below).

**Tick** — `StyleGaugeTick(dt)` = ctrl slot 829 0x14141C4F0, every controller frame:

```
g = ctrl.vt+0x1418(); if g && g+0x84C != 0: return                              // [?] a game mode byte
if ctrl+0xBB8 (link) || +0xC08 (attraction) || +0xC09 (team attack) || +0xC18 || (+0xB78 & 4) || ctrl+0x880 == 0xC9:
    pending = 0; return                                                          // getters slots 879, 890, 894, 902, 865
if pawn invalid: return
if pawn.slot902() (swimming) || pawn state id == 169: pending = 0; return
if current state is of class 0x1428CCE40 (StyleChange): return                    // points wait during the transformation
x = ctrl.vt+0x1420(); if !x || !(x+0x95B0 & 8): return                           // [?] game flag: formchanges available
if GameFlag(pawn, 7) (0x1405AD320): if ctrl+0x888 != 0x8B || form != 0x8B: pending = 0; return
max = (float)pawn.MyStylePointMax; if max <= 0: max = 100
old = gauge; gauge = min(old + pending, max)
if pending > 0 && old < max: hud.SetStyleGauge(gauge / max)                      // 0x14158C410
pending = 0
ctrl+0xB5C = AwardAtMax(dt)                                                      // slot 830, below
if ctrl+0xB60 (style fix) != 0 || ctrl+0x888 == 0x8B || pawn.vt+0xA48() (pawn+0x518 bit 0): return
flags(+0xB78) &= ~8
if !(flags & 1) && form_cmd != 0:                                                // bit 0 = BP_NotifyPauseSituationTimer
    form_time_left (+0xB54) -= dt
    if form_time_left <= 0:
        if CanEndStyle() (slot 857): EndStyle(0, 0) (slot 832)                   // 10.7
        else: form_time_left += dt                                               // stays at <= 0, retried next frame
        flags |= 8
decStart -= dt; if decStart > 0: return
decStart = 0; decInterval -= dt
if decInterval <= 0:
    decInterval = pawn.MyStylePointDecInterval (1.0); gauge -= pawn.MyStylePointDec (5)
    if gauge > 0: hud.SetStyleGauge(gauge / max)
if gauge <= 0: gauge = 0; decStart = decInterval = 0; points[0..7] = 0; last[0..7] = 0; hud.SetStyleGauge(0)
```

The decay also runs during the form. The HUD shows the gauge in four phases (thresholds 0.25 / 0.5 / 0.75 / 1.0,
`docs/sora-ui-spec.md` 5.7).

**Award** — `AwardAtMax(dt)` = ctrl slot 830 0x1413F0ED0 (returns the new "formchange command pending" byte):

```
// 1. bookkeeping of the two flags against the HUD (0x141583BB0 = "hud has this command")
if +0xB5D (finish pending): for forms 0x94..0x98 nothing is checked; other forms clear +0xB5D when their finish command left the HUD
elif +0xB5C: weapon type 1 && form_level == 0: keep +0xB5C only while the HUD still has 0x94
// 2. periodic add [? flag meaning]
if ctrl+0x8C8 > 0 (slot 677): return +0xB5C
if GameFlag(pawn, 5) (0x1405AD350) && !pawn.vt+0x1ED8() && pawn.vt+0x2A88():
    acc (+0xB74) += dt; if acc >= 1.0: acc = 0; AddStylePoint(3.0, 3, 0)
// 3. full gauge
if gauge < (float)pawn.MyStylePointMax || pawn.slot902() (swimming): return
SetStyleFinishCommand(true, false)                              // slot 837: only acts while a form is active, 10.6
if ctrl+0x888 == 0x8B: return (+0xB5D == 0)
offered = RollGrandMagic(pawn.vt+0x17E0(), 0)                   // slot 838 0x1413ED500, magic spec part I 7.2
if !offered && !+0xB5D && pawn type (pawn+0x5F0) ∉ {2, 7}:
    if pawn type == 6: … (0x99; not Sora)
    elif form_cmd == 0x8B || form_cmd ∈ 0x94..0x98: nothing     // no second formchange inside Second Form
    else switch weapon type (pawn slot 601 → equipment vt+0x428)(-1):
        1 (Kingdom Key): if form_level (+0xB34) == 0: NotifySetActionCommand(0x94, -1.0, null)
        0x12 → 0x95, 0x10 → 0x96, 0x11 → 0x97, 0x13 → 0x98; 2..0xF → the other keyblades' first / second forms
gauge = 0; decStart = decInterval = 0; points[] = last[] = 0; hud.SetStyleGauge(0)
return (+0xB5D == 0)
```

So a full gauge is always consumed. Outside a form it yields the formchange command unless grand magic was rolled;
inside Second Form it yields a finish level (and possibly grand magic).

### 10.3 From the command to the transformation

**Enqueue** [I] `NotifySetActionCommand(0x94, -1.0, null)` 0x14140DA40: not enqueued while ctrl+0xB60 (style fix) is
set; lifetime = `Param0` (30) × (1 + 0.2 × AbilityCount(0x31 FORM_TIME)), capped at 99.9 s; then the HUD insert
(section 11). `FORM_TIME` lengthens the **command's** lifetime; the form duration has no ability term [I: the only
reader of ability 0x31 in the controller is 0x14140DB76].

**Use** [I] Triangle on the entry → HUD → `ExecuteCommand(0x94)` (0x14140EAA0 writes ctrl+0x87C) → executor
0x14141B470, jump-table target 0x14141B9A6 (kinds 0x80..0x9A, 0xDD..0xE0):

```
if !StartStyle(): fail (the entry stays in the list)            // slot 831 0x14141A9D0
ctrl.slot828(cmd); ctrl.slot676(1.0)                           // [C] time-rate channel reset / camera
0x1414158F0(ctrl, cmd, 0)                                      // ctrl+0x880 = cmd; pawn slot 947 0x1414488A0: delegate OnPlayerBeginCommandAction, pawn+0xE2C = cmd ("command in action") [I]
ctrl+0x8F4 = 0.1
```

`StartStyle()` 0x14141A9D0 (reads ctrl+0x87C) [I]:

```
if pawn.slot902() (swimming) || !(ctrl+0x878 & 2) || ctrl+0x8C8 > 0: false
// formchange branch (finish kinds: 10.6)
if ctrl+0xB60 != 0 || !pawn.CanStyleChange(cmd) (slot 1343 0x14142DEF0): false
0x1414145C0(ctrl, 0, null, 0, 0); 0x1414142D0(ctrl, 0); ctrl+0x878 &= ~0x100       // close the counter window (5.6.1), clear the just-guard flags
iface.ClearQueue()                                                                  // slot 12 0x1405A8010
want = weapon type of the command (0x94 → 1); if pawn weapon type != want: pawn.slot633(want, 0, 0, 0, 0, 0, 1)   // never for the Kingdom Key
iface.TryStyleChange()                                         // slot 190 0x141445940 = PushState(31)
form_cmd (+0xB38) = cmd; form_level (+0xB34) = TresCmdData[cmd].Param1 (1)
form_time_full (+0xB58) = 30.0; d = TresCmdData[cmd].Param3; if d > 0: form_time_full = d
form_time_left (+0xB54) = form_time_full                       // 30 s for row 148; the clock starts here, at the press
gauge = 0; decStart = decInterval = 0; points[] = last[] = 0; +0xB5C = +0xB5D = 0
0x1405B9230(pawn, 1, 1)                                        // pawn+0x634 bit 0 = 1: bad status cured and blocked [C meaning]
hud.SetStyleGauge(0); ctrl.slot791(1); ctrl.slot840() (lock-on targets cleared [C]); ctrl.slot845(); ctrl.slot738()
pawn+0xE60 &= ~0x40000
delete the finish commands 0x9D, 0xA1, 0xA2, 0xA3, 0x9E, 0x9C, 0xA6, 0x9F, 0xA0, 0xA7, 0xA8, 0xA9, 0xE4, 0xAB, 0xAC, 0xAD, 0xAE, 0xE1, 0xE2
true
```

`CanStyleChange(cmd)` 0x14142DEF0 [C for the chain, I for the leaf getters]: controller exists; none of pawn+0x1130
bit 7, pawn+0xE60 & 0x8080, `pawn.vt+0x1FE8(6)`; pawn+0xE48 bit 6 clear; pawn+0x518 bit 0 clear; `pawn.vt+0x29C0()`
(0x141427690: the "can act" test of the magic spec); pawn+0xE62 bit 0 clear; not swimming; `!pawn.vt+0x2540()` (no
DIE state current or queued); **no state queued** (iface+0x150 → 0x14150D3D0 == 0); `!iface.vt+0x618()`; pawn+0xE60
bit 17 clear; and the state gate 0x141425510 [I, jump table 0x1414256E0]: pawn+0x634 bit 2 clear, pawn+0xE2C ∉
{0x4B, 0xC0}, current state ∉ {DANGLE 6, SHOOTERMODE 40, SHOOTFLOW 41, ATHLETICFLOW 42, GIGAS 96, SPECIAL_SHIP 103,
SMALLAIRPLANE 119}; FIREMAGIC 18 only when `!(+0xD8 & 0x10) && +0xD0 > 0`; USE_ITEM 111 only with `m_bEnableChange`;
COVER 142 conditional; **every other state passes — including ATTACK, DAMAGE, GUARD and DODGEROLL, without waiting
for `TresEnableChange`**. The current state is replaced at the next `MainProc` (the queue was cleared and holds only
STYLECHANGE).

### 10.4 STYLECHANGE (state 31)

Fields (state+): 0x88 `m_pTargetPawn`, 0x94 form command, 0x98 wear form to apply (0x0B = none), 0x99 "has wear
form" byte, 0x9C facing timer, 0xA0 flags (bit 0 animation missing, 1 Second Form family, 2 anti, 3 forms 0x99 /
0x9A, 4 short version, 6 forms 0xDD..0xE0), 0xA4 start argument.

```
Reset (0x1413D0C50) [I, verdict same]: base reset; +0x9C = 0.2; flags = 0; +0x88 = 0; +0x94 = 0; +0xA4 = 0; +0x98 = 0x0B; +0x99 = 0
Begin (0x1413CBA70) [I for the calls and the 0x94 case; verdict differs]:
  pawn.slot616(1, 1, 1.0); pawn.vt+0x1048(0); pawn+0x6E4 = 1 (vt+0x1780); pawn.vt+0x16E8() (stop movement [C]);
  pawn+0x630 bit 2 = 1 (vt+0x2068); pawn.vt+0x19E0(1); x = pawn.vt+0x8C0(): 0x140E9E290(x, 1)
  ctrl missing: DoFinish
  form (+0x94) = ctrl.form_cmd; 0x141413910(ctrl, 1, 0) (HUD hidden [C]); pawn+0xE60 bit 19 = 0 (slot 1266); 0x1405B9230(pawn, 1, 1)
  form ∈ 0x94..0x98: flags |= 2; +0x98 = 5 (TRES_WEAR_FORM_05); +0x99 = 1            // 0x1413CBC54 mov word [rbx+98h],105h
  WorldSlow(ctrl, on)                       // 0x14140E500(ctrl, 1): object types mask 0x419C at rate 0.1, ctrl+0x878 bit 3
  0x14140BF30(ctrl) (hud 0x141577DD0); SetStyleHud(ctrl, form, -1.0)                 // 0x14140C040: banner + time gauge, total +0xB58, left +0xB54
  flags bit 4 = !(save+0x2E9 bit 2)         // option "Keyblade Cinematics": Normal (bit set, the save default 0x1F) → 0; Basic → 1 [bit ↔ option name D]
Tick (0x1413D3AC0) [I: dispatch, sub-steps 0, 2, 3, 4; C: the facing tail]:
  pawn.vt+0xDC8(dt * 12); pawn.vt+0xDB8(dt * 12)
  sub 0: pawn.slot741(0) (gravity off)
         if flags & 0x10: facing timer = 0; flags &= ~1; Play("StyleChangeLoop"); sub = 3
         elif flags & 0x4A (Second Form): Play("StyleChange"); flags &= ~1; facing timer = 0; sub = 2
         else (other keyblades): Play("StyleChange2".."StyleChange21" by form) → sub 1 → weapon change → sub 5
  sub 2: if AnimEnded: Play("StyleChangeLoop"); sub = 3
  sub 3 (the next tick, no condition):
         if +0x98 != 0x0B: pawn.vt+0x1498(+0x98, 1.0)       // equipment vt+0x4F8: wear form 5 → textures + form abilities (10.5)
         if +0x99: pawn.vt+0x13F0(0, 0.0)
         Play(flags & 0x10 ? "StyleChangeEasy" : "StyleChangeEnd")   // "StyleChangeEnd" if the easy clip is missing
         sub = 4; WorldSlow(ctrl, off); hud 0x14157AFE0(hud, form, 0.0)
  sub 4 / 5: if m_bEnableChange: WorldSlow(ctrl, off); m_bEnableChange = 0             // [I] the bit is cleared again
         if AnimEnded || flags & 1: if iface.Fall(actor yaw) != null: DoChain      // iface slot 172 0x141444C00: pushes JUMP at its falling sub-step (movement spec 2); on the ground it lands at once
  tail (only while facing timer > 0; never for Second Form, the timer is zeroed at sub 0):
         target = ctrl.slot721(); Turn(yaw toward target or actor yaw, rate * dt * 60); facing timer -= dt
End (0x1413CE760) [C; the slot-867 call I]:
  pawn.slot741(1) (gravity on); pawn+0x630 bit 2 = 0; pawn.vt+0x19E0(0); SetMovementMode(Falling) (vt+0x17A0(3)); pawn.slot948() (0x1414329E0: OnPlayerEndCommandAction, pawn+0xE2C = 0 [C]); pawn.vt+0x22E8()
  if sub ∈ {2, 3} (left before sub 3 ran): apply the wear form and play the end clip exactly as sub 3; sub = 4
  0x141413910(ctrl, 0, 0); WorldSlow(ctrl, off); hud 0x14157AFE0(hud, form, 0.0)
  ctrl.slot867(1, 0) → SetStyleFinishCommand(true, false)                              // 0x1413CEA6D: finish level 1 appears, 10.6
```

Play = interface (state+0x40) vt+0x18 with mode 2 (one-shot), start −1, rate 1.0, as the attack state. All three
clips have no root motion [A]. Clips [A] (sheet `sora-combat-form-clips`):

| AnimSet name | Clip | Length | Gameplay notifies |
|---|---|---|---|
| `StyleChange` | 1401 | 0.8333 s | swing sound 0.024–0.61, effect 0.235–0.54, sound 0.623; no hit, no `TresEnableChange` |
| `StyleChangeLoop` | 1402 | 1.0 s | none (shown for one tick) |
| `StyleChangeEnd` = `StyleChangeEasy` | 1403 | 1.4 s | `TresLandAnimPose` 0; voice 0.019; effects 0.338 / 0.384; **TresAtk group `StyleChange` row `LM_CHANGE` 0.3667–0.9667**; `TresEnableChange` 0.9667 |

`StyleChange` shape [A]: sphere radius 550 at pawn socket `center` (pawn `TresAtkColl0`, sheet collisions). Row
`LM_CHANGE` [A]: power 0.5, `BLOW`, attribute NOTYPE, ignores guard, form points 0.

Timeline [D] from the Triangle press (frame N, 60 Hz): STYLECHANGE begins in S(N); 1401 ends ≈ 0.83 s; 1402 for one
frame; 1403 from ≈ 0.85 s (world back to normal speed), blast 1.22–1.82 s, `TresEnableChange` 1.82 s, end 2.25 s.
World slowed (rate 0.1 for enemies, gimmicks, avatars, map, background, physics) for ≈ 0.85 s. With "Keyblade
Cinematics: Basic" 1401 is skipped: blast 0.38–0.98 s. **The form clock does not run during STYLECHANGE** (corrected
2026-10-12): the gauge tick 0x14141C4F0 (controller slot 829) returns as soon as the pawn's current state IsA
`TresPlayerState_StyleChange` [C; the test sits before the gauge add, the form-time decrement `+0xB54 -= dt` at
0x14141C833 and the decay], and that decrement is the only per-frame writer of +0xB54 [I: scan of `movss [reg+0B54h]` over
0x1413E0000–0x141430000 finds only StartStyle 0x14141A9D0, SetStyleFinishCommand 0x1413EE030 (raises / adds, never lowers),
the time-break drain 0x1413E9D30, 0x1413F3C10 and this tick]. So the full form time (30 s × the form-time factor) remains
when control returns, about 2.25 s after the press; revision 2 said 27.8 s. (The gauge does not fill and does not decay
during the transformation either.)

Leaving early: `m_bEnableChange` is cleared by Tick sub 4 in the same `MainProc` in which the notify set it [I], so
gates that test "current state's `m_bEnableChange`" never see it, and `CanAttack` (section 3.2) has no case for
state 31 (→ refused). The state therefore normally lasts until 1403 ends (2.25 s after the press) and leaves through
`Fall`; the jump / dodge / guard / magic gates against state 31 were not re-read here [?] (runtime probe). A damage
reaction replaces the state as usual (`End` then applies the wear form if sub 3 had not run).

### 10.5 While the form is active

- `ctrl.form_cmd` (slot 846) = 0x94; `form_level` = 1 → gauge gain × 0.8 (10.2) [I].
- Attack state id (pawn slot 1337 0x141432F00) = **90 `COMBO_LIMIT`** [I table 0x14143316C]; `MaxCombo` (slot 928
  0x141433570) = **6** [I]; `Finishers` (slot 1253 0x141433A60) = **3** [I]. Combo Plus / Air Combo Plus are not
  added in the form (the ability term is only in the no-form branch).
- Abilities [A + C 0x140E87610]: wearing `TRES_WEAR_FORM_05` adds one equipment copy each of HIGHJUMP (16), OVERCOME
  (179), COMBO_MASTER (30), MASTER_DRAW (61), LUCK_UP (39), MP_HASTE (53) (`p_ex001_FormAbility`; they are removed
  when the wear form returns to NORMAL, by the equipment rebuild of `docs/sora-stats-spec.md`). COMBO_MASTER sets
  pawn+0xE28 in `CanAttack` (section 3.2).
- Bad statuses [C]: pawn+0x634 bit 0 is set at the start (0x1405B9230(pawn, 1, 1) cures the current one) and cleared
  by `EndStyle`; 0x1405B7C50 (bad-status application) returns early while it is set — except `TIME_BREAK` (10.7).
- Looks: wear form 05 swaps three textures on Sora's material slots 8–10; the Keyblade is unchanged
  (`docs/sora-vfx-spec.md` 2.8). The HUD shows the form logo with a time gauge = `form_time_left / form_time_full`.
- Movement abilities, guard, dodge, magic: unchanged code paths (the form adds no state other than 90 and 33).

**`ComboLimit::Decide`** 0x141362AB0, as a diff of section 4.2 [I for every differing branch and constant; the
unchanged parts compared at C level and by the list of virtual calls]:

| Part of 4.2 | In `ComboLimit` |
|---|---|
| reach rule at combo 0 / after kinds 1C, 1D; last-kind reset; launch stocks | identical (400 cm; numEnemies > 1) |
| wall-run request (state+0x1EC bit 3) | present, with its own choice of kinds 31–34 [C; out of scope] |
| launch (bit 2): kinds 1C / 1D | identical |
| sneeze branch (ctrl+0x920 → 1E), unison branch (bit 7 → 3F…43), `pawn.slot1304()` one step before the last | **absent** |
| finisher slots A / B / C from TRIPPLE_SLASH, CHARGE_THRUST, MAGICFLUSH, FINAL_HEAVEN | **absent** |
| normal step condition | `combo < max − fin` only (6 − 3 = 3); the endless-combo flag state+0x234 & 0x200000 is not tested |
| rising attack | `targetHigh && last ∉ {8, 17}` → kind 8, fixed DirZ = pawn+0xBAC × 200. (Base also excludes 1F and needs dist ≤ 400.) |
| air-down (`bAir && dz < −100`): 0D with AIR_DOWN, dz ≤ −250, target on ground, last ∉ {0D, 0E}, dist ≤ 400; else 0F | identical |
| cycle table by last kind / first / combo % 3; `+0F` when `bAir \|\| (combo > 0 && targetHigh)`; finish count = 0 | identical |
| finisher step | below |
| ability upgrades 1→9, 2→0A, 3→0B, 8→1F, 0F→20, 10→18, 11→19, 12→1A | identical (abilities 0x6B, 0x6C, 0x6D, 0x6F, 0x70) |

```
finisher step (combo >= max - fin):                               // 0x141362FD5
    n = pawn.finishCount (+0xE14); bFinish (+0x140) = 1
    if n >= fin - 1 || combo >= max - 1: pawn+0xE48 bit 5 = 1; finishEnd (+0x141) = 1
    if bAir && dz < -100 && has(AIR_DOWN) && dz <= -250 && targetOnGround && last ∉ {0D, 0E} && dist <= 400:
        pawn.finishCount = n + 1; kind = 0E; store, return           // no upgrade
    kind = (n == 1) ? 7B : (n == 2) ? 7C : 7A
    pawn.finishCount = n + 1
    if bAir: kind += 3                                               // 7D / 7E / 7F; targetHigh alone does not lift a finisher
```

Kinds 0x7A–0x7F in `PlayAttack` 0x1413378B0 [I, case trace] — all keep facing the target for the whole attack
(state+0x1F4 |= 0x10); the air kinds set `airMode`; defaults otherwise (stop distance 160, z gain 3.0, XY homing
channel on, no root-scale channel):

| Kind | Finish index | Clip | Length | Root travel | `TresEnableInput` | `TresEnableChange` | `TresTraction` | Hit windows: group, row (hit sound) [A] |
|---|---|---|---|---|---|---|---|---|
| 7A | 0 (and ≥ 3) ground | `M_1412` Explosion | 1.3 s | 38 cm | 0.5 | 0.8667 | – | 0.5333–0.8 `SpreadS` `LM_EXPLOSION` (re-hit every 0.1 s); 0.8333–1.0 `KB_SphereS` `LM_EXPLOSION2` |
| 7B | 1 ground | `M_1411` Stride Break | 1.6333 s | 742 cm | 0.7333 | 1.2 | 0.5333–0.7 | 0.5–0.7333 `KB_StickM` `LM_STRIDEBREAK` (HIT2); 0.9333–1.2333 `KB_SphereSS` `LM_STRIDEBREAK2` (HIT5) |
| 7C | 2 ground | `M_1413` Ripple Drive | 1.8333 s | 99 cm | 0.6333 | 1.6667 | – | 0.6667–1.0667 `SphereM` `LM_RIPPLEDRIVE` |
| 7D | 0 (and ≥ 3) air | `M_1414` Hurricane Period | 1.6667 s | 416 cm | 0.7667 | 1.2 | 0.0667–0.7667 | 0.1333–0.7667 `KB_StickM` `LM_HURRICANEPERIOD` (HIT4, re-hit 0.2333 s); 0.9667–1.3 `KB_SphereS` `LM_HURRICANEPERIOD2` (HIT5) |
| 7E | 1 air | `M_1415` Aerial Finish | 1.9667 s | 0 | 1.0333 | 1.4667 | 0.1333–0.7333 | three `KB_StickM` `LM_AERIALFINISH` (HIT4) 0.1618–0.3951, 0.4333–0.7, 0.7333–1.0333; 1.2333–1.5667 `KB_SphereS` `LM_AERIALFINISH2` (HIT6) |
| 7F | 2 air | `M_1416` Magnet Splash | 2.3667 s | 0 | 1.7 | 2.2 | – | 0.3–1.7 `Range1400` `LM_MAGNESPLASH` (`MAGNET`, effect time 1.5); 1.7667–2.0667 `ImpactLL` `LM_MAGNESPLASH2` |

Attack rows `LM_*` (28 rows, all 30 fields): sheet `sora-combat-attack-data`. Form points of the finisher rows: 12
(main blows), 10 (`LM_EXPLOSION2`), 4 (multi-hit parts), 2 (`LM_MAGNESPLASH`); the finish-command rows give 0.

Consequences [D]: ground, one near target, no abilities: kinds 1, 2, 3 (`1301`, `1302`, `1303`), then 7A, 7B, 7C.
The three finishers are always all played if the player keeps pressing (finishEnd on the third). In the air: 10, 17
(`1308`), 12, then 7D, 7E, 7F. With the upgrade abilities the first three steps become 9 / 0A / 0B (18 / 19 / 1A).
A finisher step never becomes the magic finisher by this function (the magic side keeps its own rule, section 14).

### 10.6 Finish commands

**Giving the command** — `SetStyleFinishCommand(bLevelUp, bWeaponChange)` = ctrl slot 837 0x1413EE030 [I]. For forms
0x94..0x98:

```
lv = finish_level (+0xB64)
if (bLevelUp || lv == 0) && lv < 3: lv++
SetFinishLevel(lv)                       // slot 853 0x141414C70: +0xB64 = min(lv, 3); +0xB68 = {0: 0, 1: 0xAB, 2: 0xAC, 3: 0xAD (0xAE in form 0x95)}
cmd = +0xB68; if cmd == 0: return false
if !+0xB5D:                              // first time in this form
    off = bWeaponChange ? MyStyleFinishOffsetTimeWeaponChange (1.0) : MyStyleFinishOffsetTime (3.0)
    if form_time_left < off: form_time_left = off
    t = form_time_left
else:                                    // a further level
    t = min(form_time_left + MyStyleFinishOffsetTime (3.0), form_time_full (+0xB58))
    form_time_left = t
ctrl+0x8F0 = t
NotifySetActionCommand(cmd, t, null); +0xB5D = 1; return true
```

Callers: STYLECHANGE `End` (level 1 `LM_STUNIMPACT`), `AwardAtMax` at every full gauge during the form (level 2
`LM_SONICRAVE`, level 3 `LM_LASTARCANUM`; the form time grows by 3 s, never above the full 30 s), keyblade switching
(not here). `NotifySetActionCommand` removes the other `LM_*` commands first (0xAB removes 0xAC / 0xAD; 0xAC removes
0xAB / 0xAD / 0xAE; 0xAD / 0xAE remove 0xAB / 0xAC), so one finish command exists at a time; it sits in the HUD's
finish slot (section 11.1), which has no countdown of its own: it lives exactly as long as the form.

**Using it** [I] executor target 0x14141B923 (kinds 0x6D, 0x9C..0xA8, 0xAB..0xAE, 0xB3, 0xE1, 0xE2): needs
`+0xB5D` (slot 856) and `StartStyle()`; then `ctrl.slot676(1.0)`, 0x1414158F0, ctrl+0x8F4 = 0.1. `StartStyle` finish
branch (0x14141AAAF):

```
(same first three refusals as 10.3)
if !pawn.CanStyleFinish(cmd) (slot 1344 0x14142E030): false
    // = the chain of CanStyleChange without the anti test, plus: form_time_left > 0 (pawn types other than 2, 3, 7);
    //   cmd 0xAB / 0xAC / 0xAD need form_cmd ∈ 0x94..0x98; 0xAE needs 0x95                       [C; the kind table I]
if !iface.TryStyleFinish(cmd) (slot 192 0x141447840): false
    // refused when the current state id is already 33; pawn.slot934(cmd, 0); SetState(33 STYLEFINISH)
ctrl.slot845(); gauge = 0; decStart = 0; (+0xB50 qword = 0 →) decInterval = 0 **and form_time_left = 0**; points[] = last[] = 0; hud.SetStyleGauge(0)
true
```

The same state gate as 10.3 applies: a finish can be started out of an attack at any moment.

**STYLEFINISH (state 33)** — subclass of the attack state; it inherits `End`-less helpers (facing slot 126, traction
slot 116 / 128, `PreDecide`, hit windows and the request fields of section 5) [C unless tagged]:

```
Reset (0x1413D0CA0) [I, same]: Attack::Reset; +0x28C &= ~7; +0x110 = 0; m_pCatchPawn (+0x280) = 0; sub = 0; +0x158 = +0x15C = +0x16C = +0x174 = +0x17C = 0;
                    pending extra command (+0x288) = 0; +0x260 = 0.2; six vectors +0x290.. = 0
Begin (0x1413CBF70): cmd = pawn "command in action" (slot 959, pawn+0xE2C, set by the executor through 0x1414158F0 before this `MainProc`); unknown cmd or no controller: DoFinish
    bind the per-command tick (delegate at state+0xD0): 0xAB → slot 152, 0xAC → slot 153, 0xAD → slot 154, 0xAE → slot 155   // [I] thunks jmp [rax+4C0h / 4C8h / 4D0h / 4D8h]
    begin event (base slot 103 0x14150A1F0): hud cleared (0x14140BF30), WorldSlow(on), post effect 0x14140DF80(ctrl, 1, 1.0, 1.0), HUD hidden,
        pawn.vt+0x1EB8(1), pawn.slot616(1, 1, 1.0), pawn+0x6E4 = 1, 0x1405B9230(pawn, 1, 1), stop movement, gravity off (slot 741(0)), pawn.slot731(1),
        pawn+0x630 bit 2 = 1, pawn.vt+0x19E0(1)
    state+0x1FC = actor yaw; pawn+0xE60 bit 19 = 0
Tick (0x1413D4550): turn helpers (dt * 12); target (+0x118): dropped when not targetable, then ctrl.slot722() (current target), else ctrl.vt+0x1698()
    done = per-command tick(); if !done: Facing (slot 126); return
    DoChain
End (0x1413CEBD0): end event (base slot 111 0x14150C440)(cmd) [I for the calls]:
        pawn.vt+0x23B8() (clears pawn+0x634 bit 0); hud 0x14157B150(hud, cmd);
        **ctrl.slot859(0, 0)**: if ctrl+0xB60 == 0 → EndStyle(0, 0) (10.7), else +0xB5D = 0
        camera reset 0x14140DF20, ctrl.vt+0x17B8(0), WorldSlow(off), post effect off, HUD shown; movement mode Falling when airborne, gravity on (slot 741(1))
    release m_pCatchPawn; if +0x28C bit 0: remove the extra commands 0xAF, 0xB0, 0xB1, 0xB2, 0xF3; equipment vt+0x1A8(-1, 1); pawn.vt+0x23C8()
```

Per-command ticks (return true when finished; `Play` as above; "extra(cmd)" = slot 162 0x1413E3690: hud
0x14158C180(hud, 0, cmd, 0) shows an extra command in the finish slot and sets +0x28C bit 0; the executor's target
0x14141B981 for kinds 0xAA, 0xAF..0xB2 calls pawn slot 1346 0x14143EE20, which stores the kind in state+0x288
"pending" when the current state is a StyleFinish [I]):

```
Common to the three [I]: ctrl = pawn+0x418; if it is not a valid player controller: DoChain; done.
  Play(name, blendIn, mode) = iface40.vt+0x18(name, 0, blendIn, mode, 0, rate 1.0, …); when it returns a length > 0: iface40.vt+0x1F0(0)
  FINISH = SetMovementMode(3 Falling, true) (pawn.vt+0x17A0); gravity on (pawn.vt+0x1728(1)); GravityScale = class default (pawn.vt+0x1658); DoChain; done
  traction tick = slot 128 (dt) (state.vt+0x400: the delegate of 5.2, i.e. the traction update of 6.3)
  extra(k) = slot 162 (state.vt+0x510);  remove(k, b) = slot 163 (state.vt+0x518)

Stun Impact, slot 152 (0x1413E2630) [I, whole function]:
  sub 0:   m_bFaceAt = 1; Play(OnGround(pawn) ? "M_1421" : "M_1422", -1, 2); sub = 1; return
  sub 1:   if +0x164 (the first hit window has opened): WorldSlow(ctrl, off) (0x14140E500(ctrl, 0)); pawn.vt+0x1048(1, 0, 0); sub = 2
           (falls into sub 2)
  sub 2:   if m_bEnableChange: gravity on; GravityScale = class default; sub = 0xC
           (falls into sub 0xC)
  sub 0xC: if AnimEnded: DoChain; done
  (no Flying mode, no traction tick: gravity was switched off by the begin event and returns at TresEnableChange)

Sonic Rave, slot 153 (0x1413E1FC0) [I, whole function]:
  DASH_SETUP = PreDecide (slot 121); +0x234 = (+0x234 | 1) & ~2 (XY homing channel on, root-scale channel off); z gain (+0x14C) = 3.0;
               +0x21C = +0x220 = +0x224 = +0x228 = 0 (callback mode, channel phases); root length (+0x1D8) = 1100; airMode (+0x1DF) = 1
  sub 0:   +0x234 |= 4; GravityScale = 0.1 (pawn.vt+0x1650); +0x234 |= 0x10; SetMovementMode(5 Flying, false); m_bFaceAt = 1
           Play(OnGround(pawn) ? "M_1423" : "M_1424", -1, 2); sub = 1
           // AS READ [I]: SetMovementMode(5) is the call at 0x1413E2569, OnGround (0x1405AD850 = movement mode Walking) the
           // call at 0x1413E257A, and SetMovementMode writes the mode at once (0x140594A00 [C]): OnGround is false, so the
           // clip is ALWAYS "M_1424"; "M_1423" is unreachable from this function. (Probe request 13.3: clip on the first
           // frame of state 33 for a Sonic Rave started on the ground.)
  sub 1:   until AnimEnded: nothing. Then: DASH_SETUP; install the traction delegate (thunk 0x140430C70 → state+0x90); stop distance (+0x24C) = 0
           Play("M_1425", -1, 2); sub = 5; count (+0x128) = 1; pending (+0x288) = 0; extra(0xAF LMEX_RAVE)
           attack camera 0x14140DF20(ctrl, 0, 0, null, 0), then 0x14140DF20(ctrl, 1, 3, target (+0x118), 0); WorldSlow(ctrl, off)
  sub 5:   facing timer (+0x1F8) = 0
           if m_bEnableChange && pending != 0:                      // the extra command was used (the executor stored its kind in +0x288)
               StopMovement; count += 1; +0x1F4 |= 4 (the facing update skips its next tick); m_bFaceAt = 1; DASH_SETUP; m_bEnableChange = 0
               remove(pending, 1)
               pending == 0xAF: Play("M_1425", -1, 2); extra(count >= 6 ? 0xB0 LMEX_FINISH : 0xAF); pending = 0; sub = 5
               pending == 0xB0: attack camera off (0x14140DF20(ctrl, 0, 0, null, 0)); root length = 1200; Play("M_1426", -1, 2); sub = 0xB
               if target valid: SetRootScale(2 × |target.Location().xy − self.xy| / root length)       // 0x1413E22AB–0x1413E2323: 0x1405A1D30(pawn+0x440,
                                                                                                    // s, 1.0, null); self = slot 134; no clamp
           elif AnimEnded: attack camera off; FINISH
           else: traction tick
  sub 0xB / 0xC: (0xB: facing timer = 0) if AnimEnded: if +0x28C bit 0: remove(count >= 6 ? 0xB0 : 0xAF, 0); FINISH
                 else: traction tick

Last Arcanum, slot 154 (0x1413E1800) [I, whole function; jump table 0x1413E1FA0]:
  sub 0:   +0x234 |= 4; +0x164 = 0; GravityScale = 0.1; +0x234 |= 0x10; SetMovementMode(5 Flying, false); PreDecide; install the traction delegate
           m_bFaceAt = 1; +0x234 = (+0x234 | 1) & ~2; stop distance = 160; z gain = 3.0; callback mode and channel phases = 0; root length = 700; airMode = 1
           Play("M_1427", blendIn 0, mode 0); count (+0x128) = 0; finishEnd (+0x141) = 0; sub = 1; pending = 0
  sub 1:   if count == 1: WorldSlow(ctrl, off)
           if count >= 6: pending = 0; extra(0xB1 LMEX_BREAK); sub = 5
  sub 5..10, only when m_bEnableChange && pending != 0:
           StopMovement; m_bFaceAt = 1; PreDecide; m_bEnableChange = 0; +0x234 = (+0x234 | 1) & ~2; z gain = 3.0; callback mode and channel phases = 0;
           airMode = 1; +0x164 = 0; remove(pending, 1); then by sub-step:
             5: root length 200; Play("M_1428", -1, 2); sub = 6;  extra(0xB1)
             6: root length 190; Play("M_1429", -1, 2); sub = 7;  extra(0xB1)
             7: root length 180; Play("M_1430", -1, 2); sub = 8;  extra(0xB1)
             8: root length 160; Play("M_1431", -1, 2); sub = 9;  extra(0xB1)
             9: root length 150; Play("M_1432", -1, 2); sub = 10; extra(0xB2 LMEX_THEEND)
            10: root length 200; name = "M_1433"; ground probe 0x14063A650(pawn, 0, 800): if its distance > 50: name = "M_1434", root length = 330
                Play(name, -1, 2); sub = 0xB
           pending = 0
  every tick, every sub-step (tail 0x1413E1E9D):
           if !AnimEnded: traction tick; return
           if +0x28C bit 0: remove(finishEnd ? 0xB2 : 0xB1, 0)
           SetMovementMode(3 Falling, true); gravity on; GravityScale = class default; (pawn+0x598) sub-object +0x200 .vt+0x1A8(-1, 1); pawn.vt+0x23C8(); DoChain; done
```

Read at instruction level at the merge; what changed against the decompiled-C reading and what stays open: 10.9. Clips and hit windows of `M_1421`–`M_1434`: sheets `sora-combat-form-clips` / `sora-combat-hits` [A]. Rows [A]: `LM_STUNIMPACT` power
2.0, STUN 100 % for 8 s, reaction power 5, armor 10; `LM_SONICRAVE` 1.2 per dash (HIT2, `KB_StickL`),
`LM_SONICRAVE_END` 1.5 `TOSS`; `LM_LASTARCANUM` 0.8 × 6, `LM_LASTARCANUM2` 1.2, `LM_LASTARCANUM_BREAK…5` 0.8 each,
`LM_LASTARCANUM_END` 0.1 multi-hit (re-hit 0.1333 s), `LM_LASTARCANUM_END2` 1.5 `BEAT`.

### 10.7 End of the form

`EndStyle(bKeepPose, bDeleteGrand)` = ctrl slot 832 0x14141CA20 [I for the 0x94 case and the tail]:

```
ctrl.slot791(1); ctrl.slot840(); ctrl.slot845(); 0x1414145C0(ctrl, 0, null, 0, 0); 0x1414142D0(ctrl, 0); ctrl+0x878 &= ~0x100
form ∈ 0x94..0x98:
    pawn.slot1295(0, 0xCB2)                              // pawn+0x10D8 bit 1 = 0; pawn+0x10DC = 0xCB2 [? purpose]
    delete commands 0xAB, 0xAC, 0xAD, 0xAE; hud 0x14157B130(hud, form, 0) (banner out); SetFinishLevel(0)
if bDeleteGrand: delete commands 0x7E, 0xE7
decInterval = 0; form_time_left = 0; form_level = 0; form_cmd = 0; ctrl+0xB60 = 0; gauge = 0; decStart = 0; points[] = last[] = 0; hud.SetStyleGauge(0)
0x1405B9230(pawn, 0, 1)                                  // bad-status block off
pawn+0xE60 &= ~0x40000; pawn+0xE60 bit 19 = 0
if !bKeepPose:
    pawn.vt+0x13E0(1, 0, 0, 0.2)                          // equipment back to the base variant: weapon form 1 → wear form NORMAL (abilities removed) [C]
    if pawn.vt+0x1330(): in IDLE with Idle+0x78 == 0: play "BattleStart" and mark it; in RUN: re-pick the run clip (slot 910) [C]
    a ShooterMode state is left through iface.vt+0x560 [C]
```

What calls it:

| Cause | Path | Evidence |
|---|---|---|
| time out | `StyleGaugeTick`: `form_time_left <= 0 && CanEndStyle()` → `EndStyle(0, 0)` | I |
| a finish command was used | `StartStyle` zeroes `form_time_left`; the StyleFinish end event calls `ctrl.slot859(0, 0)` → `EndStyle(0, 0)` when the state ends | I |
| scripts / other modes | `BP_SetAbilityStyleChange` 0x1413EABF0, `DebugSetStyleFix`, attraction and link starts (callers of vt+0x1A00: 0x140507A00, 0x1412476C0, 0x1412546D0, 0x1413AED70) | call sites listed, not read |

`CanEndStyle()` = ctrl slot 857 0x141401D10 [I, both jump tables decoded]: false while the **queued next** state id or
the **current** state id is in {12, 13, 14, 15, 16, 18, 20, 31, 33–39, 75–90, 156, 157, 159, 160, 161} (attack
states, GUARD, DODGEROLL, FIREMAGIC, AIR_SLIDE, STYLECHANGE, STYLEFINISH_*, COMBO_*), or pawn+0xE60 bit 17 is set;
true otherwise. So a form whose time ran out during a combo ends on the first frame Sora is idle, running, jumping,
falling, hit …; the `+0xB78` bit 3 "time up" flag is raised every frame meanwhile.

Other effects on the form time [C]: an enemy attack row with bad status `TRES_BADSTAT_TIME_BREAK` (19) reaches
0x1405B7C50 → pawn slot 1052 0x141424700 → ctrl slot 876 0x1413E9D30(t): `form_time_left -= t` (not below 0), the
action-command time +0xB6C likewise, and the HUD breaks its timers (0x14158EC10 → 0x1416303E0). The countdown stops
while `+0xB78` bit 0 is set (`BP_NotifyPauseSituationTimer`, called by level scripts) and while ctrl+0xB60 "style
fix" is set (then also: no formchange command is enqueued and a finish does not end the form).

The combo does **not** carry across the transformation: STYLECHANGE and STYLEFINISH never set the carry timer
pawn+0x10E4 (see corrections), so the first attack in the form and the first attack after it start at combo 0.

### 10.8 Implementation checklist and tests [D]

State to add to the port: gauge {value, pending, chain, decStart, decInterval}, form {cmd, level, timeLeft, timeFull},
finish {level, cmd, pending flag}, flags {paused, styleFix}; pawn finish counter (already section 4). Events to the
host: gauge ratio, command added / removed, form banner with time, wear form 5 on / off (three texture swaps),
world-slow on / off (mask of object types at 0.1), `StyleChange` blast shape on the pawn.

| Test | Expected |
|---|---|
| base form, 25 hits of `KB_VERTICAL`-class rows one per frame apart, no abilities | each adds `m_FormPoint × 1.0`; two hits in the same frame add 1 + ½ |
| no hit for 20 s, then wait | −5 points at 20 s, then every 1 s; arrays cleared at 0 |
| gauge reaches 100 (no magic points) | gauge 0; command 148 in the list with 30.0 s (36 s with one FORM_TIME) |
| Triangle during `1301` before `TresEnableInput` | accepted: queue cleared, STYLECHANGE next `MainProc` |
| transformation | 1401 (0.8333 s) → 1402 (1 tick) → 1403; world slow until 1403 starts; blast 550 cm 0.3667–0.9667 of 1403; `TresEnableChange` 0.9667 |
| after it | finish slot shows 171; form time ≈ 30 − 2.25 s; attack state 90; MaxCombo 6 |
| ground combo on one near dummy | kinds 1, 2, 3, 7A, 7B, 7C; clips `1301` `1302` `1303` `M_1412` `M_1411` `M_1413` |
| air combo, dummy level | 10, 17, 12, 7D, 7E, 7F |
| gauge full twice more in form | 172 then 173; form time +3 s each (cap 30); gain per hit × 0.8 |
| Triangle on 173 | STYLEFINISH; `M_1427`; after the sixth hit window extra 177; five presses → `M_1428`…`M_1432`; extra 178 → `M_1433` / `M_1434`; form ends when the state ends |
| form time reaches 0 during a combo | form ends on the first frame outside the listed states; wear form NORMAL, gauge 0, finish command gone |

### 10.9 The three finish-command bodies: what the instruction-level read settled

The per-command ticks of 10.6 — Stun Impact 0x1413E2630 (468 bytes), Sonic Rave 0x1413E1FC0 (1639 bytes), Last Arcanum
0x1413E1800 (1949 bytes), all verdict "differs" — were read whole in the disassembly at the merge
(`build/sora_tmp/combat/P2F/1413E2630.dis`, `1413E1FC0.dis`, `1413E1800.dis`); the clip names are the UTF-16 strings at
0x14693D960…0x14693DA40 ("M_1421" … "M_1434"). The pseudocode of 10.6 is the result. Against the decompiled-C reading of
strand F:

| Point | C reading | Instruction level |
|---|---|---|
| structure, sub-step numbers, clip per step, extra-command sequence | as given | confirmed |
| root lengths | 1100 / 1200 (Sonic Rave); 700, 200, 190, 180, 160, 150, 200 / 330 (Last Arcanum) | confirmed (0x44898000, 0x44960000; 0x442F0000, 0x43480000, 0x433E0000, 0x43340000, 0x43200000, 0x43160000, 0x43480000 / 0x43A50000) |
| stop distance | 0 (Sonic Rave), 160 (Last Arcanum) | confirmed; written once (sub 1 / sub 0), not per step |
| "count > 5" | — | `count >= 6` in both (`cmp [state+0x128], 6` / `jl`): Sonic Rave offers `LMEX_FINISH` from the dash that makes the count 6 (first dash = 1, so five `LMEX_RAVE` presses); Last Arcanum offers `LMEX_BREAK` when the count reaches 6 |
| Sonic Rave "with a target: root scale from the distance" | unspecified factor | **`2 × dxy / root length`**, set once at each dash start with the un-led lock-on position; vertical scale 1.0; the root-scale *channel* of the traction update is off (bit 1 cleared), so the value stays for the whole dash |
| Sonic Rave, each further dash | — | begins with `StopMovement`, sets state+0x1F4 bit 2 (facing skips one tick) and `m_bFaceAt` |
| Sonic Rave, `LMEX_FINISH` | — | `pending` is not cleared on this path; the attack camera is switched off first |
| Last Arcanum, `M_1427` | "start 0, mode 0" | blend-in 0.0 and mode 0 (the other clips: blend −1, mode 2) |
| Last Arcanum, end clip | "`M_1434` when the ground probe reports more than 50 cm below" | `M_1434` (root length 330) when the probe (range 800) returns a distance > 50; the result is pre-set to 0, so **no ground within 800 cm gives `M_1433`** [the probe's result layout: C] |
| Last Arcanum, extra removed at the end | — | `LMEX_THEEND` (0xB2) when `finishEnd` (state+0x141) is set, else `LMEX_BREAK` (0xB1) |
| Stun Impact | as given | confirmed; `pawn.vt+0x1048(1, 0, 0)` at the first hit window; ends by `DoChain` alone (no mode change) |

Settled by the fourth pass (2026-10-12; the numbered list below is kept for reference):

- (1) `count`: the StyleFinish class overrides the `TresAtk` begin handler of the state's anim interface (slot 64 of vtable
  0x146947F60: **0x1413DD7D0** [I, whole function]): `Attack's handler (0x1413459E0); if pawn.vt+0x1DF8() == 0xAD: inc dword
  [state+0x128]`. One per hit window that opens, in every sub-step of Last Arcanum: `M_1427` has seven windows, the count is
  1 at 0.3 s (world slow off) and 6 at 1.4333 s (`LMEX_BREAK` offered). (The other overrides of that vtable: slot 69 the shot
  handler 0x1413DD180, slots 105 / 109 / 120: other forms' commands.)
- (2) `finishEnd`: **no writer**. Byte scans of 0x141330000–0x141350000 and 0x1413C8000–0x1413E6000 for every store form on
  +0x141 (and +0x101 / +0xF9 through the sub-objects) find only `Decide` and its form variants, which the finish commands do
  not call. It stays 0: the extra removed at the end is asked for as `LMEX_BREAK`, and no hit of Last Arcanum carries the
  `finish` flag.
- (3) state+0x1D8 is read only at 0x14134A4B2 / 0x14134A56F (the root-scale channel of 6.3): Last Arcanum's root lengths are dead.
- (4) `pawn.vt+0x1048(a, b, c)` 0x1405D33C0 → controller interface (ctrl+0x828) slot 4 0x141444270 → 0x144C73F00 =
  `ClientPlayForceFeedback(effect table[a] of the game object, bLooping b, Tag c)` [I: the name static 0x149E02628 is initialised
  from "ClientPlayForceFeedback"]: Stun Impact's blow is rumble effect 1 (a guard hit: effect 0). `pawn.vt+0x23C8()`
  0x1405A8610 = `EndEffect` on the pawn's effect interface (group pawn+0x900 for pawn type 6; else 0xC62 / 0xC48 by the state
  of the pawn+0x460 interface) and `(pawn+0x598)+0x200 .vt+0x1A8(−1, 1)` = `EndEffect(all groups)` on the weapons
  (`docs/sora-vfx-spec.md` 4.2): the attached effects of the finish are ended.
- (5) the attack camera: 7.12.1 (mode 3 = the lock-on camera on the dash's target).
- `Tick`'s stick block (0x1413D4550, [C]): after Facing, `if state+0x16C`: with the stick active (+0x158) and its magnitude
  (+0x160) ≥ 0.5: yaw +0x17C turns toward the stick yaw +0x15C by clamp(dt × 6, 0, 1), factor +0x174 = min(1, +0x174 + dt × 7.5),
  velocity = pawn.vt+0x1B30() × dir(yaw) × factor; else factor ×= 0.5, velocity = pawn.vt+0x1580() × dir × factor. state+0x16C is
  written only by Reset (0) and slot 144 0x1413DED90 (another form's command): **dead for the three Kingdom Key commands**.
- Begin / end events at instruction level: begin 0x14150A1F0 = hud clear 0x14140BF30, WorldSlow(ctrl, 1), post effect
  0x14140DF80(ctrl, 1, …, 1.0), 0x141413910(ctrl, 1, 0), pawn.vt+0x1EB8(1), **pawn.vt+0x1388(0, 1, 0)**, pawn.vt+0x1340(…, 1, 1.0),
  pawn.vt+0x1780(1), 0x1405B9230(pawn, …, 1), vt+0x16E8(), vt+0x1728(0), vt+0x16D8(1), vt+0x2068(1), vt+0x19E0(1),
  **MyBodyColl.vt+0x720(8)** (the body-push level is 8 during a finish: 6.5), state.vt+0x2D8, a world call. End 0x14150C440 =
  pawn.vt+0x23B8() (restores the push level among others); with a controller: cmd ≠ 0xB3: hud 0x14157B150(hud, cmd) and
  ctrl.slot859(0, 0); attack camera off; ctrl.vt+0x17B8(0); WorldSlow(ctrl, 0); post effect off; 0x141413910(ctrl, 0, 0); then the
  pawn calls and Falling when airborne, as in 10.6.
- Gates against states 31 and 33 [I, each function's jump table on the current state id]: jump (slot 41 0x14142A3D0), cast
  (`CanMagic` slot 52 0x1414286F0), guard (slot 54 0x1414294C0), dodge roll (slot 57 0x141427CC0), air slide (slot 68
  0x141425780) and attack (slot 44 0x141426330) all send 31 STYLECHANGE and 33 STYLEFINISH to their default target = false;
  guard, dodge and air slide refuse also while one of the two is queued. Nothing but a situation command (and a hit) gets out
  of them.

1. **Who advances `count` (state+0x128) during `M_1427`** (Last Arcanum sub 1 waits for 1 and for ≥ 6): no instruction of
   the StyleFinish code range (0x1413C8000–0x1413E4000) writes it except the two functions above (searched: `inc` / `add` /
   `mov` forms on +0x128 and on +0xE0 of the notify sub-object), and slot 115 is not overridden. Strand F read it as "one
   per hit window" (`M_1427` has seven windows); the writer is expected in a notify handler of the base class. Probe:
   state+0x128 over `M_1427`.
2. **Who sets `finishEnd` (state+0x141)** in Last Arcanum (it is cleared in sub 0 and only read at the end).
3. In Last Arcanum the root lengths are written while the root-scale channel is off and no explicit scale call exists
   (unlike Sonic Rave): within the traction update (6.3) they have no reader. Whether `PreDecide` or another slot reads
   state+0x1D8 here was not checked; the port can carry the values without using them.
4. `pawn.vt+0x1048(1, 0, 0)` (Stun Impact), `pawn.vt+0x23C8()` and the equipment call `(pawn+0x598)+0x200 .vt+0x1A8(-1, 1)`
   (Last Arcanum end; also in the StyleFinish `End`): not identified.
5. The attack-camera calls `0x14140DF20(ctrl, 1, 3, target, 0)` of Sonic Rave (mode 3) belong to the "combo camera" of
   7.12 (last paragraph); its behaviour is not specified.
6. Slot 155 (0x1413E2810, command 0xAE of form 0x95) was not read (not the Kingdom Key).

---

## 11. Situation commands

The container, the insert, the selection, L2, Triangle and the timers were read for the magic spec
(`docs/sora-magic-spec.md` part J section 5) and the UI spec (`docs/sora-ui-spec.md` 5.6–5.8). This section
restates what a Sora port needs, with what this pass re-read (tagged [I]) and what it adds (classes per command,
the finish slot, pause, producers).

### 11.1 Containers [I]

The queue is in the HUD part `TresUIP_HudCommand`, not in the controller; the controller only keeps what it is about
to execute (ctrl+0x87C) and the form bookkeeping of section 10.

| Container | Holds | Counted | Countdown |
|---|---|---|---|
| list `TArray` at hud+0x760 (entries of 0x58 bytes, number +0x768, counted entries +0x6F8) | formchange, grand magic, attraction, team attack, link, shotlock entries; plus the style-gauge entry (entry+0 == 3, class 9), always kept last | yes (not the gauge entry) | yes |
| slot A hud+0x770 | one context command (Type 4 Cate 13: save, talk, open …) or reaction command entered as an "ex kind" (below) | no | no (appear time only) |
| slot B hud+0x7C8 (stack at +0x820) | one finish command (Cate 15 / 16); not shown while slot A is occupied | no | **no**: the entry lives with the form time |

Reaction commands 1–26 (`TresCmdData.Type` 1) never enter the list [C]: their kinds are outside the jump table of
`NotifySetActionCommand` (11.2). They are owned by the controller's reaction prompt 0x1414145C0(ctrl, time, component,
…) — the function that also holds the counter window ctrl+0x8A0 (5.6.1) and the prompt kind ctrl+0x8A4 — and reach the HUD
as type-2 "ex kind" entries through 0x141575640 → 0x1416158F0(hud, 2, 0, exKind, time, info) and 0x14158C180: ex kinds
1–3 classify as 5 and 5 / 9 / 13 as 6 and take the single-slot path (slot B), the others class 2 (slot A). Their
lifetime is the controller's prompt timer, not a list countdown. Details: 5.6.1 and 9.5.

Class of a command (0x141619E60; Type-1 mapping and the "not counted" mask 0x1854 = classes 2, 4, 6, 0xB, 0xC read
in the disassembly, the Cate chain at C level): sheet `sora-combat-queue-classes`. Entry layout: `docs/sora-magic-spec.md` J 5.1
(+0x04 class, +0x08 kind, +0x14 total, +0x18 remaining, +0x1C start delay, +0x20 appear time, +0x34 serial).

### 11.2 Enqueue [I]

`NotifySetActionCommand(kind, time, info)` 0x14140DA40 (jump table 0x14140DDDC decoded):

```
t = (row.Type == 4 && row.Param0 > 0) ? row.Param0 : 0
grand magic (30..32, 34..36, 38..40, 42..44, 46..48, 110..126, 231):  t *= 1 + 0.2 * AbilityCount(0x48 MAGIC_TIME)
attraction 186..191:                                                  t *= 1 + 0.2 * AbilityCount(0x2F ATTRACTION_TIME)
formchange 128..153, 221..224: if ctrl+0xB60 != 0: return;            t *= 1 + 0.2 * AbilityCount(0x31 FORM_TIME)    (154: ctrl.slot901() first)
team attacks 193..210, 237..240, 242: if time != -1: t = time; time = -1;  t *= 1 + 0.5 * AbilityCount(0xC5 COMBI_TIME) of the friend (info+0x40); ctrl.slot901() (friend points = 0)
171: delete 172, 173; 172: delete 171, 173, 174; 173 / 174: delete 171, 172
192 (AF_FINISH): time = 0 when the roller-coaster state says so; 109: time = 0
kinds outside the table (e.g. 33, 37, 41, 45, 49..74, 85, 86, 95..98, 101, 127, 155, 170, 175..178): return, nothing is enqueued
if time != -1: t = time
t = min(t, 99.9)
hud.AddCommand(kind, t, info)             // 0x1415755A0 → 0x1416158F0(hudCommand, 1, kind, 0, t, info) when the cockpit mode is 0
```

Removal by the controller: `NotifyDeleteActionCommand(kind)` slot 688 0x14140C790 → 0x14157A590 → 0x14161B910.

### 11.3 Insert, capacity, order [I 0x1416158F0 for the constants]

- A kind already present is refreshed in place (time reset; with OVER_TIME the longer time is kept [C]): never two
  entries of one kind.
- Capacity: **6 counted entries** (`cmp ebx,6` at 0x141615B47; the style-gauge entry is not counted and is hidden
  while the list is full). The 7th first removes the **first entry in list order, among indices 0..5, whose total
  time is ≥ 0.1 s** (untimed entries are skipped), then is appended. The gauge entry itself is refused at 7.
- New entry: total = remaining = t (t ≤ 0 → −1 = untimed), start delay 0.5 s, appear time 0.2 s, serial = hud+0x9CC++.
  Position: end of the list, before the gauge entry.
- Classes 2 / 0xB / 0xC replace slot A; classes 4 / 6 go to slot B.

### 11.4 Timers and pause

Per HUD tick 0x141630AF0 [C, verdict same-partial; `docs/sora-magic-spec.md` J 5.6 has the instruction addresses]:
skipped entirely while hud+0x751 is set or a command is executing (hud+0x9B8 / +0x9B9). List entries: appear time
counts down in real dt; the start delay (0.5 s) runs first; then `remaining -= dt × time dilation of the player's
object type`; at 0 (and grace ≤ 0) the entry is removed and the pawn's notify interface (pawn+0x448 vt+0x1D0) is told
the kind. Slots A and B only tick their appear time and start delay.

Controller-side clocks [I]: the form time ctrl+0xB54 is counted by `StyleGaugeTick` with the controller's dt, not by
the HUD; it stops while ctrl+0xB78 bit 0 (`BP_NotifyPauseSituationTimer`) or ctrl+0xB60 is set and is not affected by
hud+0x751. `BP_NotifyPauseActionCommandTimer` (slot 696 → vt+0x1598) pauses the HUD side [C, not read].
`TIME_BREAK` drains both (10.7).

### 11.5 Selection, L2, Triangle

[C; re-read at C level: 0x141627B70]. Default selection: slot A if occupied and past its appear time; else slot B;
else, among list entries that are selectable and past their appear time, the smallest serial when hud+0x9C4 == 1
("Situation Commands: Prioritize Older", the default) or the largest when 0 ("Newer"). A choice made with L2
(hud+0x954 / +0x6FC) wins while its entry exists. L2 (0x14162BAF0): next selectable list entry, wrapping through the
single slot (index −1). Triangle (0x1416220D0 → `CanExecuteCommand` slot 682 0x141401F80 → `ExecuteCommand` slot 680
0x14140EAA0 → executor 0x14141B470): the entry is removed only if the executor returned true; a refused use leaves
it (no buffer); classes 3, 5, 7, 0xB hide the menu until the action ends.

**AUTO_FINISH (ability 70)** [C 0x141618830, called from the command menu update 0x14162FF80; third pass 2026-10-12]:

```
controller valid && hud+0x708 != 0 [?: "in combat" by the help text] && hud+0x9BB (has(70), refreshed by 0x141616B10)
&& menu state hud+0xAC8 ∈ {1 MainSel, 2 SubSel, 3 Shortcut, 5 Attraction}
&& kind = hud+0x7D0 (slot B's command) != 0 && the slot-B stack (hud+0x820, count +0x828) is not empty && its first entry+0x18 (remaining) <= 0.5
&& kind ∉ {0x6D, 0xB3, 0xD3, 0xF1} && hud+0x73F == 0 && (hud+0x806 || kind != 0xC0)
&& ctrl.CanExecuteCommand(kind, 0) && ctrl.ExecuteCommand(kind):   0x14161B910(hud, kind, 0, 1)        // removed as used
```

The finish command's entry lives with the form time, so with the ability the finish command fires by itself half a second
before the form would end. Port: `combat/situation.rs` `auto_finish` (the battle flag stands in for hud+0x708).

For Second Form: while the form runs, the finish command (slot B) is the default Triangle target and a formchange /
grand-magic entry in the list needs L2 first; with a reaction command in slot A that one comes first.

### 11.6 Producers (stubs for a Sora-only port)

| Producer | Condition | Command, time | Sora-only |
|---|---|---|---|
| formchange | gauge full, no form, nothing rolled (10.2) | 148, 30 s × FORM_TIME | yes |
| finish | form start; full gauge in form (10.6) | 171 → 172 → 173, slot B, form time | yes |
| finish extras | inside Sonic Rave / Last Arcanum | 175–178, slot B extra | yes |
| grand magic | gauge full: `RollGrandMagic` 0x1413ED500 (magic spec I 7.2) | 110–125, 20 s × MAGIC_TIME; using it clears gauge and arrays | yes |
| reaction commands 1–26 | guard, damage, risk dodge, friend (5.6, 9.5): controller prompt 0x1414145C0, HUD ex-kind entry 0x141575640 | single slot (A or B), never the list | 1–21 yes |
| context commands by script | `BP_NotifySetActionCommand1(kind, time, actor, component)` slot 694 0x1413EA480 → `NotifySetActionCommand` | slot A | needs level scripts |
| context (save, talk, open …) | level objects | slot A | needs level objects |
| attraction | attraction marks on enemies; `BP_SetRestrictAttraction` | 186–191, 40 s × ATTRACTION_TIME | no |
| team attack | friend gauge (ctrl+0xC0C / +0xC10, `MyFriendPointMax` 200, `MyFriendPointTime` 30) | 193–210, 30 s × COMBI_TIME | no |
| link | link menu | 69–75 | no |

Restrictions [C]: `BP_SetRestrictSituation` (slot 660 0x1413EB550) / `BP_SetRestrictStyle` (slot 664 0x1413EB630) set
pawn restriction bits (pawn+0xE60: bit 3 situation per the magic spec); with hud+0x73E set only slot-A context
commands can be used.

### 11.7 HUD events for the host [D from 10 and 11, with `docs/sora-ui-spec.md` 5.6–5.8]

`gauge(ratio)`, `command_added(kind, class, total, list index | slot)`, `command_refreshed`, `command_removed(kind,
used | expired | evicted | deleted)`, `selection(index | -1)`, `form_banner(kind, total, left)` / `form_banner_out`,
`finish_extra(kind | none)`.

---

## 12. Host interface

What the combat code reads from the world and what it sends out, as an abstract seam. Four parts; the
first is section 7.14 and is not repeated here.

| Part | Where | Content |
|---|---|---|
| targets | 7.14 | `trait LockonTarget` (one per lock-on component), `trait LockonHost` (target list, line test, cursors, enemy gauge, camera lock), per-frame inputs and outputs of the lock-on update. Revision 1's `CombatTarget::radius()` does not exist: the field is `auto_lock_on_range()` |
| hits Sora deals | 12.1 | `struct Hit`, `trait CombatVictim` |
| hits Sora takes | 12.2 | `struct HitOnSora`, events |
| gauge and commands | 12.3 | style gauge, form, command queue events |

Movement seam (unchanged from the movement spec): the host supplies collision queries (ground probe,
sweeps); the hit shapes' socket transforms come from the port's own animation of Sora and the keyblade
(`ATK_00`, `ATK_01`, `Z_00`, `vfx_cure`, `center`). New in revision 2 (6.5): per enemy a **push shape
and a push level** (or a fixed class "immovable / equal / light"; default "equal", weight 0.5); without
it Sora passes through enemy bodies, exactly as his capsule does in the game.

### 12.1 Hits Sora deals [from 8.2.5–8.7]

```
struct Hit {                                   // = FTresPointDamageEvent + the call arguments
    amount: f32, attacker: ActorId, direction: Vec3,               // 3D unit vector, 8.7 (not flattened)
    time: f32, location: Vec3, impact_point: Vec3, normal: Vec3, impact_normal: Vec3, bone: Name,
    row: &'static str, command_kind: u8, damage_kind: DamageKind, attribute: Attribute,
    power: f32, power_scale: f32, parabola_angle: f32, move_length: f32, brake: f32, effect_time: f32,
    knockback: KnockbackType,
    bad_status: (BadStatus, f32 /*time*/, f32 /*param*/),          // already rolled against m_BadStatusGenRate
    reaction_power: i32, armor_attack_power: i32, form_point: i32, revenge_point: f32, phys_force: u8,
    flags: { magic, finish, killer, ignore_guard, rapid_fire, armor_damage, aerial,
             attraction, counter, just_guard, weak_guard, full_mp_burst, magic_draw, no_react_body_correction },
    counters: [u8; 4], reflect_level: i8, reflect_reaction: ReflectReaction,
    victim_result: ReactionResult,             // what the victim must play: DAMAGE, NOREACTION, GUARD, GUARD_REFLECT, REFLECT, ARMOR_BREAK, STRONG_BODY_DAMAGE1..4
}
trait CombatVictim {                           // asked before the hit is built (8.2.6)
    fn collision_enabled(&self) -> bool;
    fn team_same_side(&self, attacker: ActorId) -> bool;
    fn immune_to_kind(&self, kind: DamageKind) -> bool;
    fn body_reaction(&self, shape: ShapeId) -> BodyReaction;       // DEFAULT, NO_REACTION, STRONG, GUARD, WEAK_GUARD, GUARD_NOREACT, INVINCIBLE(_NOREACT), GHOST(_NOREACT), REFLECT1, GHOST_INVINCIBLE
    fn guarding(&self) -> bool;                                    // turns DEFAULT / WEAK_GUARD / unweakened STRONG into GUARD
    fn attribute_weak(&self, a: Attribute) -> bool; fn attribute_nullified(&self, a: Attribute) -> bool;
    fn armoured(&self) -> bool; fn take_armor(&mut self, power: i32, weak: bool) -> i32;      // returns what is left
    fn take_reaction_power(&mut self, power: i32) -> i32;                                     // STRONG bodies
    fn stats(&self) -> VictimStats;                                // 8.3: defense, resist[8] (percent), second resist, optional floor / cap, kind
    fn veto(&mut self, hit: &Hit) -> bool;                         // false = the hit did not happen
    fn take_hit(&mut self, hit: &Hit) -> i32;                      // HP actually lost → drives style points
    fn touched(&mut self, hit: &Hit);                              // result != NONE but no damage (blocked, armoured)
}
```

There is no `HitResult` returned by the victim (revision 1 had one): the outcome is computed by the
attacker's component from the victim's answers before the call (8.2.6); "ignored" has two forms —
GHOST body types drop the hit without recording the victim, INVINCIBLE types record it for the
attacker (blocked effect, no damage).

Sora → host events per hit: effect by name at the impact (8.6), hit sound (8.6: none when the
attacker result is REFLECT, none from the window's sound on a map hit), `style_points += f(form_point)`
only when HP was lost (8.5.2 → 10.2), target HP gauge refresh, the recoil request when
`attacker_result` ∈ {REFLECT, GUARD_REFLECT, GUARD} (5.6.3). **No hit-stop, no shake, no rumble, no
focus.** Other Sora → host events of the attack state: swing sound alias (SWING1–9), effect attach /
detach, voice.

### 12.2 Hits Sora takes [from 8.4, 9]

```
struct HitOnSora {                         // what the Damage state reads from the hit
    amount: f32, causer: ActorId, instigator_pos: Vec3, instigator_forward: Vec3, instigator_radius: f32,
    dir: Vec3,                             // shot direction; zero = "push along the attacker's facing"
    damage_kind: u8, attribute: u8,
    power_scale: f32, brake: f32 /* 0 = table */, parabola_angle: f32 /* 0 = 45 */, move_length: f32, effect_time: f32,
    bad_status: (u8, f32 /* time */), reaction_type: u8 /* info+0x60: 2 normal, 1 no reaction, 3..6 no damage */,
    no_kill_class: u8 /* info+0x66 */,
}
host -> Sora:  take_hit(HitOnSora);  landed event (already in the movement seam);  blocking-hit event with the wall normal
Sora -> host:  HpChanged(old, new), DamageNumber(n), ReactionStarted(kind), ReactionEnded, ReactionCommand(kind, seconds) /
               ReactionCommandCleared, PlayerDied(causer), AutoRevived, voice / effect notifies of the damage clips
abilities read: AIR_RECOVERY 1, BLOW_COUNTER 2, LAST_LEAVE 44, COMBO_LEAVE 45, BATTLE_GRAPHER 98; item count 0xA01
```

`reaction_type` is the victim result of 8.2.6 (`ETresReactionResultType`: 2 DAMAGE, 1 NOREACTION,
3 REFLECT, 4 GUARD_REFLECT, 5 GUARD, 6 ARMOR_BREAK) as the attacking side resolved it against Sora's
hurt shape; a host whose enemies do not run that classification passes 2, or 5 / 4 while Sora's guard
shapes are the ones hit.

### 12.3 Gauge, form and commands [from 10, 11]

State the port keeps: gauge {value, pending, chain, decStart, decInterval}, form {cmd, level,
timeLeft, timeFull}, finish {level, cmd, pending flag}, flags {paused, styleFix}, the command list (6
counted entries + slots A and B). Events to the host (11.7): `gauge(ratio)`, `command_added(kind,
class, total, list index | slot)`, `command_refreshed`, `command_removed(kind, used | expired | evicted
| deleted)`, `selection(index | -1)`, `form_banner(kind, total, left)` / `form_banner_out`,
`finish_extra(kind | none)`, wear form 5 on / off (three texture swaps), world-slow on / off (object
types at rate 0.1), the `StyleChange` blast shape on the pawn.

---

---

## 13. Confidence, open questions, probe requests

### 13.1 Function table

Verdict from `sheets/impl/progress` ("same" = the decompiled C was proven identical; "differs" / none = the
disassembly governs); "Read" = how this spec used the function. Sorted by address; one row per function or
group of functions.

| Address | Function | Verdict | Read |
|---|---|---|---|
| 0x140561F40 / 0x140564B10 | attack notify begin / end | same | disassembly |
| 0x14056B420 | anim instance root-scale setter (vt+0x428) | differs | disassembly |
| 0x1405795F0 | `CalcVelocity` (skip flag 0x140579658, tail 0x14057A286) | see movement spec | disassembly |
| 0x14057ABD0 | movement slot 414 (body push into the move) | — | disassembly |
| 0x14057AE30 | movement slot 404 (encroach check) | — | C |
| 0x140592DD0, 0x140592E10, 0x140592E40, 0x1405795F0 (tail) | direct velocity setters, `CalcVelocity` tail | — | disassembly |
| 0x1405A2A00 | `ApplyHP` | differs | disassembly to 0x1405A2CE8, C after |
| 0x1405A94E0 / 0x1405A9790 | direction index 4-way / 2-way | — | disassembly |
| 0x1405AF5B0 | pawn tick (Withstand Combo timer) | differs | disassembly of 0x1405AF814–0x1405AF84A |
| 0x1405B3FE0 / 0x1405B2D80 / 0x141787280 | pawn interface begin / end, weapon fan-out | same | disassembly |
| 0x1405BB850 | `TakeDamage` | differs | C; predicates in disassembly |
| 0x1405BE820 | item effect (auto-revive branch) | — | C; 1.0 in disassembly |
| 0x1405DEFA0 → 0x1405E0590 | two-point framing | differs | call structure only |
| 0x1405F0140 | camera manager `SetLockon` | no-verdict | C |
| 0x1405FC7E0 / 0x1405FC450 | `TresCameraNormalLoc` / `TresCameraNormal` constructors | — | disassembly (lock-on defaults) |
| 0x1405FFA60 | auto follow, lock-on branch | same-partial | disassembly of the branch |
| 0x140606190 / 0x140607390 / 0x140613300 | lock-on base rotation / stick scale / look-at | differs / no-verdict / differs | C, constants in the disassembly |
| 0x14060AB70 / 0x14060A930 | normal camera `SetLockonTarget` / parameter copy | no-verdict | C, blend constants in the disassembly |
| 0x14060EDB0, 0x140611170, 0x1406100D0 | lock-on camera update, rotation step, position step | no-verdict / differs / no-verdict | C, constants in the disassembly |
| 0x140618940 | apply | same-partial | per-hit loop in disassembly (0x140619650–0x14061A2E3), clash result in disassembly, rest C |
| 0x14061EA30 | knockback direction | same-partial | dispatch and constants in disassembly, bodies C |
| 0x14061FE20 | guard / reaction classification | no-verdict | disassembly (whole, with jump table) |
| 0x1406217F0 / 0x140614B80 | sort reference points / sort | no-verdict / differs | C |
| 0x140625500 / 0x140625000 | `Activate` / `Deactivate` | — | C, field writes in disassembly |
| 0x1406294F0 / 0x140628960 | hit effects / chooser | no-verdict / same-partial | C, call arguments in disassembly |
| 0x14062A920 | sweep | same-partial | C, channel set and ignore set in disassembly |
| 0x14062F560 | `TresBodyCollPrimitive` ctor | — | disassembly (defaults) |
| 0x140632240 | damage formula | differs | full disassembly |
| 0x140637430 | body-push solver | same-partial | C only |
| 0x140642CA0 | primitive init from the shape | same-partial | disassembly of the flag mapping |
| 0x140645900 / 0x140645BB0 / 0x140646070 / 0x140570870 | row → primitive, default row | same-partial / same | disassembly (whole) |
| 0x1406472A0 / 0x140626310 / 0x14062CFC0 | manager tick, component update, record timer | — | C / C / disassembly (whole) |
| 0x140647950 / 0x140649E40 / 0x14064BA60 | primitive update, size, socket transform | differs | C + disassembly of first-tick tail and size |
| 0x14064C130 / 0x140DC4270 | `TresDamageCommonParam` / `TresEnemyCommonParam` ctor | — | disassembly |
| 0x140F96570 / 0x140F75F30 / 0x140F95CA0 / 0x140ACF250 | component `Targetable` / team / look-at / location | same-partial / — / same / same | disassembly |
| 0x141330D30 | `TargetMetrics` (123) | same-partial | C, constants in disassembly |
| 0x141331AF0 | Attack slot 129 (recoil hand-off) | — | constants in disassembly, structure C |
| 0x1413325A0 | Attack slot 113 (chase armed) | same | disassembly |
| 0x141332790 | Attack slot 115 (window counters) | same | disassembly |
| 0x141332930 / 0x14134C5E0 | AirSlide Begin (recovery branch) / slot 113 | — | C / disassembly |
| 0x141333210 | Attack `Begin` (96) | same-partial | C + spot disassembly |
| 0x141335120 | Attack `End` (slot 98) | same-partial | full disassembly incl. jump table 0x141335CC4 |
| 0x1413364D0 | `Approach` (Attack slot 135) | — | disassembly |
| 0x1413378B0 | `PlayAttack` cases 0x7A–0x7F | same-partial | disassembly (case trace) |
| 0x14133D230 | Attack `Reset` (slot 95) | same | disassembly |
| 0x141341340 | Attack `Tick` (97) | same-partial | disassembly |
| 0x1413417F0 | Attack slot 114 (pitch / roll levelling) | — | disassembly |
| 0x141341870 | Attack slot 131 (athletic kinds) | — | C |
| 0x141341B10 / 0x141342760 / 0x1413426A0 / 0x141332790 | Attack state on-hit, map-hit, count-up, counter slot 115 | same-partial / no-verdict / no-verdict / same | C / C / disassembly / disassembly |
| 0x141343160 | rush-suppression handler | — | C |
| 0x141343800 | hit-description pre-processor | same-partial | C |
| 0x141344580 | shot handler | same-partial | not read |
| 0x1413459E0 / 0x141342DF0 | TresAtk begin / EnableChange handlers | same / same-partial | disassembly |
| 0x141349CC0 | traction (Attack slot 116) | same-partial | full disassembly |
| 0x14134AA80 | Attack slot 117 (warp countdown) | — | C |
| 0x14134ACA0 | `Decide` (122) | same-partial | full disassembly incl. jump tables |
| 0x14134C420 | `PreDecide` (121) | same-partial | C, constants in disassembly |
| 0x14134C660 | Attack slot 141 (reaction command off) | same | disassembly |
| 0x14134CAD0 | Attack slot 142 (guarding attack) | — | C |
| 0x14134CC10 | Attack slot 139 (weapon effect group) | — | C |
| 0x14134D0E0 | facing (126) | same-partial | C |
| 0x14134D510 | Attack slot 128 (delegate run, hover end) | — | disassembly |
| 0x14134F750 / 0x141350C20 / 0x141352FF0 | Begin / End / Reset | no-verdict | C |
| 0x1413518D0 … 0x141351170, 0x14043A5D0, 0x14043A5E0 | parameter getters (slots 127–150) | — | disassembly |
| 0x1413553F0 | Damage `Tick` (97) | no-verdict | disassembly |
| 0x141356930 / 0x141356DA0 | wall hit / landed (event interface 9 / 75) | no-verdict | C / disassembly |
| 0x14135A870 | Beat (121) | no-verdict | C; constants in disassembly |
| 0x14135B150, 0x14135E260, 0x14135BDA0, 0x14135D680 | Frozen, Suction, Eat, PuddingEat | no-verdict | C (summary) |
| 0x14135B300 | Blow (117) | no-verdict | C; constants and field writes in disassembly |
| 0x14135CAB0 | Parabola (120) | no-verdict | C; constants in disassembly |
| 0x14135D740 | Small (116) | no-verdict | C + disassembly of sub 1, sub 4, tail, constants |
| 0x14135EEA0 / 0x14135C410 | Toss (118) / Launch (119) | no-verdict | C; constants, timer arithmetic in disassembly |
| 0x141362AB0 | `ComboLimit::Decide` | same-partial | C + disassembly of the split, the rising attack, the finisher step, constants |
| 0x141365930 | Damage `Setup` (114) | no-verdict | C; kind table, dead conversion, timers in disassembly |
| 0x141368650 / 0x1413733D0 / 0x14136B780 | Die Begin / Tick / End | no-verdict | C; constants in disassembly |
| 0x141386050 / 0x1413944D0 / 0x141433F80 | Idle Begin / Tick lock flag, idle name | — / — / same | disassembly of the flag code |
| 0x1413D0C50, 0x1413CBA70, 0x1413D3AC0, 0x1413CE760 | StyleChange Reset / Begin / Tick / End | same / differs / differs / same-partial | disassembly (Reset, Tick dispatch and sub 0–4, Begin calls and 0x94 case); C for the rest |
| 0x1413D0CA0, 0x1413CBF70, 0x1413D4550, 0x1413CEBD0 | StyleFinish Reset / Begin / Tick / End | same / same-partial / no-verdict / same-partial | Reset disassembly; others C |
| 0x1413E2630, 0x1413E1FC0, 0x1413E1800 | Stun Impact / Sonic Rave / Last Arcanum ticks | differs ×3 | full disassembly (10.9) |
| 0x1413E60E0 / 0x1414156A0 / 0x140F1D6E0 | list sort / truncate / weak-pointer compare | differs / — / — | C / C / disassembly |
| 0x1413E9000 | `AddStylePoint` (ctrl slot 848) | same-partial | full disassembly incl. jump table |
| 0x1413E9D30 | form-time drain (876) | — | C |
| 0x1413EC9F0 | controller slot 681 (common command gate, table 0x1413ECB48) | same-partial | disassembly |
| 0x1413EE030 | `SetStyleFinishCommand` (837) | same-partial | C; 0x94 case and time block in disassembly |
| 0x1413F0700 / 0x1413F0D00 | slots 712 / 713, RB / RT tap timers | — / same-partial | disassembly |
| 0x1413F0750 | line of sight | no-verdict | C, constants and call arguments in the disassembly |
| 0x1413F0ED0 | `AwardAtMax` (830) | same-partial | C; the full-gauge part and the 0x94 case in disassembly |
| 0x1413F1AB0 | controller input update | differs | disassembly: key reads, order of the combat calls, the two direct attack calls |
| 0x1413F8130 | controller slot 637 (counter window countdown) | — | disassembly |
| 0x1413F85D0 | controller slot 636 (just-guard timer countdown) | — | disassembly |
| 0x1413F9790 | controller slot 635 (command cool-down countdown) | — | disassembly |
| 0x1413FA580 | slot 752 `Unlock` | differs | C |
| 0x1413FA990 / 0x141419C00 | slots 791 / 790 (multi-lock end / start) | same-partial / no-verdict | C |
| 0x141401D10 | `CanEndStyle` (857) | same-partial | disassembly, tables decoded |
| 0x141401F80 | controller slot 682 `CanExecuteCommand` | differs | disassembly |
| 0x141404E50 | controller slot 714 `UpdateLockon` | same-partial | **full disassembly** |
| 0x141407920 / 0x141406F80 | slot 718 (shotlock search) / slot 715 (shooter mode) | same-partial / no-verdict | heads only |
| 0x14140B8A0 / 0x141442F70 / 0x140549A80 / 0x1414471A0 | Sora on-contact chain | same / same / no-verdict / same | disassembly (0x140549A80: C) |
| 0x14140DA40 | `NotifySetActionCommand` (687) | same-partial | C; table, formchange branch, constants in disassembly |
| 0x14140EAA0 | controller slot 680 `ExecuteCommand` | same-partial | disassembly |
| 0x1414110A0 / 0x141415020 / 0x1413ECEA0 | forced locks (slots 753 / 751 / 754) | no-verdict / no-verdict / same-partial | C / C / not read |
| 0x1414130A0 | slot 731, set attack target | same-partial | disassembly |
| 0x1414145C0 | counter window set / close | — | disassembly |
| 0x141414B70 | just-guard timer set | — | disassembly |
| 0x141414C70 | `SetFinishLevel` (853) | same-partial | disassembly |
| 0x141415A50 | controller slot 676 (command cool-down set) | — | disassembly |
| 0x1414162A0 | lock-on init | differs | disassembly |
| 0x14141A9D0 | `StartStyle` (831) | differs | C; gates, both jump tables and the form-start block in disassembly |
| 0x14141B470 | executor | same-partial | jump table decoded; formchange / finish / extra targets in disassembly |
| 0x14141C4F0 | `StyleGaugeTick` (829) | same-partial | full disassembly |
| 0x14141CA20 | `EndStyle` (832) | no-verdict | C; 0x94 case and tail in disassembly |
| 0x141423210 | player pawn ctor | — | disassembly (constants) |
| 0x141425780 | air-slide gate (attack branch 0x141425D3E) | — | disassembly of the branch |
| 0x141426330 | `CanAttack` (iface 44) | differs | full disassembly incl. jump tables 0x141426EFC / 0x141426F60 / 0x14142700C |
| 0x141427690 | pawn slot 1336 ("can act") | — | disassembly |
| 0x141427CC0 | dodge gate (attack branch 0x141428190) | — | disassembly of the branch |
| 0x1414286F0 | `CanMagic` (attack branch 0x14142899A) | differs | disassembly of the branch |
| 0x1414294C0 | guard gate (attack branch 0x1414296EF) | — | disassembly of the branch |
| 0x14142A3D0 / 0x14143D8F0 / 0x141426330 / 0x141440600 | jump gate / JumpRequest / CanAttack / MoveInput: DAMAGE branches | differs | disassembly of the branches |
| 0x14142B3F0 | `CanLaunch` (iface 47) | differs | full disassembly |
| 0x14142CE00 | `CanRun` (attack ids → false) | — | disassembly of the branch |
| 0x14142DEF0 / 0x14142E030 / 0x141425510 | `CanStyleChange` / `CanStyleFinish` / state gate | same-partial / same-partial / differs | C; leaf getters and the state table in disassembly |
| 0x14142F640 / 0x1405A35D0 / 0x14143A190 | survive / die path / die request | — | disassembly / C / C |
| 0x141430A60 | pawn slot 1304 (friend unison roll) | — | early exit in disassembly, rest C |
| 0x141432780 | pawn slot 981 (chain reset) | same | disassembly |
| 0x141433570 / 0x141433A60 / 0x141432F00 | `MaxCombo` / `Finishers` / attack state id | same-partial / same-partial / differs | disassembly, tables decoded |
| 0x141435200, 0x1414333A0, 0x141433F10 | clip tables | — | C |
| 0x141435AC0, 0x1405AD200, 0x1405AD1A0, 0x141435A90, 0x1405AD120, 0x1405ADBF0 | no-damage / no-reaction / invincible / immune predicates | — | disassembly |
| 0x1414365D0 | pawn slot 989 | — | disassembly (conditions not traced to their setters) |
| 0x141437330 | `TryAttack` (iface 45) | differs | C in full; every write and compare in disassembly |
| 0x141439A50 | reaction gate (iface 88) | differs | disassembly, whole |
| 0x14143AEB0 | `TryMagic` (attack branch 0x14143B0FA) | differs | disassembly of the branch |
| 0x14143EAE0 | `TryLaunch` (iface 48) | differs | full disassembly |
| 0x141442DE0 | Sora on-damage-dealt | differs | disassembly (whole) |
| 0x141444090 / 0x141431C90 / 0x1415246C0 / 0x141520180 / 0x141520690 | hit-stop entry points | — / — / unconfirmed | disassembly (whole) + image-wide call scan |
| 0x141445940 / 0x141447840 | `TryStyleChange` / `TryStyleFinish` | same / same-partial | disassembly |
| 0x141448550 | pawn slot 1058, search origin | same-partial | C, the +100 in the disassembly |
| 0x14150A1F0 / 0x14150C440 | command begin / end events (state base) | no-verdict | C; the `slot859(0, 0)` call in disassembly |
| 0x1415759C0 → 0x141668C60, 0x14157A7A0 | HUD cursor add / remove | no-verdict / not-compiled | C |
| 0x1416158F0 / 0x141619E60 / 0x141630AF0 / 0x141627B70 | HUD insert / classify / timers / select | differs / same-partial / same-partial / same-partial | capacity and constants in disassembly; rest C |
| 0x1416220D0 | command menu "MainSel" update (the attack press) | — | disassembly of the decide branch 0x141622513–0x1416226E1 |
| 0x143F4EA20 | montage advance (root translation × scale, 0x143F4EB47) | — | disassembly |

### 13.2 Open questions

The eleven blocking questions of revision 1 are answered (section 16); what is left is listed by area. Answered
across strands and therefore not repeated below: the meaning of the Attack counters +0x12C…+0x138 (5.5); the
writers of `info+0x60` / `+0x66` and of the flag bits `info+0x50` bit 6 / bit 2 (8.2.6, 8.7); whether an attack
press writes the 0.1 s reaction block (no: 3.1.1). Still open from revision 1: the `kind 0` result of an empty
finisher slot (4.2) and the body of the `TresAnimNotify_Shot` handler 0x141344580.

**Request path and attack state (3, 5, 14)**

1. `FUN_141620D10(hud, row)` and the bytes hud+0x739 / +0x73A[row] / +0x1EC (row enable) of the command menu: when the
   Attack row is disabled (the UI spec's row-enable pass 0x141632660 owns this). For the port: Attack row always enabled
   outside menus.
2. pawn slot 989 0x1414365D0 (`pawn+0x1130 & 8`, `pawn+0xE60 & 0x8001`, `vt+0x1FE8(0)`): which game conditions set
   them (event / cutscene locks by the look of the callers; not traced).
3. Wall (map) hits as hit confirm: the on-hit handler's last branch is taken for a map hit that is not repelled (8.5.1, tagged [C] there); whether hitting scenery alone lets a combo continue needs the running game or a read of
   0x141342760.
4. `ctrl+0x8C8` writers outside the executor (0x14129BDA0, 0x14132EBA0, 0x1413E46C0, 0x1413E5540, 0x14140C790,
   0x14140E9F0 …, 0x141421D70): reaction commands, form start / end; times not tabulated (they gate the attack).
5. Slot 142(1) callers (which attack kinds guard while swinging: `+0x23C bit 1`): not in the base combo; not traced.
6. `Jump+0xA0 == 2` entry with forced kind 0x37 and the dive-fall kind 0x35: belong to the jump / flowmotion specs.

**Movement (6)**

1. Body-push solver 0x140637430 was read in decompiled C only: the exact penetration vector (0x144524AB0: full MTD or
   capped), the alternate branch taken when the game object's byte +0x84C is set, and where in the frame it runs relative
   to the movement tick (it is slot 0x145D77938 of the component's vtable; its caller was not traced).
2. Which enemy `m_BodyPushPower` levels occur (enemy base-param tables): decides how hard enemies shove Sora.
3. `mi+0x140 > 0` in the root-scale setter: field identity (blend weight or position?). If it is the weight, a scale
   written in the first tick after a play with blend-in could be refused; the traction window never opens that early.
4. Component vt+0xC30 0x1405891D0 ("push ignored"): besides bit 2 it returns true when the pawn is attached, when
   0x143EA19C0(owner) or the driven-move record is active; who sets bit 2 for the player (vt+0xC28 callers 0x1405AEC88,
   0x1405B9280, 0x1405D1C39) was not traced.
5. Readers of the montage scale other than the main advance (0x143F4EECA, 0x143F5381A, 0x143F5432A, 0x1405686F9): assumed
   to be the looping / section-jump paths of the same extraction.
6. `pawn.vt+0xE38()` (speed carried into slide kinds, +0x234 bit 12) and the bit-17 "behind" rule: formchange / slide kinds,
   not reached by the base combo.

**Hits (8)**

1. **Tick order of the attack-collision manager against the pawn** (tick-group byte 8): does a window whose notify
   fired in M of frame N sweep in N or N+1? Static reading stops at the engine's tick-group table.
2. **comp+0x880 / +0x888 / +0x890 of the keyblade's component**: which actors (weapon, pawn, none). All rules above
   are written with the fallbacks (owner = Sora); if +0x888 is the weapon actor, the damage causer is the weapon and
   the on-damage notify (8.5.2) would go to the weapon's interface (vtable 0x146111258 slot 31 is empty) — then the
   style points would have to come from somewhere else. 0x141442DE0 is the only caller of `AddStylePoint` found on
   the hit path; the other callers of controller slot 848 were not enumerated. Confirm with the probe.
3. **comp+0x968 bits 0 / 1 and comp+0x958** on the keyblade (expected 0, 0, −1 from the constructor).
4. `A+0x3CC == −1` in the actor gate: field identity.
5. Whether any `TresAnimNotifyState_BpEvent` of the attack clips plays a camera shake or force feedback in blueprint.
6. `0x1405AD4A0(victim)`: the class of victims that grant style points without losing HP.
7. The victim interface slots named by inference (`attribute_weak`, `attribute_nullified`, slot 89 "guarding"): they
   forward to the victim's state machine (0x14150A660, 0x14150A6E0, 0x141514920); only their use is established.

**Targeting and lock-on (7)**

1. `owner.vt+0x698()` inside `Targetable` (0x140F96570): the owner-side condition (dead, invincible events, scripted
   `EnableLockon(false)`); and the per-enemy values of `m_AutoLockOnRange`, `m_LockonPriority`, `m_ViewTargetIndex`.
2. Slot 717 0x1414095E0 (runs right after the search; touches +0x954 and pawn vt+0x1100 / +0x1108 of the target's
   pawn): not read.
3. Slot 754 0x1413ECEA0 and the callers of the forced locks (slots 751 / 753): which scripts or enemies force a lock.
4. `UTresGameInstance`+0x84C (slot 643): which game mode suppresses the whole update.
5. Who calls the rush-suppression handler 0x141343160 (it was not found on the hit path of 8.5.1).
6. Lock-on camera: the framing solver 0x1405E0590 (how distance and look point are fitted to keep both points on
   screen, the role of the target size from camera vt+0x8E8 and the player height from vt+0x8F0), the clamp 0x140611760
   in detail, the obstruction blend 0x14061BA10, flag bits of camera+0xA70 (bits 0–5), parent +0x948 bit 0.
7. The value of controller+0xB38 (form kind) for base Sora with the Kingdom Key (needed only to prove that multi-lock
   and shooter mode never trigger; Second Form is 0x94).
8. HUD: `Remove`'s type argument 8 (all types?) and 0x14166C730; the marker's on-screen position rule (UI spec open
   item 7).

**Sora being hit (8.4, 9)**

1. `MyShapeSize` semantics of `TresBodyCollPrimitive` capsules (radius / half height), how the named groups
   (`Guard`, `GuardBody`, `BaymaxRide`) are switched, and that the hurt shapes answer the TresAtkCollision trace (8.2.3).
2. pawn+0x15B4 (pawn vt+0xD80): value 0x0B makes Sora undamageable, 0x12 disables the wall hit and changes clips: which
   modes these are.
3. The exact call path of the event interface slots 9 (blocking hit) and 75 (landed): who calls them and in which phase of
   the frame.
4. Game-over flow after DIE (game instance 0x140EBA4D0; RESCUE 109, GAMEOVER 118): not needed for the test level.
5. Auto-revive item branch with item 0x905 (full heal variant) — C only.
6. Small in the air relies on the ordinary air braking (2048 cm/s²) acting on a velocity that was written through the
   direct record: derived from the `CalcVelocity` tail, not measured.

**Second Form and the command queue (10, 11)**

1. **Cancel window of `StyleChangeEnd`**: Tick sub 4 clears `m_bEnableChange` in the frame the notify sets it and
   `CanAttack` refuses state 31, so control should return only when 1403 ends (2.25 s after the press). The jump /
   dodge / guard / magic gates against state 31 were not re-read: confirm with a probe (first frame a request is
   accepted after a formchange).
2. `StyleChangeLoop` really shows for one tick only (read: sub 3 has no condition); a probe of state+0x78 over time
   confirms.
3. Game flags in the gauge tick: `ctrl.vt+0x1420()+0x95B0` bit 3 (formchange unlocked?), `ctrl.vt+0x1418()+0x84C`,
   game flag 7 (0x1405AD320) and game flag 5 (0x1405AD350, the 3-points-per-second auto gain with pawn slot 1361).
4. ctrl+0x878 bit 1 (must be set for `StartStyle`); ctrl+0x8C8 is the command cool-down (3.1.3), its writers outside the
   executor are listed under "Request path".
5. `pawn.slot1295(0, 0xCB2)` at the end of the form (pawn+0x10D8 bit 1, +0x10DC); who sets it during the form.
6. Weapon charge effect (groups 3000–3002, `docs/sora-vfx-spec.md` question 15): the level is weapon+0x66C; setter =
   weapon-interface function 0x14178A480(kind 1, level) → 0x141792220(weapon, level, burst 1); its virtual callers
   (hence whether the level follows the gauge phase or the finish level) were not traced.
7. The stick-steering block of the StyleFinish `Tick` (state+0x16C kinds) was not attributed to a command. Open points of the three
   finish-command bodies after the instruction-level read: 10.9.
8. `EndStyle` when `bKeepPose == 0`: the exact equipment call (`pawn.vt+0x13E0(1, 0, 0, 0.2)`) → wear form NORMAL and
   the ability removal are inferred from the symmetric start; not read in the equipment component.
9. The Second Form attack clips for the non-finisher steps are the base clips (same `PlayAttack`, no replaced anim
   set in `w_so010_Equip`): confirm by probe that state+0x1B0 stays in 1..0x20 for the first three steps.
10. HUD pause `BP_NotifyPauseActionCommandTimer` (vt+0x1598) and hud+0x751: writers not read.

### 13.3 Runtime-probe field requests (read-only; `docs/sora-runtime-probe.md`)

| Area | Object | Offset | Type | Why |
|---|---|---|---|---|
| decision | Attack state | +0x128, +0x1B0, +0x1AC, +0x154 | i32 ×4 | combo count, kind, last kind, reach per attack: validates section 4 against play |
| decision | Attack state | +0x1CC, +0x1D0, +0x1D4, +0x1D8 | f32 ×4 | dist, dz, DirZ, root length |
| decision | Attack state | +0x84, +0x234, +0x1EC, +0x1F4, +0x23C | u32 ×5 | notify bits and channel flags over time |
| decision | Attack state | +0x143, +0x164, +0x140, +0x141, +0x142 | u8 ×5 | request acceptance windows |
| decision | pawn | +0xE10, +0xE14, +0xE48, +0x10E4 | i32, i32, u32, f32 | last kind, finish count, finish-end bit, weapon-change carry timer (expected 0 in base form) |
| formula | game instance (FUN_141522B30) | game work +0x7E09 (battle level), +0x7DF0 (game level), inputs of 0x140EAB160 / 0x140EAB0E0 | u8 … | the level value and mode flags of the damage formula |
| formula | victim status object | +0x50, +0x5C, +0x60, +0x64, +0x7C, +0x7D, +0xA4, +0xA8, +0xBC…+0xDC, +0x114 | ints | stats, type, floor / cap, resistances: check a damage number end to end |
| request / state | pawn | +0xE28, +0xE24, +0xE18, +0xE14 | u8, i32, i32, i32 | hit confirm: whiff a first swing with and without Combo Master and press again; shared combo / finisher indices over an attack–cast–attack chain |
| request / state | pawn | +0xE34, +0xE38, +0xE3C | f32 ×3 | the three cool-downs after a whiffed air attack (expect 0.6, 0.6, 0.3333) and none after a hit |
| request / state | pawn | +0xE60 | u32 | bit 17 latch (one frame after a grand magic), bit 18, bit 19 after a last finisher |
| request / state | pawn | +0xE50, +0xE54 | i32 ×2 | launch stocks: 1 → 2 on use, reload at chain end |
| request / state | Attack state | +0x12C, +0x130, +0x134, +0x138, +0x13C | i32 ×4, u32 | window counters and the "hit landed" bit across a chain with one whiffed step |
| request / state | Attack state | +0x254, +0x234 | f32, u32 | cancel bit 7 timing (0.1333 s after the first window; never before EnableChange on the last finisher) |
| request / state | Attack state | +0x18C | f32 | 0.6 during an air attack, 0 after a hit |
| request / state | controller | +0x8C8, +0x8F4, +0x880, +0x87C | f32, f32, i32, i32 | confirm an attack press leaves both timers untouched and sets the last-command kind 0x1B |
| request / state | controller | +0x89C, +0x8A0, +0x8A4, +0x878 | f32, f32, i32, u32 | just-guard timer, counter window, command id, bits 8 / 9 through a guard counter |
| request / state | frame order | key edge frame vs first frame of state id 12 | — | confirm the U-phase latency (press → Attack `Begin` two `MainProc`s later than a C-phase request would be) |
| movement | movement component | +0x214 (Velocity), +0x314 (mode), +0x2F8 (GravityScale), +0xA3C | vec3, u8, f32, u8 | per frame through a ground attack with / without a target and an air attack: confirms the skip flag, the 0.2 / 0.5 / 0.1 approach rates, the mode timeline (5 → 3 at the window end, scale 0.1 → 1.5 at EnableChange) |
| movement | movement component | +0xA24 (root-motion velocity), +0xBA8, +0xBB4 | vec3 ×3 | root motion after scale; body push while standing inside an enemy (expect full / half / zero by level) |
| movement | movement component | +0xC0C..+0xC18 | vec3, u8 | pending record cleared in the following movement tick |
| movement | montage instance of the attack clip | +0x174, +0x178 | f32 ×2 | root scale over a `root_len` kind at several distances (expect dxy / root_len, 1.0 after the window) |
| movement | Attack state | +0x1D4 (DirZ), +0x220, +0x224, +0x228, +0x150 | f32, i32 ×3, f32 | channel phases and the DirZ decay after the window |
| movement | actor location | z per frame | f32 | air attack at a level target (free fall at −98 then −1470) versus a target 100 cm above (pinned hover) |
| movement | enemy body-collision component | +0x4A4 | u8 | push level of the enemies fought |
| hits | keyblade `TresAtkCollComponent` (weapon+0x578) | +0x880, +0x888, +0x890, +0x898 | weak ptr (i32 index, i32 serial) ×4 | who is attack owner / instigator / velocity source / owner (question 2) |
| hits | same | +0x868, +0x958, +0x95C, +0x960, +0x964, +0x968 | i32, i32, u8, f32, u8, u32 | team number, effect set index, team-check and height-limit flags, nearest-only |
| hits | same, record of the active group (map +0x908) | +0x08 flags, +0x0C interval, +0x10 timer, +0x20 / +0x28 hit list ptr / count | u32, f32, f32, ptr, i32 | watch the list fill and empty against `AttackInterval` |
| hits | active primitive (`m_AtkColls[i]`) | +0x870, +0x87C, +0x888, +0x890, +0x984–0x987, +0x9B8, +0x9BC, +0x9C8 (0x68 bytes), +0xA4C, +0xA68 | vec3, vec3, f32, f32, u8 ×4, f32, f32, bytes, f32, ptr | shape position per frame (sweep start / end), flags, the damage info actually delivered, window time, chosen hit sound |
| hits | player controller | +0xB44, +0xB48, +0xB4C, +0xB50, +0xC10 | f32 ×5 | style points and their per-hit gain (8.5.2), HUD accumulator |
| hits | pawn | +0xD00, +0xE24, +0xE28, +0x6C8 | i32, i32, u8, vec3 | focus-per-hit value, hit counter, hit flag, last impact point |
| hits | pawn | focus value read by slot 769 (`GetFocus`) before / after a hit | i32 | confirms no focus gain from hits |
| hits | game mode | +0x4738, +0x473C | u8, f32 | hit-stop enable byte and timer: must stay 0 during combat |
| hits | Sora pawn | +0xA0 (`CustomTimeDilation`) | f32 | must stay 1.0 through hits (no hidden hit-stop) |
| hits | Attack state | +0x12C, +0x130, +0x134, +0x138, +0x264, +0x268, +0x270 | i32 ×4, u32, u8, ptr | the counters copied into the damage info; recoil record after a repelled swing |
| hits | frame counter at notify begin vs first sweep of the window | — | — | question 1 (log the frame of 0x140561F40 and of 0x14062A920 for the same primitive) |
| targeting | controller | +0x994, +0x99C, +0x9AC, +0x944, +0x94C | weak ptr (i32 index, i32 serial) ×5 | soft target, pin, candidate, lock pawn over time: validates 7.6 (pin survives the end of a combo; released when nothing is within 900) |
| targeting | controller | +0x960, +0x964, +0x968, +0x969, +0x98C…+0x991, +0x9C4 | u8 / i32 | lock flag, switch index, mode bytes in normal play (expected 0, 0, 0, 0, 0/1/0/0/0/1, 0) |
| targeting | controller | +0x934, +0x938, +0x93C…+0x940 | u8, f32, u8 ×3, f32 | tap timing of RB / RT (0.2667 s) |
| targeting | controller | +0x95C, +0x970…+0x988, +0x9C8, +0x9CC, +0x9F0 | i32, f32 ×7, f32, i32, i32 | crowd count, live ranges (900 / 150 / 3500 / 4200 / 5000 / 15000 / 3750 / 2400 / 6), list length while locked |
| targeting | controller | +0xB38, +0x878, +0xA10 | i32, u32, i32 | form kind in base form, input-lock bit 6, shotlock state |
| targeting | current target component | +0x2B0 (vec3), +0x868, +0x86C, +0x870, +0x874, +0x884, +0x888, +0x898 | f32 ×3, u32, i32, u32, u8, i32, f32, u32 | location, enable / team bits, permission, priority, view index, auto range, rush suppression of real enemies |
| targeting | camera manager | +0x28B8, +0x28C0 | ptr ×2 | lock-on camera and target while locked / during the 0.5 s blends |
| targeting | lock-on camera (`TresCameraNormalLoc`) | +0x800, +0x828 (rot), +0x880, +0x884, +0x888, +0x898 (rot), +0x8DC, +0x8E4, +0x9E0, +0x9E4, +0xA24…+0xA2C, +0xA70, +0x660 (vec3) | floats, u8 | distance, base offset, limits, rotTarget, autoPower, stickWait, framing distances, stick range, flags, look point: validates 7.12 against play |
| targeting | normal camera | +0x910…+0x948 | bytes | the Lockon* properties as spawned (blueprint 2500 / 500 / 0.45, 300 / 3000) |
| being hit | Damage state | +0x78, +0x88, +0x190, +0x1A0, +0x140 | i32 ×5 | sub-step, kind, direction index, variant, hit counter per hit |
| being hit | Damage state | +0x12C, +0x130, +0x138 | f32 ×3 | timers: confirms the 0.2 s recover delay and the halved flight duration |
| being hit | Damage state | +0x8D, +0x8E, +0x8F, +0x90, +0x84 | u8 ×5 | stick / recoverable / wall / bad-status flags and `m_bEnableChange` over time |
| being hit | Damage state | +0x174, +0x178, +0x17C, +0x15C (vec3), +0x1C0 | f32 ×3, vec3, u32 | power, brake, vertical speed, knock direction, flags (bit 2 landed, bit 3 never set?) |
| being hit | Damage state | +0x98 … +0xFF | 104 bytes | the TresDamageInfo of real enemy hits (kinds, scales, +0x60 reaction type, +0x66) |
| being hit | movement component | +0x214 (vec3), +0x2F8, +0x314, +0xA3C, +0xC0C…+0xC18 | | velocity curve of a Small / Blow / Toss hit against 9.4's numbers |
| being hit | pawn | +0x5FC, +0x848, +0x638, +0xE44, +0x630, +0x634, +0x518, +0x15B4, +0x5F0, +0x85D | i32, f32 ×3, u32 ×2, u8 ×4 | HP, Withstand Combo timer, no-damage timer, invincible timer, flags, mode byte, character kind, bad status |
| being hit | controller | +0x8F4, +0x8B8, +0x8BC, +0x898, +0x878 | f32 ×2, u32 ×3 | reaction block, reaction command time / kind / shown kind, flags |
| being hit | game mode load asset `m_PlayerDamageCommonParam` CDO | +0x40 … +0xF4 | f32 ×46 | confirms the parameter table (native + asset) in memory |
| form / queue | player controller | +0xB34, +0xB38, +0xB3C | i32 ×3 | form level, form command, HUD form: 0 / 0x94 around a formchange |
| form / queue | player controller | +0xB40, +0xB44, +0xB48, +0xB4C, +0xB50 | f32 ×5 | gauge, pending, chain factor, decay timers: gain per hit (× 0.8 in form), halving, 20 s / 1 s / 5 |
| form / queue | player controller | +0xB54, +0xB58, +0x8F0 | f32 ×3 | form time left / full (30), last finish time: +3 s per full gauge, 0 after a finish |
| form / queue | player controller | +0xB5C, +0xB5D, +0xB60, +0xB64, +0xB68, +0xB78 | u8, u8, i32, i32, i32, u8 | pending flags, style fix, finish level and command, pause / time-up bits |
| form / queue | player controller | +0x8C8, +0x878, +0xBB8, +0xC08, +0xC09, +0xC18 | f32, u32, u8 ×4 | the block timer and mode bytes that gate the gauge and `StartStyle` |
| form / queue | StyleChange state (id 31) | +0x78, +0x84, +0x94, +0x98, +0x99, +0x9C, +0xA0 | i32, u32, i32, u8, u8, f32, u32 | sub-step timeline (questions 1, 2), wear form 5, flags (bit 4 with "Keyblade Cinematics: Basic") |
| form / queue | ComboLimit state (id 90) | +0x128, +0x1B0, +0x1AC, +0x140, +0x141, +0x142 | i32 ×3, u8 ×3 | combo index, kind (1, 2, 3, 7A, 7B, 7C …), finisher flags per step |
| form / queue | pawn | +0xE14, +0xE2C, +0x634, +0x10D8, +0x10DC, +0x10E4 | i32, i32, u8, u32, i32, f32 | finish counter, command in action, bad-status block bit, question 5, carry timer (expected 0 throughout) |
| form / queue | StyleFinish state (id 33) | +0x78, +0x128, +0x288, +0x28C, +0x1D8, +0x24C | i32, i32, i32, u32, f32, f32 | per-command sub-step, dash / window count, pending extra, root length, stop distance (question 7) |
| form / queue | HudCommand part | +0x6F4, +0x6F8, +0x768, +0x778, +0x7D0, +0x7DC, +0x7E0, +0x9C4, +0x9CC | i32 … | selection, counted entries, list length, slot A kind, slot B kind / total / remaining (no countdown expected), priority mode, serial |
| form / queue | equipment component (pawn+0x598) | +0x229, +0x22A and the wear form byte | u8 … | weapon form 1, grow level, wear form 0 ↔ 5 at sub 3 and at `EndStyle` |
| form / queue | weapon actor | +0x66C, +0x670 | i32 ×2 | charge level and running group across gauge phases and finish levels (question 6) |

---

## 14. Attack and magic steps in one combo [I both sides]

Shared fields: combo index pawn+0xE18 (= `iface+0x9C8`), finisher index pawn+0xE14, "combo over" pawn+0xE48 bit 5,
hit confirm pawn+0xE28, counters pawn+0xE1C / +0xE20.

| Step | Gate | Request |
|---|---|---|
| attack → cast | `CanMagic` attack branch 0x14142899A: queued is not a cast; cur+0x143 == 0; cur+0x164 != 0; cur.combo < MaxCombo() (**no** Berserk exception); !(pawn+0xE48 bit 5); !(cur+0x1EC & 0x80). **No `m_bEnableInput` test and no hit-confirm test** | `TryMagic` 0x14143B0FA: idx = cur.combo + 1; cur+0x143 = 1; PushState(18); Magic+0xB4 = idx; pawn+0xE18 = idx |
| cast → attack | `CanAttack` FIREMAGIC branch (3.2): no attack queued; !(flag 0x10); BulletCount (Magic+0xD0) > 0; Magic+0xB4 < MaxCombo(); pawn+0xE28 != 0 (set by `OnShot`); combo not over | `TryAttack` 0x141437BEA: flag 0x10; PushState; combo = Magic+0xB4 + 1; bAir = flag 1 |
| cast → launch | `CanLaunch` FIREMAGIC branch (3.4): additionally the cast must not be a finisher (flag 0x40) | `TryLaunch`: combo = 1, launch bit |

- A cast may be requested as soon as the swing's first hit window has opened — earlier than an attack, which also waits
  for `TresEnableInput` — and without a landed hit. Both start at the attack's `TresEnableChange` (Tick, 5.2).
- Counters: the Attack `End` (5.4) writes the next cast's `+0xB8 = Attack+0x12C`, `+0xC4 = Attack+0x138` (zero unless
  this attack landed a hit); the cast's `End` does the same toward an attack (magic spec 5.3).
- Finisher rule: identical arithmetic on both sides and **both write pawn+0xE14**. Attack `Decide` (section 4.2) and
  the cast's first tick (0x1413798DA–0x141379981): `if idx + 1 == max − 1: pawn.slot1304()`; `if idx < max − fin:
  pawn+0xE14 = 0` (normal step); else finisher: `f = pawn+0xE14`; last finisher when `f >= fin − 1 || idx >= max − 1`
  (pawn+0xE48 bit 5 = 1); `pawn+0xE14 = f + 1`. So a finisher cast advances the finisher slot the next attack would use
  (revision-1 section 14 said only Attack states advance it: wrong).
- The one difference: an Attack with Berserk Charge active (state+0x234 bit 21) may exceed `MaxCombo`; `CanMagic` out of
  it uses the plain `combo < MaxCombo()` and refuses.
- After a **grand magic** command (kinds 0x20, 0x24, 0x28, 0x2C, 0x30, 0x6E–0x7C) the executor arms the command
  cool-down with 0.1 s (0x14141B82D) and the reaction block `ctrl+0x8F4 = 0.1`: no command, attack included, is accepted
  for 0.1 s. Ordinary spells do not.

No disagreement with `docs/sora-magic-spec.md` part A (2.2, 2.3, 5.1, 5.2, 5.4) was found; its 5.4 "attack" row should
add the hit-confirm test (`interface+0x9D8` there = pawn+0xE28, satisfied by `OnShot`): `docs/sora-spec-reconciliation-2.md`.

---

## 15. Implementation order and test list (rewritten for revision 2)

### 15.1 Order

1. **Request path** (3.1–3.3): the attack press is a command-menu decision in phase U; `CanAttack`
   with the hit-confirm rule; `TryAttack` per state; no buffer. Sheets `can-attack`, `try-attack`,
   `controller-order`.
2. **Decision** (4; sheet rules) with the host's target interface (7.14) and the `TargetMetrics` lead.
3. **Attack state** (5): `Begin`, `PlayAttack` settings, `Tick` with the cancel bit, `End` by case
   (sheet `attack-end`), the window counters, the three whiff cool-downs.
4. **Movement** (6): skip-`CalcVelocity` flag, facing, XY traction, then air mode (Flying → Falling
   timeline, Z channel), root scale, body push.
5. **Targeting** (7): search from the facing, soft target, pin, RB toggle lock, RT switch; then the
   lock-on camera (7.12) in `kh-camera`.
6. **Hits** (8): activation, shape update, sweep, `Apply` steps, `Classify`, the formula of
   `status/damage.rs`, the hit record; then the attacker's gains (8.5: hit confirm, style points on HP
   lost, recoil) and effect / sound choice (8.6). No hit-stop.
7. **Launch attacks, counters, recoil** (3.4, 5.6).
8. **Reactions on Sora** (8.4, 9), Second Chance / Withstand Combo, death.
9. **Second Form and the queue** (10, 11); tests in 10.8.

### 15.2 Request and timing expectations [D]

Scripted tests against training dummies (a dummy = one lock-on point, never moving; "near" = 200 cm,
"far" = 600 cm centre distance). "Press at t" = the pad edge is seen by C and U of that frame.

| Check | Expected |
|---|---|
| press in frame N from Idle / Run | request accepted in **U(N)**; `End` of the old state, `Begin` and first `Tick` of the Attack in **S(N+1)**; the clip clock first advances in M(N+2). (Revision 1 said S(N) / M(N+1).) |
| press during an attack, accepted in U(N) | state+0x143 = 1; the next attack begins in S of the first frame ≥ N+1 whose M has `TresEnableChange` set |
| second press inside the window of `1301`, **no dummy in reach** (whiff), Combo Master not equipped | **refused** (pawn+0xE28 == 0); the swing plays out to its end; a press after it starts a new combo at step 0 (kind 1 again) |
| same with Combo Master equipped (it is in the level-99 loadout) | accepted |
| first swing hits, second whiffs, third press | accepted (the byte stays 1 for the chain) |
| cast (magic) pressed during `1301` after the first hit window opened, before `TresEnableInput`, no hit landed | accepted (`CanMagic` tests neither); the cast starts at `TresEnableChange` |
| attack pressed during a cast | accepted once the spell has left (`OnShot` sets the hit-confirm byte), not on the last combo step |
| press outside the windows (before the first hit window / before `TresEnableInput`) | lost: no buffer |
| jump pressed during an attack | never a jump: `TryLaunch` only (below); the stick does nothing during an attack |
| guard / dodge pressed during an attack | accepted once the cancel bit is up: 0.1333 s after the first hit window opened, or at `TresEnableChange`; for the last finisher only at `TresEnableChange`. They are issued in C(N) and replace the attack in S(N) |
| attack pressed within 0.1 s after a grand-magic command | refused (command cool-down + pawn+0xE60 bit 17); after an ordinary spell or an attack: no block |
| air attack whiffs and nothing follows | for 0.6 s no attack / launch (pawn+0xE34), for 0.6 s no air slide (pawn+0xE38), for 0.3333 s no air guard (pawn+0xE3C); none of the three after a hit |

Per-step windows of the base ground chain [A / D] (clip times):

| Check | `1301` | `1302` | `1303` | `1304` |
|---|---|---|---|---|
| attack request accepted from (later of `TresEnableInput` and the first hit window; plus a landed hit) | 0.1667 s | 0.2667 s | 0.2333 s | — (combo over) |
| cast request accepted from (first hit window) | 0.1667 s | 0.2667 s | 0.2333 s | — (combo over: pawn+0xE48 bit 5) |
| queued attack / cast starts (`TresEnableChange`) | 0.3667 s | 0.4667 s | 0.4667 s | 0.8333 s (other states) |
| guard / dodge / air slide accepted from (cancel bit) | 0.30 s (window + 0.1333) | 0.40 s | 0.3667 s | 0.8333 s (last finisher: `TresEnableChange` only) |
| hit window, group, row | 0.1667–0.4333 `KB_Stick` `KB_VERTICAL` | 0.2667–0.6 `KB_Stick` `KB_STAB` | 0.2333–0.6333 `KB_Stick` `KB_TURN` | 0.6–1.0333 `KB_StickM` `KB_BREAK` |
| homing window | 0.1333–0.3083 | 0.3–0.5 | 0.1333–0.4 | 0.2–0.7333 |
| root travel at scale 1 | 207 cm | 372.6 cm | 207.7 cm | 523.2 cm |
| may clash / be repelled (reflect level > 0 **and** first swing of the combo) | yes | no | no | no |
| style points when HP is lost (`m_FormPoint`, base form) | 6 | 7 | 7 | 8 |

The cancel timer is accumulated in the state tick (`+0x254 += dt` from the tick after `+0x164` was
set; the bit is raised when the sum is **> 0.133333**): implement it as that sum, not as a clip time.

### 15.3 Expected attack kinds (hex) and clips [D from section 4]

"Level 99" = `sheets/port/sora-loadout-99.json` as equipped: Combo Plus ×2, Air Combo Plus ×2 (max 6),
Triple Slash, Charge Thrust, Magic Flash, Final Heaven (3 finishers), Quick Shaft, Flash Step, Radial
Cut, Aerial Sweep, Aerial Dive, Air Down, Rising Spiral, Groundbreaker, Combo Master. Every press lands
inside its window; in the "no abilities" column every swing hits (hit confirm).

| Scenario | No abilities (max 4, 1 finisher) | Level 99 (max 6, 3 finishers) |
|---|---|---|
| ground, one dummy near | 1, 2, 3, 4 (`1301` `1302` `1303` `1304`) | 9, 0A, 0B, 6, 4, 0C (`M_1359` `M_1360` `M_1361` `1306` `1304` `M_1362`) |
| ground, one dummy far (dist > 400 at the first press) | 2, 1, 3, 4 | 0A, 9, 0B, 6, 4, 0C |
| ground, three dummies within 400 cm | 3, 2, 3, 4 | 0B, 0A, 0B, 5, 4, 7 (`M_1361` `M_1360` `M_1361` `1305` `1304` `1307`) |
| dummy above (dz > 160, dist ≤ 400), Sora on the ground; the dummy is level with him after the rise | 8, 10, 17, 13 (`1308` `1311` `1308` `1314`) | 1F, 18, 19, 15, 13, 1B (`1309` `M_1363` `M_1364` `1316` `1314` `M_1366`) |
| same, the dummy still > 160 cm above at the third press | 8, 10, 8, 13 | 1F, 18, 1F, 15, 13, 1B |
| Sora airborne, dummy at his height | 10, 17, 12, 13 (`1311` `1308` `1313` `1314`) | 18, 19, 1A, 15, 13, 1B (`M_1363` `M_1364` `M_1365` `1316` `1314` `M_1366`) |
| Sora airborne, grounded dummy 100–250 cm below, dist ≤ 400 | 0F each normal step while dz < −100, finisher 13 | 20 (`M_1367`), then by the new geometry |
| Sora airborne, grounded dummy ≥ 250 cm below, dist ≤ 400 | as above (no Air Down) | 0D (`1318`), then by the new geometry (0D / 0E are refused right after 0D) |
| no target at all | 1, 2, 3, 4 with `dist = dz = 0`; facing = the actor yaw at each `PlayAttack`; no homing speed | 9, 0A, 0B, 6, 4, 0C |
| jump pressed during the 2nd ground swing after a hit (Rising Spiral equipped) | — | launch **1C** (`1319`, fixed `DirZ` 1000); the combo count restarts at 0; the next attack is chosen as a first attack (`first = true`) |
| jump pressed again later in the same chain | — | launch **1D** (`1320`, Groundbreaker); a third press is refused (both stocks used) until the chain ends |
| jump pressed on a finisher step, or before any hit landed | — | refused |
| Second Form, ground, one dummy near | — | 1, 2, 3, 7A, 7B, 7C without the upgrade abilities; 9, 0A, 0B, 7A, 7B, 7C with them (10.5) |

The search runs along **Sora's facing**: with dummy A dead ahead at 700 cm and dummy B directly behind
at 200 cm the soft target is A (score ≈ 0.8 × 670 against 2.8 × 230); turning Sora around (not the
camera) makes it B. After an accepted attack on A the target stays A (pin) even when B scores better,
until A stops being targetable or no candidate is within 900 cm.

With n more copies of Combo Plus the normal part grows by n steps (cycle by the last kind, 4.2). Each
row should also be checked against the kind logged by the runtime probe (state+0x1B0) in the original,
when available.

### 15.4 Damage numbers at level 99 [D]

Computed with the formula of 8.3 as implemented in `crates/kh-core/src/status/damage.rs` (f32,
engine `Ceil`), Strength 66 (Kingdom Key), game level Standard (player factor 1.0), physical rows,
resistance 0. "Dummy" = a victim without a KH3 status object (`Defender::NONE`: Defense 0, no floor
or cap); for a victim with Defense `d` replace 66 by `max(66 − d, 1)` (and apply its floors / caps).

| Row | `m_Power` | Damage, no ability term | Example with Defense 20 |
|---|---|---|---|
| `KB_VERTICAL` (`1301`, `1311`) | 0.6 | **40** | 28 |
| `KB_STAB` (`1302`), `KB_TURN` (`1303`, `1313`), `KB_RISE` (`1308`), `KB_RADIALCUT`, `KB_THRUST_END`, `KB_ROLLING_END` | 0.8 | **53** | 37 |
| `KB_BREAK` (`1304`) | 1.2 | **80** | 56 |
| `KB_QUICKSHAFT`, `KB_ROLLING` | 0.2 | 14 | 10 |
| `KB_QUICKSHAFT_END`, `KB_FLASHSTEP`, `KB_FLASHSTEP_END` | 0.4 | 27 | 19 |
| `KB_FINALHEAVEN` (**magic row**: `m_bIsMagic`, attribute NOTYPE; the formula takes the Magic stat, 67 at level 99 with the Kingdom Key) | 0.4 | 27 (Magic 67) | 19 (Magic − 20) |
| `KB_THRUST` | 0.6 | 40 | 28 |
| `KB_FINALHEAVEN_END` (**magic row**, Magic stat) | 1.6 | 108 (Magic 67) | 76 (Magic − 20) |
| `LM_STRIDEBREAK`, `LM_STRIDEBREAK2` | 1.0 | 66 | 46 |
| `LM_RIPPLEDRIVE` | 2.4 | 159 | 111 |

Combo Boost (`COMBO_UP` ×1 in the loadout; `AIRCOMBO_UP` ×1 for aerial hits): `x += Ceil(step × 0.05 ×
x)` with `step` = the hit windows the chain had opened before this attack began (5.5; carried only
while every previous attack landed a hit). Chains against one dummy, every hit landing:

| Chain | Damage per hit window, in order | Steps |
|---|---|---|
| no abilities: 1, 2, 3, 4 | 40, 53, 53, 80 (total 226) | — |
| the same chain with Combo Boost ×1 | 40, 56, 59, 92 | 0, 1, 2, 3 |
| level 99: 9, 0A, 0B, 6, 4, 0C | 14 + 27; 30 + 30; 64 + 64; 52 + 52 + 69; 116; **54 + 216** (total **788**) | 0; 2; 4; 6; 9; 10 |
| air, Air Combo Boost ×1: 10, 17, 12, 13 (no other abilities) | 40; 56; 59; 17 per hit of `KB_ROLLING` (re-hit every 0.2333 s) + 61 | 0, 1, 2, 3 |
| Second Form, no abilities: 1, 2, 3, 7A, 7B, 7C | 40; 53; 53; 27 per hit of `LM_EXPLOSION` (re-hit every 0.1 s) + 53; 66 + 66; 159 | — |

The last attack of the level-99 chain (kind 0C, Final Heaven) uses the two magic rows: base `Ceil(0.4 × 67)` = 27 and
`Ceil(1.6 × 67)` = 108, and the combo term is the MAGIC one (`MAGIC_COMBO_UP` ×1 in the loadout: `x += Ceil(step × 0.1 × x)`,
step 10): 27 + 27 = 54 and 108 + 108 = 216. (Revision 2 computed both with Strength 66 and `COMBO_UP`: 41 + 159, total 718;
corrected 2026-10-12, pinned by `crates/kh-core/tests/combat2.rs`.)

Other terms of the loadout that can apply: Counter Boost ×1 on counter rows (`x += Ceil(0.5 x)`), the
five attribute boosts (not physical rows). On Beginner the player factor is 1.25 (`Ceil(1.25 x)` before
the ability terms). In Second Form the form adds OVERCOME and COMBO_MASTER; OVERCOME can raise Strength
([C], `docs/sora-stats-spec.md` 2.7), so the form numbers hold only without it. The implementer should
pin this table in a unit test of `compute_damage` (the numbers above come from an f32 re-computation of
that function, not from the running game).

### 15.5 Movement, hit and gain expectations [D]

- Homing: during an open window the commanded horizontal speed follows `v ← v + (800 − v) × min(0.2 × dt
  × 60, 1)`: 160, 288, 390.4, 472.3, 537.9, 590.3, 632.2, 665.8, 692.6, 714.1 cm/s over the first ten
  60 Hz ticks; target inside the stop distance (160 cm from the lock-on point) or window closed: `v ← v ×
  0.5` per tick on the ground. Without a target no homing speed is ever commanded. The stick, braking and
  friction have no effect during an attack; the homing speed is added to the root motion.
- First attack of an air combo: target speed 1200, rate 0.3. Air-mode attack: Flying with V = 0 until the
  window; after the window Falling at gravity scale 0.1 (g = −98 cm/s²) until `TresEnableChange`, then the
  default scale 1.5 (g = −1470); with a vertical offset to the target the Z speed stays pinned by `DirZ`.
- Sora's capsule passes through enemy capsules; only the body push (6.5) separates them.
- A hit window hits each actor once; with `AttackInterval` the whole hit list is emptied `interval`
  seconds after the window's **first** hit. The first tick of a window tests the shape in place.
- Only `KB_Stick` can touch level geometry; a wall hit plays effect `VW_SO010_WHT0` and **no sound**.
- **No hit-stop**: no time dilation, no pause, no animation freeze on a hit; `MyHitStopTimeSmall` / `Big`
  (0.02 / 0.05) are dead properties.
- No hit sound when the attacker result is REFLECT (a repelled first swing); a guarded hit plays the
  guard effect (with its own sound) instead of the chooser's.
- Style points: `m_FormPoint × rate` is added **only when the victim lost HP** (blocked, guarded,
  armoured-without-break and zero-HP-loss hits give nothing); two hits in one frame count 1 and ½; base
  chain `1301`–`1304` on a dummy that loses HP: 6 + 7 + 7 + 8 = 28 points. No focus from hits.
- Knockback direction handed to the victim: the 3D unit vector from Sora's root to the victim's root.
- Being hit (9): SMALL on the ground slides 550 × scale cm/s × 0.9 per frame (91.7 cm in total), control
  from 0.4667 s; a hit during DAMAGE re-enters DAMAGE (no invincibility); Second Chance keeps 1 HP from
  ≥ 2 HP; Withstand Combo keeps 1 HP inside DAMAGE or within 0.6 s of the previous damage.

---

## 16. Changes from revision 1

Everything below is already applied in the text above; the lists are kept so that code and sheets written
against revision 1 can be re-checked. Section numbers inside the items are those of **revision 1**
(`build/sora_tmp/combat/spec_rev1.md`) unless they point at a subsection that exists only in revision 2.
Where the sections moved: revision-1 3.5 → 3.1.4; 5 (`Tick`, `End`, "what ends an attack") → 5.2–5.4; 6.3 →
6.3 (rewritten) with 6.4–6.6 new; 6.4 (warps) → 6.7; 7 → 7.1–7.14; 8.2 → 8.2.1–8.2.6; 8.4 → 8.4 (rewritten);
9 → 9.1–9.10; 10, 11 → rewritten; 14 → rewritten; 15 → rewritten.

Summary of what changes behaviour (the full lists follow):

| Topic | Revision 1 | Revision 2 |
|---|---|---|
| who issues the attack press | the controller update (phase C) | the HUD command menu in the UI tick (phase U); effect one frame later |
| chaining | first hit window + `TresEnableInput` | plus a landed hit of this chain (pawn+0xE28), unless Combo Master |
| cast out of an attack | same windows as an attack | first hit window only; no `TresEnableInput`, no landed hit |
| guard / dodge out of an attack | `m_bEnableChange` | cancel bit: 0.1333 s after the first hit window, or `TresEnableChange` |
| jump / stick during an attack | "their own gates" | never (launch only) |
| launch attacks 0x1C / 0x1D | reachability unknown | reachable for base Sora, once each per chain |
| attack `End` | call list | by case; a whiffed air attack starts three cool-downs; counters carried only after a hit |
| velocity during an attack | "direct flags not read" | `CalcVelocity` body skipped: no braking, friction or stick |
| air attack | "falls with normal gravity after the window" | gravity scale 0.1 until `TresEnableChange`; Z speed pinned by `DirZ` |
| root scale | clamp unknown | no clamp; horizontal only; per montage instance |
| enemy bodies | "collision limits it [?]" | no capsule collision; soft body push by level |
| search direction | unknown | Sora's facing; origin root + 100 up + 30 forward |
| target `radius` | radius | `m_AutoLockOnRange` (a range, not a radius) |
| manual-lock flag | controller+0x878 bit 6 | controller+0x960; bit 6 is the UI's input lock |
| hit list / interval | per-victim re-hit time | one timer per window, started by the first hit, emptying the whole list |
| hit-stop | constants found, use unknown | none (dead properties) |
| damage formula | no difficulty factor inside; RECOVER → 0.1 | difficulty factor on both paths; RECOVER returns the attack stat |
| combo step in the damage info | the combo count | state+0x130 (hit windows before this attack) |
| style points | per hit | only when the victim lost HP; gauge on the controller; level = form level |
| `CanAttack` out of DAMAGE | sub-step ∈ {1, 2, 7, 8, 9, 10} | damage kind (state+0x88) ∈ that set |
| HP subtraction | pawn slot 424 | `ApplyHP` 0x1405A2A00 (slot 424 is statistics); Second Chance / Withstand Combo located |
| combo carry timer pawn+0x10E4 | "formchange states" | `WeaponChange` (state 32) only, 0.5 s |

### 16.1 Request path, attack state, interleave (strand A)


1. 3.1 / 13.2: the normal attack press is **not** issued by the controller update. It is the command menu's decide in the
   UI tick (`0x1416220D0` → slot 682 → slot 680 → executor); the direct call 0x1413F257D belongs to link / attraction mode
   (bytes ctrl+0xBB8 / +0xC08). Slot 677 (`ctrl+0x8C8`) is the command cool-down and gates both paths (through slot 681).
2. 3.5: all normal-path latencies are one frame longer (request in U(N), effect in S(N+1)).
3. 3.2: `iface+0x9D8` = pawn+0xE28 = hit confirm (rule 3.2.1); the GUARD branch is "counter window open **or** the guard-hit
   clip's input window"; `ctrl+0x8A0` is the counter window, not the just-guard timer (that is `ctrl+0x89C`).
4. 3.3: IDLE / RUN use the carry timer; DODGEROLL does not test it; the GUARD branch retargets to the stored attacker and
   gives a forced kind only while the window is open; ids 11, 74, 97, 102, 111, 122 are plain `SetState` entries.
5. 3.4: `iface+0x1A0` is `m_ChrUniqueID == SORA`, not an unknown switch: launch attacks are reachable; the stock rule and
   the hit-confirm / not-a-finisher conditions are as in 3.4.
6. 5 (`Tick`): state+0x254 / 0.1333 s raises the **cancel bit** (+0x234 bit 7) used by guard, dodge, air slide and swim; it
   does not run for the last finisher. `m_bEnableChange` is not what those gates test.
7. 5 ("What ends an attack"): jump and run never end an attack (gates return false for attack ids).
8. 5 (`End`): full branch list above; there is no unconditional `StopMovement`; the hit counters are carried only after a
   landed hit; a whiffed air attack starts three cool-downs.
9. 5 (fields): +0x12C / +0x138 are window counters (5.5), +0x13C bit 2 "a hit landed", +0x18C the air-attack cool-down
   arming value (0.6), +0x234 bit 13 "a hit landed", bit 7 cancel window, bit 19 chase armed, bit 23 reaction command shown.
10. 6.3: the traction update is the delegate run by slot 128 (confirmed at instruction level); the hover ends at
    `TresEnableChange` (slot 128), not when the window closes only.
11. 14: both sides advance pawn+0xE14; a cast needs neither `TresEnableInput` nor a landed hit.
12. 13.6: slot 1304 is the friend unison roll (no effect for solo Sora).

### 16.2 Movement (strand B)


1. 6.1 "How the walking / falling code treats the direct flags [?]": they do not; only the `CalcVelocity` tail does, and
   every attack runs with the body of `CalcVelocity` skipped (6.1.1). Braking, friction and the speed clamp do not exist
   during an attack.
2. 6.3: instruction-level corrections listed at the end of 6.3.
3. 6.3 net effect "then falls with normal gravity": after the window the gravity scale is still 0.1 until
   `TresEnableChange`, and with a vertical offset to the target the Z speed is pinned (no fall) for the rest of the state.
4. 6.3 "body collision with the target limits it [?]": no collision limit exists (6.5); the limit is the stop distance and
   the soft body push.
5. 13.4: root scale: no clamp; per montage instance; horizontal only for attacks.
6. 4.1 `TargetMetrics` "velocity lead [?]": measured velocity × 0.2667 s.
7. 5 (`Begin`): "pawn slot 616 (1, 1, 1.0)" is `Draw(weapon)` with the 10 s put-away timer; "pawn+0x6E4 = 1" is the stance value.
8. `docs/sora-actions-spec.md` II.2 calls component+0xBA8 "per-tick push displacement, 0 in plain play" and +0xB90 the
   force-volume velocity: +0xBA8 / +0xBB4 are the **body push** of 6.5 (non-zero whenever push bodies overlap).

### 16.3 Hits (strand C)


1. 8.1: "Collision profile `TresProjectile` … swept between frames" — the profile name is only stored; the sweep is a
   trace on channel TresAtkCollision with the response set of 8.2.3. Only `KB_Stick` can touch level geometry.
2. 8.2: "'already hit' list per activation (re-hit after `AttackInterval`)" → one timer per window, started by the
   first hit, emptying the whole list; already-hit actors are excluded from the query itself.
3. 8.2: the `m_DamageExecRate` roll uses the engine xorshift128 (`(r >> 8) × 100 / 2²⁴`); a failed roll records the
   victim. Never rolled for Sora (all rates 100).
4. 8.2: "hit effect / sound / camera / gauge handling in 0x14061EA30 and 0x1406294F0" → 0x14061EA30 is only the
   knockback direction; 0x1406294F0 only effects and sound. Gauges are in the causer's notify slot 31 (0x141442DE0),
   called from the victim's HP code; there is no camera or hit-stop handling.
5. 8.2 / 8.3: "Hit-stop constants [I, use not read]" → unused; hit-stop exists only behind a debug exec.
6. 8.2: `TresDamageInfo` "+0x54 combo step" → +0x52..+0x55 are four counters; +0x52 is the combo count, +0x54 (the
   one the formula reads) is state+0x130. Section 2's "writes the combo count (state+0x128) as the damage info's combo
   step" is wrong in that respect. "+0x65 calc type" stays; +0x66 is a survive rule read by 0x1405A2A00.
7. 8.2: flag masks of `TresDamageInfo` +0x50 / +0x51 are now confirmed (8.7), including which are set by code.
8. 8.4: the reaction results "3, 4, 5" are `ETresReactionResultType` REFLECT / GUARD_REFLECT / GUARD, decided by the
   attacker's `Classify` before `TakeDamage`, not by the victim inside it; `TakeDamage` is not called at all for a
   blocked hit (`noReact`) or a zero amount (victim notify slot 28 instead).
9. 10: "gain per hit = the row's `m_FormPoint` × `MyStylePointAddRate` × a level rate … The adding function … not
   read" → 0x1413E9000, decoded in 8.5.2; the gain is granted only when the victim actually lost HP.
10. 12: `Hit.knockback_dir` "INSTIGATOR_PAWN for most kb_* rows: away from Sora" → the 3D unit vector from Sora's
    root to the victim's root (not horizontal); NORMAL rows give the same for Sora. `HitResult` as an enum returned by
    the victim does not exist: the result is computed from the victim's body reaction type before the call.
11. Section 0 item 6 stays true, with the addition that the *guard outcome* is computed by the attacker's component
    from the victim's answers (8.2.6).

### 16.4 Targeting (strand D)


1. `searchDir` is **Sora's facing** (root component forward, flattened); the origin is root + (0, 0, 100) + facing × 30.
2. `tc.slot350()` is `m_AutoLockOnRange`, not a radius, and "own radius" (pawn slot 1328) is `MyAutoLockonLimit` (900):
   `dist = d3 − (range − 900)`. The host trait's `radius()` is replaced by `auto_lock_on_range()`.
3. controller+0x878 bit 6 is an input lock set by the UI, not the manual-lock flag. The manual-lock flag is +0x960
   (slot 720).
4. The constants +0x970…+0x988 are the pawn's `MyAutoLockonLimit` 900, `MyAutoLockonNear` 150, `MyManualLockonLimit`
   3500, `MyManualLockonOff` 4200, `MyShootLockonLimit` 5000, `MyAthleticLockonLimit` 15000, `MyAthleticLockonRange`
   3750 (first pass: "near range", "minimum distance", "max range", unnamed).
5. The crowd count is taken before the Enemy / priority / line-of-sight tests, on the raw 3D distance.
6. A candidate that loses on score is not line-of-sight tested; the test runs only for a would-be best.
7. Section 7.3's "No stick-direction re-targeting inside the attack state" stands, and is stronger: the attack request
   pins the current target on the controller until nothing is within 900 or the target dies (7.6).
8. Camera spec section 5 line 1: `FUN_1405F0140` is both the start and the end (argument null); `FUN_1405F0990` is a
   variant with an explicit camera. `m_RotationSpeedMax/MinDistance/MaxRate` scale the **stick** speed with the distance
   to the target; `m_LockonAutoRotationLowDist/HighDist` scale the automatic turn toward the target (they are read
   by the shared auto-follow 0x1405FFA60, in its lock-on branch).
9. Input sheet: `rt_hold_mode` is the entry to shooter mode (`TresPlayerState_ShooterMode`, state id 40) for the
   keyblade forms 0x80 / 0x83 / 0x8E / 0x8F; `lock_on` "locks the candidate" → locks the current target (7.6);
   `lock_on_target_change` steps a distance-sorted list of at most 6 within 2400 cm.

### 16.5 Sora being hit (strand E)


1. `CanAttack` out of DAMAGE tests the **damage kind** (state+0x88) ∈ {1, 2, 7, 8, 9, 10}, not the sub-step.
2. "sub-step = damage kind (so the kind selects the handler)" is wrong: state+0x88 holds the kind; the sub-step (+0x78)
   starts at 0; the handler is a delegate chosen by a jump table.
3. HP is not subtracted in pawn slot 424 (0x141424360 / 0x1405C38B0: damage statistics). It is subtracted in 0x1405A2A00
   through pawn slot 776 (0x1405B9700).
4. LAST_LEAVE / COMBO_LEAVE are located: 0x1405A2B29–0x1405A2BA2 (9.7).
5. The gate's `pawn+0xE60 bit 17` test is pawn vt+0x2788; the freeze / burn branches come before the light-kind test; BEAT
   (9) is a light kind (blocked by the no-flinch states), PARABOLA (10) is not.
6. The knockback is not computed from `m_DamageParabolaAngle` / `m_DamageMoveLength` / `m_DamageBrakeParam` alone: speeds
   come from `PlayerDamageCommonParam` scaled by `m_DamagePowerScale`; the row's brake and parabola angle override the
   table when > 0; the move length adds a separate start impulse.
7. State+0x138 (0.2 s) is the delay before Aerial Recovery, not an invincibility or control-loss timer.
8. Clips `0205` / `0206` are the alternates of `0201` / `0202` on every second consecutive hit (variant = hit counter & 1),
   not per direction.
9. `m_BlowDamagePower` (1500 in the asset) is not what the player's Blow uses: slot 131 reads `m_AirBlowDamagePower` (2000).
10. In `docs/sora-actions-spec.md` 3 / 6.1 and `docs/sora-magic-spec.md` (table "DAMAGE 19 … sub-step in {1, 2, 9}") the
    Damage tests named "types" / "sub-step" should be re-read as the damage kind state+0x88 (not re-verified here for those
    gates).

### 16.6 Second Form and the queue (strand F)


1. Section 10 "gain … × a level rate (Lv0 1.0, Lv1 0.8, Lv2 0.55; critical-mode set 1.0 / 0.5 / 0.8 / 0.55)": the
   level is `Param1` of the **current form's** command row (0 outside a form, 1 in Second Form), not a gauge or
   keyblade level. The critical-mode set is: overall rate 1.0, Lv0 0.5, Lv1 0.8, Lv2 0.55. "With Critical Charge 1.3"
   needs critical mode, the ability and a running MP charge.
2. The gauge is on the **controller** (ctrl+0xB40), not on the pawn; the pawn holds only the parameters. The probe
   request "pawn +0xCA8 and the current style-point value near it" is replaced by the controller rows above.
3. `MyStylePointDecStartTime` 20 and `MyStylePointDec` 5 are blueprint values; the native defaults are 10 and 10.
4. Section 3.3 / notes: "pawn+0x10E4 … whose two callers are in the formchange states (0x1413CF7D0, 0x1413D9E50)":
   both functions belong to `TresPlayerState_WeaponChange` (state 32, vtable pointers at 0x14694E630 / 0x14694E628),
   value 0.5 s, set only when that state ends with its +0x8C flag and nothing queued. Formchange and finish states
   never set it: the combo counter is not carried into or out of Second Form.
5. Section 10 "form-finish commands … `LM_UNIONDISCODE` (174)": 174 is level 3 only for form 0x95 (`CHANGE_L2`); the
   Kingdom Key's level 3 is 173.
6. Section 10 "Second Form finishers in the kind switch": order by finish index is 7A (`M_1412` Explosion), 7B
   (`M_1411` Stride Break), 7C (`M_1413` Ripple Drive) on the ground and 7D, 7E, 7F in the air; the air variant is
   chosen by `bAir` alone.
7. Section 4.1 "`MaxCombo()` … in a formchange a constant by command kind": for 0x88 it depends on ctrl.vt+0x1B28();
   base Sora's branch also returns 4 while swimming or without a controller, 5 for pawn type 6, 3 for type 0x0A.
8. Section 11 "The controller holds the current command kind (+0x87C)": +0x87C is written only by `ExecuteCommand`
   at the press; the selected entry lives in the HUD (hud+0x6F4).
9. Section 3.1 "controller+0xC08 || controller+0xBB8 [? meaning]": +0xBB8 is the byte returned by ctrl slot 879 and
   tested by the executor's link kinds, +0xC08 by slot 890 for the attraction kinds, +0xC09 by slot 894 for the team
   kinds: "a link / an attraction / a team attack is active". The direct attack call 0x1413F257D is therefore the
   path of those modes; a normal press goes through the HUD and `ExecuteCommand` (confirmed: 3.1.1).

### 16.7 Corrections made at the merge

1. 8.3: the difficulty factor and the RECOVER return value (see the end of 8.3).
2. 8.4 / 9.6 / 12.2: `info+0x60` is the victim result (`ETresReactionResultType`) written by `Classify`, not a
   body-collision reaction type; the values 3–6 that make `apply = false` are REFLECT, GUARD_REFLECT, GUARD,
   ARMOR_BREAK. (Strand E had named them by the other enum; the values and the code path are unchanged.)
3. 8.5.2: the style-point function is specified once, in 10.2. Checked at the merge (0x1413E9268–0x1413E92C2):
   ctrl+0xB44 is the *pending* value and the chain factor ctrl+0xB48 multiplies every grant (strand F's
   reading); strand C's text called +0xB44 the gauge and did not apply the factor.
4. 9.5: the guard counters use the counter window ctrl+0x8A0 (1.0 s); the just-guard timer is ctrl+0x89C.
5. Sheets: no value of a revision-1 row changed; the texts of the rows `swim.exit_delay` (it is the cancel
   delay), `hitstop.small` / `hitstop.big` (not applied), `air.hold_time` (cool-down arming value),
   `root.scale_rule`, `style.add_rate*`, `reaction.cooldown`, `damage.random`, `target.facing_weight`,
   `decide.crowd_radius`, `combo.base_max` and of the reaction rows `setup`, `down`, `heavy`, `die` were
   corrected.

Cross-spec disagreements and the sentences their owners should change: `docs/sora-spec-reconciliation-2.md`.

### 16.8 Third pass (2026-10-12, third combat implementer)

Corrections: 15.4 (Final Heaven rows are magic rows: 54 + 216, total 788), 9.4 Blow sub-step 3 (the start clip plays out
first; rise 92 cm), 10.4 (the form clock does not run during STYLECHANGE), 10.6 (Sonic Rave always starts with `M_1424`).
Rewritten: 9.9 (bad statuses on Sora: the status byte, STUN / FREEZE / BURN states, sneeze, STOP, HONEY, HP_BREAK).
Added: 3.3 traversal rows and 4.2 rule 4 (wall-run attacks 0x31–0x34, 0x36 / 0x38 / 0x39 out of slide, rail, wall kick,
pole), 4.3 kinds and `SetAnimTime`, 5.5 counter gate, 5.6.3 gate and guard-reaction entries, 5.6.4 Final Blow, 7.12 framing
solver / lead / obstruction blend, 8.2.5 step 9 (map-hit handler) and 8.5.1 (map-hit recoil), 8.6 (the manager's common
effects by name), 8.8 (shots: DOWNIMPACT, SHOCKWAVE_S), 11.5 (AUTO_FINISH). Sheets: `chase.*` rows of
`sora-combat-params` (strand "P3" of `tools/lib/sora_combat_pass2.json`), rows `kind_31` … `kind_34`, `kind_39` of
`sora-combat-attacks` with their windows and attack rows.

Still open after it (probe requests in `docs/sora-runtime-probe.md` 8b): who calls the stun trigger (anim-interface slot
78 0x1405B2A50); the second stun-flick key (0x149C6F9F8); hud+0x708 of AUTO_FINISH; equipment vt+0x428 (the weapon test
of the counter-allowed gate in a form) and vt+0x580 (the shot location); the copy of the chase's 0.2 s; the interpolator
curve of the camera's obstruction release; the P3 items G 1–4 (Last Arcanum's count and `finishEnd` writers, the
StyleFinish stick block) and H 3–5 (tick order of the attack-collision manager, the rush-suppression caller, the body-push
solver at instruction level), which this pass did not reach.

### 16.9 Fourth pass (2026-10-12, fourth combat implementer)

All static (decompiled C and disassembly, no runtime). Closed: P3 strand items **G 1–4** (10.9 "Settled by the fourth pass":
Last Arcanum's count writer 0x1413DD7D0 [I]; `finishEnd` has no writer; state+0x1D8 has one reader; `pawn.vt+0x1048` =
force feedback, `pawn.vt+0x23C8` and the equipment call = `EndEffect`; the stick block is dead for the Kingdom Key; begin / end
events and the gates against states 31 / 33 at instruction level) and **H 3–5** (8.2.2: the manager ticks in
`TG_SQEX_PostUpdateTransform`, after the pawn, same frame; 7.10: the rush-suppression handler is called by the body-contact
dispatcher 0x140633EE0; 6.5: the solver's penetration vector, the unpushable case, push level 8 during a finish). From the
"also open" list: the attack camera (7.12.1), the shot location (8.8: socket `ATK_01`), the counter gate's weapon test
(5.6.3: passes for the Kingdom Key), the HOPPING / SLOPESLIDE attack requests (3.3, 4.3: kind 0x2F), the obstruction release
curve (7.12.1: sin(t π/2)). Corrections: 4.3 (`FUN_14140DF30` is the sneeze latch, not a camera), 9.9.5 (the sneeze replaces
the finisher), 4.3 (state+0x234 bit 3 and state+0x23C bit 4 for Rising Spiral and Sonic Cleave), 8.8 (a whiffed Sonic Cleave
spawns no shot). Sheets: row `kind_2f` of `sora-combat-attacks`, its window in `sora-combat-hits`, the rows `RUN_SLOPE`,
`RUN_WALL_RISE / DOWN / SIDE` of `sora-combat-attack-data` (the wall-run attacks' windows had no row before).

Still open, and why:

- needs the running game (probe requests C4-1 … C4-6 in `docs/sora-runtime-probe.md` 8b, besides C3-1 … C3-9): the game-object
  flag +0x84C of the push solver; controller+0xAE0 (distance scale 1.35 after the attack camera); equipment vt+0x508 (called
  by the forced kinds); whether Last Arcanum's HUD extra `LMEX_THEEND` is really removed by the `LMEX_BREAK` call (HUD
  0x14158C180 op 3); the clip of Sonic Cleave's descent against the root motion (bit 3); pawn tick group (taken as stock).
- not read (other owners or out of scope): the cameras of modes 1 / 2 / 8; the `EndEffect` calls of the finish end (the effect
  group seam is the effect implementer's); enemy push levels; the adhere actors of CLOUD / DISCHARGE; CLIMBING.

