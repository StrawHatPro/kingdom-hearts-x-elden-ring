# Sora: weapon and visual effects specification

Scope: the Keyblade (Kingdom Key `w_so010`, base form; pointers for the others) and every visual effect Sora's own
systems can trigger, specified so a host engine can show them 1:1. Research and asset extraction only; no Rust.

Evidence marks: `[I]` read at instruction level (disassembly), `[C]` decompiled C only, `[A]` asset data (decoded
package, export, property), `[U]` stock UE 4.17 on confirmed identity (the identity basis is stated where it is
used), `[?]` not determined. Function verdicts (`node tools/q.js fn <addr>`) are in the address table (section 13);
many effect-runtime functions have verdict `differs` or none, so `[C]` statements on them are readings of the
decompiled C and are only as strong as that.

Where things are:

| what | where |
|---|---|
| sheets | `sheets/port/sora-effects.json` (1,123 rows: trigger → effect), `sora-weapon.json` (128), `sora-vfx-params.json` (33), `sora-vfx-modules.json` (1,503: module census); written by `tools/sora_vfx_sheet.js` from the evidence row files in `audit/sora_port/vfx/` |
| weapon pack | `build/sora_pack/weapon/` (tool `tools/sora_weapon_pack.js`) |
| effect pack | `build/sora_pack/vfx/` (tool `tools/sora_vfx_pack.js` 1.1.0): 516 effects as JSON, 715 materials, 406 textures, 296 meshes, `triggers.json` |
| detail behind this document | `build/sora_tmp/vfx/{weapon,wpack,fxsys,fxinv,cascade}/spec.md` (strand reports: full per-category trigger tables, per-module notes, scripts) |
| follow-up research, ready to launch | `build/sora_tmp/vfx/P2_STRANDS.md` |

Units: cm, seconds, degrees, X forward, Y right, Z up (KH3 / UE). Offsets are hexadecimal from the object start
unless a reflected (decimal) offset is quoted from the sheets.

---------------------------------------------------------------------------------------------------------

## 1. Summary

- The Keyblade is an actor of its own (`ATresWeaponBase`, itself a pawn with its own anim instance), spawned by the
  pawn's equipment component from an equip data asset and snapped to socket `R_wep_skt` (bone `R_buki`, child of the
  right hand). It is never sheathed: "weapon away" is the same attached actor made invisible. `[A]`/`[C]`
- Showing and hiding runs a small state machine on the weapon. Kingdom Key: effect `VW_SO010_APP0` starts at once,
  the mesh appears 0.22 s later with material scalar `OpacityFade` 0 and ramps to 1 in 0.06 s; hiding is the mirror
  image and waits for the effect to finish. `[I]` for the sequence heads, `[A]` for the numbers.
- All effects are UE4 Cascade (`UParticleSystemComponent`) with Square Enix extensions compiled into the engine
  module: 47 extra module classes and 9 extra type-data classes occur in Sora's set. There is no other effect
  middleware. `[C]`/`[A]`
- Effects are almost entirely data. Animation clips carry `TresAnimNotify(State)_AttachEffect` notifies with inline
  placement rows; code selects rows of attach-data assets (`ead_*`) by a numeric group id. One
  `UTresEffectAttachComponent` per actor spawns and tracks them. `[C]`/`[A]`
- An effect row spawns only if its `GroupID` is -1 or equals the effect group the animation was played with; with
  the plain Keyblade that group is 0 except for a handful of plays (section 7.4). An animation change ends every
  tracked effect marked `WhenChangeAnimationType = END`. `[C]`
- The keyblade trail is a stock AnimTrail ribbon between weapon sockets `vfx_top` and `vfx_bottom`, particle
  lifetime 0.2 s, requested by the attack clips through a three-step data chain and faded out over 0.15 s. `[A]`/`[C]`
- Inventory: 1,123 rows; 787 are reached by base-form Sora with the Kingdom Key, 6 under a named group condition,
  173 belong to other keyblades or forms, 145 are undecided, 12 are entries without effect. 349 distinct particle
  systems are reachable; the pack holds 516 (all referenced ones, including the pointer rows).
- Only 87 of the 3,241 emitters use stock modules alone, and no effect is purely stock. A host that implements only
  stock Cascade shows the right textures at the right time and place but wrong motion for most effects; the first
  five Square Enix modules (section 8.3) are what makes them move correctly, and their transform-inheritance part is
  not fully traced yet.
- Not found in code or data: a level-up effect, a low-HP effect, a landing puff, a hit flash or invincibility blink
  on Sora's materials. These need a look at the running game (section 14).

---------------------------------------------------------------------------------------------------------

## 2. The Keyblade

### 2.1 Classes

```
APawn -> ATresPawnBase (1384 bytes) -> ATresWeaponBase (1888) -> ATresWeaponKeybladeBase (1888, no own members)
   -> blueprint w_so_KeybladeBase_C -> blueprint w_so010_Wep_C                                         [A]
UTresEquipmentComponent (912)          pawn member MyEquipment: owns the spawned weapons
UTresEquipKeybladeData (160)           data assets w_soNNN_Equip
UTresWeaponMovementComponent (1040)    : UTresProjectileMovementComponent (thrown weapon)
UTresAtkCollComponent (2432), UTresEffectAttachComponent (1136)
```

`TresWeaponKeybladeBase` shares the vtable of `TresWeaponBase` (0x14610FCD8, 541 slots, 14 more than
`TresPawnBase`) `[I]` (table read; the shared-vtable statement is from `audit/class_vtables.json`). No
`TresPlayerWeapon*` class exists. `TresWeaponSet` (`p_ex001_WepSet`) is referenced by no Sora package `[A]`.

Reflected members of `TresWeaponBase` with native defaults (constructor 0x141777570 `[I]`) and the Kingdom Key's
values (class default object of `w_so010_Wep` / `w_so_KeybladeBase` `[A]`):

| offset | member | native | Kingdom Key |
|---|---|---|---|
| 568 / 570 / 578 / 580 | `MyMovement` / `MyMesh` / `MyAtkColl` / `MyEffectAtt` | subobjects `TresWeaponMovement0`, `TresWeaponMesh0` (profile `TresEquip`), `TresAtkColl0` (on the mesh), `TresEffectAttach0` (on the root) | mesh `w_so010_0`, AnimClass `w_so010_Anim` |
| 5A8 / 5B0 | `m_AppearEffect` / `m_AppearEffectAttach` | null / None | `/Game/Effects/wep/w_so010/par/VW_SO010_APP0` / None |
| 5B8 | `m_AppearVisibleDelayTime` | 0 | 0.22 |
| 5BC | `m_bIsEnableAppearScaleAnim` (bit0), `m_bIsDelayAppearScaleAnim` (bit1) | 0, 0 | 0, 0 |
| 5C0 / 5C4 | `m_AppearScaleAnimTime` / `...StartScale` | 0 / 0.01 | 0 / 0.01 |
| 5C8 | `m_bIsEnableAppearDitherAnim` (bit0), `m_bIsDelayAppearDitherAnim` (bit1) | 0, 0 | 1, 1 |
| 5CC / 5D0 | `m_AppearDitherAnimTime` / `...StartValue` | 0 / 0 | 0.06 / 0 |
| 5D8 / 5E0 | `m_DisappearEffect` / `m_DisappearEffectAttach` | null / None | `VW_SO010_APP0` / None |
| 5E8 | `m_DisappearVisibleDelayTime` | 0 | 0.22 |
| 5EC / 5F0 / 5F4 | `m_bIsEnableDisappearScaleAnim` / `...ScaleAnimTime` / `...EndScale` | 0 / 0 / 0.01 | 0 / 0.2 (inactive: enable bit is 0) / 0.01 |
| 5F8 / 5FC / 600 | `m_bIsEnableDisappearDitherAnim` / `...DitherAnimTime` / `...EndValue` | 0 / 0 / 0 | 1 / 0.06 / 0 |
| 604 / 608 | `m_DisappearWithScaleTime` / `m_DestroyScaleAnimEndScale` | 0.5 / 0.01 | 0.2 / 0.01 |
| 610 / 618 / 620 / 628 | `m_CmnMagicCastEffect` / `m_CmnBadStatesEffect` / `m_CmnChargeEffect` / `m_CmnStyleChangeChargeEffect` (attach-data assets) | - | `ead_wep_cal0` / `ead_com_abn_w0` / unset / set at spawn from the equip data (2.8) |
| 640 / 648 | `m_SoundAssets_Hit` / `_Swing` | - | `enumset_hit_w_so010` / `enumset_swing_w_so010` |
| 678 | `m_bIsNotifyOnlyVisible` | 0 | 0 |
| 758 | `m_pDisappearEffect` (live component) | null | |

Unreflected members read in code `[I]`/`[C]`: +0x674 bit0 attached, **bit1 visible flag**; +0x67C bit0
notify-visible, bit1 force-hide, bit2 apply parent scale; +0x680 / +0x688 attach parent / socket name; +0x66C
form-change charge level (int, -1 none), +0x670 running charge group; +0x6A4 rate of the running appear or
disappear; +0x6A8.. `AntiVolume` fade; +0x6D8 state table (6 states of 0x48 bytes), +0x6F0 current state id;
+0x730..+0x738 scale animation (current, target, time left); +0x73C..+0x744 dither animation; +0x748 visible-delay
timer; +0x74C bit0 scale running, bit1 dither running, bit2 delay pending, bit4 finish request.

Weapon virtuals that matter (vtable 0x14610FCD8) `[C]` unless marked: slot 134 Tick 0x14178FBD0; slot 141
PostInitializeComponents 0x141788E30 (builds the state table 0x141783270); slot 528 **`SetVisible(bool visible,
bool withEffect, float rate)`** 0x14178B320 `[I]`; slot 529 finish-now 0x14177C030; slot 530 `SetForceHide`
0x14178ADA0; slots 531 / 532 start Destroy states; slot 533 hit-effect helper 0x14178BFE0; slots 536..538 throw /
end throw; slot 540 `ShouldMeshBeVisible` 0x1417910E0 = visible flag && (!notifyOnly || notifyVisible) && !forceHide.

### 2.2 Selection and spawn

Data chain `[A]`:
1. Item id = `ETresItemDefWeapon` value (Kingdom Key `WEP_KEYBLADE00` = 1). Sora's initial equipment:
   `Load/Common/TresChrInitData` `m_PlayerSora.m_Weapons` = [`WEP_KEYBLADE00`].
2. Row `I03001` of `Load/Tres/TresEquipItemData`, column `m_EquipSubclass` →
   `/Game/Blueprints/Player/p_ex001/w_so010_Equip` (`TresEquipKeybladeData`).
3. `m_EquipMap[ETresWeaponForm][growLevel]` → `m_Equips[]` units {UnitName, m_AttachPart, AttachBoneName,
   m_bApplyParentScale, RsrcData = weapon blueprint class}, plus `m_TransformSetData[]` (units while the keyblade is
   transformed), wear form, replaced anim / magic / projectile sets.
   Kingdom Key: form `TRES_WEAPON_FORM_01` only; unit `RWeapon`, `RIGHT_HAND`, `R_wep_skt`, `w_so010_Wep_C`;
   `m_ReplaceProjectileSet` = `w_so010_ProjSet`; `m_ReplaceAnimSets` empty (so only `p_ex001_AnimSet` plays).

Code (`UTresEquipmentComponent`, vtable 0x146B8C668) `[C]`: slot 155 0x14177C4A0 change weapon (3 equip slots,
current index comp+0x378) → slot 190 0x141791120 `Equip` (reads the set from the asset, sets the wear form) →
slot 195 0x14178E5D0 **`SpawnUnits`**: per unit, reuse a matching weapon or `SpawnActor`; weapon+0x628 = charge
effect asset, +0x66C = charge level; attach (2.3); `SetVisible` per the component's weapon-out flag; start the
pawn's current animation **name** at the pawn's current time on the weapon. The weapon actor exists as long as the
keyblade is the current equip slot; "weapon out" only toggles visibility. Which pawn function equips the initial
keyblade was not traced `[?]`.

### 2.3 Attachment

- Socket **`R_wep_skt`** `[A]` (`w_so010_Equip`): a `SkeletalMeshSocket` of `p_ex001_0_Skeleton` on bone `R_buki`
  (parent `R_te`, the right hand); relative location 0, rotation roll -90° (UE quaternion (0.7071, 0, 0, 0.7071)).
  `L_wep_skt` on `L_buki` is the mirror (used by left-hand units of other keyblades).
- Attach 0x14177E4E0 `[C]`: `AActor::AttachToComponent(pawn skin mesh, rules, socket)`; the rules constant at
  0x148750ED8 is bytes `02 02 02 00` `[I]` = location, rotation and scale SnapToTarget, no weld. The weapon root is
  exactly on the socket. `TresWeaponMesh0` serializes no relative transform `[A]` and the constructor writes none
  `[C]`: identity. Result: bone `wep` relative to bone `R_buki` = rotation (0.7071, 0, 0, -0.7071) UE, zero
  translation.
- Cross-check (pack strand, asset data only): with this attach the grip origin is 9.63 cm from `R_te` and 4.33 cm
  from the finger roots in the reference pose; over the attack clips 1301..1320 the tip moves in the plane of the
  teeth (|cos| to the flat normal 0.16); renders of the battle idle show the hand closed on the grip
  (`build/sora_tmp/vfx/wpack/r_idleB_*.png`). The code reading and the asset statistics agree.
- No sheathed state and no socket switch for the Kingdom Key was found in the weapon or equipment classes `[?]`
  (treat as always right hand; probe row in section 15).

### 2.4 Appear and disappear

Who decides "weapon out" `[C]`:
- Battle flag pawn+0x630 bit1 (getter pawn vt+0x1740 0x1405AD050, setter slot 743 0x141446CE0).
- Weapon out = equipment comp+0x2C8 bit1 (pawn vt+0x1330 0x1405ADE80).
- Show: pawn vt+0x1340 (slot 616) `(visible, effect, rate)` 0x141448060 → 0x1405BA440 → equipment slot 149 →
  every weapon's `SetVisible`; reloads the pawn timer +0x10E0 = 10.0.
- Hide: pawn vt+0x1358 (slot 619) 0x1414480B0 `(effect, rate)`: only if the weapon is out and the weapon kind is
  not 1, 0x16 or 0x17.
- Idle state 0x1413944D0: battle flag on and not in battle pose → `vt+0x1340(1, 1, 1.0)` and play `BattleStart`
  (clip 1014). Battle flag off while in battle pose: a 3.0 s timer (state+0x8C) runs down, then
  `vt+0x1358(1, 1.0)` and `BattleEnd` (clip 1028). Neither clip carries a `SetWeaponVisible` notify: the state code
  toggles the weapon.
- Run state 0x1413AED70: hides the weapon with effect when not in battle, weapon out and pawn timer +0x10E0 at 0
  (10 s, reloaded by every show / hide and by the battle-flag setter).
- Many other states call `vt+0x1340` directly (grep `+ 0x1340))` in `re/decomp`); not mapped `[?]`.

Notify: `TresAnimNotify_SetWeaponVisible::Notify` 0x140561AF0 `[C]` → pawn `vt+0x1340(m_bNewVisible,
m_bAppearEffect, 1.0)`; both properties default **true** (constructor 0x14055ACB0 `[I]`). In the clips of the main
anim set only `M_1501` (clip 1501, t = 0.0999 s, visible, effect) carries it; eight other clips outside the set do
(list in `build/sora_tmp/vfx/weapon/spec.md` 4.2).

State machine (states built by 0x141783270: 0 none, 1 Appear, 2 Disappear, 3 Destroy, 4 DestroyNext, 5 FreeMove):

```
SetVisible(visible, withEffect, rate)                              // 0x14178B320 [I]
  if visible == visibleFlag: return
  clear finish bit; rateField = 1.0; visibleFlag = visible; delayTimer = 0
  if visible && withEffect:
     if m_AppearEffect && MyMesh:
        psc = SpawnEmitterAttached(m_AppearEffect, MyMesh, m_AppearEffectAttach, (0,0,0), zero rotation,
                                   keep relative offset, autoDestroy)     // 0x144118120 [C]
        psc.tag = "WeaponAppear"
     if rate > 0 && ((scaleEnable ? m_AppearScaleAnimTime : m_AppearVisibleDelayTime) > 0
                     || (ditherEnable && m_AppearDitherAnimTime > 0)):
        rateField = rate; enter Appear; return
  else if !visible && withEffect:
     m_pDisappearEffect = same spawn with m_DisappearEffect / m_DisappearEffectAttach
     if rate > 0 && (delay or scale time > 0 || dither time > 0 || m_pDisappearEffect):
        rateField = rate; enter Disappear; return
  // immediate path
  state = none; MyMesh.SetVisibility(ShouldMeshBeVisible()); mesh scale = 1; OpacityFade = 1

Appear.begin                                                       // 0x14177D600 [I]
  flags &= ~7
  if !scaleEnable && m_AppearVisibleDelayTime > 0 && rate > 0: delayTimer = it; flags |= 4
  else MyMesh.SetVisibility(ShouldMeshBeVisible())
  if scaleEnable && m_AppearScaleAnimTime > 0 && rate > 0: flags |= 1; scale from StartScale to 1 over the time
  else mesh scale = 1
  if ditherEnable && m_AppearDitherAnimTime > 0 && rate > 0: flags |= 2; dither from StartValue to 1 over the time
  else OpacityFade = 1
Appear.tick (dt = weapon+0x510)                                    // 0x141784D50 [I]
  if flags&4: delayTimer -= dt   (not multiplied by rate); at <= 0: SetVisibility(ShouldMeshBeVisible()); flags &= ~4
  if flags&1: unless (delayScale bit && flags&4): Approach(dt*rate, ...); mesh scale = max(scaleCur, 0.01)
  if flags&2: unless (delayDither bit && flags&4): Approach(dt*rate, ...)
              OpacityFade = clamp01(ditherCur)        // written every tick, also while held
  if (flags&7)==0 || finish bit: state = none
Appear.end: visibility, scale 1, OpacityFade 1; start the charge effect for the level (2.8)   // 0x14177E720 [C]

Disappear.begin                                                    // 0x14177D960 [I] head, [C] rest
  flags &= ~7
  if !scaleEnable: if m_DisappearVisibleDelayTime > 0 && rate > 0: delayTimer = it; flags |= 4
  else if m_DisappearScaleAnimTime > 0 && rate > 0: flags |= 1; scale 1 -> EndScale
  if ditherEnable && m_DisappearDitherAnimTime > 0 && rate > 0: flags |= 2; dither 1 -> EndValue
  stop the charge effect
Disappear.tick                                                     // 0x141784FF0 [I] head, [C] rest
  if flags&4: delayTimer -= dt*rate; at <= 0: flags &= ~4
  scale and dither approach only after the delay; OpacityFade = ditherCur
  if (flags&7)==0: if m_pDisappearEffect == null: SetVisibility(ShouldMeshBeVisible()) else OpacityFade = 0
  done when ((flags&7)==0 && the disappear effect component has finished) || finish bit
Disappear.end: SetVisibility(...); scale 1; OpacityFade 1; m_pDisappearEffect = null          // 0x14177E830 [C]

Approach(step, target, &cur, &left)                                // 0x143F0AA50 [C], verdict same
  if cur == target: return;  if left == 0: cur = target; return
  cur += (target - cur) / left * step, clamped at target;  left -= step      // linear
```

Kingdom Key timeline at rate 1:

| t | appear (with effect) | disappear (with effect) |
|---|---|---|
| 0 | `VW_SO010_APP0` spawned on the weapon mesh origin, mesh still hidden | `VW_SO010_APP0` spawned, mesh fully visible |
| 0.22 s | mesh made visible with `OpacityFade` 0 | `OpacityFade` starts falling from 1 |
| 0.22 → 0.28 s | `OpacityFade` 0 → 1 linearly (dt-stepped) | `OpacityFade` 1 → 0 linearly |
| after | state ends | `OpacityFade` stays 0 until the effect component has finished, then the mesh is hidden and `OpacityFade` reset to 1 |

Without effect (`withEffect` false): instant visibility change, no dither. The appear delay is not scaled by
`rate`, the disappear delay is `[I]`.

Material side: `OpacityFade` is a scalar parameter set on all materials of the weapon mesh (0x141792140 → mesh
vt+0xC40, clamped 0..1) `[I]`; the dither pattern is in the shader of `w_allBaseMat` (root of
`w_so010_mat0_Inst`, `BLEND_Masked`), not examined `[?]`. A second scalar `AntiVolume` follows the pawn's own
fade (0x14178A380), not the appear sequence `[C]`. `m_DisappearWithScaleTime` (0.2) is the scale-out
1 → 0.01 of state Destroy, when the weapon actor is thrown away on a weapon or form change `[C]`.

### 2.5 The weapon's own animation

- `w_so010_Anim` `[A]`: AnimBlueprint on native `TresAnimInstance`; graph Slot `FullBody` → cached pose → root;
  default animation `ResetPose`; no physics node in the graph.
- `w_so010_AnimSet` `[A]`: `ResetPose` = `anm/rt/0000` (2 frames, the reference pose), `BackRaid` =
  `anm/rt/1297` (43 frames, 1.4 s), `SoKcIrradiate` = `anm/rt/1814` (61 frames, 2.0 s, with Bonamik speed-rate
  and group-parameter notify states over its whole length). `rt_holo_dlc` holds a second AnimBlueprint and a
  one-row set for the photo-hologram weapon.
- The names equal Sora's animation names: the spawn code and the end of a throw start the pawn's current animation
  name on the weapon at the pawn's time (0x14178E5D0, 0x14177E920 `[C]`); a name missing from the weapon's set does
  not play. How the weapon follows each later pawn animation was not traced `[?]`. Loop flags of the weapon clips
  are decided by code and are unchecked cells in `sora-assets`.
- Skeleton `[A]` (9 bones): `kh_sk` > `wep` (the blade; every socket) > `chain1` > ... > `chain7` (keychain).
  Sockets on `wep`, cm along +X from grip to tip: `ATK_00` 0, `ATK_01` 79.04, **`vfx_top` 79.04**,
  **`vfx_bottom` 14.15**, `vfx_call` (68.54, 0, -6), `Z_00` 79.04, `Z_01` 14.15, `Z_Ca` (68.54, 0, -6).
  The Kingdom Key has no transform parts (`m_TransformSetData` empty).

### 2.6 Attack collision and hit effects

- Shapes are data on the component, not physics-asset bodies: `TresAtkColl0.CollisionShapesSrc` in
  `w_so_KeybladeBase` `[A]`, 19 rows keyed by group name (`sora-weapon` rows `kingdom_key.atk_shape.*`; also sheet
  `sora-combat-collisions`). Blade: `KB_Stick` / `KB_StickM` / `KB_StickL` = capsule between sockets `ATK_00` and
  `ATK_01`, size 50 / 80 / 120; `KB_SphereSS` / `KB_SphereS` = sphere at `ATK_00`, 250 / 350. Attack clips switch
  groups on by name with `TresAnimNotifyState_Attack`. `w_so010_Physics` (5 bodies on chain bones, collision
  response off) is not the attack collision.
- Hit-effect choice, 0x140628960 `[C]` (verdict same-partial):

```
grp  = first m_HitEffects[i] with m_GrpName == group
unit = (index in range of grp.m_HitEffSet) ? grp.m_HitEffSet[index] : grp
loc  = hit.Location (+ random direction * m_fHitEffectRandomOffsetRadius when > 0); rot = Rotation(hit.Normal)
effect = (mapHit && unit.m_bChangeMapHit) ? unit.m_MapHitEffect
       : (attr < 8 && unit.m_PawnHitEffectPerAttr[attr]) ? that : unit.m_PawnHitEffect
spawn effect at (loc, rot) in the world (0x14411B3D0); optionally attach it to the hit component keeping world
if directHit && !mapHit && unit.m_DirectHitEffect: spawn it the same way
if grp && mapHit && unit.m_bEnableDecal && m_DecalData.m_DecalMaterial: spawn a decal
```

- Kingdom Key table `[A]` (group `None`): attr 0 `vw_so010_hit0` (also the default), 1 `vs_fir_hit0`,
  2 `vs_blz_hit0`, 3 `vs_thd_hit0`, 4 `vs_wat_hit0`, 5 `vs_win_hit0`, 6 `vs_dak_hit1`, 7 `VS_non_HIT0`; direct hit
  `vw_so010_str0`; map hit `VW_SO010_WHT0`; no decal. Group `RaidRide`: direct + map only. Groups
  `MagneSplash1/2`: `vw_lmt_mgs_hit00/01`.
- The index is `ETresDamageAttribute` (0 PHYSICAL, 1 FIRE, 2 BLIZZARD, 3 THUNDER, 4 WATER, 5 AERO, 6 DARK,
  7 NOTYPE, 8 MAX): the enum has exactly these eight values in reflection and the effect names match one for one;
  the caller that passes the byte was not read, so the mapping stays `inferred`.

### 2.7 Keychain physics

`w_so010_Bonamik` `[A]` (`SQEX_BonamikAsset`, version 2401; `build/sora_pack/weapon/data/bonamik.json`): 3 groups,
10 bodies (three kinematic collider spheres on `wep`; `chain1` kinematic sphere r 1; `chain2..7` dynamic capsules
r 1.8, 1.8, 1.8, 1.8, 3, 3, mass 0.8, damping 1.2, wind drag 0.15, lift 0.45), 8 links (6 parent-child with cones:
75° for chain1-2, 70° for the rest). Same asset class, version and binding as Sora's own Bonamik asset, so the
runtime of `docs/sora-animation-spec.md` applies unchanged; the weapon's KineDriver user data serializes nothing
(no weapon driver). Not determined: whether the simulation is reset when the weapon appears.

Added by the keychain implementation (2026-10-11; `build/sora_tmp/IMPL_KEYCHAIN_STATE.md`):

- Teleport distance `[I]`: `TeleportDistance` is serialized neither in the mesh user data (+0x98) nor in the asset
  (`m_TeleportDistance` +0x560); the constructors 0x143AE89D0 and 0x143AE88A0 write neither field, so instance Setup
  0x143b61cf0 passes 0 and core setup stores `teleportDistSq` = 40000: **200 cm** per evaluation (Sora: 800).
- Runtime records after setup: `data/bonamik2.json` (10 particles, 8 links, 6 hard-length cone segments, 1 chain root,
  6 dynamic capsule objects `chain2-3 .. chain6-7` in two batches, collider list of group 2 = the three spheres and the two
  guard capsules the Lateral repulsor links make). The reference evaluator runs the asset without reaching any
  untranscribed branch; code it runs that Sora's validation never did: sphere-sphere collision 0x143b1c700 (bit-checked
  against the exe's instructions by emulation, 20,000 cases) and the degenerate shortest-arc returns of a straight chain.
- What moves the chain: the weapon's anim instance holds `ResetPose`, so every motion enters through the weapon mesh's
  ComponentToWorld at each 30 Hz evaluation (LOCAL_FORCE: nodes carried with the component, `-lfCoeff x` world travel
  re-applied as lag, clamped by `maxLF`), plus world gravity (0, 0, -981).
- Notify routing `[C]`: `Bonamik_CharaWind` begin / end (0x140562220, 0x140564CB0) and `Bonamik_SpeedRate` call the KBD
  component with the children flag set (0x143b5ef60 / 0x143b5dd90 / 0x143b600b0 -> 0x143b53410 walks the attach children for
  KBD components): a wind notify of a clip played on Sora also reaches the weapon's keychain while it is attached.
  `ChangeGroupParams` (0x143b5f410) has no such flag; `ScaleGroupParams` has `m_ApplyToAttachments`. Thirteen of Sora's
  clips (`0851S .. 0987S`) name group `KEY` in a `CharaWind`; the fall loop `0033` names no group (= all groups).
- Behaviour in the evaluator: hangs straight down when the blade points up; with the blade horizontal the 75 / 70 degree
  cones let it hang down from the second joint (settles in 80 evaluations); when a fast swing stops, the chain whips
  over and is pushed by the guard colliders after the length pass, so segments stretch for single evaluations (up to
  6.7 cm in the 180 degrees / 0.4 s test) and it rests on the guard with 0.34 cm of contact stretch.
- Port: `kh_anim::Keychain` (`crates/kh-anim/src/keychain.rs`), a second instance of the unchanged solver; the app steps it
  after Sora's pose with ComponentToWorld = `R_buki` x socket x Sora's component transform, freezes it while the weapon is
  hidden and soft-resets it when the weapon appears (port decision), mirrors Sora's all-group character wind; `--no-keychain`.

### 2.8 Formchange

- `w_so010_Equip` `[A]`: one weapon form with two grow levels: LVL01 / wear form NORMAL and LVL02 / wear form
  `TRES_WEAR_FORM_05`. Both spawn `w_so010_Wep_C` as `RWeapon` at `R_wep_skt`, with no transform set and no replaced
  anim, magic or mesh set. **In Second Form the Keyblade keeps blueprint, shape and socket**; the wear form swaps
  three textures on Sora's material slots 8..10. That LVL02 / wear form 05 is Second Form rests on it being the
  Kingdom Key's only non-normal entry and on its ability list, not on a label in the data.
- Charge effect: `m_CmnFormChangeChargeEffect` = `ead_wep_chrg0` is stored on the weapon at spawn with a level.
  0x141792220 `[C]`: level < 0 none; level 0 → group 3000 (`vs_com_lv1`, socket `wep`); level > 0 → group 3001
  (`vs_com_lv2`) plus, with its burst argument, group 3002 (`vs_com_kyp01`, socket `vfx_top`). Started at the end
  of Appear, stopped at the start of Disappear / Destroy. Who raises the level was not traced `[?]`.

### 2.9 Throw

`TresAnimNotify_ThrowWeapon::Notify` 0x140561C20 `[C]` → current state → equipment slots 169..171 → weapon
slots 536 / 537: movement speed and velocity set, optional weapon animation, state FreeMove (root switched to
absolute transform, movement component active; end: back on the socket with zero relative transform, animation
re-synced). Kingdom Key movement values `[A]`: homing on, start delay 0.1 s, turn max 360, speed 6000. In the main
anim set only `BackRaid` (clip 1297, notify at 0.4667 s; the weapon plays its own `BackRaid`).

### 2.10 Inventory of keyblades (pointers)

Blueprint `/Game/Blueprints/Player/p_ex001/w_soNNN_Wep`, mesh `/Game/Character/wep/w_soNNN/mdl/w_soNNN_0`; one row
each in `sora-weapon` (`keyblade.w_soNNN`, value = name, blueprint, mesh, formchange variants). Names are the
English locres strings.

| id | item | name | formchange blueprints (weapon form) |
|---|---|---|---|
| w_so010 | 1 | Kingdom Key | none (form 01 at grow level 2 = Second Form) |
| w_so020 | 3 | Shooting Star | 022 + 023 (02), 024 (05); transform unit 021 |
| w_so030 | 2 | Hero's Origin | 032 / 033 / 034 by grow level (0B), extra 037; transform 031 |
| w_so040 | 8 | Hunny Spout | 042 + 043 (10), 044 (11); transform 041 |
| w_so050 | 4 | Favorite Deputy | 052 (03), 053 (06); transform 051 |
| w_so060 | 6 | Happy Gear | 062 + 063 (0C), 064 + 065 (04); transform 061 |
| w_so070 | 5 | Ever After | 072 (0A); transform 071 |
| w_so080 | 7 | Crystal Snow | 082 + 083 (12), 084..087 (07); transform 081 |
| w_so090 | 10 | Wheel of Fate | 092 (0E), 093 (0F); transform 091 |
| w_so110 | 12 | Classic Tone | 112 (14), 113 (15); transform 111 |
| w_so121 | 19 | Dead of Night | none (form 01 level 2) |
| w_so140 | 11 | Grand Chef | 142 / 143 / 144 by grow level (13); transform 141 |
| w_so150 | 18 | Starlight | none (form 01 level 2) |
| w_so170 | 9 | Nano Gear | 172 (08) |
| w_so190 | 15 | Ultima Weapon | 192 (09); transform 191 |
| w_so200 | 13 | Oathkeeper | form 16; form 18 = 200 right + 210 left (`L_wep_skt`) |
| w_so210 | 14 | Oblivion | form 17; form 18 as above |
| w_so100, 120, 160, 180 | - | no item row | 160 / 180 spawn `w_so010_Wep_C` |

---------------------------------------------------------------------------------------------------------

## 3. Weapon pack (`build/sora_pack/weapon/`, 1.8 MB)

Built by `node tools/sora_weapon_pack.js` (deterministic; `--check`). Same conventions as `sora.glb`
(`manifest.conventions`, referenced from the weapon manifest).

| file | content |
|---|---|
| `kingdom_key.glb` | LOD 0 of `w_so010_0`: 3,523 vertices, 5,048 triangles, one material slot (`w_so010_mat0_Inst` → `w_allBaseMat_UseReal` → `w_allBaseMat`, masked), 9 bones, 7 skin joints |
| `textures/w_so010_0d00.png`, `_0d00_alpha.png`, `_0m00.png`, `_0n00.png` | colour, its alpha, mask, normal |
| `anims/0000.glb`, `1297.glb`, `1814.glb`, `ResetPose.glb` | the three rows of `w_so010_AnimSet` and the hologram set's `ResetPose`, with notifies |
| `data/bonamik.json`, `data/bonamik2.json`, `data/physics_asset.json` | keychain simulation asset, the same plus the runtime records after setup, physics asset |
| `data/weapon_cdo.json` | appear / disappear parameters, hit-effect table, attack shapes, common effect list, sound sets |
| `manifest.json`, `validation.json`, `sheet_rows.json` | description (skeleton with reference pose, sockets, attach block with the evidence above, sources with sha256); validation; the 13 rows merged into `sheets/port/sora-assets.json` |

Validation: every glb passes `gltf-validator` with 0 errors and the independent read-back; PNGs read back; the
colour texture was viewed and is the Kingdom Key; `validation.json` -> `bonamik` holds the keychain checks of
`tools/lib/bonamik/weapon.js` (hang, segment lengths, swing). Not in the pack: the opacity-mask input of the material (the glb material is
opaque).

---------------------------------------------------------------------------------------------------------

## 4. Effect system

### 4.1 Request paths

**(a) Animation notifies.** `UAnimNotify::Notify` is vtable slot 72; notify states: slot 72 is a Square Enix
pre-begin hook, 73 `NotifyBegin`, 74 `NotifyTick`, 75 `NotifyEnd` `[I]`. Census of Sora's body clips (all 516
clips with effect notifies, in and out of the main anim set) `[A]`:

| notify class | instances | clips |
|---|---|---|
| `TresAnimNotifyState_AttachEffect` | 848 | 391 |
| `TresAnimNotify_AttachEffect` | 580 | 291 |
| `TresAnimNotify_SetWeaponVisible` / `_ThrowWeapon` | 7 / 4 | 7 / 4 |
| `TresAnimNotify_SetMeshVisible` / `_SetActorVisible` | 1 / 1 | 1 / 1 |
| blueprint `AnimNotify_TresCameraShake_C` / `AnimNotify_TresPlayEffect_C` (no template set) | 1 / 1 | 0962 / 5401 |
| EffectCurve, LensEffect, CameraPosEffect, Alpha, ChangeMeshMaterialParam, ChangeMeshVisible, FootStep, SnowTail, stock PlayParticleEffect / TimedParticleEffect / Trail | 0 | - |

So for Sora one mechanism matters: the two `AttachEffect` classes. Both hold two lists `[A]`:
`MetaAttachDataList[]` {AttachType, SocketName, MetaAttachData[] = inline `SQEX_StructParticleAttachData`} and
`AttachDataList[]` {bUseCommonAttachData, GroupID, AttachType, SocketName, AttachDatas[] = `ead_*` assets}.
`SQEX_StructParticleAttachData` is one placement (sockets, offsets, flags, end type, fades, delays, colour) with
`EffectDataList[]` of {GroupID, AttachEffect, CustomAttachName, ...}. Field layouts with meanings:
`audit/sora_port/vfx/attach_structs.json`; all notify classes: `notify_classes.json`.

- `TresAnimNotify_AttachEffect::Notify` 0x14176C560 and `TresAnimNotifyState_AttachEffect::NotifyBegin`
  0x14176C870 `[C]`: find the owner's `UTresEffectAttachComponent`s (create one if none), then start
  0x141771790(component, mesh, animation, notify). `NotifyEnd` 0x14176CB90: end every tracked entry whose source
  object is this notify (0x141773980). No tick.
- Start is skipped when the component has *spawn stop* set (component+0x46C bit1) or the owner has bit0 of
  owner+0x168 set (meaning `[?]`).
- Attach target by `AttachType` (`ESQEX_ATTACH_OBJECT_NAME`) `[C]`: 0 `OWNER` = the notify's mesh; 1
  `ATTACH_TO_OBJECT` / 2 `ATTACH_TO_WEAPON` = a mesh component attached to the mesh at socket `SocketName` (type 2:
  its owner must be a weapon), fallback the notify's mesh; 3..9 (right / left / both hands, legs, all weapons) = the
  weapon mesh list from the equipment component, one spawn per weapon mesh.

**(b) Attach-data assets (`ead_*`).** 206 packages under `Effects/**/ead/`; 204 are
`SQEX_ParticleAttachDataAsset` with the same row struct as the notifies (1,247 rows) `[A]`. Code plays them
through the pawn's effect interface (pawn+0x440) `[C]`:

| interface slot | function | meaning |
|---|---|---|
| +0x1A0 | 0x1405B9370 | `SpawnAttachEffect(asset, group, useDesignated)`: spawn the rows whose `GroupID` matches |
| +0x1A8 / +0x1B0 | 0x1405A85F0 / 0x1405A8B80 | `EndEffect(group, bEqual)` / with fade |
| +0x1B8 | 0x1405A6360 | `DestroyEffects(group, bEqual)` |
| +0x1C0 | 0x1405B9F80 | `SetSpawnStop(bool)` |
| +0x1C8 / +0x1D0 | 0x1405B8C30 / 0x1405A84B0 | set / end a common slot: 0 aura (`m_CmnAuraEffect`), 1 magic cast (`m_CmnMagicCastEffect`), 2 bad state, 3 appear, 4 bad state by status kind; forwarded to the weapons |

Sora's assets `[A]`: `m_CmnAuraEffect` = `ead_pc_aura_com0` (13 rows: athletic-flow aura 3200, rail slide 3201 /
3202, warps 11..18, critical / just-guard aura 3405); `m_CmnBadStatesEffect` = `ead_com_abn0` (37 rows: stun
3010 / 3160, cure 3090, freeze 3100..3104, item use 3120..3155, Cure magic 1420..1425, burn 3080, ...); weapon
`m_CmnMagicCastEffect` = `ead_wep_cal0` (group = the spell's `m_EffectGroup`: 1301 Fire ... 1424 Curaga, socket
`vfx_call`); weapon charge `ead_wep_chrg0` (3000..3002); game-wide death `ead_com_sys` (group 1 blur
`vs_com_dth_bra00` in the world at `kh_root`, group 2 heart `vs_com_dth_hrt00` at `vfx_lit_mune`, start delay 0.1 s).

**(c) Load assets and pawn properties** `[A]`. `TresPlayerPawnLoad_ex001` (`/Game/Load/Player/`) holds 48 state
event cameras, 15 cameras, 3 camera anims and only three effects (game-over heart `vs_so_dth_ui00_2`, game-over
light `vs_so_dth_ui00`, mission-failed light `vs_so_misn_ui00`; consumer `UTresPlayerState_GameOver` 0x141390B50).
`TresGameModeLoad` holds the game-wide ones: guard reflect `vs_com_haj00`, stagger reflect `vs_com_yor00`,
invincible hit `vs_com_mtk0`, armor hit, the death attach data, and the physical-material table. Pawn properties:
`m_pHopEffect` `vs_crs_icn0`, `m_pAthleticHitEffect` `vp_ffa_arv0`, `m_LensEffectClass` `BP_slc_camatch00_C`,
`m_pFocusAspirEffect` `vp_fca_eng00`, `m_pSwanDiveEffect` `vp_div_sbk1`, `m_pGuardReflectEffect` `vs_ddg_haj00`,
`m_pJustGuardEffect` `vs_com_haj01`, `m_pCriticalCounterHitEffect` `vs_com_haj_hit00`,
`m_pCoverShotDestroyEffect` `vs_bom_hit01`; `m_pFireEffect`, `m_pPoleEffect(H)`, `m_pWallRunEffect`,
`m_pSprintEffect` unset. No hard-coded effect path is used by player code (exe strings checked).

**(d) Footsteps and water.** Sora's clips carry no FootStep notify and the FootStep notify slots are empty stubs
on the player pawn `[I]`. Ground and water effects come from `ATresCharPawnBase` 0x1405B05D0 `[C]`, fed by the
mesh's collision foot probes (`m_CheckFootSteps`: capsules on `L_ashi1` / `R_ashi1`, size (8, 8, 5), offset
(4, 8, 0), yaw 90, touched distance 1.8, effect scale 1 `[A]`):
- unit = live table[`UPhysicalMaterial::SurfaceType`]; the live table is `ATresGameState.m_PhysMatEffects`, built
  (0x14129E010) as a copy of `TresGameModeLoad.m_CmnPhysMatEffect` overridden per unit by the area load asset and
  the world settings.
- ground (surface < 32): on foot touch, or every 1/15 s while in contact; template = `m_StillEffect` when the pawn
  is still, else `m_HighEffect` when speed ≥ `m_HighSpeedParam`, else `m_NmlEffect` when speed ≥ `m_NmlSpeedParam`;
  spawned in the world at the contact point, scale = the probe's `m_EffectScale`.
- water (surface ≥ 32): on touch / enter, or every 0.1 s; rotation from the surface normal; same choice.
- Data `[A]`: the common table `TresPhysEffectsEX` defines **only surface 33** (knee-deep water:
  `vl_dw02_hmn0/1/2`). Ground dust exists only in per-world tables (e.g. `TresPhysEffectsHE`). **In a level without
  a world table Sora's steps make no ground effect.** All 17 tables: `audit/sora_port/vfx/physmat_effects.json`.

**(e) Direct spawns from state code** `[C]` (lower bound: calls through the pawn interface are not in the call
index): `UTresPlayerState_Die` 0x1413733D0 (death groups 1 and 2); `_GameOver` 0x141390B50; `_AirSlide`
0x1413347F0 (groups 100 and, conditionally, 13; asset not resolved `[?]`); `_Run` 0x1413AED70 (world effect at
`kh_root`, not traced `[?]`); pawn 0x1414471A0 (critical counter hit), 0x141446EC0 (death-sentence counter),
0x141447290 (group 0xD4D toggle); 0x141415C30 (lens effect); style-change / shoot-flow states; bad-state handlers
0x1405B77B0 (frozen: `FadeOutEffect(0.1, -1, true)` `[I]`), 0x1405A8850, 0x1405AFF90; hit effects 0x140628960.
Guard, just-guard, hop, athletic-hit, focus-aspir and swan-dive users were not traced individually.

### 4.2 The attach component and effect groups

`UTresEffectAttachComponent` (size 1136, vtable 0x146B70DB0; pawn+0x5A8, weapon+0x580): +0x418
`CommonEffectList` {CommonKey, GroupID, CommonEffect}; +0x428 `DesignatedAttachDataList` {Key, asset}; +0x448
active entries (stride 0x68); +0x458 current animation groups (`TArray<int32>`); +0x468 current group; +0x46C
flags (bit1 spawn stop) `[C]`.

| script function | native | behaviour `[C]` |
|---|---|---|
| `Set(mesh, group, bContinue)` | 0x141770D30 | group list := [group]; unless `bContinue`, every live entry with `WhenChangeAnimationType == END` is ended |
| `SpawnAttachEffect(mesh, asset, group, useDesignated)` | 0x141773240 | spawn the asset's entries whose `GroupID` matches (or either is -1, or request -2) |
| `EndEffect(group, bEqual)` | 0x141765500 | end matching (or, with `bEqual`, non-matching) entries by their end type; -1 / -2 match all |
| `FadeOutEffect(time, group, bEqual)` | 0x141765940 | component `FadeOut(time)` and drop the entry |
| `DestroyEffects(group, bEqual)` | 0x141764EB0 | destroy now |
| `ResetEffects`, `SetEffectVisibility`, `IsComplete`, `Set*Parameter`, `GetDesignatedAttachData`, `SetSpawnStop` / `IsSpawnStop` | see section 13 | by group |

**The group rule.**
1. The pawn's play API (0x1405A0D70 play asset, 0x1405A0E50 play by name; signature
   `play(name, slot, blendIn, mode, effectGroup, playRate, rootScale)`) calls `Set(mesh, effectGroup, false)` when
   the play succeeded and `effectGroup > -99`. That ends the END-type effects of the previous animation and makes
   `effectGroup` the only current group.
2. Inline notify rows (`MetaAttachDataList`) request group -2 = "every current group": an entry spawns when its
   `GroupID` equals a current group or is -1. `AttachDataList` rows carry their own request group (-2 in 122 of
   Sora's rows).
3. Duplicate check: an entry with `PlayNumber != 0` is skipped once the same source has played it that many times.
4. The mesh can override the requested group (mesh vt+0xA28 returns mesh+0x864; -2 = no override; default and
   writer `[?]`, probe row in section 15).
5. Code-driven effects use the group as a row selector of an `ead_*` asset and later as the handle for
   `EndEffect` / `FadeOutEffect` / `DestroyEffects`.

**Common effects** (`bUseCommonEffect`, 0x141766EB0 `[C]`): the template is looked up on the owner of the attach
target (pawn or weapon `MyEffectAtt`): first `CommonEffectList` entry with `CommonKey == CustomAttachName` and
`GroupID == entry.GroupID`; no match → nothing spawns. `bUseCommonAttachData` on an `AttachDataList` row spawns all
`DesignatedAttachDataList` assets of the target owner. Sora's pawn has neither list; the weapon has
`DesignatedAttachDataList = [{None, ead_key_trail0}]` (on `w_so_KeybladeBase`) and `CommonEffectList =
[{key_trail0, 0, VW_SO010_LOC0}, {shf_tame, 0, null}]` (on `w_so010_Wep`) `[A]`.

### 4.3 Spawning one effect

0x141765EC0 (argument set-up and flag bits read in disassembly `[I]`). Flags of the row: bit0 `ToAttach`, bit1
`DetachAtEnd`, bit2..4 `bAbsoluteLocation / Rotation / Scale`, bit5 `bUseCameraPositon`.

```
template = entry.AttachEffect, or the common lookup when bUseCommonEffect and CustomAttachName != None
if template == null: return
if ToAttach and not bUseCameraPositon:                       # attached
    psc = SpawnEmitterAttached(template, targetMesh, SocketName, LocationOffset, RotationOffset,
                               groupId = entry.GroupID, KeepRelativeOffset, autoDestroy)        # 0x14411B7A0
elif ToAttach and bUseCameraPositon:                         # camera lens effect
    lens = AddLensEffect(owner, ATresCameraEmitterLensEffect); lens.m_FieldOfView = row.FieldOfView; ...
else:                                                        # world
    T     = targetMesh.GetSocketTransform(SocketName, World)      (camera transform when bUseCameraPositon)
    loc   = bAbsoluteLocation ? LocationOffset : T.TransformPosition(LocationOffset)
    rot   = bAbsoluteRotation ? RotationOffset : T.Rotator() + RotationOffset      # component-wise add
    scale = ScaleOffset * (bAbsoluteScale ? 1 : T.Scale3D)
    psc   = SpawnEmitterAtLocation(owner, template, loc, rot, entry.GroupID, autoDestroy)       # 0x14411B280
psc.SetRelativeScale3D(scale)
postSpawn(psc)                                               # 0x141772CD0
if not entry.bNoCleanupTarget: psc.tags.Add("sqexcleanuptarget")
```

Post-spawn `[C]`: `FadeIn` → component `FadeIn(t)`; `StartDelayTime` → component +0xB88; `ParticleColor` /
`ParticleAlpha` → component +0xB78 / +0xB84; object parameter `AttachOwnerMeshComp` = target mesh; cast-shadow
bit; absolute bits copied to the component; `bTakeoverAttachObjectVisible` → visibility of the target mesh copied
once; when `SocketName` and `EndSocketName` are both set, the two sockets are given to every emitter instance and,
for a notify **state** source, trail source data is written (section 6).

Statistics of Sora's 1,432 inline rows `[A]`: `ToAttach` true 1,218 / false 214; `DetachAtEnd` 201;
`bAbsoluteRotation` 128, `bAbsoluteScale` 78, `bAbsoluteLocation` 1, `bUseCameraPositon` 12; `FadeIn` 2 rows
(0.1); `StartDelayTime` 0.3 / 0.12 / 0.08 (2 each); sockets `vfx_root` 433, `vfx_center` 116, `vfx_top` 92,
`kh_sk` 85, None 76, `mune` 43, `kh_root` 41, `vfx_call` 38, `center` 35, `vfx_Rstep` / `vfx_Lstep` 31 each.

### 4.4 Ending, fading, time, visibility

End of one tracked entry, 0x141763E60 (dispatch read in disassembly `[I]`):

| `AttachEffectEndType` | action | Sora's rows |
|---|---|---|
| 0 `LOOPEND` | component `EnableLoopEnd()`: emitters stop looping and run out; the component destroys itself when complete | 1,372 |
| 1 `FADEOUT` | component `FadeOut(row.FadeOut)` | 31 (0.15 ×11, 0.25 ×8, others) |
| 2 `TIMER` | start a countdown of `EndDelayTime`, then loop end | 3 |
| 3 `NONE` | nothing | 26 |

then, with `DetachAtEnd`, the component is detached (rules at 0x148750EE0 not read `[?]`).

What ends effects: (1) `NotifyEnd` of the state notify that spawned them; (2) an animation change (`Set`) for
entries with `WhenChangeAnimationType == END` (1,389 of 1,432 rows), so an interrupted animation leaves no looping
effect behind; (3) group calls from code; (4) the effect's own timeline (every spawn is auto-destroy); (5) a global
cleanup of components tagged `sqexcleanuptarget` by the VFX manager (callers not traced).

Component functions added by Square Enix: `EnableLoopEnd` 0x1442626D0 `[C]` (loop-end flag on every emitter
instance, trails ended, `SecondsBeforeInactive = 0`; completes at once when `Template.bLoopEndAtComplete`);
`FadeOut(t)` 0x1442633A0 `[I]`: rate = -value / t, flag fading-out (t ≤ 0: value -1); `FadeIn(t)` 0x144262FF0
`[I]`: value 0, rate 1 / t. Tick 0x144270D30 `[C]`: fading out `value += rate * dt` down to 0, then
`EnableLoopEnd()`; fading in up to 1. The fade value multiplies the rendered colour / alpha together with the
component colour and alpha (the consumer in the emitter render path was not traced `[?]`).

Time `[C]`/`[U]`: `StartDelayTime` is counted down before anything ticks; delta time is multiplied by the
component's `CustomTimeDilation`; attached effects and world effects owned by the pawn tick with the pawn's time
dilation. No code writing a particle component's time dilation for hit stop was found in the attach component or
the pawn effect functions; the Square Enix per-object-type time dilation was not traced `[?]`.

Visibility `[C]`: hiding the pawn hides attached components through stock attachment propagation `[U]`;
world-spawned effects stay visible. The see-through camera fade (0x1405FA570) writes the material parameter
`CameraFade` on the meshes and the alpha of all the actor's particle components.

---------------------------------------------------------------------------------------------------------

## 5. Engine side

- **Renderer**: UE4 Cascade. `ParticleSystemComponent` (size 3056), `ParticleSystem`, `ParticleEmitter`,
  `ParticleLODLevel`, `ParticleModuleRequired` are the stock classes with Square Enix fields added in place. No
  Niagara and no other effect middleware class exists in reflection `[C]`; the other VFX classes are renderer
  actors (ocean, water ripple, break simulation) and `SQEX_VFXManager` / `TresVFXManager`.
- **Component additions**: `m_GroupID` (+0xBD4); fade flags +0xB6C (bit0 fading out, bit1 fading in, bit2
  loop-ended), fade value +0xB70, rate +0xB74, colour +0xB78, alpha +0xB84, start delay +0xB88, tags +0xBE0,
  emitter instances +0xB00; sounds attached to effects (`SeAttachDatas`, driven by `ParticleSystem.ParticleSeAssets`).
- **Asset additions**: system `ParticleSystemTimeType` (Normal / Relative / Absolute), `SqExParticleFlag` (bits
  `[?]`), `SQEXBaseColor`, `SQEXBaseScale`, `bLoopEndAtComplete`, sort-priority fields; Required: emitter timeline
  `EmitterStartTime`, `EmitterLoopTime`, `EmitterLoopDuration`, `EmitterDurationTime`, `bIsLoopStartAndEnd`,
  `LoopTime`, `LoopDuration`, plus `EmitterBlendType`, `bUseEmitterLocalSpace`, `ScaleSpaceType`, UV scales.
- **Emitter timeline predicate** 0x14427C760 `[I]`. `t` = raw emitter time (inst+0x12C), `R` = Required, `TT` =
  time type:

  | value | TT 0 Normal | TT 1 Relative | TT 2 Absolute |
  |---|---|---|---|
  | start | `EmitterStartTime` | `EmitterStartTime` | `EmitterDelay` |
  | loopStart | `EmitterLoopTime` | `EmitterLoopTime + EmitterStartTime` | `LoopTime` |
  | loopEnd | `EmitterLoopTime` | `EmitterLoopDuration + EmitterStartTime` | `LoopDuration` |
  | end | `EmitterStartTime` | `EmitterDurationTime + EmitterStartTime` | `EmitterDuration` |

  true when `start ≤ t ≤ loopStart` (intro), or `t ≤ loopEnd` once past the start (loop), or, only after loop end
  was requested, `loopEnd ≤ t ≤ end` (ending). **All of Sora's systems use time type 0**, for which the table
  degenerates to `EmitterStartTime ≤ t ≤ EmitterLoopTime`. What the 13 callers gate with it, and how emitter time
  advances and wraps with the loop fields, is not traced `[?]`: this is the largest open item for looping effects
  (strand P2-B).
- **Module extensions**: 123 classes exist (92 `ParticleModuleSQEX_*`, 31 `ParticleModuleTypeDataSQEX_*`); 47 + 9
  occur in Sora's set. They implement a node hierarchy on top of Cascade: an emitter is a node with its own
  transform track, inheritance rules, child emitters created at given times, particles glued to the node (section
  8.3). The emitter instance caches one pointer per module class (0x144294000: +0x720 EmitterTransforms ... +0x7C0).
- **LOD / quality** `[A]`: every system of the set has a single LOD level per emitter (one emitter has a second
  LOD with other module objects), `LODDistances` [0] or [2500], no warm-up; `DefaultScalability.ini` keeps
  `r.EmitterSpawnRateScale` at 1.0 on every level. No LOD or quality switching needs to be reproduced. A
  distance-based spawn-rate scaling exists in the fork and was not traced `[?]`.

---------------------------------------------------------------------------------------------------------

## 6. Keyblade trail

Request chain `[A]` joined by rules read as `[C]`:
1. Attack clips carry an `AttachEffect` notify state with an `AttachDataList` row `{bUseCommonAttachData true,
   GroupID -2, AttachType ATTACH_TO_WEAPON, SocketName R_wep_skt, AttachDatas []}`.
2. Target = the weapon mesh attached at `R_wep_skt`.
3. `bUseCommonAttachData` → the weapon's designated attach data `ead_key_trail0`: `{bUseCommonEffect true,
   AttachEffectEndType FADEOUT, FadeOut 0.15, SocketName vfx_top, EndSocketName vfx_bottom, ToAttach true,
   effects [{GroupID 0, CustomAttachName key_trail0, AttachEffect = a placeholder (vw_ex110_loc00)}]}`.
4. Common lookup on the weapon: `key_trail0`, group 0 → `/Game/Effects/wep/w_so010/par/VW_SO010_LOC0`. Because the
   entry's group is 0 the trail spawns only while the current animation group is 0 (true for every base-form attack
   play, section 7.4).
5. Spawned attached to the weapon mesh at `vfx_top`; post-spawn writes trail source data: first socket `vfx_top`,
   second `vfx_bottom`, width 1.0, width mode 2 (`ETrailWidthMode_FromSecond`), trail enabled (= stock
   `BeginTrails(vfx_top, vfx_bottom, FromSecond, 1.0)`) 0x141772CD0 `[C]`.
6. End (`NotifyEnd` or animation change): `FadeOut(0.15)`: the component fades linearly to 0, then loop end.

Asset `VW_SO010_LOC0` `[A]` (one emitter): type data stock `ParticleModuleTypeDataAnimTrail` with class defaults
(`DistanceTessellationStepSize` 10, `bDeadTrailsOnDeactivate` true, per constructor 0x1443393A0 `[C]`); Spawn rate
20 / s; **Lifetime 0.2 s**; Size 25; Color (3, 3, 3), alpha 1; ColorScaleOverLife constant 1;
`SQEX_DepthOffset` -10; `SQEX_AllParticleKill.KillEmitterTime` 0.385; `SQEX_EmitterTakeover` defaults; Required
timeline fields 0.055 / 0.075 / 0.185. Material `/Game/Effects/wep/w_com/mat/jmt_loc_00sa`: unlit, translucent,
two-sided, texture parameter `Tex Locus(BC7)` = `/Game/Effects/wep/w_so010/tex/ttx_loc_00`, alpha texture
`/Game/Effects/texcom/tex/jtx_loc_02`, vector parameters `Line Color` (500, 100, -20, 1) and `Tex Multi Color`
(1, 1, 1, 1).

Ribbon runtime: stock `FParticleAnimTrailEmitterInstance` by asset class `[U]`; its functions were not located in
the exe `[?]`. With width mode "from second" and width 1.0 the two edges are the two socket positions sampled each
tick (79.04 cm and 14.15 cm along the blade), and the trail is as long in time as the particle lifetime.

Trail windows of the base ground combo `[A]` (sheet `sora-effects`, e.g. `anim.1301.S.AttachEffect.0.0`: clip
1301, start 0.163 s, duration 0.088 s). 61 such rows exist in Sora's body clips.

---------------------------------------------------------------------------------------------------------

## 7. Inventory

### 7.1 Method and scope

Mined from asset data with scripts (`build/sora_tmp/vfx/fxinv/`): every notify event of the 5,442 animation
sidecars under `Character/pc/p_ex001/anm` and the 4 weapon clips (19,206 events), all 85 anim sets, a reference
walk from the pawn / weapon / set / load-asset roots, and an index of who references `Effects/system` and
`Effects/PC`. Scope rule: `w_so010_Equip` replaces no anim set, so a clip is in scope when it is a row of
`p_ex001_AnimSet` (428 rows); the 1,410 notify rows of other sets are kept in
`build/sora_tmp/vfx/fxinv/rows_effects_outofscope.json`.

### 7.2 Counts

| category | rows | yes | conditional | no | unknown | na |
|---|---|---|---|---|---|---|
| locomotion | 227 | 200 | 3 | 0 | 22 | 2 |
| keyblade_appear | 4 | 4 | 0 | 0 | 0 | 0 |
| melee | 164 | 148 | 0 | 5 | 6 | 5 |
| guard | 19 | 11 | 2 | 6 | 0 | 0 |
| magic | 252 | 226 | 0 | 17 | 9 | 0 |
| cure | 26 | 26 | 0 | 0 | 0 | 0 |
| shotlock | 96 | 20 | 1 | 32 | 42 | 1 |
| item | 11 | 11 | 0 | 0 | 0 | 0 |
| formchange | 7 | 4 | 0 | 3 | 0 | 0 |
| damage | 6 | 4 | 0 | 0 | 2 | 0 |
| abnormal | 26 | 25 | 0 | 0 | 1 | 0 |
| death | 10 | 10 | 0 | 0 | 0 | 0 |
| prize | 15 | 15 | 0 | 0 | 0 | 0 |
| levelup / lowhp | 1 / 1 | 0 | 0 | 0 | 1 / 1 | 0 |
| link_attraction (pointers) | 70 | 58 | 0 | 1 | 10 | 1 |
| form_pointer (pointers) | 171 | 10 | 0 | 109 | 49 | 3 |
| other | 17 | 15 | 0 | 0 | 2 | 0 |
| **total** | **1,123** | **787** | **6** | **173** | **145** | **12** |

By trigger kind: anim_notify_state 257, anim_notify 282, component_table 212, projectile 228, data_table 113, code
16, load_asset 12, unknown 3. Distinct particle systems: 515 referenced, 349 reachable (`yes` or `conditional`).
`ref_conf`: 1,087 confirmed (read from the named export and property), 36 inferred (joined through the common-key
rule of 4.2). 88 rows have no effect: pure triggers (Shot, ThrowWeapon, SetWeaponVisible), empty entries, unset
properties and the three "none found" markers.

### 7.3 What each category contains

Full tables per category: `build/sora_tmp/vfx/fxinv/spec.md` section 3 (generated from the same rows). Summary:

- **Locomotion**: speed lines `vs_spd_lin0/1/2` on arm and leg bones (double flight, dodge roll, air dodge, air
  slide, glide), `vs_air_slide0`, `vs_adg_rng0`, wall-run smoke `vs_wrn_smk0` on `vfx_Lstep` / `vfx_Rstep`,
  slide effects `vs_ddg_*`, athletic-flow `vp_ffa_*` / `vp_atf_*`, pole spin `vp_ffa_spn_*`; code-driven aura
  groups 3200..3202; footsteps through the physical-material tables (4.1 d; 108 `physmat.*` rows, world tables
  only). **Landing puffs: none found** (`Land`, `RollLand`, `LightLand*` carry no effect notify; no landing
  property on the pawn). Swim clips of the main set carry no effect notify.
- **Keyblade appear / disappear**: `weapon.appear`, `weapon.disappear` (`VW_SO010_APP0`), the `SetWeaponVisible`
  notify of `M_1501`, the `ThrowWeapon` notify of `BackRaid`.
- **Melee**: the trail (section 6) in every swing window; swing effects `vp_cmb_*` (`Effects/PC/Attack`) on
  `kh_sk` / `kh_root`; `vp_vld_*` for the dive attack; hit sparks from the weapon's table (2.6); wall-kick and
  flow-attack effects `vp_ffa_*`. Which combo step a clip is: sheet `sora-combat-attacks`.
- **Guard**: `vs_com_grd01` at `vfx_root` on `Guard` / `AirGuard` (t = 0.034 s), `vs_com_grd00` at `vfx_center`
  and `vs_com_grd_smk00` at the step sockets on `GuardHit`; in water (effect group 0x209) `vp_div_flo1`,
  `vp_rvj_ksk0`; reflect effects of the load asset and pawn properties (code).
- **Magic**: cast effect on the keyblade at `vfx_call` from `ead_wep_cal0` by spell group (Fire 1301.. →
  `vm_fir_cal0`, etc.); projectile body, trail, impact and map-hit effects from the projectile blueprints (228
  rows: Fire / Fira / Firaga, Blizzard, Thunder, Water, Aero and their grand-magic variants). **Cure**: cast
  `vm_cur_cal0`, body effects `vm_cur_hed0` / `vm_cur_bdy0` at `vfx_abn_center` from `ead_com_abn0` groups
  1420..1425.
- **Shotlock**: the projectiles of `w_so010_ProjSet` (`KC_SHOT`, `KC_RAGNAROK`; effects `vw_so010_rgn_*`, `vw_sor_*`), lock-on lens
  effect `BP_slc_camatch00_C`, focus-aspir effect; the shotlock clips of the main set also hold other keyblades'
  rows (7.4).
- **Item use**: `ead_com_abn0` groups 3120..3155. **Abnormal status**: stun, freeze, burn, cloud, ... by group.
  **Damage**: `vs_grd_sor_arv00` on `AirRecovery`, invincible / armor hit effects. **Death**: `ead_com_sys` groups
  1 / 2, game-over effects. **Prize**: pickup effects of the prize blueprints.
- **Formchange**: charge groups 3000..3002 and the transition clip rows; whether an aura persists during Second
  Form is not determined.
- **Level up, low HP**: nothing found in the mined data; if the game shows something it is UI- or code-driven.

### 7.4 Which rows play with the Kingdom Key (column `kk_base`)

The inventory strand could only name-match effects that belong to other keyblades but sit inside shared clips.
The group rule of 4.2 resolves most of them. Effect groups the player states pass to the play API `[C]`
(`docs/sora-actions-spec.md`, `docs/sora-magic-spec.md`, `build/sora_tmp/native_A.md`):

| group | passed when |
|---|---|
| 0 | default: every play of the plain Keyblade (weapon kind 1) not listed below |
| 0x139 (313) | `WallKickF/R/L` |
| 0x209 (521) | `Guard` / `GuardHit` in water (Guard flag 0x40) |
| spell `m_EffectGroup` (1301..1445) | magic cast animations |
| 0x631 (1585) | shotlock state, kind 1 |
| 0x71F, 0x720, 0x76C, 0x7DA, 0x834, 0x898, 0xD48, 0xD52 (1823, 1824, 1900, 2010, 2100, 2200, 3400, 3410) | form-change weapon kinds only (0x10, 0x11, 0x12, 0x15, controller modes 0xDD / 0xDE) |

That the plain Keyblade is weapon kind 1 is the actions strand's reading with a probe pending (P3 of
`docs/sora-actions-spec.md`); everything below inherits that `inferred` status. Rule applied by
`tools/sora_vfx_sheet.js` to the `AttachEffect` notify rows (basis text in `params.group_gate`):

| entry | `kk_base` | rows |
|---|---|---|
| `GroupID` 0 or -1, generic or Kingdom Key effect | yes | (most of the 787) |
| `GroupID` one of the form-kind groups | no | 68 of the 128 "other keyblade" rows are resolved this way or by another non-zero group |
| `GroupID` 313 on a `WallKick*` clip, 521 on a guard clip, a spell group, 1585 | conditional | 6 |
| `GroupID` 0 but effect of another keyblade, in a shotlock or `StyleChangeN` clip | unknown | 60: not gated by group; the clip is chosen by the weapon's shotlock or form, so clip selection decides |
| `GroupID` 0, clip of the Second Form or another form's moveset | no | 52 + 53 |
| other non-zero groups on generic effects (3200 athletic-flow radial lines, 314 pole spin, 520 air dodge, 262, 283, 2500..2700, 3192, 3196, 10000) | unknown | 34: no play with that group is known |

The 3200 rows deserve a note: `vp_ffa_rad0` (radial speed lines) sits in wall-run, slide, wall-kick and pole
clips with `GroupID` 3200, the same number as the athletic-flow aura group of `ead_pc_aura_com0`. The plays are
known to pass 0 or 313, so by the rule as read these would never spawn, which is implausible for effects authored
on eleven base clips. Either some code adds 3200 to the current group list while athletic flow is active, or the
mesh group override (mesh+0x864) is used. Until that is traced or probed these rows are `unknown` (strand P2-D).

### 7.5 Requests from the other agents

`build/sora_tmp/{combat,magic,actions}/asset_requests.json` were merged after the inventory was built (they did
not exist when it ran). Of 113 + 128 + 29 effect entries every particle system is in the pack except four:
`/Game/Effects/magic/water/par/vm_wat_han010`, `_han040`, `_han060`, `_han080` (template of the `ParticleComp` of
`TresProj_WaterBurn`, magic strand) — **arrives later**: add them to the effect list and rerun the pack. Requested
entries that are not particle systems and are therefore not effect JSON: the attach-data assets (`ead_com_abn0`,
`ead_com_abn_w0`, `ead_pc_aura_com0`, `ead_key_trail0`, `ead_wep_cal0`, `ead_shf_tame0`: their rows are in
`sora-effects` and in `vfx/triggers.json`), decal materials (`sdc_blz_00a`, `sdc_blz_trl_00a`, `tdc_fir_zmn00a`),
blueprint actors (`AthleticFlowRootEffect`, `BP_slc_camatch00`, `TresLateralDecalBlizzard`,
`TresSplineMeshBlizzard`), and the FName `sora_kaihuku` (a heal effect name played on the mesh by the item
recover code; not resolved to an asset `[?]`).

---------------------------------------------------------------------------------------------------------

## 8. The Cascade subset

Effect set: 516 systems, 3,241 emitters (LOD 0). Game-wide comparison: 10,409 systems, 57,420 emitters. Sheet
`sora-vfx-modules` has one row per class and feature with both counts.

| | Sora set | game-wide |
|---|---|---|
| module classes in use | 86 (39 stock, 47 Square Enix) | 140 |
| type data: mesh / sprite / SQEX_Control / GPU sprite | 1,245 / 896 / 509 / 399 | 25,465 / 12,377 / 6,670 / 9,544 |
| type data: TailPolygon / Decal / TailShape / GrassForce / AnimTrail / SkeletalMesh / TailShapeEx / TailShapeTypeMesh | 91 / 64 / 22 / 6 / 5 / 2 / 1 / 1 | 1,011 / 1,579 / 262 / 19 / 79 / 134 / 26 / 24 |
| distinct materials | 669 (100 Material, 569 instances) | 6,497 |
| blend (emitter references): additive / translucent / masked / opaque | 1,275 / 1,037 / 78 / 64 | 18,434 / 24,232 / 2,729 / 1,291 |
| shading: unlit / default lit / subsurface | 2,311 / 91 / 52 | 42,759 / 3,060 / 867 |
| two-sided | 2,165 of 2,454 | 40,361 of 46,686 |

### 8.1 Data model facts

- Only non-default properties are serialized. The defaults come from the C++ constructors;
  `tools/lib/vfx_class_defaults.js` runs each constructor in the emulator `[I]` →
  `audit/sora_port/vfx_class_defaults.json` (263 of 267 classes). Defaults of this build that differ from what one
  might assume: Required `EmitterLoops` 0, `EmitterDuration` 1, `MaxDrawCount` 500, `bScaleUV` false,
  `bUseLegacyEmitterTime` true (but every emitter of the set stores false), `EmitterBlendType` opaque.
- **The shipped game samples baked lookup tables, not curves** `[U]` (stock `FRawDistribution::GetValue` uses the
  table when it exists outside the editor; not read in the exe). Every distribution carries one `[A]`. Sampling:
  `x = max(t - TimeBias, 0) * TimeScale`, linear between entries `floor(x)` and `floor(x)+1` (clamped); `Op` 2
  holds minimum and maximum sets and the result is `min + (max - min) * r`. The cooker's baking (128 samples, then
  reduction to 64..2 entries) was reproduced numerically: 628,059 of 628,740 baked values match an independent
  Hermite evaluator within 1e-4. For 1:1 behaviour sample `lookup_table`; the curve is the authored source. Four
  distributions have a table that is not their curve (`vm_thd_str1`, `vs_itm_acu0`, `vs_com_grd_smk00`,
  `vw_so010_hit0`, one each, flagged in the JSON).
- Distribution semantics implemented by the converter `[U]`: constant; uniform `Max + (Min - Max) * r` (vector:
  own `r` per axis, mirror flags, locked axes copy the first axis of the lock group, `bUseExtremes`);
  constant-curve = `FInterpCurve::Eval` with constant / linear / cubic Hermite segments and stored tangents;
  uniform-curve; particle-parameter distributions exported with name and mapping, not evaluated; the four
  `SQEXDistribution*` classes (Gauss / Uniform, 33 instances) verbatim `[?]`.
- Sampling time `[U]`: spawn-time modules use emitter time in seconds; over-life modules use the particle's
  relative time 0..1 (`bEmitterTime` / `bUseEmitterTime` switch to emitter time).
- **Cooked materials have no math nodes**: only parameters, texture samples, function calls, dynamic parameters
  and static switches survive `[A]`; the compiled shader map is the full truth and is not decompiled. The pack
  exports the uniform expressions of the shader map as text (e.g. `mul(time, sparam("speed", -6))` for a panner).

### 8.2 Stock modules (39 classes; all `[U]` by stock name, stock property set and stock spawn / update flags
read from the constructors `[I]`; the `Spawn` / `Update` bodies were not read, and several stock classes carry
Square Enix properties, so some bodies are certainly touched: open question 5)

| module | effects / emitters | behaviour |
|---|---|---|
| Required | 516 / 3,241 | material; `ScreenAlignment` (Square 2,934, Rectangle 116, TypeSpecific 107, Velocity 44, AwayFromCenter 39); `SortMode` (None 3,201); `bUseLocalSpace` (418); duration, loops (1: 2,683; 0 = forever: 555), delay (181 non-zero); `bKillOnCompleted` (3,231); sub-UV grid (1×1 3,160; 8×4 43; 8×8 18; 8×5 14; 2×2 4; 4×4 2) with interpolation (None 3,139, Linear 51, Linear_Blend 45); `EmitterOrigin` (557 set), `EmitterRotation` (241 set) |
| Spawn | 516 / 3,241 | `Rate × RateScale` per second at emitter time, fractions carried; bursts fire once per loop at `Time × EmitterDuration`, count `Count` or random in [`CountLow`, `Count`], × `BurstScale` |
| Size | 470 / 2,610 | `Size += StartSize(emitter time)`, also BaseSize |
| Color | 469 / 2,518 | `Color = StartColor`, alpha `StartAlpha`; values above 1 are HDR intensity |
| SizeScale | 459 / 2,408 | `Size = BaseSize × SizeScale(relative time)` per enabled axis |
| ColorScaleOverLife | 458 / 2,394 | `Color = BaseColor × ColorScaleOverLife(t)`, alpha × `AlphaScaleOverLife(t)` |
| Lifetime | 426 / 2,049 | `OneOverMaxLifetime = 1 / Lifetime(emitter time)` |
| ParameterDynamic | 377 / 1,130 | up to 4 scalars to the material's DynamicParameter node; per entry value method (user set / velocity), spawn-time-only, emitter-time flags |
| CameraOffset | 143 / 451 | moves the rendered particle toward the camera by `CameraOffset` cm |
| AccelerationConstant | 181 / 401 | `Velocity += Acceleration × dt` |
| Velocity | 198 / 389 | `Velocity += StartVelocity + radial direction × StartVelocityRadial` |
| AccelerationDrag | 123 / 196 | `Velocity -= DragCoefficient × Velocity × dt` |
| Rotation | 121 / 176 | sprite `Rotation += StartRotation × 2π` (turns) |
| Light | 155 / 162 | point light per particle: colour × `ColorScaleOverLife` × `BrightnessOverLife`, radius = `RadiusScale` × size |
| ColorOverLife | 49 / 100 | `Color = ColorOverLife(relative time)` (replaces) |
| RotationRate | 62 / 76 | `RotationRate += StartRotationRate × 2π` |
| SizeMultiplyLife | 48 / 74 | `Size *= LifeMultiplier(relative time)` |
| MeshRotation | 53 / 64 | mesh rotation = `StartRotation × 360°` per axis |
| SubUV | 50 / 63 | sub-image index = `SubImageIndex(relative time)` (integer = frame, fraction = blend) |
| Location | 44 / 59 | `Location += StartLocation` |
| Orbit | 26 / 43 | render-time offset rotated by `RotationAmount` + ∫`RotationRateAmount` |
| VelocityOverLifetime | 18 / 20 | `Velocity *= VelOverLife(t)` or `=` when `Absolute` |
| MeshRotationRate, PivotOffset, SubUVMovie | 16 emitters each | mesh rotation rate; sprite pivot shift; flipbook at `FrameRate` |
| 14 more (≤ 11 emitters each) | | AttractorPointGravity, SpawnPerUnit, CollisionGPU, Collision, Acceleration, AttractorPoint, MeshRotation_Seeded, Size_Seeded, AccelerationOverLifetime, ParameterDynamic_Seeded, SizeScaleBySpeed, LocationPrimitiveSphere, Location_Seeded, VectorFieldGlobal |

Type data `[U]`: sprite (no type data) = camera-facing quad; mesh = one instance of `Mesh` per particle, scale =
`Size`, material = Required.Material when `bOverrideMaterial`; GPU sprites carried verbatim; AnimTrail = ribbon
between two sockets (section 6).

### 8.3 Square Enix modules (emitters in Sora's set; state of knowledge from
`audit/sora_port/vfx/sqex_modules.json`, column `semantics_conf` of the sheet)

| module | effects / emitters | what is known | what is not |
|---|---|---|---|
| EmitterTransforms | 458 / 2,329 | `[C]` node transform track sampled at emitter time every tick (0x144294B90, 0x1442A17A0): `LocalAddPosition / Rotation / Scale` are either absolute values (`bCurveParameterDirect*`) or rates integrated with dt (rotation in degrees, wrapped to [0, 360)); `bInitializeCurve*ParameterUseOnly` samples once; gravity: `g = dist(t) × 60`, `v += g × axis.z × dt`, z offset `+= v × dt` | `Velocity` consumer; `DisplayType` billboard modes (0 none, 1 camera, 2..4 axis-locked, 5 screen); `bIsUseTransformScale` |
| EmitterTakeover | 398 / 2,127 | `[C]` colour / alpha part (0x1442A1450): each `*TakeoverType` is NONE / ALWAYS / SPAWN_TIME_ONLY; parent particle colour and alpha, and the emitter's own ColorScaleOverLife at emitter time, combine into an emitter colour multiplier | the transform chain (component / parent position, rotation, scale; `bLeavePlaceInWorld`): functions 0x1442A0CA0, 0x1442A57C0, 0x144280490 not traced |
| AttachParticle | 358 / 1,297 | `[C]` Spawn 0x1442F20C0 / Update 0x1442FB700: `bIsAttachPosition` → particle location = emitter node origin every tick; `bIsAttachRotation` → sprite rotation = roll of the node matrix, mesh rotation composed with it | - |
| ParticleRotation | 263 / 755 | `[C]` per-particle 3-axis rotation in degrees: start + (direct value or integrated rate) × scale; time source emitter / own / relative; sprite uses x only; mesh = R(start) × R(added) | `bMeshRotAdjustEmitterRot`, `bUseCalcMeshRotMat` |
| Parent | 212 / 536 | `[C]` Update 0x14437A3B0: creates child emitter instances of the emitter named `GenerateEmitterName` at `TimeDatas{GenerateTime, GenerateNumber}` (once per loop, limits), optionally at spawn-shape positions; `LoopEndTime` / `AllLoopEndTime` give children loop end | PARTICLE_DEATH condition; `bToChildren`, `bChildrenTakeOVerTheParentTime` |
| Range | 266 / 513 | `[C]` partial: spawn location inside a sphere / disc of `RangeRadius` with `Deviation`, `DirectionAngle`, scaled by `RangeScale(emitter time)`, rotated by `Direction`; base class adds initial velocity by `DirectionType` × `Speed` | exact point distribution (0x144356A00) |
| ComponentCutoff | 171 / 431 | by reflection and name: cuts the node off from the component rotation / scale / fade | readers 0x144284A80, 0x1442A57C0 not traced |
| DepthOffset | 151 / 379 | `[C]` Spawn 0x1442F2650: per-particle payload = \|node scale\| × `DepthOffset`, a camera-depth bias in cm (negative = toward the camera) | vertex-factory consumer |
| AllParticleKill | 93 / 379 | by name: all particles killed when emitter time reaches `KillEmitterTime` | reader not located |
| Ring | 181 / 325 | by reflection: `VertexNumber` points on radius `RangeRadius` (second radius, order type, interval, offsets); `bAttachParticle` keeps particles on the ring | body not traced |
| CurlNoise | 139 / 239 | `[C]` Spawn stores the start offset; parameters `Intensity`, `PositionScale`, `TimeScale`, per-axis scale | Update 0x1442FC8C0 (1,537 bytes) not traced |
| LocalParticle | 70 / 115 | keeps particles in the node's local space (position / rotation flags) | body not traced |
| TailShape (+ type data TailPolygon 91, TailShape 22) | 76 / 115 | reflection only | tail (ribbon) emitters entirely |
| MeshPivotOffset 65, SphereForce 43, SpawnParticleSystem 41, Material_InLight 37, VertexSpawn 33, Material_Scalar 31, EmitterInitializeTransforms 30 (`[C]`: initial node position / rotation / scale sampled once), Filter 25, Raycast 25, Material_Aura 20, RadialBlur 20, ChangeSceneColor 14, Quadrangle 9, CameraShake 7, ... | | reflection + name, or reflection only | everything else |

`TypeDataSQEX_Control` (509 emitters) `[C]`: a non-rendering node (base emitter instance) used as parent /
transform carrier for `SQEX_Parent` hierarchies and for blur / camera modules.

Coverage, counted over the pack: emitters that are sprite or mesh and whose stock modules are all within the 25
most used: 2,103 of 3,241 (65%); of those, the ones that need no Square Enix module at all: 87. Adding Square
Enix modules in usage order, the share of emitters fully covered grows: + EmitterTransforms, EmitterTakeover,
AttachParticle 18.5%; + ParticleRotation 27.5%; + Parent, Range 31.6%; + ComponentCutoff, DepthOffset 45.5%;
+ AllParticleKill 53.8%; + Ring, CurlNoise, LocalParticle 60.4%. The rest needs control emitters (509), GPU
sprites (399), tails, decals.

---------------------------------------------------------------------------------------------------------

## 9. Effect pack (`build/sora_pack/vfx/`, about 210 MB)

`node tools/sora_vfx_pack.js` (1.1.0; deterministic; `--check`, `--census [--game]`, `--effect`, `--dir`,
`--list`). Run `node tools/sora_vfx_sheet.js` first: the pack copies the trigger table from the sheet.

| part | content |
|---|---|
| `effects/<name>.json` (516) | one per particle system: system fields; per emitter (LOD 0) type data, Required (curated + all params), Spawn (rate, bursts), ordered `modules[]` with every property (class defaults overlaid with serialized values), every distribution raw (keys, tangents, flags, baked table) and evaluated (33 samples, or min / max pairs); Square Enix modules verbatim with `semantics` text where known; per-effect `unsupported` census |
| `materials/<name>.json` (715) | parent chain, blend, shading, two-sided, textures by parameter, scalars, vectors, dynamic-parameter names, which expressions feed emissive / opacity, shader-map uniform expressions, classes not interpreted |
| `textures/` (404 PNG + 2 raw half-float tables) | top mip, RGBA 8 bit; `sRGB` chunk or `gAMA` 1.0 by the texture's flag; BC4 masks replicated to RGB (247 of 406) |
| `meshes/<name>.glb` (296) | LOD 0 of the static meshes of mesh emitters in pack conventions, all UV sets and vertex colours |
| `triggers.json` | the 1,123 rows of `sora-effects` as objects, each with `effect_file` (the pack file of its effect): the pack alone says when and where each effect is spawned |
| `manifest.json` | format `kh3-sora-vfx/1`; conventions; field-by-field schema (generated from the tool's `SCHEMA` constant); per-effect source hashes, emitter counts, references, unsupported census |
| `sheet_rows.json`, `validation.json` | rows merged into `sora-assets`; validation results |

Conventions: effect JSON keeps source units and axes (cm, s, degrees or turns, X forward, Y right, Z up; colours
linear and unclamped). To the pack's glTF space: position `(x, y, z) cm → (x, z, y) × 0.01 m`, direction
`(x, z, y)`, quaternion `(-x, -z, -y, w)`. Sub-UV frame `i` of an H×V grid = column `i mod H`, row
`floor(i / H)`, row 0 at the top.

Verification (numbers of the final run):
- `--check`: 1,934 files, 0 problems (hashes, JSON parse, PNG read-back, `gltf-validator` on 296 meshes: 0 errors).
- `build/sora_tmp/vfx/cascade/verify.js` (separate code path that walks the raw properties by name): 516 effects,
  3,241 emitters, **1,590,551 values compared, 0 mismatches** (module order, material names, sub-UV grids, burst
  counts and times, lifetime / size / colour constants, uniform bounds and curve keys, blend mode resolved through
  the parent chain).
- Evaluated tables: 590,766 samples recomputed independently, 0 differences at 1e-5.
- Textures viewed: `VW_SO010_APP0` (keyblade appear; all emitters 1×1): `jtx_sho_00a` 512² vertical streak mask,
  `stx_rng_00a` 1024² quarter ring, `jtx_glw_01a_clamp` 128² soft round glow, `jtx_crs_03a` 64×192 flare strip;
  `vw_so010_hit0`: `otx_fls_00m` 1024² radial ray burst; flipbooks `jtx_fir_8x5_00c` (`vs_fir_hit0`, 8×5, 40 clean
  fire frames) and `atx_expc_ver_01_00` (`vw_so010_rgn_hit00`, 8×8, 64 smoke frames). All recognisable.

Approximated or carried verbatim (per effect in `unsupported`, totals in `manifest.census.unsupported`): every
Square Enix module and type data; GPU sprite emitters (399: simulation not described); anim trails (parameters
only); particle lights (162), collision, attractors, vector fields (parameters exported, need host features);
`_Seeded` modules (seed info exported, exact sequence needs `FRandomStream`); material graphs (parameters,
textures and uniform expressions only); only LOD 0; one cube map (`T_ca_Sky`) is not exported.

The default effect list is still read from `build/sora_tmp/vfx/fxinv/effect_list.json` (scratch); a copy is kept
as evidence only through the row files. If the scratch folder is removed the tool falls back to its seed set, so
pass `--list` explicitly then.

---------------------------------------------------------------------------------------------------------

## 10. Screen effects, decals, lights, material parameters

- **Camera shakes**: `docs/sora-camera-spec.md`. Requests seen here: blueprint notify
  `AnimNotify_TresCameraShake_C` (one clip, 0962) and particle module `SQEX_CameraShake` (7 emitters of the set).
- **Lens effects**: `ATresCameraEmitterLensEffect` (`m_FieldOfView`, `m_bOnlyCameraSee`), requested by attach rows
  with `ToAttach` + `bUseCameraPositon` (12 rows), by the pawn's `m_LensEffectClass` (shotlock) and by
  `TresAnimNotifyState_LensEffect` (not used by Sora's clips); all through 0x140E6BB60 → camera manager
  `AddCameraLensEffect` `[C]`.
- **Blur, flash, scene colour**: driven from inside particle systems by modules, not by player code:
  `SQEX_RadialBlur` (20 emitters), `ChangeSceneColor` (14), `BloomRadialBlur` (2), `ScreenBlur`, `GaussBlur`,
  `PostprocessAura`, `PostprocessFringe` (1 each) `[A]`; parameters are in the effect JSON, behaviour is
  reflection + name only. The death blur is the particle system `vs_com_dth_bra00` (one control emitter with `SQEX_ScreenBlur`). No radial blur request by
  finisher code was found.
- **Decals**: `TypeDataSQEX_Decal` emitters (64); gameplay decal `UTresDecalComponent` for hit-effect
  `m_DecalData` (tick 0x1417747D0: fade in / hold / fade out on parameter `OpacityFade`); the Kingdom Key's hit
  table has decals off.
- **Lights**: stock `ParticleModuleLight` on 162 emitters (155 systems).
- **Material parameters on the character** `[C]`:

  | visual | parameter(s) | function |
  |---|---|---|
  | in-light, in-water, lock-on, wetness, aura | scalars by kind 0..7 = `InLight`, `InWater`, `LockonSwitch`, `Wetness`, `Aura`, `WetnessLength`, `InWater`, `InWaterDeepVolume`; vectors `InLightCenterPosition`, `InLightColor`, `InLightOutColor`, `AuraColor`, `AuraOutColor` | mesh vt+0xDA8 0x1405D1CF0; 0x1405D2130 / 0x1405D2040; also driven by effects (`SQEX_Material_InLight` / `_Aura`) |
  | see-through camera fade | `CameraFade` (+ particle alpha) | 0x1405FA570 |
  | opacity / dither of the character or weapons | `OpacityFade`, `VisChara`, `Vis_Keyblade` | 0x140E6AD70 (warp-type actions); weapon appear 0x141792140 |
  | form change | textures `FormChangeTex0`, `FormChangeNomTex0`, scalar `FormChangeSlide` | 0x1405B6A00 |

  **Not found**: a hit flash, an invincibility blink or a damage / death dissolve on the player (no such parameter
  name among the FName literals of the pawn, state, camera and weapon code). Confirm visually.

---------------------------------------------------------------------------------------------------------

## 11. Implementation order and test list

Each test is a scripted run of the test level (`khgame.exe --script ... --shots`) with a screenshot at the stated
clip time; "seen" is what must be on screen. Times are clip-local seconds from `sora-effects`.

### Tier 1: right effect, right time, right place, real textures (stock subset)

Build: the trigger runtime of section 4 (notify rows, group rule, end types, animation-change cleanup), spawn /
attach transforms of 4.3, and a Cascade evaluator for sprite and mesh emitters with the 25 stock modules of 8.2 in
usage order: Required, Spawn, Size, Color, SizeScale, ColorScaleOverLife, Lifetime, ParameterDynamic,
CameraOffset, AccelerationConstant, Velocity, AccelerationDrag, Rotation, Light (skip until tier 3),
ColorOverLife, RotationRate, SizeMultiplyLife, MeshRotation, SubUV, Location, Orbit, VelocityOverLifetime,
MeshRotationRate, PivotOffset, SubUVMovie. Sample the baked `lookup_table`. Materials: blend mode, two-sided,
the texture slots and the particle colour; additive and translucent unlit cover 2,312 of 2,454 emitter materials.
Square Enix modules are ignored in this tier, so 65% of the emitters draw (most with wrong motion) and control
emitters, GPU sprites, tails and decals draw nothing.

| test | script | screenshot | seen |
|---|---|---|---|
| T1.1 keyblade appear | idle, raise the battle flag (Idle plays `BattleStart`, clip 1014) | 0.10 s, 0.25 s, 0.40 s after the show call | 0.10: appear effect at the right hand, no blade; 0.25: blade visible and half-dithered (`OpacityFade` about 0.5); 0.40: blade solid |
| T1.2 keyblade disappear | clear the battle flag, wait 3 s (`BattleEnd`, clip 1028) | 0.10 s, 0.25 s, 0.35 s after the hide call | 0.10: effect + solid blade; 0.25: blade half-dithered; 0.35: blade invisible, effect still running out; blade stays attached to `R_wep_skt` throughout |
| T1.3 trail | first ground combo attack (clip 1301) | 0.20 s and 0.35 s | 0.20: ribbon between 14.15 and 79.04 cm of the blade covering about the last 0.04 s of the swing (window opens at 0.163 s); 0.35: ribbon faded out (window closed at 0.251 s, 0.15 s fade) |
| T1.4 trail group rule | same attack with the play group forced to 1900 | 0.20 s | no trail |
| T1.5 swing effect | second combo attack (clip 1302) | 0.42 s | `vp_cmb_tuk0` at `kh_sk` (window 0.398..0.431 s) |
| T1.6 hit spark | first attack against a dummy, physical attribute | hit frame + 2 frames | `vw_so010_hit0` at the hit location facing the hit normal, plus `vw_so010_str0`; against level geometry: `VW_SO010_WHT0` |
| T1.7 guard | `Guard` (clip 1281) | 0.10 s | `vs_com_grd01` at `vfx_root` (spawned at 0.034 s) |
| T1.8 dodge roll | `DodgeRoll` (clip 0051) | 0.16 s | `vs_spd_lin1` on the leg bones (e.g. `R_ashi1`, window from 0.144 s). The speed-line systems are 2 sprite + 2 tail-polygon emitters: in this tier only the sprite part draws, at the right bone |
| T1.9 air slide | `AirSlide` (clip 0055) | 0.10 s | `vs_air_slide0` at `kh_sk`, `vs_spd_lin0` on the arms |
| T1.10 animation-change cleanup | start `GlideLoopF` (clip 0043, looping `vs_spd_lin2` on `L_ude2` / `R_ude2`), interrupt with a jump | 0.5 s after the interrupt | speed lines have stopped spawning and run out; nothing left attached |
| T1.11 fire cast | Fire | cast frame of the spell (group 1301) | `vm_fir_cal0` on the keyblade at `vfx_call`; projectile body `vm_fir_blt0`; on impact `vs_fir_hit0` (8×5 flipbook) |
| T1.12 footsteps | run on a surface with no world table | any | no ground effect (4.1 d); with the HE table loaded and a grass surface: the table's effect at each foot contact |

### Tier 2: Square Enix modules, in order of emitter usage

Implement in this order; each step is testable on its own with the effects named. Known / unknown per module is
the table of 8.3; the unknown parts are the follow-up strands of `build/sora_tmp/vfx/P2_STRANDS.md`.

| step | module(s) | needs first | test (effect, seen) |
|---|---|---|---|
| T2.1 | EmitterTransforms (2,329) | - | `VW_SO010_APP0`: the emitters move / rotate / scale along their node tracks instead of sitting at the origin |
| T2.2 | EmitterTakeover (2,127), ComponentCutoff (431) | strand P2-A (transform chain) | an attached looping effect (`vs_spd_lin2` on the forearm during `GlideLoopF`): follows or does not follow the bone exactly as in the game; colour inheritance from the parent |
| T2.3 | AttachParticle (1,297) | T2.1 | single-particle mesh emitters of `VW_SO010_APP0` stay glued to their moving node |
| T2.4 | ParticleRotation (755) | - | mesh rings of `vw_so010_hit0` / `VW_SO010_APP0` oriented and spinning on three axes |
| T2.5 | Parent + TypeDataSQEX_Control (536 / 509) | strand P2-B (emitter timeline) | `VW_SO010_APP0` (3 Parent, 3 Control): child emitters appear at their generate times |
| T2.6 | Range (513), Ring (325), Quadrangle | strand P2-C | hit sparks spread over the authored sphere / ring instead of a point |
| T2.7 | DepthOffset (379), AllParticleKill (379) | - | trail drawn 10 cm toward the camera; emitters cut at `KillEmitterTime` |
| T2.8 | CurlNoise (239), LocalParticle (115) | strand P2-C | `VW_SO010_APP0` (both modules): its sparkle sprites drift with noise and stay in the node's space |
| T2.9 | loop end (`EnableLoopEnd`) on looping effects | strand P2-B | T1.10 again: the effect plays its authored ending instead of stopping |

### Tier 3: host features

GPU sprites (399 emitters; data verbatim), particle lights (162), tails (`TailPolygon` 91, `TailShape` 22 and
module `TailShape` 115), decals (64), post effects driven by modules (radial blur 20, scene colour 14), camera
shake modules (7), `_Seeded` exact sequences. Tests: glide speed lines (`vs_spd_lin2`: tail polygon following the
forearm); `vm_thd_str1` (lights, 2 decal emitters, GPU sparks); the dive attack (`DiveFallAttack`, clip 1050,
`vp_vld_sho00` from 1.038 s: radial blur, camera shake, 5 decal emitters on the ground); the death sequence
(`vs_com_dth_bra00`: one control emitter with `SQEX_ScreenBlur`, in the world at `kh_root`; `vs_com_dth_hrt00`
heart at `vfx_lit_mune` 0.1 s later).

---------------------------------------------------------------------------------------------------------

## 12. Cross-check of the strands

| question | result |
|---|---|
| attach socket and transform | weapon strand (code) and pack strand (asset data + swing statistics + render) agree: `R_wep_skt`, snap, identity; the pack's attach block quotes the code addresses |
| trail sockets | code-side chain (effect-system strand) and the socket positions of the weapon skeleton (pack) agree: `vfx_top` 79.04 cm, `vfx_bottom` 14.15 cm on bone `wep` |
| defaults the inventory could not read from assets | resolved by code: `SetWeaponVisible` defaults true / true (0x14055ACB0 `[I]`); `m_AppearEffectAttach` None (0x141777570 `[I]`); the function that spawns `m_AppearEffect` is 0x14178B320 (the inventory's candidates 0x140AE0D60 / 0x140C52940 are unrelated) |
| notify classification | the inventory classified by class name before `notify_classes.json` existed; checked afterwards: the only effect-spawning classes on Sora's clips are the two `AttachEffect` classes and one blueprint notify without template, as the inventory assumed; `Shot` and `Attack` rows are kept as pure triggers |
| hit attribute index | reflection enum `ETresDamageAttribute` has exactly values 0..7 + MAX in the order assumed |
| "other keyblade" rows | 68 of 128 resolved to `no` by the group rule, 60 remain `unknown` (group 0: clip selection decides); section 7.4 |
| converter labels for Square Enix modules | pack rebuilt after the semantics file was complete: 31 classes carry a `semantics` text, the census `support` column follows; `verify.js` rerun: 0 mismatches |
| requests of the other agents | merged; four water effects missing from the pack (7.5) |
| disagreement left open | trail length: the effect-system strand states "trail length in time = particle lifetime (0.2 s)" as stock behaviour without having located the AnimTrail instance code; the Required timeline fields (0.055 / 0.075 / 0.185) and `KillEmitterTime` 0.385 of the same asset are not explained by any strand |

---------------------------------------------------------------------------------------------------------

## 13. Address table

Verdicts are those of `sheets/impl/progress` at the time of the research (`same` = proven identical to the original;
`same-partial`, `differs`, `no-verdict`, `unconfirmed`, `-` = check the disassembly before relying on a `[C]` reading).
More than one address in a cell = functions of one group, verdicts in the same order.

| address | name | verdict | what it does | area |
|---|---|---|---|---|
| 0x141777570 | ATresWeaponBase ctor | same | subobjects, native defaults of all appear/disappear fields | weapon |
| 0x141783270 | weapon state table init | differs | 6 states with begin / tick / end pointers | weapon |
| 0x14178B320 | weapon SetVisible (slot 528) | same-partial | effect spawn, choose Appear / Disappear / immediate | weapon |
| 0x1417910E0 | weapon ShouldMeshBeVisible (slot 540) | - | visibility predicate | weapon |
| 0x14177D600 / 0x141784D50 / 0x14177E720 | Appear begin / tick / end | differs / same-partial / differs | section 2.4 (begin and tick read in disassembly) | weapon |
| 0x14177D960 / 0x141784FF0 / 0x14177E830 | Disappear begin / tick / end | differs / same-partial / differs | section 2.4 | weapon |
| 0x14177D7D0 / 0x141784EF0 | Destroy begin / tick | differs / same-partial | scale-out over m_DisappearWithScaleTime, ResetPose, destroy | weapon |
| 0x14177D950 / 0x141784FB0 | DestroyNext begin / tick | - / same-partial | destroy after the running animation flags clear | weapon |
| 0x14177DB30 / 0x14177E920 | FreeMove begin / end | differs / same-partial | thrown weapon | weapon |
| 0x141792140 | set "OpacityFade" | unconfirmed | scalar parameter on the weapon mesh | weapon |
| 0x1417921B0 | set mesh scale | unconfirmed | uniform relative scale, floor 0.01 | weapon |
| 0x143F0AA50 | Approach | same | linear move toward a target over the remaining time | weapon |
| 0x14178FBD0 | weapon Tick (slot 134) | differs | AntiVolume fade + state machine | weapon |
| 0x141788E30 | weapon PostInitializeComponents (slot 141) | no-verdict | init | weapon |
| 0x14178A380 / 0x14178A2E0 | AntiVolume apply / set | differs / same-partial | second fade scalar | weapon |
| 0x141792220 | charge effect by level | unconfirmed | groups 3000 / 3001 / 3002 | weapon |
| 0x14177C030 | weapon finish-now (slot 529) | same-partial |  | weapon |
| 0x14178F080 / 0x14178EF20 / 0x14177F050 | throw / throw / end throw (slots 536..538) | differs / differs / - |  | weapon |
| 0x14178BFE0 | weapon hit effect helper (slot 533) | same | calls 0x140628960 | weapon |
| 0x140561AF0 | TresAnimNotify_SetWeaponVisible::Notify | same | section 2.4 | weapon |
| 0x14055ACB0 | TresAnimNotify_SetWeaponVisible ctor | same | defaults true, true | weapon |
| 0x140561C20 / 0x14055AD30 | TresAnimNotify_ThrowWeapon::Notify / ctor | same / same | section 2.9 | weapon |
| 0x1405B3E50 / 0x1405B4680 | pawn anim-interface thunks +0x2E0 / +0x3C0 | same / same | forward to the state, then the pawn | weapon |
| 0x141448060 / 0x1405BA440 | pawn SetWeaponVisible (slot 616) player / base | same / same | section 2.4 | weapon |
| 0x1414480B0 | pawn HideWeapon (slot 619) | same |  | weapon |
| 0x141436BF0 | pawn slot 620 | same | weapon hidden and slot kind not 1 / 0x16 / 0x17 | weapon |
| 0x1405ADE80 | pawn IsWeaponOut (slot 614) | - |  | weapon |
| 0x141446CE0 / 0x1405AD050 / 0x141435A10 | battle flag setter / getter / battle-pose test | same / same / same |  | weapon |
| 0x141112B60 / 0x14144A5D0 / 0x141423210 | weapon timer set / decrement / pawn ctor (10.0) | - / - / differs |  | weapon |
| 0x1413944D0 / 0x1413AED70 | Idle tick / Run tick | no-verdict / same-partial | stance logic | weapon |
| 0x141219F60 | NPC locomotion battle start / end | differs | RunBattleStart names | weapon |
| 0x14178B670 / 0x14178B690 / 0x141784670 | equipment SetWeaponVisible / apply / getter | - / same-partial / - |  | weapon |
| 0x141791120 | equipment Equip (slot 190) | unconfirmed | section 2.2 | weapon |
| 0x14178E5D0 | equipment SpawnUnits (slot 195) | same-partial | section 2.2 | weapon |
| 0x14177C4A0 | equipment change weapon (slot 155) | differs |  | weapon |
| 0x14178EE30 / 0x14177ED40 | transform on / off | same / same-partial |  | weapon |
| 0x14177E4E0 | weapon attach | no-verdict | AttachToComponent with rules at 0x148750ED8 | weapon |
| 0x143E8FC60 / 0x1441B9170 | attach wrapper / SpawnActor | differs / same | engine | weapon |
| 0x140628960 | TresAtkCollComponent hit effect | same-partial | section 2.6 | weapon |
| 0x140643BD0 | IsMapHit | same-partial |  | weapon |
| 0x140616C90 | TresAtkCollComponent ctor | same-partial |  | weapon |
| 0x144118120 / 0x14411B3D0 | emitter spawn attached / at location | differs / same-partial | engine-side, identity not confirmed | weapon |
| 0x144042970 | set visibility on the mesh (arg 2) | same-partial | engine-side, identity not confirmed | weapon |
| 0x14178F400 / 0x14177F070 | equipment throw / stop throw | no-verdict |  | weapon |
| 0x141778520 | TresWeaponMovementComponent ctor | same |  | weapon |
| 0x14176c560 | `UTresAnimNotify_AttachEffect::Notify` | no-verdict | find / create attach component, start | effects |
| 0x14176c870 | `UTresAnimNotifyState_AttachEffect::NotifyBegin` | same-partial | same start | effects |
| 0x14176cb90 | `UTresAnimNotifyState_AttachEffect::NotifyEnd` | no-verdict | end by source object | effects |
| 0x141771790 | attach component: start from notify | same-partial | class dispatch -> lists | effects |
| 0x141771470 / 0x1417715f0 | `AttachDataList` / `MetaAttachDataList` walkers | same-partial | target resolution per row | effects |
| 0x1417623b0 | data-asset row | (not queried) | assets + designated data | effects |
| 0x141771320 | designated attach data of the target owner | same | all `DesignatedAttachDataList` assets | effects |
| 0x1417640d0 / 0x1417602b0 / 0x1417677e0 | target resolution helpers | same-partial / - / same | child mesh at socket, weapon list | effects |
| 0x1417719e0 | per attach-data list | same-partial | group override, theme filter, group loop | effects |
| 0x141764560 | per effect entry | not-compiled | weapons / area effects / plain | effects |
| 0x141762070 / 0x141762460 | spawn lambdas (notify / script) | differs / - | group filter, PlayNumber, append entry | effects |
| 0x14176a090 | fill runtime entry from data | differs | field copies | effects |
| 0x141765ec0 | spawn one effect | not-compiled | transform + spawn call (args read in disassembly) | effects |
| 0x141772cd0 | post-spawn set-up | same-partial | fade in, delay, colour, sockets, trail data | effects |
| 0x141766eb0 | common effect lookup | same-partial | key + group on owner's list | effects |
| 0x141773980 | end by source object | same | -> 0x141763e60 | effects |
| 0x141763e60 | end one entry | same-partial | end-type dispatch, detach | effects |
| 0x141764230 | component tick | same-partial | timers, list pruning | effects |
| 0x141770d30 / 0x141771460 | `Set` / thin wrapper used by the play API | same-partial / - | animation change | effects |
| 0x141773240 | `SpawnAttachEffect` | differs | script / code spawn from an ead asset | effects |
| 0x141765500 / 0x141765520 | `EndEffect` | - / same-partial | group end | effects |
| 0x141765940 | `FadeOutEffect` | same-partial | group fade out | effects |
| 0x141764eb0 | `DestroyEffects` | no-verdict | group destroy | effects |
| 0x141770890 / 0x1417718b0 / 0x14176b4c0 | `ResetEffects` / `SetEffectVisibility` / `IsComplete` | no-verdict / same-partial / same-partial |  | effects |
| 0x141770c40 / 0x1417749a0 | component slot 21 / TickComponent | same / differs |  | effects |
| 0x1405a0d70 / 0x1405a0e50 | pawn play asset / by name | differs / differs | call `Set` with the effect group | effects |
| 0x1405b9370, 0x1405a85f0, 0x1405a8b80, 0x1405a6360, 0x1405b9f80 | pawn iface +0x1A0..+0x1C0 | same / differs / differs / differs / same | spawn / end / end-fade / destroy / spawn stop | effects |
| 0x1405b8c30 / 0x1405a84b0 / 0x1405b8be0 | pawn common slots | same-partial / differs / same | set / end / bad-state | effects |
| 0x1405b77b0, 0x1405a8850, 0x1405aff90 | frozen / bad-state handling | (0x1405b77b0 fade constant read in disassembly) / same-partial / same-partial |  | effects |
| 0x1442626d0 | `UParticleSystemComponent::EnableLoopEnd` (vt+0xAE8) | unconfirmed | loop end | effects |
| 0x1442633a0 / 0x144262ff0 | PSC `FadeOut` / `FadeIn` | unconfirmed (disassembly read) | fade rate set-up | effects |
| 0x144270d30 | PSC `TickComponent` | differs | delay, fade, stock tick | effects |
| 0x14425e860 | PSC deactivate / destroy | same-partial |  | effects |
| 0x144268800 / 0x14426acc0 / 0x14426abe0 | PSC `IsEnd` / `ResetEffect` / init of the SQEX fields | unconfirmed |  | effects |
| 0x14426fc60 / 0x14426cfa0 / 0x14426dbe0 / 0x144270350 | PSC set sockets / set alpha / float param / vector param | unconfirmed / unconfirmed / unconfirmed / same |  | effects |
| 0x14411b7a0 / 0x14411b280 / 0x144118020 / 0x144118120 / 0x14411b3d0 / 0x1440fd450 | spawn primitives | same / same / same / differs / same-partial / same | section 4.1 e | effects |
| 0x144294000 | emitter instance: cache SQEX modules | same |  | effects |
| 0x144294b90 / 0x144277b40 / 0x1442a17a0 / 0x1442a1450 / 0x1442a1380 | emitter transforms / helper / gravity / colour takeover / per-tick driver | differs / differs / differs / differs / same |  | effects |
| 0x14427c760 | emitter timeline predicate | differs |  | effects |
| 0x14437a3b0 | `SQEX_Parent::Update` | differs | child emitters | effects |
| 0x1442f20c0 / 0x1442fb700 | `SQEX_AttachParticle` Spawn / Update | same-partial |  | effects |
| 0x1442f2dd0 / 0x1442fd7b0 / 0x1442ef550 | `SQEX_ParticleRotation` Spawn / Update / core | differs / no-verdict / differs |  | effects |
| 0x1442f2650 / 0x1442f2770 | `SQEX_DepthOffset` / `SQEX_InitSizeScale` Spawn | null / null |  | effects |
| 0x144375a40 / 0x144382fe0 | `SQEX_Range` placement / `TypeDataSQEX_Control::CreateInstance` | differs / same |  | effects |
| 0x1405b05d0 (thunk 0x1405b05c0) | pawn foot-step / water effects | differs | section 4.1 d | effects |
| 0x1405caff0 | mesh collision foot-step detection | differs | not traced | effects |
| 0x14129e010 / 0x140eca220 / 0x140ecdd70 | physmat table build / unit lookup / override merge | same / same / same |  | effects |
| 0x14176cd60 / 0x14176cda0 / 0x1417671c0 | EffectCurve slot 72 / tick / evaluation | same / same-partial / differs |  | effects |
| 0x140563c20 / 0x140565c30 / 0x140e6bb60 / 0x140e51e10 / 0x1405eaf70 | LensEffect begin / end / add / end / remove | same / same / same-partial / same / same |  | effects |
| 0x140563330 / 0x1405654a0 / 0x140e6b600 / 0x1405f1760 / 0x140e51af0 / 0x1405eb030 | CameraPosEffect chain | same / same / same / same-partial / same / no-verdict |  | effects |
| 0x140566cd0 / 0x140561e90 / 0x140567240 / 0x140564a70 | Alpha slot 72 / begin / tick / end | same / differs / differs / differs |  | effects |
| 0x1405b3f90 / 0x1405b46c0 / 0x1405b2cc0 | pawn iface alpha forwarders | same | to the state interface | effects |
| 0x140563510 / 0x140565600 | ChangeMeshMaterialParam begin / end | no-verdict |  | effects |
| 0x140561af0 / 0x140561a50 / 0x140561940 | SetWeaponVisible / SetMeshVisible / SetActorVisible | same |  | effects |
| 0x1405b3e50 / 0x1405b3e10 / 0x1405b3d00 / 0x141448060 / 0x1405ba440 | visibility chain on the pawn | same |  | effects |
| 0x1405fa570 / 0x1405fbff0 | camera fade apply / update | no-verdict | `CameraFade` + PSC alpha | effects |
| 0x1405d1cf0 / 0x1405d2130 / 0x1405d2040 / 0x140e5c570 / 0x140e5c5a0 | material kind setters / name tables | same |  | effects |
| 0x140e6ad70 / 0x1405b6a00 | `OpacityFade` on pawn + weapons / form-change parameters | differs / no-verdict |  | effects |
| 0x1417747d0 / 0x14176be90 | `UTresDecalComponent` tick / material set-up | differs / same-partial |  | effects |
| 0x1413733d0, 0x141390b50, 0x1413347f0, 0x1413aed70, 0x141446ec0, 0x1414471a0, 0x141447290, 0x141415c30 | player call sites (1d) | no-verdict / same-partial / same-partial / same-partial / same-partial / same / same-partial / same-partial |  | effects |
| 0x144e18ce0 | Construct of `SQEX_AttachEffectNotifyDataToMetaAttachData` | null | group default -2 | effects |
| 0x1445ceb80 | world "diving" test | no-verdict | attach theme filter | effects |
| 0x144B43ED0 | InternalConstructor<UParticleModuleRequired> | none recorded (status decompiled) | entry the emulator runs for the Required defaults; calls 0x1442B03B0 | converter |
| 0x144B6BE60 | class constructor of ParticleModuleSpawn | not looked up | Spawn defaults | converter |
| 0x144C01B60 / 0x144C01B40 | class constructors of ParticleSystem / ParticleSpriteEmitter | not looked up | system and emitter defaults | converter |
| 0x144BB0CD0 | class constructor of ParticleModuleTypeDataMesh | not looked up | mesh type-data defaults | converter |
| 0x144B6BCA0 | class constructor of ParticleModuleSQEX_EmitterTransforms | not looked up | defaults of the most used Square Enix module | converter |
| 0x14478FC50 | class constructor of DistributionVectorUniform | not looked up | `MirrorFlags` default `EDVMF_Different` ×3 | converter |

---------------------------------------------------------------------------------------------------------

## 14. Open questions

Grouped by what they block. "P2-x" names the ready-to-launch research strand in `build/sora_tmp/vfx/P2_STRANDS.md`.

Effect runtime (blocks tier 2):
1. **Emitter timeline for time type 0** (P2-B): what the predicate 0x14427C760 gates in its 13 callers, how emitter
   time advances and wraps with `EmitterStartTime` / `EmitterLoopTime` / `EmitterLoopDuration` /
   `EmitterDurationTime` (with `bUseLegacyEmitterTime` false on every emitter), how this replaces stock
   `EmitterDuration` / `EmitterLoops`, and where `SQEX_AllParticleKill.KillEmitterTime` is consumed.
2. **Loop end** (P2-B): the exact effect of the loop-end flag per emitter type for time type 0 (does a looping
   emitter stop spawning at once, finish its loop, or play an ending).
3. **Transform chain** (P2-A): `SQEX_EmitterTakeover` / `SQEX_ComponentCutoff` position, rotation and scale
   inheritance (0x1442A0CA0, 0x1442A57C0, 0x144280490, 0x144284A80), `bLeavePlaceInWorld`, the `DisplayType`
   billboard modes and `Velocity` of `SQEX_EmitterTransforms`.
4. **Untraced module bodies** (P2-C): `SQEX_Range` point distribution, `SQEX_Ring`, `SQEX_CurlNoise` update,
   `SQEX_LocalParticle`, `SQEX_Parent` PARTICLE_DEATH condition, `SQEXDistribution*` sampling, tails
   (`TailShape`, `TailPolygon`), `SQEX_Material_*`, blur and scene-colour modules.
5. Are `Spawn` / `Update` of the stock modules unmodified? Classes that carry Square Enix properties (Size,
   SizeScale, ColorScaleOverLife, ParameterDynamic, Spawn, TypeDataMesh) are certainly touched (P2-C).
6. That the shipped executable samples the baked lookup table is assumed stock behaviour, not read in code (P2-C).
7. How the component fade value, colour and alpha (+0xB70..+0xB84) and the emitter colour multiplier reach the
   vertex colour; `Required.EmitterBlendType` against the material's blend mode (P2-A).
8. Location of the AnimTrail instance code; `TilingDistance` / `WidthTessellationStepSize` defaults; whether the
   trail length is the particle lifetime as in stock; the role of the trail asset's timeline fields (P2-B).
9. `SqExParticleFlag` bits; detach rules at 0x148750EE0; hit-stop and object-type time dilation on effects; the
   meaning of owner vt+0x668 in the component tick.

Triggers (blocks rows marked `unknown`):
10. **Group override and extra groups** (P2-D): default and writer of mesh+0x864; whether any code adds groups to
    the component's current list (the 3200 rows of 7.4); the remaining non-zero groups on generic effects (262,
    283, 314, 520, 2500..2700, 3192, 3196, 10000).
11. **Direct spawns from player states** (P2-D): Run state world effect, AirSlide groups 100 / 13 and their asset,
    guard / just-guard / reflect, hop, athletic hit, swan dive, focus aspir, style-change groups, group 0xD4D,
    critical counter; calls through the pawn interface are missing from the call index.
12. Which code selects a world physical-material table over the common one; speed thresholds
    (`m_NmlSpeedParam` / `m_HighSpeedParam` are 0 or unset in most rows); the body-in-water block and the
    reference height of 0x1405B05D0; whether landing uses the Enter slot (P2-D).
13. Caller of the hit-effect function 0x140628960 for a blade hit: attribute byte, group name, `m_HitEffSet`
    index, direct-hit flag (belongs to the combat spec; the mapping here is `inferred`).
14. Item id → `ead_com_abn0` group mapping; cure area groups; the FName effect `sora_kaihuku`.
15. Which `StyleChange` clip the Kingdom Key's Second Form transition plays; whether an aura persists in the form;
    who raises the charge level (groups 3000..3002).
16. Area custom effects (`bUseAreaEffect`, 37 entries): the provider behind game state vt+0x958.
17. The 60 other-keyblade rows with `GroupID` 0 inside shotlock and `StyleChangeN` clips: confirm the Kingdom Key
    never plays those clips.

Weapon:
18. Which pawn function equips the initial keyblade and what picks the grow level at run time.
19. How the weapon follows later pawn animations by name; loop flags of the weapon clips; who uses the
    `rt_holo_dlc` anim blueprint.
20. Identity of 0x144118120, 0x14411B3D0 and 0x144042970 against stock UE (verdicts `differs` / `same-partial`).
21. Material side of `OpacityFade` (dither pattern) and `AntiVolume`; opacity-mask input of `w_allBaseMat`.
22. Hand switching and cutscene weapon tracks (`TresInterpTrackWeaponControl`) were not examined; unit and
    semantics of `m_Size` of the attack shapes; the other states that call the show function directly.
23. Whether the keychain simulation is reset when the weapon appears; teleport distance.

Only the running game can settle (also in the probe table):
24. **Level up and low HP**: no particle asset found. Is there an effect at all?
25. **Landing puff**: none in the data for the common table. Does the game show one on plain ground?
26. **Hit flash / invincibility blink / damage dissolve** on Sora: not found in code. Confirm visually.
27. That the plain Keyblade is weapon kind 1 and plays with effect group 0 (basis of column `kk_base`).

## 15. Runtime-probe requests (object, offset, type, why)

The read-only probe (`docs/sora-runtime-probe.md`) takes these as field requests.

| object | offset | type | why |
|---|---|---|---|
| Sora pawn `MyEffectAtt` (pawn+0x5A8) | +0x458 / +0x460 | ptr / i32 | current animation group list per frame during idle, attack, wall run, guard, cast: settles the group rule and the 3200 rows (q. 10, 27) |
| same | +0x468, +0x46C | i32, i16 | current group, spawn-stop flag |
| same | +0x448 / +0x450 → entries (stride 0x68): +0x28 weak component, +0x30 i32, +0x55 u8, +0x56 u8, +0x58 u16, +0x60 ptr | mixed | tracked effects per frame (template, group, change type, end type, flags, source): ground truth of the inventory per action (q. 11, 24, 25) |
| Sora mesh component | +0x864 | i32 | group override (expect -2) (q. 10) |
| Sora pawn interface (pawn+0x440) | +0x4C0..+0x4D0 | 5 × i32 | running groups of the common slots (aura, magic cast, bad state, appear, status) |
| equipment component (pawn `MyEquipment`, pawn+0x598) | +0x378, +0x224, +0x229, +0x22A, +0x2C8 | i32, i32, u8, u8, u8 | current slot, item id, weapon form, grow level, flags (bit1 weapon out): weapon kind of the plain Keyblade and Second Form (q. 27) |
| weapon actor (`m_WeaponEquips[0]`) | +0x674, +0x6F0 | u8, i32 | visible flag and state id during BattleStart / BattleEnd (expect 1 then 0; 2 then 0) |
| weapon actor | +0x748, +0x73C, +0x744, +0x74C, +0x6A4 | f32, f32, f32, u8, f32 | delay timer, dither value, time left, flags, rate: confirms 0.22 s + 0.06 s |
| weapon actor | +0x758 | ptr | disappear effect component lifetime (length of the Disappear state) |
| weapon actor | +0x680, +0x688 | ptr, FName | attach parent and socket (hand-switch check) |
| weapon actor | +0x66C, +0x670 | i32, i32 | charge level and running group in Second Form (q. 15) |
| weapon mesh component (weapon+0x570) | +0x2F0, +0x2FC, +0x330 | FVector, FRotator, FVector | relative location / rotation / scale: expect 0 / 0 / 1 (attach identity) |
| weapon `MyEffectAtt` (weapon+0x580) | +0x418 / +0x420, +0x428 / +0x430 | ptr / i32 | common effect list and designated data at run time |
| trail `UParticleSystemComponent` (from a tracked entry) | +0xB6C u32, +0xB70 f32, +0xB74 f32, +0xB84 f32, +0xB88 f32, +0xBD4 i32 | mixed | fade flags / value / rate during the 0.15 s fade, alpha, start delay, group id |
| same, emitter instance 0 (component+0xB00[0]) | +0x8E0 FName, +0x8E8 FName, +0x8F0 f32, +0x8F4 i32, +0x900 u16, +0x66C u32, +0x12C f32 | mixed | trail sockets, width, width mode, enabled flags, loop-end flag, emitter time (q. 1, 8) |
| a live emitter instance of `VW_SO010_APP0` | +0x12C f32, +0x66C u32 per frame | mixed | emitter time and loop-end flag over the effect's life (q. 1, 2) |
| Sora pawn | +0x10E0, +0x630, +0x3C4 | f32, u32, f32 | 10 s weapon timer, battle flag (bit1), camera-fade value |
| Idle state object | +0x8C | f32 | 3 s battle-stance timer |
| `ATresGameState` | +0x1208 → +0x48 + 33 × 0x38 (56 bytes) | struct | live physical-material unit for surface 33 in the test area (q. 12) |
| Sora mesh foot unit (mesh+0x11B0 array) | +0x380 f32, +0x38C f32, +0x394 u32 | mixed | foot contact timers and flags (q. 12) |
| class default objects named in `undetermined_defaults` of the effect JSON (e.g. `Default__ParticleSystem` +124 `FVector` SQEXBaseScale) | reflected offset | per property | constructor copies these from globals |
| world: every `UParticleSystemComponent` owned by the pawn or its weapon | template path, attach socket, +0xBD4 | mixed | per-action ground truth, including level up, low HP, landing, prize pickup (q. 24..26) |

## 16. Files and how to regenerate

```
node tools/sora_vfx_sheet.js [--import]   the four sheets from audit/sora_port/vfx/ (--import copies the strand row files first)
node tools/sora_vfx_pack.js               build/sora_pack/vfx/ (after the sheet tool: it copies the trigger table)
node tools/sora_vfx_pack.js --check       validate the effect pack on disk
node tools/sora_vfx_pack.js --census --game   module census (rows_modules.json in build/sora_tmp/vfx/cascade)
node tools/sora_vfx_host.js                 build/sora_pack/vfx/host.json (after every pack run: the pack run deletes it)
node tools/sora_vfx_shaders.js              build/sora_pack/vfx/shaders/ (the materials' pixel shaders translated from the cooked DXBC)
node tools/sora_vfx_assets_rows.js          the vfx/ rows of sheets/port/sora-assets.json (pack rows + host.json + shaders) without the main pack run
node build/sora_tmp/vfx/cascade/verify.js independent value round trip of the effect JSON
node tools/sora_weapon_pack.js [--check]  build/sora_pack/weapon/
node tools/sora_pack.js                   main pack; merges weapon/ and vfx/ sheet_rows.json into sheets/port/sora-assets.json
node tools/extract.js && node tools/preflight.js --summary
```

| file | role |
|---|---|
| `docs/sora-vfx-spec.md` | this document (assembled by `build/sora_tmp/vfx/assemble_spec.js` from `spec_body.md`, the strand address tables and `spec_tail.md`) |
| `sheets/port/sora-effects.json`, `sora-weapon.json`, `sora-vfx-params.json`, `sora-vfx-modules.json` | the sheets (declared in `tools/lib/sheet_defs.js`) |
| `audit/sora_port/vfx/*.json` | evidence row files and companion data: `rows_effects`, `rows_weapon`, `rows_fxsys`, `rows_modules`, `sqex_modules` (123 module classes with properties and semantics), `notify_classes`, `attach_structs`, `physmat_effects`, `weapon_effects_seen` |
| `audit/sora_port/vfx_class_defaults.json` | class defaults of 263 particle classes from their constructors |
| `tools/sora_vfx_sheet.js`, `tools/sora_vfx_pack.js`, `tools/lib/vfx_class_defaults.js`, `tools/sora_weapon_pack.js` | tools |
| `build/sora_pack/weapon/`, `build/sora_pack/vfx/` | the packs (local build output, never distributed) |
| `build/sora_tmp/vfx/<strand>/` | research scratch: strand reports, scripts, the out-of-scope notify rows, the game-wide scan caches |
| `build/sora_tmp/vfx/P2_STRANDS.md` | follow-up research prompts |

State of the sheets at hand-over: `node tools/preflight.js --summary` reports no error and no dangling reference
for the four sheets; unchecked cells: `sora-effects` 42 `effect` + 4 `attach` (entries without effect or without
placement data, and the three "none found" markers), `sora-weapon` 1 and `sora-vfx-params` 1 `value` (not
determined), `sora-assets` 4 `loop` (weapon clips).

## 17. Corrections and additions of the third effect pass (2026-10-12)

Readings of this pass; `[C]` decompiled C, `[I]` instruction level, as in section 1. They replace what sections 5, 8.3
and 14 say where they differ.

**Emitter instance vtable** (found through the pointer to 0x144290A60, e.g. file offset 0x77030A8): +0x248
0x144290A60 position / rotation, +0x250 0x1442A0CA0 component part, +0x278 0x14427A150 node display (not read),
+0x2C8 0x144284A30 emitter time, +0x2D8 0x1442881B0 spawn point, +0x398 0x1442A3E80 scales, +0x400 0x1442859C0. Driver
0x1442A1380: tracks (0x144294B90), component, colour (0x1442A1450), scales, position / rotation. Module cache: +0x720
EmitterTransforms, +0x728 InitializeTransforms, +0x730 Takeover, +0x738 ComponentCutoff, +0x740 InitSizeScale, +0x748
Vertex, +0x750 LocalParticle, +0x758 ColorScaleOverLife, +0x760 AttachParticle, +0x768 SizeScale, +0x770 Lifetime, +0x778
CurlNoise. Property offsets used: Required +0x60 mask 2 `bLeavePlaceInWorld`, +0x7C `RotationPriority`, +0x7D
`ScaleSpaceType` (LOCAL on all 3,500 emitters of the pack), +0x94 mask 1 `bUseLocalSpace` / mask 2
`bUseEmitterLocalSpace`; Takeover +0x50 mask 4 `bLeavePlaceInWorld`, +0x54 .. +0x59 Component Position / Rotation /
Scale, Parent Position / Rotation / Scale, +0x5C ParentParticleSizeScale, +0x66 ParticleSizeScale; Cutoff +0x54 rotation,
+0x55 scale; template +0x7C `SQEXBaseScale`, +0x88 mask 1 `bUseBaseScaleBugFix`, mask 0x40 `bUseOwnerRotaion`.

**Node chain** `[C]`. Every matrix of the chain is rotation + translation. Per instance: component matrix +0x1F0 with
its first-tick copy +0x240 / +0x5B0 (a child copies its parent's), component scale +0x230 (first-tick +0x280), local
matrix +0x2D0 (own x parents), node matrix +0x60 (`EmitterToSimulation`), `SimulationToWorld` +0xA0 (identity in world
space, the component matrix in local space), chain scale +0x390, world scale +0x39C, particle size multiplier +0x4AC
(for children) and +0x4B8 (applied).
- Component: leave-place (Required or Takeover flag, or the parent's) = the first-tick matrix; Cutoff rotation
  SPAWN_TIME_ONLY = first-tick rotation, NONE = translation only; Cutoff scale NONE = 1, SPAWN = first-tick.
- Scales: chain = init x track x parent chain (ParentScale takeover: live / snapshot / 1) x child offset scale; world =
  component (1 with `bUseLocalSpace`) x chain x `SQEXBaseScale`; size multiplier = (init x track unless
  ParticleSizeScale takeover NONE) x parent's multiplier (ParentParticleSizeScale takeover) x child offset x
  SizeScale(emitter time, when that module runs on emitter time) x `SQEXBaseScale`.
- Position = track + init + `EmitterOrigin` + child offset, x world scale with `bIsUseTransformScale` (2,464 of 2,518
  transform modules); + the `Velocity` sum x component scale x parent chain scale. With a parent: position through the
  parent's local matrix, rotation = own x parent's (each NONE / ALWAYS / SPAWN snapshot; no Takeover module = ALWAYS).
  World space: node = local x component (Cutoff rotation NONE: + translation only); ComponentPosition takeover NONE /
  SPAWN = component origin + the own position unrotated / turned by the first-tick component.
- Spaces: `bUseLocalSpace` alone = particles in the component's space (node = `EmitterToSimulation`); with
  `bUseEmitterLocalSpace` = in the node's own space, to the world through 0x144284A80 = local x scale(world) x
  scale(component) x component.
- 0x1442A57C0 and 0x144280490, named in sections 8.3 and 14 as the untraced transform chain, are not part of it (the
  first is called from the tail path 0x1442A1AE0, the second from module helpers).

**Spawn point** (vt+0x2D8 0x1442881B0 `[C]`): (point x shape scale x world scale) x matrix, then turned by the node;
not turned with `bUseEmitterLocalSpace` or `SQEX_LocalParticle.bIsPosition`; `bApplySpawnLocationOwnerScale` false =
without the world scale.

**`SQEX_LocationBase`** (slots 110 .. 115; Range / Ring / Quadrangle add 116 point, 117 placement):
- Deviation, slot 115 0x144352710 `[I]`: r1, r2 random; `(1 - d) < r1 ? f = (1 - d) r2 + d : f = d r2`; radius = sqrt(f) x
  radius. d = 1 the surface, 0.5 uniform over the area, 0 the centre. (Pack: 0.5 on 265 ranges, 1 on 163.)
- Velocity, slot 111 0x14435B130 `[C]`: location += point; `VERTEX_NORMAL`: velocity += normal x NormalDirection x Speed
  x s; `EMITTER_NORMAL`: normal = the normalised point; `NONE` / `VELOCITY`: nothing. s = component scale x
  `SQEXBaseScale` with `bApplyOwnerScale` in plain world space, else 1 (or the base scale with the bug-fix flag). Speed
  is sampled at the particle's own time.
- Range 0x144356A00: a cap of half angle `DirectionAngle` about +X turned by `Direction`; its vertex normal is
  `Direction`'s X axis. Ring 0x14434FCB0 / 0x1443484B0 / 0x144355000: corner k = (radius, 0, 0) turned
  k x `DirectionAngle` / `VertexNumber` about **Z** + `RingOffsetPosition`; a point lies `j / Interval` of the way to the
  next corner; `RangeRadiusTwo`: corners picked between the two rings; x `RingOffsetScale`, turned by
  `RingOffsetRotation` x `Direction`; counters j, k per instance (RANDOM draws both); vertex normal = the ring's axis.
  Quadrangle 0x1443565C0: one face of the box of half extents sqrt(f) x `QuadrangleSize`.
- Mesh rotation, slot 113 0x14436F560 / 0x144356E70 (tangent mode): the inverse of a look-at matrix along the spawn
  direction (nudged by (0.01, -0.01, 0.001)), up = `Direction`'s Y axis: the mesh's Z looks outward.

**`SQEX_CurlNoise`** Update 0x1442FC8C0 `[C]`, 0x1442E1920 `[I]`, 0x1443B8F30 `[C]`: velocity += node x
(N((x + t) sx, y sy, z sz), N(x sx, (y + t) sy, z sz), N(x sx, y sy, (z + t) sz)) x Intensity, with (x, y, z) =
(location + offset) x PositionScale, t = TimeScale x seconds since creation, s = `CurlNoiseScale`, offset = three
randoms x 100 per instance (0x144289180), N = Perlin's improved noise with the reference permutation (table
0x147763A50). `TimeScaleAxisType` X / Y / Z: the time on that one axis in all three samples.

**`SQEX_LocalParticle`** Spawn 0x144373420, post-spawn 0x14428DA00 `[C]`: payload = the location the earlier modules
gave (unrotated); location = node origin + payload (turned by the node with `bIsLocalRotation`); the velocity moves
the payload. Update 0x144378C00 handles `bIsRotation` (mesh rotation with the node) only.

**Sizes** `[C]`: `Size` 0x1442F5840 adds StartSize(emitter time) with no component scale; `SizeScale` 0x1442F38D0 /
0x1442FEE70: Size = BaseSize x SizeScale(particle time) x multiplier (+0x4B8), the value counted as 1 with
`bUseEmitterTime`; `SQEX_InitSizeScale` 0x1442F2770 and `SQEX_ParticleScale` 0x1442F30F0 multiply by +0x4B8 too.
`Location` 0x1442F3B80: turned by the node, not scaled. `Velocity` 0x144335AD0: x world scale with `bApplyOwnerScale`.

**Decal** 0x144281110 `[C]`: a decal component at the node; rotation = `DecalRotation` x node; `DecalSize`(emitter
time) x world scale = the component's `DecalSize` (stock half extents, X = depth); `bItUseScaleRotMatrixTransfrom`:
size = |size x R(DecalRotation)|; colour, opacity, custom parameters and Z fade written as material parameters, all
sampled at the emitter time (seconds, not normalised).

**`SQEX_CameraShake`** Update 0x1442FC5E0 `[C]`: section 10's request; once per instance from `ShakePlayTime`; scale x
((Min - 1) x clamp((distance - Offset) / (Distance - Offset)) + 1) when Min < 1; play space camera local.

**Material texture indices** (tool): `TextureIndex` of a uniform texture expression counts the default texture of
`TextureObject` / `TextureObjectParameter` / `TextureSampleParameter2D` expressions whose `Texture` is not serialized,
also inside /Engine/ material functions (`TwoSidedTexturing`).

**Trail** (section 6): the 26 clips with the trail notify are 1141, 1142, 1144 - 1147, 1183, 1295, 1299, 1301, 1303,
1306, 1308 - 1314, 1316, 1318 - 1320, 1401, 1655_2. The level-99 ground combo plays M_1360, M_1359, M_1361 and 1305,
which carry none.

**The pawn's `vt+0x23C8`** 0x1405A8610 = `EndEffect` of common slot 0 (aura): its running group is pawn+0x900,
written by 0x141442930 (a per-state switch that picks the aura group, e.g. 0xC62).

Open after this pass: section 14 items 1, 2 (child looping), the display types, tails' twist / wave / inertia, the
count of `EVARTEX_ODER_TYPE_ALL`, `SQEX_VertexSpawn`, `SQEX_SphereForce`, sub-mesh offsets, particle collision, the
per-tick writer of a local particle's location, the renderer's use of the component scale (taken as stock).

## 18. Corrections and additions of the fourth effect pass (2026-10-12)

Tags as in section 1; `[D]` = derived from the data, here also checked against the user's captures of the real game
(`build/sora_tmp/ref/`, Thunder and Water).

- **Particles without a lifetime** `[D]`: 1,298 of the pack's 3,500 emitters have no `Lifetime` module; their particle is
  the emitter's own body (`SQEX_AttachParticle`, colour by emitter time) and every one of them sets `bKillOnCompleted`.
  Such a particle ends when the emitter's timeline ends. (The fork's `HasCompleted` was not traced; stock 4.17 would keep
  the emitter alive while a particle lives.) Without this rule a system never completes: the Ragnarok charge star stayed.
- **Node rotation** `[C]` 0x14427DAA0 / 0x14427D630 / 0x1404BFA00: per axis in `RotationPriority` order, each step an
  `FRotationMatrix` of one component of (track + init) + `Required.EmitterRotation` (raw floats, +0x70) + instance+0x64C.
- **Mesh billboards** `[D]`: `SQEXBillboardType` X/Y/Z_CAMERA_BILLBOARD turns about the component's axis; with
  `bUseSQEXBillboradRotaionAxis` about the node's rotated axis; `SQEXBillboardRotaionOffset` is applied inside the
  billboard frame. Type data offsets: +0x58 type, +0x5C offset, +0x68 flags (reader not located).
- **Ring of order ALL as a parent's spawn shape** `[D]`: one child per vertex and generation.
- **Fire effects of a spell actor** `[C]` (magic spec B 1, A 1.4 step 8): played at the FIRE position of the caster's
  call (the muzzle), not at the actor; for the direct-position spells the two differ.
- **`SQEX_ChangeSceneColor`** `[A]`: all 15 users of the pack set only `LightIntensity` (1 -> 0.3 -> 1 over the emitter's
  time; every flag false). The capture shows the whole frame, sky included, near black at 0.3: the port squares the value.
- **Colour pipeline**: one filmic curve over the blended frame (camera), effects in scene-linear colour.
- **Regenerate order**: `sora_vfx_pack.js` -> `sora_vfx_host.js` -> `sora_vfx_extra_textures.js` (new) -> `sora_vfx_shaders.js`.

## 19. Corrections and additions of the fifth effect pass (2026-10-12)

Tags as in section 1; `[S]` = read from the compiled shader of the cooked material (D3D bytecode disassembled).

- **Mesh-particle vertex shader** `[S]` (base pass, `FMeshParticleVertexFactory`; e.g. kmt_wat_01a). Inputs ATTRIBUTE0
  position, 1 / 2 tangent X / Z (packed), 3 vertex colour, 4.. UV sets, 8..10 the instance's three transform rows, 11
  sub-UV parameters (integers), 12 sub-UV blend + relative time, 13 dynamic parameter, 14 particle colour, 15 velocity.
  Its uniform buffer: row 2 xy = UV scale, row 3 = (UVType: 1 turns (u, v) to (v, -u), 2 to (-v, u); bTexUReverse;
  bTexVReverse: each negates its coordinate before the turn), row 4 = pivot offset added to the position, row 5 x = depth
  offset (moves the vertex toward the camera along its view ray, keeping its screen position; clamps at the near
  plane), row 5 y = emitter seconds (handed on in COLOR3.z). Outputs: TEXCOORD10 / 11 world tangent X / Z (rows applied,
  normalised; the NORMAL is turned by the rows, not by their inverse transpose), TEXCOORD0 = the turned UV, COLOR0
  vertex colour, COLOR1 particle colour, COLOR2 dynamic parameter, TEXCOORD7 vertex fog.
- **World position offset** `[S]`: computed between the instance transform and the projection, in translated world
  space. Water burst dome (kmt_wat_01a): offset = normal (rows applied, so it grows with the instance scale) x 30 x
  DynamicParameter.x ("WPO": 2 -> 0.6 -> 0.3 over the particle's life) x saturate(2 x tex(uv)) x (0.4 x A(uv + t x (3, 4))^2 +
  B(uv - t x (1.3, 2))^2), t in seconds. Its pixel shader: alpha = (fresnel^2.5 + two scrolling noises) x mask^2 where the
  mask texture is read at (2u, 2v + DynamicParameter.z "mask_UV": 0.25 -> -2): a wipe with a dripping edge; holes where a
  blotch texture is 0; "Break Power" (DynamicParameter.y) is 0 on the burst. The Thundara / Thundaga bolt jag is the same
  mechanism in omt_thd_wld_00sk.
- **No sprite-like vertex shader of the pack computes an interpolant from the material** `[S]` (all 318 shader owners
  analysed): UVs, colours and the dynamic parameter reach the pixel shader as the factory hands them. 20 owners have a
  world position offset in the sprite-like factories.
- **Distortion pass** `[S]` (stock 4.17 layout): offset = view-space normal.xy x (IOR - 1) x (P00 W, P00 ratio H) x
  0.00023, faded by the scene depth behind (RefractionDepthBias), written x 4 as (+x, +y, -x, -y); IOR = lerp(Rif Min,
  Rif Max, the base pass's opacity) on the water materials. The loose constant `DistortionParams` sits at a different
  slot in every shader.
- **Ground decals** `[S]`: the water decals multiply their opacity by saturate(receiver normal . (0, 0, 1) x ...): they
  only show on upward surfaces. The receiver normal is read from GBufferA with sample_l.
- **Dynamic parameters / drive modules** `[A]`: ParticleModuleParameterDynamic reaches every translated shader
  (COLOR2 for meshes, TEXCOORD1 for sprites). None of the 714 emitter materials of the magic set uses an
  `SQEX_Material_Scalar / Vector / InLight / Aura` module (48 / 5 / 38 / 20 users in the whole pack: they target the
  owner character's materials, not the particle's).
- **Water without a target** `[C, A]`: the bolt starts 60 degrees up at Sora's root + (50, 0, 100) and homes to the point
  1000 cm below itself after 0.0667 s: in the port it rises above his head and lands about 4 m ahead of him 0.43 s after
  its launch, which is the geometry of the user's capture (no lock-on there).
- **Regenerate order** unchanged: pack -> host -> extra_textures -> shaders. `sora_vfx_shaders.js` now also writes
  `kh_vertex` (def KH_VS) and `kh_distort` (def KH_DISTORT) per owner and the flags `vs`, `vs_why`, `distort`,
  `vs_material_outputs` in shading.json.

Open after this pass: world position offset of sprite-like factories; the 13 mesh vertex shaders not translated
(integer sub-UV input, packed outputs, more than 10 textures); distortion of effect layers behind a refracting
surface; filter / radial blur; the tail polygon's vertex builder (width convention: the cast band of Water is thinner
than the capture's); GPU sprite spawn rate (taken as stock, per second; not traced); DBM_Translucent decals write only
their colour (no normal / roughness: the water's wet mark is nearly invisible).
