# Sora audio specification

What the player character Sora (`p_ex001`, pawn `p_ex001_Pawn` → `p_ex001_PawnBase` →
`TresPlayerPawnSora`) makes audible, and how a host must play it to sound like the game.

- Data: `build/sora_pack/audio/` (`manifest.json` + 1,714 WAV files, `tools/sora_audio_pack.js`).
- Sheets: `sheets/port/sora-sounds` (2,962 rows, one per trigger and sound),
  `sheets/port/sora-sound-rules` (60 constants), audio rows of `sheets/port/sora-assets`.
- Formats and decoders: `docs/formats/sead_audio.md`.
- Research data the pack is built from: `audit/sora_port/audio/*.json`.

Evidence marks: **[I]** read at instruction level; **[C]** decompiled C only (the verdict of
the function in `sheets/impl/progress` is given where it is not `same`); **[A]** read from a
converted asset, a bank or the SEAD configuration; **[UE]** stock UE 4.17 behaviour;
**[inf]** inferred. Addresses are virtual addresses in `base/KH3.exe`. The four research passes
behind sections 2 to 5 marked their own statements; the lead pass re-checked the data side
(every asset path resolves, every stream decodes) but did not re-read their disassembly.

## 1. Overview

The game does not use Unreal's mixer for its sounds. A sound is a `SQEXSEADSound` asset (a
`USoundWave` subclass) that names a sound **index** inside a SEAD **bank** (its own embedded one,
or `ReferenceBank` + `SoundIndex`). It is started through the ordinary UE audio-component API;
the Square Enix sound source then creates a sound in the native driver `SQEX::Sd`, which does
everything audible: picking the variation, envelopes, random pitch, distance falloff, panning,
category limits [C `0x145814480`].

A host therefore needs four things, all in the pack:

1. the decoded streams (`wav/<bank>/m<NNN>.wav`);
2. per sound, the playback data of the bank (`banks[bank].sounds[index]`);
3. per sound asset, the UE-side properties (`sound_assets[path]`: volume, attenuation, loop);
4. the triggers (`triggers[]`, or the sheet `sora-sounds`).

Sora's sounds by SEAD category in the pack: Vo_BattlePC 276 (two languages), SE_footstep 252,
SE_action 131, SE_magic 114, SE_gimmick 32, SE_weapon 29, SE_system 28, SE_event 26, SE_cloth 14,
SE_system2 14, others 31. All their streams are CRI HCA, 48 kHz (1,714 materials, 2,193 s);
none is Vorbis, so the pack holds WAV only.

## 2. What is heard when a sound index is started (driver `SQEX::Sd`)

Everything runs on the driver's own thread ("SeadUpdate", `0x14594C2B0`) with real-time dt.
Randomness is the C runtime `rand()`, seeded once from the clock at thread start [I for the
calls, C for the seeding]: pick sequences are not reproducible, only their distributions.

### 2.1 Before anything plays [C]

1. **Start macro** (`macro_start`, interpreter `0x145976BA0`). A macro filters the set of other
   playing sounds and then acts if the set size passes a comparison. The ones Sora's sounds use
   are listed in `config.macros` of the manifest; in words:
   - 1 / 3 / 5 / 7: the same sound (bank number and sound number) started at most
     0.067 / 0.133 / 0.266 / 0.533 s ago → the new one is cancelled. (Most SE_action and
     SE_magic sounds use 1; looping footstep slides and some weapon sounds use 3.)
   - 2 / 4: copies older than 0.067 / 0.13 s are stopped with a 0.05 s fade; a younger copy
     cancels the new sound. 11: stop every older copy (0.05 s). 12: stop every sound that has
     the same macro (0.066 s). 10: cancel if the same sound started within 2 s.
   - 106 / 110 (battle voices): within the same bank (character) — the same line (106) or a
     line with the same macro (110) is playing → cancel the new one; a line of higher priority
     is playing → cancel; otherwise stop the playing line (0.066 s) and play. Lines of the bank
     numbered 1000 lower/higher are arbitrated by a priority difference of 64. 108: stop the
     other lines of the bank (0.066 s). 107 (field voice): stop lines of the same bank.
   The time filter is taken to be seconds since the other sound started [inf].
2. **Category pool** (`0x14596E140`, `0x145951140`). Pools [A]: SE 52 instances, shared by
   SE_action, SE_weapon, SE_magic, SE_footstep, SE_cloth and every other SE_* category except
   SE_system2 (own pool of 8); VOICE 12, shared by the Vo_* categories. When the pool is full
   the lowest-priority sound in it (oldest on ties) is found: if the new sound's priority is
   lower it is dropped, else that sound is stopped at once and the new one starts on the next
   update. Priorities [A]: voices 128, most action and magic sounds 128, weapon 64 or 128,
   footsteps and cloth 0 — footsteps are the first to go.

### 2.2 Which sequences start (by sound type)

A sound has sequences; consecutive sequences with the same group byte form a group.

| Type | Rule | Evidence |
|---|---|---|
| Sound (672 of Sora's) | all sequences start together | C `0x145952080` |
| Random (110) | groups of weight 0 always play; `r = rand() % 101`, the first weighted group with `r ≤` running weight sum is chosen; if it is the previous pick and flag 0x20 is clear, the heaviest other group plays instead | I `0x145983E10` |
| Order (142; walk and run footsteps, cloth rustles) | round robin over the non-fixed groups. The cursor is a byte in the loaded bank: shared by all instances of the sound, starts at group 0 when the bank is loaded, advances by one per start | C `0x1459845C0` |
| Cycle (23; e.g. the slope-slide loop) | fixed groups plus one weighted pick (not the last one, not a still-active one); a further pick whenever the random interval `[min, max]` ms runs out while fewer than `max_active` groups are active | C `0x1459850A0`, `0x145984B90` |
| Switch (none of Sora's) | every sequence whose `[min, max)` holds the switch value | C `0x145985530` |

`selection` in the manifest gives the rule and the groups already resolved.

### 2.3 Sequences and tracks [C `0x1459831D0`, `0x145982A40`, `0x1459B0450`]

- A sequence runs its commands on a clock that advances by `pitch × dt`: `wait` and
  `waitRandom` (`min + rand() % (max − min + 1)` ms [I]) are pitch-scaled. `keyOn` starts a
  track (one at a time per sequence; the previous one is released) with the time overshoot as
  start offset into the stream; `keyOff` releases it; `loopStart`/`loopEnd` repeat.
- A track plays one material (one WAV of the pack; a mono or multichannel voice) and runs its
  own commands: `volume`, `pitch`, `pan`, `panDepth` (target, fade ms, curve), their `random…`
  forms (drawn once: `min + (rand() % 1000) × 0.001 × (max − min)`, mode 1 added, mode 2
  multiplied), `lfo…` forms, `lfeSend`, `centerMix`. Typical for Sora [A]: `randomPitch`
  −0.1…+0.1 added (695 tracks), −0.1…+0.3 (132), `randomVolume` 0.2…1 multiplied (88).
- Fade curves 0–5: `t`, `1−(1−t²)³`, `1−(1−t)²`, `t²`, `2^t−1`, `2−2^(1−t)` (`0x14596A9C0`).

### 2.4 Volume, pitch, filter [C]

```
volume = clamp01( sound.volume × sound fades (5 slots: user, stop, pause, macro, 3D)
                  × category volume (base × 8 layer fades)
                  × Π zero-one volume factors
                  × track volume  [+ random if mode 1 | × random if mode 2]  × (1 + LFO) )
pitch  = clamp( 1/512 .. 2,  sound pitch fade × category pitch × zero-one pitch
                  × track pitch [+|× random] × (1 + LFO) )          (frequency ratio)
```

(`0x145952DB0`, `0x1459B1440`, `0x145989D20`; pitch `0x145952990`, `0x1459B12D0`,
`0x145989BA0`.) The sound's own volume factor from the UE side enters through the user fade
slot (section 3.2). Which fade slot is which is [inf].

Zero-one curves are evaluated every update. For Sora's 3D sounds the only input in use is
`zeroone_distance_rate` (0 at the listener … 1 at max distance) [A]:

- low-pass factor 1.0 → 0.3 (first segment curve 1) on 1,279 sequences; the mapping from the
  factor to a cut-off frequency was not read (voice parameter 0x400 via `0x14599CA60`);
- reverb send (aux bus "Revibe") −12 dB → +12 dB on 462 sequences, as `exp(dB × 0.115129255)`
  times the sound's aux send volume (1.0, 0.251 or 0.178).

Buses [A]: 3DSE (602 of Sora's sounds), HeliumVoice (250: battle voices), Master (63: UI and
system), Voice (32). Sound flag 0x08 (4 sounds) = centre channel only.

### 2.5 Ending and loops

- One-shot material: the voice ends → the track ends → the sequence ends once it has reached
  `end` → the sound ends [I `0x1459837D5`].
- Looping material (`loop` in the manifest, key-on loop flag 1): plays `[0, loop.end)` once,
  then `[loop.start, loop.end)` until key-off or stop [C `0x145986E00`, HCA branch].
- Stop(fade, curve): runs the stop macro, then fades; with flags2 bit 0 and no fade given the
  sound's own `release_seconds` is used [C `0x145954120`].
- No stealing below the category pool: a full material-track pool delays the key-on (the sound
  then seeks), a full voice pool drops the track [C].

## 3. From a sound asset to the driver (UE plugin `SQEXSEAD`)

### 3.1 Starting [C unless marked]

- Bank and index: the embedded bank if the asset has one, else `ReferenceBank`; index
  `SoundIndex` (default 0) (`0x145814480`, same-partial).
- **2D or 3D** [I `0x1458146A1`]: 3D if and only if the wave instance is spatialized and is
  neither a UI sound nor music. In practice: a sound asset with `AttenuationSettings` played
  at a location or attached is 3D; one without, or one played with a 2D call, is 2D
  (`sound_assets[].attenuation` in the manifest).
- A sound asset with `Duration == 10000` is a loop [C `0x145815C90`]; `sound_assets[].looping`.

### 3.2 Volume handed to SEAD [C `0x145819CD0`, differs]

`ActiveSound.VolumeMultiplier × asset Volume × ParseParams.Volume × AudioDevice[+0xB4]`.
The trigger's volume multiplier is the first factor, the asset's `Volume` the second
(`sound_assets[].volume`; `null` = not serialized, see open question 1). Sound-class volume,
UE distance attenuation and concurrency ducking are **not** applied. Pitch multiplier likewise.
Fade in/out are handed to SEAD as (start, target, time, curve).

### 3.3 Distance and panning [C `0x14598B5F0` same-partial; parameter block I `0x145814735`]

SEAD attenuates, UE does not: every attenuation asset Sora's sounds use stores only
`bAttenuate = false` [A], so the stock shape values apply [UE]:

```
inner = 400 cm, max = 400 + 3600 = 4000 cm        d = |source − listener|
t = 1 if d ≤ inner, else max(0, (max − d) / (max − inner))
3D volume = curve[n](t)          n = byte 0x21 of the bank sound header (0 = linear on Sora's banks)
distance_rate = 1 − t  (input of the zero-one curves of 2.4)   [inf for the exact definition]
pan angle = atan2(x, z) / π in listener space; no doppler (factor 0) [I]
```

Listener = audio-device listener 0 (position, forward, negated up; `0x14580C290`), assumed to
be the camera [UE, not verified against a native override].

Concurrency [A]: footstep and cloth assets override concurrency with rule `PreventNew`
(max count not stored: stock 16) — the UE-side limiter, before SEAD's pool.

### 3.4 Category events the game sends [A config `evt `, C `0x14580EE30`]

Sent by name through `0x141515D60` / `0x141515C90`. Those that change Sora's sounds:

| Event | Effect |
|---|---|
| `STATE_SLOW_START` / `_END` | SE categories (incl. footstep, cloth) volume 0.8 and pitch 0.3 on the "slow" layer, 0.2 s fade; battle voices and SE_event volume 0; back to 1.0 at end |
| `STATE_MENU_OPEN` / `_CLOSE` | pauses all SE_* except SE_system and SE_LSI, and all voices (0.02 s); resumes with 0.2 s |
| `STATE_ALLPAUSE_START` / `_END` | the same plus BGM |
| `STATE_BLACKOUT_START` | game layer volume 0; voices stopped; footstep and cloth stopped (0.02 s) |
| `STATE_MOVIE_START`, `STATE_SILENT_START` | event layer volume 0 |
| `CMD_STOP_RT_SOUNDS` | stops voices and battle SE (0.5 s), footstep and cloth (0.02 s) |

Options sliders: config-layer category volume = slider × master × 0.01, 0.5 s fade
[C `0x140EC5EC0`]. There is no ducking table; voice-over-voice is handled by the macros of 2.1.

Environment stubs: `SQEXSEADAudioVolume` (reverb preset, obstruction, attenuation override;
all defaults neutral), tracing occlusion (`bEnableSEADTracingOcclusion`: none of Sora's
footstep or cloth sounds), external parameters (11 in the config, incl. `sora_speed`, no
setter found). A host may leave all of these out; the reverb send of 2.4 then feeds whatever
room reverb the host has, or nothing.

## 4. Auto SE: footsteps, landings, cloth

No animation of Sora carries a footstep notify [A: 0 `TresAnimNotifyState_FootStep` in 499
sequences]. Footsteps, jump and landing sounds and cloth rustle come from the component
`SQEXSEADAutoSeComponent` on `p_ex001_PawnBase` with `BoneSet_Sora1`, `FsTable_Sora`,
`DetectioinSetting_Sora` [A], plus 34 explicit notifies (4.4).

### 4.1 Table: (surface, motion) → sound [A; lookup C `0x145811C50`]

`FsTable_Sora.PerSurfaceInfos[surface].SoundAssetPaths[type − 1]` for motion types 1–6,
`NonSurfaceInfos.SoundAssetPaths[type − 7]` for 7–16. No fallback: an empty entry is silence.

- Surface `NN` → bank `SE/footstep/10000_sora/100NN/se100NN`; surface 0 (default) uses 10010
  (rock). Sound index: 1 Walk, 2 Run, 3 Jump, 4 LandNormal and LandHard (same asset),
  5 FootShuffle; aux records 6 `slide` (loop), 7 `railslide` (loop), 8 `sliding`, 9 `step`.
- Surfaces with sounds: 0, 1 mud, 2 soil, 4 sand, 6 grass, 8 leaves, 9 snow, 10 rock,
  11 marble/ice, 12 carpet, 13 wood, 14 cloth, 15 glass, 16 plastic, 17 roof tile, 18 net,
  19 metal, 20 iron plate, 21 treasure, 22 cardboard, 25 gravel, 30 frost, 31 snow, 32 puddle,
  33–37 water. Other surfaces are silent (44 aux entries point at folders that do not exist;
  `unresolved` in the manifest).
- Cloth: `SE/cloth/00901/se00901` index 1 RustleShort, 2 RustleLong, 3 RustleFootShort,
  4 RustleFootLong (Order sounds of 8 variations, volumes 0.3 / 0.7 / 0.1 / 0.15).
- Bank volumes of surface 10 for scale: walk 0.6, run 0.4, jump 0.3, land 0.4, shuffle 1.0.
- Other costumes use other tables (`FsTable_Sora_mi`, `_ts`, `_ca`, `FsTable_EW_Sora`), chosen
  by pawn blueprint; not packed.

The surface is the `SurfaceType` of the physical material under the pawn, written into the
auto-SE callback every tick by `0x1405BC800` [C same-partial] from the mesh's foot probe
(`0x1405CAFF0`, virtual slot 465 of the mesh component: sweeps tagged "FootStep") or the
pawn's own trace. A host supplies the surface type per ground hit.

### 4.2 The detector [component tick `0x1458232B0` → `0x145818B70`; analyzer `0x14594E6D0` C differs]

Once per rendered frame, after physics, in metres (bone positions × 0.01). Inputs: Base
`Trans`, Waist `koshi`, per foot Femur/Tibia/Foot/Toe `L_momo`/`L_sune`/`L_ashi1`/`L_ashi2`
(heel computed: Tibia + (Foot − Tibia) × 1.2), per arm `L_ude1`/`L_ude2`, R likewise [A].

- body grounded = movement mode walking [I]; speeds = |Δposition| / dt;
  running = waist speed ≥ 4.1666675 m/s;
- foot grounded = body grounded and (heel.up − Base.up) < 0.2 × |Tibia − Foot|;
- nothing while the owner is hidden, beyond 20 m from the listener, or for 3 frames after a
  visibility/attach change; at most 2 events per frame; detector order Landing, FootStep,
  Rustle.

| Event | Condition | Volume |
|---|---|---|
| Jump | grounded last frame, not this frame [C] | 1.0 |
| LandNormal / LandHard | grounded this frame, not last, air time ≥ 0.3 s / ≥ 1.5 s [C `0x14597D930`] | 1.0 |
| FootShuffle | foot grounded on both frames, waist speed < 0.08 m/s, \|d(heel speed)/dt\| > 5.0 m/s² (Sora's asset values) [I `0x14597D600`] | as steps, range −0.5…3.0 |
| Walk / Run (by `running`) | rising edge of the pawn's foot-contact flag, or of foot-grounded with previous foot speed > 0.125 m/s [I] | `clamp(map(waist speed, 0…1.5 walk / 0…5 run) + map(\|½v²now − ½v²prev\| of the foot, 0…130))` |
| RustleLong | take-off or landing | 1.0 |
| RustleLong (walking) / RustleShort (running) | elbow speed − waist speed crossing upward through 0.45 / 0.8 m/s, within 10 m of the listener [I `0x14597E180`] | `clamp((rel − T) / (1 − T))` [C]; 0.1333 s cooldown on both arms |
| RustleFootLong / RustleFootShort | dot(normalize(Tibia − Femur), normalize(Base − Waist)) crossing upward through 0.6 [I] | `clamp(knee speed / 1.0)` |

Each event arms a 0.3333 s cooldown on its part (foot, arm); events of a part in cooldown are
dropped; Jump and leg rustle are exempt [C]. The sound is played attached at the part's bone
(foot → `…ashi1`, arm → shoulder) with volume `clamp(event volume × VolumeScale)`,
`VolumeScale` 1.0, and stops when the owner is destroyed.

Two cautions for the port: the analyzer is frame-rate dependent (per-frame "motionless"
distance, finite-difference accelerations), and for Tres pawns the step trigger is normally the
pawn's foot-contact flag (leg-IK array of the mesh, entry flag `+0x394 & 5`), whose exact
meaning is open (question 4).

### 4.3 Enable windows [A; C `0x145816110`]

`SQEXSEADAnimNotifyState_ChangeAutoSeEnable` (35 in Sora's set) switches the detectors off or
on for its window and restores the state at the end: 32 windows disable (fall loop, landings,
glide idle, wall run, slides, rail slide, balance, hops), 3 glide-loop clips enable. It does
not block the explicit notifies below.

### 4.4 Explicit auto-SE notifies [A; C `0x145815C10` → `0x14581BCE0`]

`SQEXSEADAnimNotify_PlayAutoSe` (24) and `…State_PlayAutoSe` (10) play a table entry for the
current surface at a given time: LandNormal in `Land`, `LandB`, `RollLand`, `LightLand*`,
`Balance`; Jump in `JumpSuperStart`, `WallRun*`; Run in `WallRunF`, `WallRunLoop*`; aux `step`
in `HopR/L`, `BalanceR/L`; aux `slide` in `SlopeSlide`, `FallSlide`; aux `railslide` in the
rail-slide loops. Volume and pitch 1. The state form starts at Begin and fades out (0.5 s) at
End. The normal jump start has none: its sound is the detector's Jump event.

## 5. Triggers

Counts are rows of `triggers[]` in the manifest; the sheet `sora-sounds` has one row per
trigger and sound (2,962). Every notify has trigger chance 1 and no LOD filter [A].

### 5.1 Animation notifies (780 in the 499 sequences of `p_ex001_AnimSet`) [A]

| Route | Count | What plays | Native behaviour |
|---|---|---|---|
| `notify.weapon_swing_alias` | 192 | `TresAnimNotify(State)_PlayWeaponSwingSE`: entry `m_SoundAliasID` of the weapon's swing set (5.3) | `0x1405615F0` [C differs; unit → equip part and vfunc offsets I]: at the mesh location, or attached at `m_SocketName` when `m_bFollow` (99 not following, 28 following without socket, 65 on `R_wep_skt`); no visibility test. State form: after `m_StartWaitTime`, stop or fade at End |
| `notify.attack_hit_se` | 151 | `TresAnimNotifyState_Attack`: `m_WeaponHitSEAliasID` (hit set) or a direct `m_HitSEAsset` | Begin `0x140561F40` only hands the parameters over; the sound plays when the attack connects (5.4) |
| `notify.voice_random` | 139 | `TresAnimNotify_PlayVoice.m_RandomVoiceAssets` (weights) | `0x140560960` [differs; offsets, calls, volume clamp I]: weighted pick without immediate repeat — `r = (xorshift >> 1) % total` with the last pick's weight removed (`0x14056CAE0`); the memory lives on the notify object, so per animation asset; a one-entry list plays once and then never again |
| `notify.sead_play_sound` | 100 | `SQEXSEADAnimNotify_PlaySound.Sound` | `0x145815C90` [C differs]: refuses loops; skipped when the owner is hidden (`bFollowOwnerVisible`); attached at `AttachPointName` (none 76, `center` 19, `L_te` 3, `R_te` 1, `L_ude2` 1); survives the animation and the owner |
| `notify.sead_attach_sound` | 63 | `SQEXSEADAnimNotifyState_AttachSound.Sound` (loops allowed) | Begin `0x145815F40`, Tick `0x145816C70`, End `0x1458162E0` [C]: starts after `StartWaitTime`; at End (also on interruption) `Stop`, or `FadeOut(FadeOutDuration)`; then `LoopEndSound` if set |
| `notify.auto_se_enable` / `notify.auto_se` | 35 / 34 | sections 4.3 / 4.4 | |
| `notify.voice_direct` | 29 | `m_VoiceAsset` | as voice_random without the pick |
| `notify.voice_group` | 23 | `m_VoiceGroupName` → unit by the pawn's voice group number (5.2) | `0x1405865D0` → `0x14150DC50` [C] |
| `notify.weapon_hit_alias` | 6 | `TresAnimNotify_PlayWeaponHitSE` | `0x1405612A0` [C differs] |
| `notify.actor_voice_null` | 8 | nothing (`m_ActorVoice` is null) | `0x1405604A0` |

Voice notifies: play only when the owner is not hidden, the mesh is visible, and voices are
enabled (`0x1413ECBC0`: false in three states, see open question 5); volume
`clamp(scale × m_VolumeMultiplier, 0, 1)` with Sora's scale 1.0 [I]; attached to socket
`atama` (head), following; all 191 use volume 1, pitch 1. The notify does not stop an earlier
voice: arbitration is SEAD macro 106/110 (2.1).

The weapon's own animation set has no sound notifies. Form-change sets (`p_ex001_AnimSet2`–`23`)
were not scanned.

### 5.2 Voices [A; code C]

- All battle voices are sounds of the bank `VOICE/Localization/<lang>/battle/PC/Battle_Sora`
  (177 cues). Assets are always referenced under `/Localization/ja/`; the English build places
  its files at those paths through the overlay pak (`docs/formats/pak.md`). The pack holds both
  languages under their real folders; resolve a voice by asset name per language — 7 cues
  (`bt1012510sr0`…`bt1012590sr0`) have a different sound index in en and ja, and the asset
  volumes differ on some (e.g. 0.8 en vs 0.6 ja).
- Voice groups: `p_ex001_VoiceGroupSet`, 13 groups, 82 units (`voice.group` rows). A group
  plays the unit whose number equals the pawn's voice group number (int at pawn+0x4D8); no
  match plays nothing (`m_bPlayOnGroupNotFound` is false on all). The number is set through
  the pawn's anim interface (pawn+0x440, slot +0x1F0 = `0x1405A1D50`) by the states after they
  start an animation: magic = the spell's `m_VoiceGroup` / `m_VoiceGroupFinish`, or a
  heal-target table (`code.voice_magic` 77 rows); items by target (`code.voice_item` 20);
  attack kinds 0x25 → 52, 0x4D → 54, 0x4E → 55, 0x5D/0x67 → 1, 0xB6/0xB7 → 2
  (`code.voice_group_no`); death 1 (`Sora_dead`).
- Direct plays from code: `sora_kaihuku` 1 when healed by another character (`0x141445F40`),
  `sora_sneeze` 1 (attack state under status 13).
- No cooldown or probability logic exists in native code.

### 5.3 Kingdom Key [A]

Weapon `w_so010_Wep`, bank `SE/weapon/02001/se02001`, sound index = asset number, attenuation
`weapon_Attenuation`. Swing set `enumset_swing_w_so010`: SWING1–10 → `se02001_101`–`110`
(11–13 empty: the 11 notifies of `M_1484` are silent with this weapon). Hit set
`enumset_hit_w_so010`: HIT1–7 → `se02001_001`–`007`. Lookup is by direct index, bounds
checked (`0x14178BED0`, `0x14178BEB0`). Keyblade appear: effect `vw_so010_app0` →
`se02001_010` (5.5).

### 5.4 Hits [C `0x140628960` same-partial]

On a connecting attack the collision component picks, in order: the map-hit SE (hit on level
geometry with `bChangeSEMapHit`), the per-attribute SE of its hit-effect entry, the entry's
default SE, else the primitive's `m_HitSE` — which is what the attack notify set. Sora's pawn
and the Kingdom Key carry no SE in their hit-effect entries, so a keyblade hit plays the notify
window's hit sound (HIT4 ×48, HIT6 ×21, HIT5 ×20, HIT2 ×19, HIT1 ×18, HIT3 ×17, HIT8 ×1 —
silent — and 7 direct assets `se00001_057/060/061`). Played with `PlaySoundAtLocation` at the
hit point plus a random offset, volume 1, pitch 1. There is no table by victim material: the
victim's side enters only through its own hit effect (5.5). Magic hits: `code.hit_magic` (25).

### 5.5 Effect-attached sounds (the largest code route) [A; C `0x144272370` → `0x14425F080`]

A particle system can carry sounds: `UParticleSystem.ParticleSeAssets` →
`SQEX_EffectSeAttachDataAsset.SEDatas`, assets under `/Game/Sound/SE/_Data/SeAttach/…`. When
the effect is spawned the sound plays on a component attached to it. This is how guard
(`vs_com_grd00` → `se00001_032`), reflect (`se00001_067`), magic casts (`vm_fir_cal0` →
`se00002_001`, cure `se00002_301`–`304`), item use (`se00001_102`), treasure chests, save
points and the keyblade appearing make sound. Rows: `code.effect_se_pc` 98, `_magic` 103,
`_common` 49, `_weapon_w_so010` 11, `_system` 3. A host that spawns the effect by name plays
the row's sound at the effect.

### 5.6 Other code routes

| Route | Rows | Summary |
|---|---|---|
| `code.ui_gameplay`, `code.ui_other` | 45 / 141 | UI parts call `PlaySystemSE(kind)`; kinds 0–6 → static SE 0, 1, 3, 4, 2, 2, 5 (Click, Decide, Cancel, Beep, Tab, GameHelpOpen, GameHelpMessage) (`0x14176F7D0`), or a raw static id (`0x14176F6B0`); all 2D, volume 1. The id per call site is [I]; the menu action behind a call site is [inf] from class and ActionScript names |
| static SE table | 43 ids | `ETresStaticSE` → `/Game/Load/Common/TresStaticSE` → `se00000_NNN` in `SE/common/00000/se00000` (`0x141515B30`); ids 9, 21, 34 are loops |
| `code.alarm` | 4 | low HP: HP ≤ MaxHP × 25 / 100 (integer) [I `0x1405AD600`] → static 9 (`se00000_010`, loop) after 0.8 s **and** 15 ticks; stopped when no request is left; pausing stops and re-arms it [I `0x14158A340`, `0x14158D800`] |
| `code.level_up`, `item_get`, `bonus_get` | 2 / 2 / 1 | static 33 on a gained level (`0x141584180`), 12 for the level-up panel, 33 item info, 38 bonus |
| `code.shotlock` | 1 | static 26 when the HUD is told a lock rate ≥ 1.0 (`0x14158C410`) |
| `code.prize` | 34 | orbs: get / bounce = HP `se00000_022/021`, MP `028/027`, munny `024/023`, focus `041/040`, box `026/025` |
| `code.projectile` | 13 | `ATresProjectileBase` fire effects' `MySoundCue` (magic fire SE `se00002_015`–`018`) |
| `code.game_over` | 2 | loop static 21 (variant 34), handle in the game instance |
| `code.blueprint_*` | 74 | blueprint bytecode; Sora's own blueprints have none; 3 player-state event cameras, the rest world gimmicks |

Not found as code or notify sounds: MP charge start/end, focus gauge full, situation command
appear, shotlock lock tick and fire (open question 6).

## 6. The pack

`node tools/sora_audio_pack.js` (inputs `audit/sora_port/audio/`, the banks, the config):

| | |
|---|---|
| `wav/<bank>/m<NNN>.wav` | 1,714 files, 331 MB, 2,193 s; PCM s16 as stored (48 kHz), loop as `smpl` chunk. Source codec HCA on all (442 obfuscated); 1,409 mono; nothing kept as Ogg |
| `manifest.json` `banks` | 383 banks: per used sound the playback data of section 2 (type, category and pool, priority, volume, buses, macros, selection, sequences with commands and zero-one curves, tracks with commands and material), per material file, samples, loop, peak |
| `sound_assets` | 947 assets: bank, index, `Volume`, duration, loop marker, attenuation, sound class, concurrency |
| `triggers` | 1,972 triggers (section 5) with their sounds, attach, volume, pitch, conditions |
| `config` | categories and pools, layers, buses, the macros in use |
| `ue_assets` | the attenuation and sound class assets referenced, as stored |
| `unresolved` | 47 asset paths that do not exist in the game (44 aux footstep entries of surfaces without sounds; 3 requested paths that are the weapon set assets, not sounds) |

Included on request of the other research passes: every sound path named in
`build/sora_tmp/{actions,combat,magic}/asset_requests.json` (copied to
`audit/sora_port/audio/requests_*.json`). Ids given there without a path (alias labels such as
`SWING1`, voice group names) are covered through the weapon sets and voice groups. No request
file from the UI pass existed at build time: re-run with `--import` when it appears.

Checks (`manifest.validation`, `--check`): every WAV read back (format, frame count, loop,
sha256); every key-on resolves to a packed material; of 1,836 key-ons of one-shot materials,
1,752 have a key-on duration equal to the decoded length within 1.5 ms — the rest are tracks
with a pitch change, which shortens them (e.g. pitch 1.6: 1.034 s → 0.659 s).

## 7. Host interface (summary)

```
play(asset or (bank, index), position | attach(bone) | 2D, volume, pitch, fade_in) -> handle
stop(handle, fade)     set_volume / set_pitch(handle)     event(name)
per update: listener transform; per 3D handle: position
auto SE: per frame bone positions (section 4.2), grounded flag, surface type, foot contacts
```

Minimum for a faithful result: selection rule and Order cursor per sound (2.2), track random
pitch/volume (2.3), volume product (2.4, 3.2), linear falloff 400–4000 cm with the low-pass
curve (3.3, 2.4), start macros 1/3/5/7 and 106/110 (2.1), the SE pool of 52 with footsteps at
priority 0, loops with their loop points, the slow and pause events (3.4).

## 8. Open questions

1. `Volume` of a sound asset when not serialized (643 of 947 packed assets, most footstep and
   weapon sounds): stock `USoundWave` gives 1.0; the `SQEXSEADSound` constructor
   (`0x14580BC90`) was not checked for this field. Row `asset.volume_default` is unchecked.
2. `AudioDevice+0xB4` in the volume product and `+0x7F4` (blocks new non-UI sounds).
3. Low-pass factor → cut-off mapping; the pan law (tables at `0x147CA2D80`); which of the five
   sound fade slots is which; the exact definition of `distance_rate`.
4. The foot-contact flag (`mesh+0x11B0` entries, `+0x394 & 5`) that triggers steps for Tres
   pawns; walk/run sounds per footfall depend on it.
5. The voice-enable test `0x1413ECBC0`: controller vf +0x1A70 == 0x8B, equipment vf +0x488 == 7,
   attack-state flag 0x1000000 (sneeze).
6. Sounds of MP charge, focus full, situation command, shotlock lock/fire: not in native code
   or notifies; likely Scaleform timeline sounds or effect-attached (`vw_so010_rgn_*`,
   `vw_so010_shf1` unconfirmed). 13 static SE ids are never passed as constants.
7. Macro filter 0x16 as "seconds since start", the order of the working set, macro ops
   0x18–0x1A and 0xA7–0xAB.
8. The language remap `0x142F21BA0` was identified by its strings only.
9. Listener = camera is assumed.
10. The HCA decoder has not been compared with a recording of the game
    (`docs/formats/sead_audio.md`, "Not covered").
11. Everything marked [C] with a verdict other than `same`.

## 9. Runtime-probe requests (object, offset, type)

| Object | Field | Purpose |
|---|---|---|
| Sora pawn | +0x4D8 int32 | voice group number during magic, items, attacks, death |
| Sora pawn | +0x9B0 int32; (+0x9A8)→+0x90 int32 | cached surface type / callback surface while walking over floors |
| Sora pawn | (+0x9A0)→+0x240 float, +0x2A0 int32, +0x2A4 uint8 | auto-SE volume scale, enable state, flags |
| Sora mesh (pawn+0x580) | +0x11B0 array (stride 0x3A0, count +0x11B8): entry +0x394 uint32 | foot-contact flags per step |
| Sora mesh | +0x11C0 / +0x11C8 / +0x11D0 pointers → +0x68 uint8 | detected physical material and surface |
| `TresAnimNotify_PlayVoice` object | +0x84 int32, +0x88 int32 | last random index, total weight across repeated dodge rolls |
| loaded bank se10010, sound 1 | header byte +1 | Order cursor over successive footsteps (expected 0, 1, …, 7, 0) |
| `Driver::Sound` of a playing footstep | +0x38 float, +0x3C float, +0xCC float, five fade slots at +0xF8 + 0x40·i | distance, distance rate, category volume, fades |
| main `FAudioDevice` | +0xB4 float, +0x7F4 uint8, listener 0 transform | volume factor, block flag, listener = camera? |
| a playing footstep `FWaveInstance` | +0x48, +0x4C, +0xF8 floats | volume chain |
| HUD manager (`UTresUIP_HudStatus`+0x1C8) | +0x248 ptr, +0x250 float, +0x254 int32, +0x258 uint8 | low-HP alarm state |
| SEAD globals | `0x14A168910`, `0x14A168884`, `0x14A168478`, `0x14A168480` | voice pool, material-track limit, sound capacity, near pan distance |
| category objects `*(0x14A168468 + 8·n)` | +0x10, +0x14 int32, +0x48 float | pool capacity, count, volume |
