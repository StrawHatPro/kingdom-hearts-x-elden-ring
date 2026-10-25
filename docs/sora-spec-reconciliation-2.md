# Sora specs: reconciliation 2 (combat revision 2 against the other specs)

Written at the merge of `docs/sora-combat-spec.md` revision 2 (2026-10-11). It lists every disagreement
between the combat spec and the other Sora specs that the six combat strands, the status implementer and
the other agents raised; for each: the two readings, which is right with the evidence, and the exact
sentence its owner should change. **Only the combat spec was edited** (it is now consistent with every
resolution below); the other files are untouched. Evidence tags as in the specs: [I] instruction level,
[C] decompiled C, [A] asset. "Combat n" = a section of combat revision 2.

Earlier reconciliation (movement / animation / camera): `docs/sora-spec-reconciliation.md`.

## Summary

| # | Topic | Specs | Right | Who changes text |
|---|---|---|---|---|
| 1 | dodge / guard out of an attack: cancel bit, not `m_bEnableChange` | actions II.0.2 (1) vs combat rev. 1 | actions | combat (done); actions: status sentence |
| 2 | controller+0x8A0 counter window, +0x89C just-guard timer | actions II.0.2 (2) vs combat rev. 1 | actions | combat (done); actions: countdown located |
| 3 | `TresDamageInfo` +0x60…+0x63 | actions II.0.2 (3) vs combat rev. 1 | actions | combat (done) |
| 4 | "ignored" hit has two forms | actions II.0.2 (4) vs combat rev. 1 | actions | combat (done) |
| 5 | movement component +0xBA8 "0 in plain play" | actions II.2 vs combat 6.5 | combat | actions |
| 6 | Aerial Recovery: delay and kind-2 jump | actions 6.6 vs combat 9.5 | both (combat adds) | actions (addition) |
| 7 | Damage-state gates: "sub-step" vs damage kind | magic part A table, actions 3 / 6.1 vs combat 9.5 | combat | magic; actions (wording) |
| 8 | attack ↔ cast interleave, finisher index | magic part A vs combat rev. 1 section 14 | magic | combat (done); magic: name the byte |
| 9 | `Param1` of the formchange rows | magic J 5.2 vs combat 10.2 | combat | magic |
| 10 | grand magic produced from the style gauge | magic I 7 vs combat 10.2 | agree | — |
| 11 | form time gauge: total / remaining getters | UI 5.8 vs combat 10.1 | neither label; see below | UI |
| 12 | caller of the time-drain function | UI 5.6 vs combat 10.7 | combat (fills the gap) | UI |
| 13 | EXP popup order | UI 8 vs stats 4.2 | stats | UI |
| 14 | FORM_TIME lengthens the command, not the form | stats 5.4 vs combat 10.3 | combat | stats |
| 15 | damage formula: difficulty factor, RECOVER | combat rev. 1 8.3, stats 7 vs the listing | listing (`status/damage.rs`) | combat (done); stats step 1 |
| 16 | HP rule flags of `ApplyHP` | stats 6.1 vs combat 9.7 | combat (fills the gap) | stats |
| 17 | hit sound: REFLECT and map hits | audio 5.4 vs combat 8.6 | combat (adds two conditions) | audio |
| 18 | lock-on camera stub | camera 5 vs combat 7.12 | combat | camera |
| 19 | input sheet rows `rt_hold_mode`, `lock_on`, `lock_on_target_change` | `sora-input-layout` vs combat 7 | combat | sheet owner (actions) |
| 20 | attack press path and frame phase | combat rev. 1 3.1 / 3.5 vs UI "update order" | UI order + combat 3.1 | combat (done) |

---

## 1–4. `docs/sora-actions-spec.md` II.0.2: the four combat-side corrections

All four were confirmed by strand A (instruction level) and by strand C; combat revision 2 is changed.

1. **Gates out of an attack.** Actions: dodge / guard test Attack+0x234 bit 7. Combat rev. 1: `m_bEnableChange`.
   Right: actions. Evidence [I]: guard gate 0x1414294C0 @0x1414296EF, dodge gate 0x141427CC0 @0x141428190,
   air-slide gate 0x141425780 @0x141425D3E test bit 7; `Tick` 0x141341340 raises it 0.1333 s after the first hit
   window (timer +0x254, constant 0x146903A28) or at `TresEnableChange`, for the last finisher only at
   `TresEnableChange`. Combat 5.2, 5.3.
2. **The two timers.** Actions: +0x8A0 = 1.0 s counter window, +0x89C = 0.1333 s just-guard timer. Combat rev. 1
   had them the other way round. Right: actions. Evidence [I]: writers `FUN_1414145C0` (+0x8A0) and
   `FUN_141414B70` (+0x89C); countdowns controller slot 637 0x1413F8130 and slot 636 0x1413F85D0. Combat 5.6.1.
3. **`TresDamageInfo` +0x60…+0x63.** Missing in combat rev. 1. Now in combat 8.7: +0x60 victim result, +0x61
   attacker result (both `ETresReactionResultType`, written by `Classify` 0x14061FE20 [I]), +0x62
   `m_ReflectReaction`, +0x63 `m_ReflectLevel`.
4. **"Ignored" has two forms.** Now in combat 8.2.6 and 12.1: GHOST body types return without recording the
   victim; INVINCIBLE types record it for the attacker (blocked effect, no damage).

Sentence for the actions owner (II.0.2, paragraph "Combat"): replace "— combat 5 should be corrected", "—
combat 3.2 / 3.3 / 13 name them the other way round" and "are missing from combat 8.2" by "(combat revision 2
agrees: 5.3, 5.6.1, 8.7, 12.1)".

Also for the actions owner, II.1 (line "The countdown of +0x89C was not located [?]."): replace by "The
countdown of +0x89C is controller slot 636 0x1413F85D0 (`-= dt`, floor 0) [I]."

## 5. Movement component +0xBA8

- Actions II.2 (`PhysFlying` pseudocode): "`Adjusted += vec(+0xBA8)` // per-tick push displacement
  (PerformMovement 0x14058CEB7), 0 in plain play".
- Combat 6.5 (strand B): +0xBA8 / +0xBB4 are the **body push** of the body-collision component: written every
  movement tick by component slot 414 0x14057ABD0 from `UTresBodyCollComponent`'s push vector, zeroed by
  `PerformMovement` after the physics (0x14058CEAF) [I for the movement side; the solver 0x140637430 is C].
- Right: combat. It is zero only while no push bodies overlap; it is how enemies and Sora separate (the
  capsules ignore each other).
- Sentence: replace "per-tick push displacement (PerformMovement 0x14058CEB7), 0 in plain play" by "body-push
  displacement of the body-collision component (slot 414 0x14057ABD0; non-zero whenever push bodies overlap,
  zeroed after the physics; combat spec 6.5)".

## 6. Aerial Recovery

No contradiction (actions 6.6: jump press → `AIR_SLIDE` with flag 0x20, kinds 2, 7, 8, 10, ability 1). Combat
9.5 adds [I]: the "recoverable" byte Damage+0x8E is 1 only from **0.2 s after the hit** (+0x12C > +0x138) and
only in the airborne sub-steps (Blow 4; Toss / Launch 13–15; Parabola 4–5), 0 again at the down / get-up clip
and at a wall hit; damage kind 2 also lets a plain jump through at `m_bEnableChange` without the ability (jump
gate 0x14142AA57, table 0x14142B01C). Sentence to add to actions 6.6: "Available from 0.2 s after the hit and
only in the airborne sub-steps of the Blow / Toss / Launch / Parabola handlers (combat spec 9.5); with damage
kind 2 a plain jump is also accepted at `m_bEnableChange` without Aerial Recovery."

## 7. Damage-state gates: the tested field

- Magic spec, part A gate table: "| DAMAGE 19 | +0x84 bit 0 set, +0x90 == 0, sub-step in {1, 2, 9} |". Actions
  3 (line "19 DAMAGE : +0x90 == 0 && type ∈ {1, 2, 9} && (+0x84 & 1)") and 6.1 ("type(+0x88) in {1, 2, 9}").
- Combat 9.2 / 9.5: Damage+0x88 is the **damage kind** (copy of `info+0x0A`); the sub-step is Damage+0x78 and
  starts at 0 for every kind [I `Setup` 0x141365930; `CanAttack` table 0x14142700C].
- Right: combat (and the actions spec, which already names +0x88). Evidence for the magic gate: `CanMagic`
  0x1414286F0 reads `Damage+0x88` in its DAMAGE branch [C].
- Sentence for the magic owner: replace "sub-step in {1, 2, 9}" by "damage kind (Damage+0x88) in {1, 2, 9}".
  For the actions owner: in section 3 write "kind(+0x88)" instead of "type" (same meaning, one name).

## 8. Attack ↔ cast interleave

- No disagreement was found between combat 14 (strand A, both sides [I]) and magic part A (2.2, 2.3, 5.1, 5.2,
  5.4). Combat rev. 1 section 14 was wrong in two points, corrected: (a) the finisher index pawn+0xE14 is
  advanced by **both** sides (cast first tick 0x1413798DA–0x141379981); (b) a cast out of an attack needs
  neither `TresEnableInput` nor a landed hit (`CanMagic` attack branch 0x14142899A).
- Magic 5.4, row "attack": "… +0xB4 < MaxCombo; interface+0x9D8 != 0; combo not over." The byte is unnamed
  there. Sentence: replace "interface+0x9D8 != 0" by "interface+0x9D8 != 0 (= pawn+0xE28, the hit-confirm byte
  of combat spec 3.2.1; the cast's `OnShot` 0x14137C9CA sets it, so the test passes once the spell has left)".

## 9. `Param1` of the formchange rows

- Magic J 5.2: "`Param1` of the formchange rows (1) was not seen read in this strand [?]." (also open item 6:
  "`Param1` (= 1) and the unused `Param2` of the formchange rows.")
- Combat 10.1 / 10.2 (strand F) [I]: controller slot 847 0x141401160 returns `TresCmdData[kind].Param1`
  (row+0x28); `AddStylePoint` 0x1413E9000 uses it as the **level** that selects the gauge rate (0 outside a
  form → ×1.0, 1 in Second Form → ×0.8, 2 → ×0.55), and `StartStyle` 0x14141A9D0 stores it as the form level
  (controller+0xB34). `CanLaunch` 0x14142B3F0 also reads it (`> 0` refuses the launch outside states 159–161).
- Right: combat. Sentence: replace the quoted sentence by "`Param1` of the formchange rows is the form level:
  read through controller slot 847 0x141401160 (row+0x28) for the style-gauge rate and stored in
  controller+0xB34 at form start (combat spec 10.2, 10.3)"; drop it from open item 6.

## 10. Grand magic from the style gauge

Agreement (magic I 7.1, 7.2, J 5.3; combat 10.2, 11.6): a full gauge is always consumed; `AwardAtMax`
0x1413F0ED0 first calls `RollGrandMagic` (slot 838 0x1413ED500); only when nothing was rolled (and no finish is
pending) the formchange command 148 is enqueued; grand-magic casts add no style points (kinds 32, 36, 40, 44, 48,
110–124, 126, 231); inside Second Form a full gauge raises the finish level (and may still roll grand magic).
Capacity 6 and the eviction rule agree. Nothing to change.

## 11. Form time gauge (UI 5.8)

- UI 5.8: "total = controller vt+0x1A98, remaining = vt+0x1B30 while vt+0x1A70 names this form's command".
- Code [I for the getters, C for the caller]: vt+0x1A98 = slot 851 0x141108F20 = `controller+0xB54`, the
  **remaining** form time (counted down by `StyleGaugeTick` 0x14141C4F0; the full duration is +0xB58, getter not
  used here); vt+0x1B30 = slot 870 0x14110A4F0 = `controller+0xB6C`, the action-command time written by
  slot 871 0x141415D20. The gauge update 0x1416318A0 does, per slot-B entry: if the entry's kind equals
  `vt+0x1A70()` (the form command): `v = vt+0x1A98()`; `if v > entry.total (+0x14): entry.total = v`;
  `entry.remaining (+0x18) = v`; **else** `entry.remaining = vt+0x1B30()`. `0x141631BC0` then draws
  `remaining / total`.
- So neither getter is a "total": both feed the entry's *remaining* (the first for the form's own entry, the
  second for any other entry, e.g. a finish command), and the total is the entry's own +0x14, raised whenever
  the remaining exceeds it (which is how the +3 s of a new finish level show as a full gauge only up to the
  largest value seen).
- Sentence: replace "total = controller vt+0x1A98, remaining = vt+0x1B30 while vt+0x1A70 names this form's
  command" by "remaining = controller vt+0x1A98 (controller+0xB54, the form time left) when the entry is the
  form's own command (vt+0x1A70), else vt+0x1B30 (controller+0xB6C, the action-command time); total = the
  entry's own total (+0x14), raised to the remaining when that is larger (0x1416318A0)".

## 12. Time drain (UI 5.6)

- UI 5.6: "Time drained by `0x1416303E0(part, seconds)` (caller not found)"; open item: "the caller of the
  time-drain function `0x1416303E0`".
- Combat 10.7 (strand F) [C]: bad status `TRES_BADSTAT_TIME_BREAK` (19) → bad-status application 0x1405B7C50 →
  pawn slot 1052 0x141424700 → controller slot 876 0x1413E9D30(t) (`form_time_left -= t`, the action-command
  time likewise) → HUD 0x14158EC10 → 0x1416303E0.
- Sentence: replace "(caller not found)" by "(caller: bad status TIME_BREAK → 0x1405B7C50 → pawn slot 1052
  0x141424700 → controller slot 876 0x1413E9D30 → 0x14158EC10; combat spec 10.7)" and close the open item.

## 13. EXP popup order

- Stats 4.2: `CheckLevelUp` for the player and the party (step 4), then the EXP popup, then the next-level
  counter (step 5). UI 8: "Order within one award: EXP point, level-up popup(s), Next LV."
- Code, `GiveExp` 0x140E9E830 [I, call order in the disassembly]: 0x140E9E8CF `vt+0x1910` (next-level EXP
  *before* the add) → 0x140E9E8E5 `AddExp` 0x140E864B0 → **0x140E9E92C `CheckLevelUp` 0x140EBE400 (player)** →
  0x140E9E960 `CheckLevelUp` per party slot (loop of 6) → **0x140E9E98B EXP point 0x141575840** (only with a
  position and when the earlier next-level EXP was > 0) → 0x140E9E996 `vt+0x1910` again → **0x140E9E9A8 Next LV
  0x141589570**. `CheckLevelUp` calls the level-up HUD function 0x141589330 itself [C], so the level-up popup is
  requested before the EXP point.
- Right: stats. Call order: **level-up popup(s) → EXP point → Next LV**. (All three happen inside one call;
  what the player sees first is decided by the clips, which the UI spec owns.)
- Sentence for the UI owner (section 8): replace "Order within one award: EXP point, level-up popup(s), Next
  LV." by "Call order within one award (`GiveExp` 0x140E9E830): level-up popup(s) (0x140E9E92C / 0x140E9E960),
  EXP point (0x140E9E98B), Next LV (0x140E9E9A8)." The test expectation of UI section 12 item 6 lists the same
  three in the old order; reorder it.

## 14. FORM_TIME

- Stats 5.4 ("Situation commands"): "New command timer (0x14140DA40): formchange x(1 + 0.2 FORM_TIME), …".
- Combat 10.3 [I]: the only reader of ability 0x31 in the controller is 0x14140DB76 inside
  `NotifySetActionCommand` 0x14140DA40: it multiplies the **lifetime of the formchange command in the queue**
  (`Param0` = 30 s → 36 s with one copy, cap 99.9). The **form duration** is `Param3` (30 s) taken by
  `StartStyle` 0x14141A9D0 with no ability term.
- Both specs read the same function; the stats text is right but easy to misread as the form's length. Sentence:
  replace "formchange x(1 + 0.2 FORM_TIME)" by "the formchange command's time in the queue x(1 + 0.2 FORM_TIME)
  (not the duration of the form, which is `Param3` of the command row with no ability term: combat spec 10.3)".
  If the ability catalogue (`sora-abilities`) describes FORM_TIME as "longer form", change it the same way.

## 15. Damage formula

- Combat rev. 1 8.3: difficulty factor only for victims without a status object; "No … difficulty multiplier
  inside the formula"; RECOVER returns 0.1. Stats 7 step 1: "Zero result when power <= 0 or the info says no
  damage (0x14064E770)"; step 8: `dmg = ceil(dmg * DifficultyFactor(attacker))`.
- Listing `build/sora_tmp/stats/dis_140632240.txt` [I], as implemented in `crates/kh-core/src/status/damage.rs`:
  - the call of 0x140642820 at 0x140632865 is reached from the status path and from the no-status path
    (0x140632711 `je 0x140632855` → 0x14063285D): the factor applies **always** (stats step 8 right);
  - 0x1406323F1: `x = max(A, 0)`; 0x140632400 `call 0x14064E770`; when it returns true the code jumps to
    0x140632422 (`bpl = 0`) **without zeroing x**; 0x14063241F zeroes x only for the flagged call (sixth
    argument) of a non-RECOVER hit; 0x140632425 zeroes x when `power <= 0`. So a RECOVER hit with `power > 0`
    returns the **attack stat** `max(A, 0)` (0.1 when that is 0) — both specs were wrong.
- Combat 8.3 is rewritten. Sentence for the stats owner (section 7 step 1): replace "Zero result when power <= 0
  or the info says no damage (0x14064E770)." by "`power <= 0`: result 0 (returned as 0.1). Info says no damage
  (0x14064E770, damage kind RECOVER): the result is the attack stat `max(S, 0)` unchanged (0x140632400–
  0x140632422). With the sixth argument set, any other hit returns 0 (0x14063241F)."

## 16. `ApplyHP` flags

- Stats 6.1 step 2: "`new = 1` when the pawn's `vt+0x2090` is true, or `vt+0x20A0` is true and this is not an
  armor hit ([?] the two tests; candidates are the reflected flags `m_bIsHpLimitOne` and
  `m_bIsHpLimitOneExceptPlayerAttack`)".
- Combat 9.7 [I 0x1405A2A00–0x1405A2CE8]: pawn+0x630 bit 5 = `m_bIsHpLimitOne`, bit 6 =
  `m_bIsHpLimitOneExceptPlayerAttack` (the second applies when `bReact` — the fifth argument — is false). The
  rest of 6.1 (Second Chance at HP ≥ 2, Withstand Combo in DAMAGE or within 0.6 s, the 0.6 s timer on every
  non-forced damage) agrees with combat 9.7.
- Sentence: replace the bracketed "[?] …" by "(pawn+0x630 bit 5 `m_bIsHpLimitOne`; bit 6
  `m_bIsHpLimitOneExceptPlayerAttack`, combat spec 9.7)".

## 17. Hit sound (audio 5.4)

- Audio 5.4 has the precedence right (map-hit SE → per-attribute SE → entry SE → the window's sound). Combat 8.6
  (chooser 0x140628960 [C, arguments I]) adds two conditions and one detail:
  1. **no sound at all when the attacker result is REFLECT (3)** (a repelled first swing);
  2. the window's hit sound is **not used on a map hit** unless the hit component simulates physics — for the
     Kingdom Key a wall hit (`KB_Stick` only) is silent: `m_bChangeSEMapHit` is false and the entry has no SE;
  3. "at the hit point plus a random offset": the offset exists only when `m_fHitEffectRandomOffsetRadius` > 0;
     it is 0 on the keyblade's component (property absent, constructor 0x140616C90 zero).
- Sentence: after "… else the primitive's `m_HitSE` — which is what the attack notify set." add "No sound is
  played when the attacker's result is REFLECT, and the primitive's `m_HitSE` is skipped on a hit on level
  geometry (non-simulating map component): a keyblade wall hit is silent (combat spec 8.6)."; replace "at the
  hit point plus a random offset" by "at the hit point (plus a random offset only for components with
  `m_fHitEffectRandomOffsetRadius` > 0; the keyblade has 0)". Sheet `sora-sound-rules`: route
  `notify.attack_hit_se` needs the two conditions in its `condition` text, no other change.

## 18. Lock-on camera (camera spec section 5, table row 1)

- Camera 5 row 1: "… start `FUN_1405F0990`, end `FUN_1405F0140` …" and the row is a stub ("not stage 1").
- Combat 7.12 (strand D): `FUN_1405F0140(manager, comp)` is both the start (valid component) and the end
  (null); `FUN_1405F0990` is a variant with an explicit camera. The lock-on camera is a second camera actor
  (`TresCameraNormalLoc`) blended in and out over 0.5 s (ease exponent 2); its parameters, per-frame rotation
  and position steps, and the port summary are in 7.12 (constants [I], update structure [C], the framing solver
  0x1405E0590 not read).
- Also camera section 1 table: "`m_RotationSpeedMaxDistance` / `MinDistance` / `MaxRate` … only copied to the
  lock-on camera" is right; add that they scale the **stick** speed with the distance to the target (1.0 within
  500 cm, 0.45 beyond 2500), and that `m_LockonAutoRotationLowDist` / `HighDist` (300 / 3000) are read by the
  shared auto-follow 0x1405FFA60 in its lock-on branch (they scale the automatic turn toward the target).
- Sentence for the camera owner: in row 1 replace "start `FUN_1405F0990`, end `FUN_1405F0140`" by "start and
  end `FUN_1405F0140(manager, component | null)` (`FUN_1405F0990` = the variant with an explicit camera);
  specified in `docs/sora-combat-spec.md` 7.12".

## 19. Input sheet (`sheets/port/sora-input-layout.json`)

- `rt_hold_mode` (ref_conf guess, "what this mode is was not determined"): it is the entry to shooter mode
  (`TresPlayerState_ShooterMode`, state 40) for the keyblade forms 0x80 / 0x83 / 0x8E / 0x8F with a full gauge
  (request interface slot 191 0x141447640 [C]); **never with the Kingdom Key** (combat 7.5 step 6).
- `lock_on`: "locks the candidate" → locks the **current target** (the soft target; the pinned attack target
  when a pin is active), up to 3500 cm; a tap while locked unlocks (toggle) (combat 7.6, 7.7).
- `lock_on_target_change`: steps a distance-sorted list of at most 6 candidates within 2400 cm, by a running
  index (not by stick direction) (combat 7.7).

## 20. The attack press: path and phase

Combat rev. 1 (3.1, 3.5) had the controller issue the press in phase C. The UI spec's "Update order" places
the command menu update in the UI tick after the controller, movement, pawn and state ticks; strand A found the
press there [I]: command menu "MainSel" 0x1416220D0 @0x141622513 → `CanExecuteCommand` (slot 682 0x141401F80) →
`ExecuteCommand` (slot 680 0x14140EAA0) → executor 0x14141B470 @0x14141B5E7 → `TryAttack`. The controller's
direct call 0x1413F257D belongs to link / attraction mode. No sentence of the UI spec changes; combat 3.1 is
rewritten and every normal-path latency is one frame longer (request in U(N), effect in S(N+1)). The port's
frame loop needs a phase after the state tick in which the command menu decides (the same phase issues magic,
items and situation commands: magic spec part J).

---

## Consistent, checked, nothing to change

- Movement spec 6 (collision profile: the player capsule ignores pawns) — combat 6.5 agrees. The movement
  spec's "direct-velocity tail, off in stage 1" is now specified by combat 6.1 (`CalcVelocity` body skipped
  during an attack; only the tail runs).
- Actions spec Reflect (weapon recoil, travel 400) — combat 5.6.3 agrees. Actions II.7 key layout — combat
  3.1.2 uses it.
- VFX spec 2.8 (the Keyblade keeps blueprint, shape and sockets in Second Form; wear form 05 swaps three
  textures) — combat 10.1 / 10.5 agree. VFX question 15 (weapon charge effect level) stays open (combat 13.2).
- Magic spec part J 5.1 / 5.3 / 5.6 (queue entry layout, capacity 6, timers) — combat 11 agrees and cites them.
- UI spec 5.7 (gauge phases 0.25 / 0.5 / 0.75 / 1.0) — combat 10.2 cites it.

## Left for the running game (cross-spec)

- Whether the level-up popup or the EXP point appears first on screen (clip timing; the call order is settled).
- The tick order of the attack-collision manager against the pawn (combat 13.2, hits question 1): decides
  whether a hit window opened in M of frame N sweeps in N or N+1 — the audio and VFX specs' hit timing inherit it.
