# Sora port: HUD and menus

What the game's HUD and menus are made of, which native function drives which piece with which
values, and what was extracted for a 1:1 rebuild. Research and extraction only: no Rust is specified
here beyond the data formats of the pack.

Evidence marks: **[I]** read at instruction level (disassembly), **[C]** decompiled C only (the
function's `verdict` from `sheets/impl/progress` is given: `same` = decompiled C proven identical),
**[A]** read from an asset (a movie's timeline or ActionScript, a data table, a data asset),
**[U]** stock UE 4.17 on confirmed function identity. Everything not marked is derived from marked
statements. Unknowns are listed in section 13, never filled in.

Sheets: `sheets/port/sora-ui-elements.json` (196 rows: one per element, one per Config option, plus
one per method of the three ActionScript interfaces), `sora-ui-screens.json` (22), `sora-ui-params.json` (127 numeric
constants, generated into `build/gen/rust/sora_ui_params.rs`). Written by `tools/sora_ui_sheet.js`.
Pack: `build/sora_pack/ui/` written by `tools/sora_ui_pack.js` (section 2).

## 1. Technology

- **Everything is Scaleform GFx 4 with ActionScript 3. There are no UMG widgets** in the HUD or the
  menus [A]: the 276 movies under `UI/Scaleform/` are `SwfMovie` exports (module `/Script/ScaleformUI`);
  the only six `WidgetBlueprintGeneratedClass` packages of the game are debug and test widgets
  (`DebugUMG_BPEM_EmotionMap`, `g_zz_Dance*UI`, `TresRemyUMGDebug`, `BP_DebugWidget`,
  `TresWinnieUMGDebug`), and `UI/Blueprints/HUD/*` are blueprint subclasses of the native `TresHUD*`
  actors.
- **Container** [A]: the export's `RawData` byte array is an *uncompressed* `GFX` file (signature
  `GFX`, version byte 21 for most movies; 17, 22, 25 and 36 also occur), i.e. a SWF processed by the
  GFx exporter: bitmaps are stripped and replaced by tag 1009 `DefineExternalImage2` (id, format,
  declared size, export name, file name) that names a sibling `Texture2D` package (BC7,
  `TEXTUREGROUP_UI`); `ReferencedAssetNames` lists those packages. Texture sizes are rescaled to a
  multiple of 4 (`FlashTextureScale_Mult4`): fills must be mapped with declared size -> texture size.
  Localised textures (`*_loc`) live in a language sub folder (`en/`, `ja/`, ...).
- **Tags that occur** (all 276 movies parse, no unknown tag) [A]: shapes 2 / 22 / 32 / 83, morph
  shapes 46 / 84, sprites 39, PlaceObject2 / 3, RemoveObject2, DefineEditText 37, FrameLabel 43,
  DefineScalingGrid 78, ImportAssets2 71 (shared libraries `90_rsl/*`), SymbolClass 76, DoABC 82,
  DefineSceneAndFrameLabelData 86, GFx tags 1000 (exporter info), 1005 (compacted font), 1008 (sub
  image), 1009. In a GFx file PlaceObject3 with the *HasImage* flag places an **image character
  directly** (no class-name string follows): the MP gauge base is such an image.
- **Stage** [A]: 1920 x 1080 at 60 fps for every HUD and menu movie (face portraits and form logos
  are 1 x 1 movies loaded into a host clip). Timelines advance one frame per 1/60 s.
- **ActionScript** [A]: one DoABC block per movie (AVM2). Timeline scripts are methods `frameN`
  registered with `addFrameScript`. `tools/lib/abc.js` parses the ABC and rebuilds readable
  pseudo-AS3 (1,694 classes in the packed movies).
- **Fonts** [A]: `91_font/TresFont` holds four GFx compacted fonts (tag 1005), bound by class name:
  `$KH2Menu` (KH2 ALL MENU, 219 glyphs: command menu, HUD labels), `$jimakuNE` (270), `$JimakuJP`
  (7,932: subtitles), `$sougei` (7,904: body text). Text fields name their font by class. Format
  decoded by `tools/lib/gfx_font.js` (all 16,325 glyphs decode): header, glyph table, paths of
  4-bit-tagged edges (H / V line 12 | 20 bits, line 2 x 6 | 10 | 14 | 18 bits, quadratic curve
  4 x 5 | 7 | ... | 19 bits), contours may reuse an earlier contour's edges.
- **Text** [A]: English tables are the `.locres` files `kh3_system` (commands, HUD words),
  `kh3_camp` (menus), `kh3_menu`, `kh3_item`, `kh3_settings`, `kh3_level`, ... Text may carry tags:
  `<icon(Triangle)>`, `<color(strong)>`, `<br>`, `<pb>`; the icon names resolve through
  `tres.managers.IconManager` and the `UI/IconText` textures per platform (`TresUIManager`
  `m_IconTextTexture*` / `m_IconTextData*`).

## 2. The pack (`build/sora_pack/ui/`, 344 files, 37.7 MB)

| Path | Content |
|---|---|
| `movies/<Name>.json` (38) | stage, images (id -> texture, declared size), shapes as **edges per fill** in pixels (lines and quadratic curves; even-odd fill) with solid / gradient / bitmap fills, and for axis-aligned bitmap rectangles a ready **quad** (destination rect, source rect in texture pixels); morph shapes (start / end); text fields; sprites with their **timeline** (per frame: place / move / remove ops with matrix, colour transform, name, mask depth, blend mode, visibility) and **frame labels**; frame scripts; symbol classes; the AS3 interface of the document class |
| `as3/<Name>.as3.txt` | pseudo-AS3 of every class of the movie (evidence for the porter) |
| `layout/<Name>.json` | every named instance with its stage position at first placement, its frame count and labels |
| `textures/**.png` + `textures/atlas.json` | the 202 bitmaps the movies reference (top mip, RGBA) and, per texture, the source rectangles its shapes use |
| `fonts/<font>.json`, `.png` | glyph outlines (Latin ranges, units per em 256, y down) and a 48 px coverage atlas for ASCII |
| `strings/en/<table>.json` | 10 English string tables, 5,257 strings |
| `data/*.json` | level table of Sora (EXP), base parameters, the 254-row command table with English names and MP / focus costs, the main-command styles, the UI sound table, world skins, the gauge rules |
| `render/hud_1080.png` | the HUD composed offline (section 11) |
| `manifest.json`, `validation.json`, `sheet_rows.json` | inventory, checks, rows for `sheets/port/sora-assets.json` (ids `ui/...`) |

Movies packed: HudStatus, HudCommand, HudLeft, HudRight, HudTarget, HudInput, HudOperation, the face
movies of Sora (base, two forms) and of Donald and Goofy, NaviMapGra, MapName, KeyItem, Message,
MultiItemGet, CenterInformation, Dialog, LsiButtonSet, Cursor, Pause, CampMenu, the shared libraries
(MainLibrary, ItemListLibrary, ItemStatusLibrary, Config, SystemLibrary, HudTimer, TutorialLibrary),
SettingMenu, ScreenOption, BrightnessOption, SaveLoad, GameHelp, BattleReport, Tutorial, TresFont.
Not packed: the 13 world skins of the command menu (`HudCommand<XX>`, same structure, other art),
`HudStatusCA`, other characters' HUDs, mini-game HUDs, form logos (`act_logo/*`), item / keyblade
icons (`UI/Icon`, 3,287 packages).

Checks (all pass): 763 bitmap fills resolve to a packed PNG; 4,961 placed characters resolve; every
PNG reads back; every symbol has its AS3 class; every import resolves to a packed library; every
text font class exists in TresFont.

A consumer plays a movie by keeping, per sprite instance, a current frame; `gotoAndStop(n)` /
`gotoAndPlay(label)` replay the timeline ops up to that frame into a depth-keyed display list;
masks (`clip`) cover the following depths up to the given depth. `tools/lib/gfx_render.js` is a
reference implementation (software, about 300 lines of rules).

## 3. Native architecture

- `UTresUIManager` (27,936 bytes) owns the system parts (`m_Cursor`, `m_Pause`, `m_MapName`,
  `m_SaveLoad`, ...), the controllers (`m_CampController`), `m_UIData` (`UI/Data/UIDataAsset`) and
  the icon-text tables [A reflection]. Its update `0x1415B0ED0` [C differs] walks the registered
  parts and calls `FUN_141774710` [C differs] on each: the part's **update (vtable slot 90)**, then
  the tick (slot 77) of each of its active AS proxies (`m_ManagedASProxy`), then its float textures.
- `UTresCockpit` (624 bytes) is the gameplay HUD: `m_pCockpitParts[5]` = `m_pHudTarget` (+0x68),
  `m_pHudStatus` (+0x70), `m_pHudCommand` (+0x78), `m_pHudLeft` (+0x80), `m_pHudRight` (+0x88), plus
  `m_pHudOperation`, `m_pCenterInformation`, `m_pNaviMapGra`, mini-game HUDs and `m_HPAlertAudio`
  (+0x248) [A reflection]. Movies come from `UI/Data/UIDataAssetCockpit`; per world
  `UI/Data/World/UIDataAssetWorld<XX>` overrides the command menu skin (`HudCommandSwfAsset`), the
  wipe and for CA the status HUD [A] (`data/world_skins.json`). A test level outside the worlds uses
  the cockpit defaults (the option "Command Menus: Classic" is configuration byte 1 bit 5, read into HudCommand+0x9C0 and by the skin choice `0x141670930`: 10.8; `setLegacyMode` is something else: 5.9).
- A part is a `UTresUIParts` (`m_strAssetName` +0x48, movie player +0x58, UIComponent proxy +0x60,
  `m_pUIManager` +0x70, `m_SwfMovie` +0x1B8); cockpit parts add the cockpit at +0x1C8. Sub parts
  (`UTresUISubParts` / `UTresUIP_HudParts`, `UTresASProxy` with `m_ASObject` +0xC0) bind to one AS3
  object each. Vtable slots seen on every part [C]: 71 bind (look up clips by instance name), 74 / 80
  show (label `in`), 75 / 81 hide (labels `out` / `off`), 78 input event, 79 set value, 90 update(dt);
  slot 83 is overridden by the popup sub parts (not read: presumably their per-frame update).
- **Native -> AS3** in two styles [C]: (a) *interface* movies (HudStatus, HudCommand, HudTarget):
  the native side invokes methods of the document class by name through small wrappers on the
  movie player: `0x141582ED0` (int), `0x141583020` (int, int), `0x1415831C0` (int, int, int),
  `0x141583870` (bool), `0x141582CF0` (argument array) [all `same`]; the names are static strings
  built at start-up. (b) *bound* movies (HudLeft, HudRight, CampMenu, Pause, ...): the native class
  keeps `GFxObject` references to clips and text fields (its reflected `m_...` members, e.g. the 36
  fields of `TresUIP_CampMenuStatus`) and sets text / frames on them directly; the document class
  only exposes arrays `parts` / `partsTypes`.
- **AS3 -> native** [A]: `tres.core.UIComponent.Callback(id, param)` =
  `ExternalInterface.call("OnCallback", id, param)` (animation ended, popup closed); each part has an
  `OnCallback` script function.
- **Common AS3 base** [A]: every document class extends `tres.core.UIComponent`: `setPlatform`,
  `setWorld`, `setLanguage` (stored in `tres.core.System`; language picks the frame of
  `tres.controls.LanguageObject` clips: labels none / it / es / de / fr), `focus`, `unfocus`,
  `focusAnchor(i)`, `moveAnchor(direction, isRepeat)`.
- **Menu navigation** [A]: focusable clips are `tres.core.AnchorComponent`s with an index, the
  neighbour indices `toLeft / toRight / toUp / toDown` and `repeatStop*` flags (authored in the
  movie); `tres.managers.FocusManager.moveAnchor` follows them and the `Cursor` movie
  (`anchorCursor`, `lightCursor`) is moved to the anchor rectangle. The native side reads the pad and
  calls `moveAnchor`; the command menu of the HUD does **not** use this: the HudCommand part runs its own
  state machine on the UI pad and calls its movie's interface (section 5).
- **Sounds** (section 9).

## 4. Status HUD (HP, MP, focus, portrait): `HudStatus`, bottom right

`UTresUIP_HudStatus` (ctor `0x14164E140`): update `0x1416608B0` [C differs; constants and calls
read in disassembly] reads the pawns and caches frames; `0x141662640` [C differs] sends what
changed. Five slots (0 = Sora at 1602,872; friends `sttsNpc1/2`; guests `sttsSnpc1/2`), each 0x68
bytes from part+0x1D0 (layout in `build/sora_tmp/ui/notes/hudstatus_native.md`).

**The gauges are timelines, one frame per gauge length; the native side computes the frame.**

`F(v, max, frames)` = `0x141598CA0` [I, `same`]: 0 if `v < 1` or `max < 1`; `frames` if `v == max`;
else `trunc(v * frames / max + 0.5)` (float) clamped to `[1, frames - 1]`. The clip receives `F + 1`
(frame 1 = empty). So a nearly empty gauge never looks empty and a nearly full one never looks full.

| Element | Clip (frames) | Rule |
|---|---|---|
| HP base | `pcHpDodai` (669) | `hpMaxFrames = min(668, trunc(maxHP + maxHP))` [I]: **two frames per HP point**. Frames 1..~90 draw the ring around the portrait, later frames extend a straight bar to the left. **Longest gauge at 334 max HP**; above that the base stays at 669 and the fill is relative to it. No wrapping, no second row |
| HP fill | `pcHpGg` (669) | `F(hp, maxHP, hpMaxFrames) + 1` [I] |
| HP red lag | `pcHpGgR` (669) | start = `min(frame sent last, hpMaxFrames)`; if the new frame is higher (heal) or the slot just appeared, the red gauge is set to the new frame at once [C]. Then AS3 `FollowGauge` [A]: wait **800 ms**, then every **33.3 ms** move by `(target - start) / 10` frames (at least 0.1) until the target: about a third of a second. A new hit during the follow keeps the red position and retargets. The timer is wall-clock |
| MP base | `pcMpDodai`, `pcMpDodaiC` (501) | `mpMaxFrames = min(500, trunc(maxMP * 1.8))` [I]; longest bar at 277 max MP |
| MP fill | `pcMpGg` (501) | `F(mp, maxMP, mpMaxFrames) + 1` [I]; no lag gauge |
| MP Charge | `pcMpPack` labels | while the pawn reports MP Charge (vfunc +0x17E0) the pack plays `charge_in` -> `charge_loop`; the pink gauge `pcMpGgC` shows `F(trunc(chargeValue), maxMP, mpMaxFrames) + 1` (charge value = vfunc +0x17F8, a float); on leaving `charge_out` and `setMPCharge(0)` [C] |
| Focus | `pcExGg` (101) | `F(focus, maxFocus, 100) + 1` [I]: a continuous gauge, **no segments**. `isMax` (focus == max) plays `max_in` -> `max_loop`, `max_out` when it drops [A] |
| Focus red | `pcExGgR` (101) | like the HP lag: start = max(new, last sent) [C]. While a shotlock is aimed the gauge shows focus minus the pending cost and the red gauge the unspent value (consume mode) [C, meaning of the pawn calls inferred] |
| Portrait | `poPcFace` | a separate movie (`01_hud/face/FacePcExSora01`) loaded at run time; its first child is re-parented at (0,0) [A]. Look: 0 normal, 1 battle, 2 dying, 3 death; state: usually / damage / link; form `f0..f10` [A]. Native choice [C]: death when a friend's HP < 1; else dying when **pinch**; else battle when the pawn or the game is in battle; else normal |
| Pinch | — | `hp <= maxHP * 25 / 100`, integer arithmetic (`0x1405AD600`, [C `same`]): dying portrait and the **low-HP alarm** (section 9). The gauge itself has no flashing frames [A] |
| Friends | `npcHpGg`, `npcMp` | fixed 180-frame gauges: `F(v, max, 180) + 1` [I] |
| Parts | `pcHp`, `pcMp`, `pcEx` labels on / off | MP shown when game flag byte +0x95B0 bit 2 is set, focus when bit 4 is set [C] (story progress) |

Values come from the pawn's virtual functions [C]: +0x17B0 HP, +0x1820 max HP, +0x17D8 MP, +0x1828
max MP, +0x1990 focus, +0x1998 max focus (addresses in section 12). Starting values of Sora [A]:
max HP 90 (base frame 181), max MP 100 (181), max focus 100. Root labels `off / loop / inShort /
outShort` and per-slot `in / out` (`in2 / out2` when Sora is alone) are requested through
`setStatus` [A]; who requests which mode is open.

## 5. Command menu: `HudCommand`, bottom left

`UTresUIP_HudCommand` (2,832 bytes; ctor `0x1416147A0`, update slot 90 `0x14162FF80`). **The part is
not a mirror: it is the command menu.** It owns the menu state machine, reads the UI pad, keeps the
lists and asks the player controller to execute (`ctrl` vt+0x1550 = may execute, vt+0x1540 =
`ExecuteCommand`). The *logic* (what the lists contain, what is selectable, shortcut contents, the
situation-command queue) belongs to `docs/sora-magic-spec.md` / `docs/sora-combat-spec.md` (strand
notes `build/sora_tmp/magic/menu2/NOTES.md`, whose field names are used here). This section is the
**display contract**: which state change makes which ActionScript call with which arguments, so a
port's command model can drive the movie. Logic rules that the display code reveals are listed in
5.10. Evidence: [C] throughout (all functions `differs` / untested) with the constants, sound ids and
call arguments marked [I] read in the disassembly; the meaning of each call is [A] (the movie's AS3).

### 5.1 Layout and clips [A]

Menu origin (-75, 910); four slots `cmdMenu.cmdMainTop.cmdMain.cmdMain1..4`, 32 px apart (labels
`up / over / down / out / in / in2 / up2 / next_N / turn1_N / turn2_N / weapon / weapon2`); per slot
a text clip `cmdText1` (labels **`off` 1, `on` 11, `disabled` 21**), a change flash `cmdEffChange`
(`off / in / in2 / loop / out`) and a right arrow `cmdRightIcon` (`off / on`). Title plate
`cmdTitleTop` at (-113, 874) with `cmdTitle` (`off / loop / change / logoOut / logoIn / scutIn /
scutOut`) and the name clip labels `cmdBlue / cmdYellow / cmdRed / cmdScut / cmdBtn / cmdSlock`.
Sub list `cmdSubTop` at (99, 750): colour labels `mahou / item / link / party`; first level
`cmdSub1Set` with 9 rows `cmdSub1_1..9` (`off / up / over / down / nextOut / nextIn / turnOut /
turnIn / in / out`), the carried-over row `cmdSub1Next`, second level `cmdSub2Set` with 6 rows.
Situation commands `cmdActTop` at (-88, 828) (`off / loop / dark / back`): 7 positions
`cmdActPos1..7` (position labels `start..start7`, slide labels `sita1..6`), each with `cmdAct`
(`off / in / loop / out / down / break`), the button `cmdAct1` (`off / up / up2 / over / down`), the
time gauge `actGgAnm` (219 frames), the counter `actCount` (`off / loop / red`) and the style-point
gauge `cmdActStylePntGg`. Event button `cmdActEvTop.cmdActEvShort` (`off / in / loop / out / down /
in2 / break`), form logo `cmdActLogo`, form gauge `cmdActLogoGg.cmdActLogoGgShort` (`off / in / loop
/ out / break`, gauge clip 201 frames), shortcut panel `cmdScutTop` and mode panel `cmdExTop` (both
`off / in / loop / out`, four rows `cmdExTextSet1..4`).

Native handles (reflected members): `m_pGFxMainCmdList[4]` (+0x4D8: the slots' text clips),
`m_pGFxSubCmdList / Text / Name[9]` (+0x4F8 / +0x540 / +0x588), `m_pGFxSubCmdListNext / NextName`
(+0x5D0 / +0x5D8), `m_pGFxShortcut[4]` / `m_pGFxShortcutName[4]` (+0x5F0 / +0x610),
`m_pGFxActionCmdText / Count / GgAnm / Time / Time2[7]` (+0x3C0 / +0x3F8 / +0x430 / +0x468 / +0x4A0),
`m_pGFxActLogoGg[2]` (+0x630 gauge clip, +0x638 frame clip), `m_pGFxActEvTopText` (+0x640). They are
set with `gotoAndPlay(label)` `0x1438F9470`, `gotoAndStop(label)` `0x1438F96E0`,
`gotoAndStop(frame)` `0x1438F9810` and the bitmap-number setter `0x1415499C0`.

### 5.2 States and the per-frame order

State object at +0xAA0 (13 states, current at +0xAC8; table built by `0x14161E540`, names are the
`FName`s it registers) [C]:

| # | Name | Enter | Update | Exit |
|---|---|---|---|---|
| 0 | In | 0x14161A650 | 0x141622010 | — |
| 1 | MainSel | 0x14161A680 | 0x1416220D0 | — |
| 2 | SubSel | 0x14161A8B0 | 0x1416236C0 | 0x14161C190 |
| 3 | Shortcut | 0x14161A830 | 0x141623360 | 0x14161C090 |
| 4 | StyleWait | 0x14161A8A0 | 0x141623620 | 0x14161C120 |
| 5 | Attraction | 0x14161A630 | 0x141621720 | — |
| 6 | ShootFlow | 0x14161A7D0 | 0x1416231E0 | 0x14161C060 |
| 7 | Action | 0x14161A600 | 0x1416215E0 | 0x14161C010 |
| 8 | Vehicle | 0x14161A940 | 0x1416244C0 | 0x14161C200 |
| 9 | Trinity | — | 0x141624250 | — |
| 10, 11 | Out, Wait | — | — | — |
| 12 | Hologram | (not read) | 0x141621DA0 | |

Order inside the update `0x14162FF80` [C]: (1) `setLegacyMode(flag)` when its dirty byte is set;
(2) cache MP, "MP full" and "MP Charge" of the pawn (`0x141632500`: vt+0x17D8, +0x1828, +0x17E0);
(3) party list (`0x1416195A0`); (4) **state update**; (5) `setBattleMode` (5.3); (6) main and sub
row enable / colour (`0x141632660`, 5.4); (7) shortcut rows while in state 3 (`0x141632AB0`, 5.5);
(8) situation timers (`0x141630AF0`, 5.6); (9) form gauge (`0x1416318A0`, 5.8); (10)
`setActionCommandMark` when dirty (`0x141631110`, 5.6); (11) `initSubCommand2`, `initSubCommand`
(item list) and `setSubCommandNumText` when their dirty bytes are set; (12) the timed override
slots (four entries at +0x9F8, not decoded: logic); (13) `unloadLogoLoader()` one frame after the
movie reported callback 20.

AS3 -> native: `OnCallback(10)` = the attack-slot text animation ended (resets the Attack text),
`OnCallback(20)` = form logo out finished (slot 81 `0x141626120`).

### 5.3 Appearing, battle colour, main cursor

| When | Call | Arguments |
|---|---|---|
| State In, after **0.01 s** [I] (`0x141622010`) | `initMainCommand(s1, s2, s3, s4)` then `setState(mode, simpleHud)` (`0x14162D720`) | texts = the four `buttons` of `MainCommandData` row **`NORMAL`** [I name] localised (Attack, Magic, Items, Link); a slot whose list is not available shows `kh3_system.command.unknown` = "? ? ?". mode 0 = first in (AS `MAIN_COMMAND_FIRST_IN`: title, main, centre and back play `in`), 2 = style in (a form is already active), 3 = vehicle in; `simpleHud` = byte +0x6E0. Then state 1 (state 8 for a vehicle) |
| Battle state changes (`0x141631D90`, every frame) | `setBattleMode(mode, keepTitle)` | mode 0 = field (`cmdBlue`), 1 = `cmdYellow` (`0x140EAAEE0`), 2 = battle (`cmdRed`: `0x140EABA10` or `0x140EAAF00` true). `keepTitle` = 1 when the title plate must not be recoloured (a vehicle style, or state 3). Also marks the item numbers and shortcut rows dirty: **numbers use the label `battle` in battle and `nomal` (sic) out of battle** |
| Link list empty / present (`0x141626290`, slot 92) | `SetLinkCommandNone(none)` | hides slot 4 and switches the frame art to three rows (`com3`); the cursor then runs over 3 slots |
| Main cursor moved (`0x141617D40`) | `selectMainCommand(index, situationIndex, attackStatic)` | normal mode: `(index, -1, false)`. Plays `over` on the slot and `on` on its text, `up` / `off` on the previous one. Sound: **static SE 0** [I] |
| Attack is replaced (`0x141618040`) | `setMainCommandText(slot, text, flash)` | used for the Attack slot of other characters and for the Item slot ("Cuisine") |
| Bad status on Attack (`0x141618290`) | `setMainCommandTextBadStatus(0, text, on)` | SNEEZE (kind 21): `on` = 1 plays `in2` on the flash clip, 0 `out` |
| Keyblade switched with left / right on Attack (`0x1416220D0`) | `setMainCommand(10)` + text set by `0x14162A610` | 10 = `ATTACK_TEXT_CHANGE` (labels `weapon` / `weapon2`, 60 frames); the movie answers `OnCallback(10)` |
| Attack has a second action (pawn request vt+0x748 and `0x141621130`) (`0x14162A220`) | `setMainCommandRightIcon(0, on)` | only while the cursor is on Attack |

Cursor rule shown by the calls [C]: up / down move over `0 .. 3` (`0 .. 2` without Link) and **wrap**
in the normal mode; the sound is played only when the index changes. Confirm on Attack executes;
confirm on Magic / Items / Link with a non-empty, enabled list opens the sub list; a refused confirm
plays **static SE 4** [I].

### 5.4 Sub lists (Magic, Items, Link) and the party target list

| When | Call | Arguments |
|---|---|---|
| A list is rebuilt (`0x14161F300`; item list from `0x1416172B0`) | `initSubCommand(list, count)` | list 1 = Magic, 2 = Items, 3 = Link; `count` = rows shown, **at most 9**. AS3: rows are bottom-aligned: with fewer than 4 rows the first used clip is `cmdSub1_6`, else `cmdSub1_(10 - count)` |
| Party changes (`0x14161EC90`) | `initSubCommand2(4, friends)` | second level: at most 6 rows (first used clip `cmdSub2_4`, or `cmdSub2_(7 - n)` from 3 rows) |
| Confirm (pad key 27) or right (12) on Magic / Items / Link (state 1 -> 2) | `setSubCommandText(t1 .. t9)` then `setSubCommand(0, cursor)` (`0x14162DDB0`) | texts = the commands' `LocKey` texts in list order (empty string for unused rows); op 0 = `SUB_COMMAND_IN`: `cmdSubTop` takes the colour label of the list, the rows play `up`, the main slots slide away (`next_N` on the chosen slot, `out` on the others). `cursor` = remembered row. Sound **static SE 6** [I] |
| Row cursor (`0x141627CF0`) | `selectSubCommand(row)` | **wraps both ways** [I]; nothing happens with a single row. Sound static SE 0 |
| Cancel (pad key 28) or left (13) at level 1 | `setSubCommand(2, 0)` on cancel (cursor back to Attack), `setSubCommand(1, 0)` on left (main cursor kept) | 2 = `CANCEL_OUT`, 1 = `OUT`; both play `out` on the set and bring the main slots back (`turn1_N` / `turn2_N`). Sound **static SE 3** [I]. State 1 |
| Command executed from the list (`0x141617CF0`) | `setSubCommand(3, 0)` | 3 = `DECIDE_OUT`; row and main cursors are reset to 0 (Attack) and the state returns to 1 (or 4 while a style change is pending). With configuration byte 1 bit 7 set (+0x9C2, default 0 = Config "Cursor Position: Lock", 10.8) the cursor stays where it was instead. No UI sound: the command's own sounds play |
| Confirm / right on a row that needs a target, with at least 2 party members and the party-target flag (+0x9BC) | `setSubCommand2Text(rowText, n1 .. n6)` then `setSubCommand(4, cursor)` | 4 = `FINISH_IN` -> AS3 level 2: the chosen row moves to `cmdSub1Next` (`nextIn`), the other rows play `out`, the party set plays `in`. Names = the party pawns' display names. Sound static SE 6 |
| Back from the target list | `setSubCommand(1, 0)` with level 2 | rows `turnIn`, next row `turnOut`. Sound static SE 3 |
| Numbers (`0x141633B60`, when dirty) | `setSubCommandNumText(list, count, inBattle, n1 ..)` and at level 2 `setSubCommandNextNumText(inBattle, n)` | **Only item rows have a number: the pouch quantity as `kh3_system.cockpit.cockpit_0001` = "x{0}" when it is 2 or more, capped at 9; 1 gives an empty string (number clip `off`). Magic and Link rows have no number: the command menu never shows an MP cost** |

Row state, every frame (`0x141632660`) [C]: for each row the driver computes `enabled` and
`mpShort` and, when one changed, sets the row's text clip (`m_pGFxSubCmdText[row]`) to `disabled`,
or `on` (selected row) / `off`, and the name clip (`m_pGFxSubCmdName[row]`) to **`col2` when
`mpShort`** (the controller's vt+0x1570 says the command costs more MP than Sora has: it can still
be cast and will start MP Charge) else `col1`. The same for the carried row at level 2. A main slot
whose list has **no enabled row** gets `disabled` on its text clip; otherwise `on` when it holds the
cursor, `off` when not.

What greys a row (from `0x141620DE0`, `0x141617830`; details are logic, 5.10): the list or the whole
menu is restricted; a magic row while **MP Charge is running** (so the Magic slot itself turns
`disabled` during MP Charge); a Link row unless **MP is full**; an item row with quantity 0; magic
and link rows before the story flag that gives the MP gauge (game flag byte +0x95B0 bit 2).

### 5.5 Shortcuts (state 3, while pad key 4 = L1 is held)

| When | Call | Arguments |
|---|---|---|
| Enter (`0x14161A830` -> `0x14162CAE0(0)`) | `setShortcutText(t1, t2, t3, t4)`, `setShortcutTextStatus(e1 .. e4)`, `setModeShortcut(0, page)`; then `setActionSelect(4, 0, 0)` | page = the set to show (0 = A, 1 = B, 2 = C), remembered per character mode (Sora: `UIManager+0x6B54`). Texts = names of the set's four commands; an empty entry shows `kh3_system.general.general_0001`. **Row order = face button right, top, left, bottom** (the movie writes the icons Circle, Triangle, Square, Cross into rows 1 .. 4; with the default layout that is B, Y, X, A). `eN` = 1 flashes the row (used when Sneeze replaces a spell). AS3 `SHORTCUT_COMMAND_IN`: panel `in`, page dots, title `scutIn`, main slots `out2` (`MAIN_COMMAND_LOGO_OUT`). `setActionSelect(4)` darkens the situation commands (`dark`) |
| Every frame in state 3 (`0x141632AB0`) | per row: `m_pGFxShortcut[i]` `on` / `disabled`, `m_pGFxShortcutName[i]` `col1` / `col2` | same `enabled` / `mpShort` rule as the sub list |
| Quantities (`0x141632E40`) | `setShortcutCommandNumText(inBattle, n1 .. n4)` | "x{0}" for item entries with 2 or more |
| D-pad down (11) / up (10) | `setModeShortcut(3, page)` / `setModeShortcut(4, page)` after resending texts and status | page + 1 / page - 1, wrapping over 0 .. 2 [C]. Sound **static SE 6** [I] |
| A face button whose entry can execute | `setModeShortcut(2, page)` (`OUT_EXEC`) when the command is of a kind that changes the menu style, else `setModeShortcut(1, page)`; state 4 | no UI sound |
| A face button whose entry is empty or refused | — | **static SE 4** [I] (`0x14161E050`, `0x14161D450`) |
| L1 released (exit `0x14161C090`) | `setModeShortcut(1, page)`; `setActionSelect(5, 0, 0)` unless situation commands are restricted | 1 = `OUT`: panel `out`, title `scutOut`, main slots `in2`; 5 = `ENABLE` (`back`) |

### 5.6 Situation commands

Entries live in a list of at most 6 (+0x760, 0x58 bytes each: kind +8, total time +0x14, remaining
+0x18, start delay +0x1C, appear time +0x20, grace +0x24, AS button index +0x28, serial +0x34,
selectable +0x38) plus two **event buttons**: slot A (+0x770: reaction commands such as talk / open)
and slot B (+0x7C8: finish commands). Cockpit entry points: add `0x1415755A0` / `0x141575640` ->
`0x1416158F0(part, type, kind, exKind, time, info)`; remove `0x14161B910` -> `0x14161AB00`.

| When | Call | Arguments |
|---|---|---|
| Added to the list | `addActionSelectButton(position, name, iconType, inEffect, hasTimer)` -> returns the button index | `position` = index in the list (new entries go above the older ones but below style-gauge entries); `iconType` 0 none ("?"), 1 `wep` (form change), 2 `atf` (attraction), 3 `magic` (grand magic), 4 `friend` (team attack), 5 `link`, 100 = the style-point gauge; `inEffect` 0 .. 3 = `in / in2 / in3 / in4` flash (3 for kind 0xD4); `hasTimer` = total time > 0 (shows counter and gauge). The existing buttons at or above the position are pushed up by the movie. A 7th entry first removes the oldest timed one |
| Same kind added again | `playActionCommandEffect(index)` | the time is refreshed; the flash replays |
| Every frame per timed entry (`0x141630ED0`) | `m_pGFxActionCmdTime[b]` = `trunc(t * 10) / 10` (seconds), `m_pGFxActionCmdTime2[b]` = `trunc(t * 10) % 10` (tenths), value capped at **99.9** [I]; `m_pGFxActionCmdCount[b]` -> **`red` below 10.0 s**, `loop` otherwise [I]; `m_pGFxActionCmdGgAnm[b]` `gotoAndStop(219 - Ff(t / total, 1, 214))` [I] | `Ff` = `0x141598C50` [I]: 0 when max <= 0, `frames` when v >= max, else `trunc(v * frames / max + 0.5)` (no clamp). Full = frame 5, empty = frame 219 |
| Timer run (`0x141630AF0`) | — | each entry first waits its start delay (**0.5 s** [I]; the appear time is **0.2 s** [I]), then `remaining -= dt * dilation` (`0x1445D0960`). Timers stop while byte +0x751 or the flags +0x9B8 / +0x9B9 (a command is executing) are set |
| Usable state per entry | `m_pGFxActionCmdText[b]` `disabled`, or `on` (selected) / `off` | re-evaluated with the same kind test as the lists |
| Selection changes (`0x141631110`, when dirty) | `setActionCommandMark(index, selected, restricted, showTriangle)` | `index` = list index of the active entry, **-1 = the event button**; `selected` true plays `up2` on that button, shows the pad icon `<icon(Triangle)>` when `showTriangle`, and `up` on all others; `restricted` true = nothing selected (menu restricted, shortcut open, shotlock aim) |
| Switch with the left trigger (pad key 6) (`0x14162BAF0`) | (mark, as above) | steps to the next selectable entry, wrapping; sound **static SE 0** [I] only when the entry changed |
| Used (triangle, pad key 0) | `deleteActionSelectButton(index, true, isFormChange)` | AS3 `DECIDE_OUT` (label `down`), the buttons above slide down (`sitaN`); `isFormChange` plays `finishIn` on the form logo |
| Expired (`0x141630AF0`) | `deleteActionSelectButton(index, false, false)` (label `out`) | when `remaining` passes 0 and the grace time is over |
| Time drained by `0x1416303E0(part, seconds)` (caller not found) | `breakActionSelectButton(index)` (label `break`), grace time set to **0.2667 s** (16 frames) [I], then the delete above | the same function drains the form gauge and calls `breakActionSelectEventButton` |
| Event button added | `addActionSelectEventButton(name, hasGauge, effect)` | effect 2 for slot A, 1 for slot B |
| Event button removed / expired | `deleteActionSelectEventButton(used, isFormChange, gaugeStays)`; `breakActionSelectEventButton(breakGauge, breakButton)` | |
| Menu modes | `setActionSelect(op, a, b)` (`0x141627E80`) | 0 in, 1 out, 2 push up, 3 push down, 4 disable (`dark`), 5 enable (`back`), 10 / 11 event row in / out, 12 disabled in |

### 5.7 Style gauge (form-change points)

`0x14158C410` -> `0x14162DC40(part, ratio)` [I thresholds]: phase 0 below 0.25, 1 below 0.5, 2 below
0.75, 3 below 1.0; a phase change calls `setStylePointPhase(phase)` (labels `in / st1`, `in2 /
st2`, `in3` on the gauge button, which is added as a list entry of icon type 100); at 1.0 the gauge
entry is removed (`0x14161BB00`).

### 5.8 Form change and attraction ("style") banners

| When | Call | Arguments |
|---|---|---|
| A form / attraction begins (`0x14158CE20`, `0x14158D7D0` -> `0x14162EF30`) | `setStyle(styleIn, styleOut, 1, iconType, hasTimer, logoPath)`; the main slots are then renamed through `setModeIn(style, lines, iconType, hasTimer, logoPath, text1, button1, ...)` (`0x14162AB00`) | the row of `MainCommandData` for the command kind gives the style id (+0x50), the texts and button types of the mode rows and the logo movie (`act_logo/*`, loaded by the movie itself with a `Loader`). The title plate shows the logo instead of "COMMANDS"; `setTitleTop(4, 1)` for the rows without a logo |
| Form time gauge (`0x1416318A0` -> `0x141631BC0`) | `m_pGFxActLogoGg[0]` `gotoAndStop(201 - Ff(t / total, 1, 200))` [I]; `m_pGFxActLogoGg[1]` -> **`alert` below 10 s**, `loop` otherwise [I]; `hideLogoGauge()` when it reaches 0 after the finish was used | total = controller vt+0x1A98, remaining = vt+0x1B30 while vt+0x1A70 names this form's command |
| The form ends (`0x14157B000` -> `0x14161C3F0`) | `setStyle(0, styleOut, mode)`, then `setActionSelect(3)`, `(0)`, `(10)` | back to the `NORMAL` texts |
| Attraction state (5) leaves | `setModeOut(style)`, `setModeIn(0)` | |

Not needed for Sora in a test level without forms: tier 3.

### 5.9 Shotlock panel (state 6) and world skins

- While the shotlock aim is held (`0x14161A7D0` -> `0x14162C120`): `setActionSelect(4)` and
  `setShootFlow(0, lines, flags, fromShortcut, text1, button1, ...)`: op 0 = in (2 = change, 1 = out
  on leaving, 3 = logo). The rows come from `MainCommandData` row **`SHOOTFLOW`** (Shotlocks =
  decide, Airstep = left face button) or **`FPCHARGE`** (Focus Syphon, Airstep) when the pawn's
  focus-charge flag (vt+0x19D0) is set [I names]. The title plate shows `cmdSlock`. On leaving
  `setActionSelect(5)`.
- **World skins**: the 13 `HudCommand<XX>` movies have the same instance names and the same AS3
  interface (`data/world_skins.json` lists movie per world); the driver is identical. A test level
  uses the default movie.
- `setLegacyMode(flag)`: the flag is configuration byte 2 bit 1 (`(GameInstance+0x870)+0x2EA`,
  **default 0**) [C]. With it set the cursor can walk from the main slots up into the situation
  commands and confirm executes them (`selectMainCommand(index, situationIndex, true)`, label `over`
  instead of `up2`). **The bit is the Config option "Control Type": Type B** (10.8, row 17; help text
  "Use up / down to select situation commands"); it is not the world-skin option as an earlier
  revision of this section said.

### 5.10 Logic rules the display code reveals (for the command-system specs)

1. Pad key numbers used by the menu (UI pad at `UIManager+0x408`): 27 decide, 28 cancel, 10 / 11
   up / down, 12 / 13 right / left, 0 top face button (situation command), 4 L1 (shortcut modifier),
   6 L2 (situation switch), 2 / 0 / 3 / 1 = right / top / left / bottom face buttons in the shortcut
   panel [C].
2. Magic rows are disabled while MP Charge runs and are **not** disabled by low MP: `mpShort` only
   recolours them. Grand-magic kinds 32, 36, 40, 44, 48 are exempt from the MP Charge test. Link
   rows need full MP. Cure rows (49 .. 51) are refused by the "magic restricted" byte +0x73B alone,
   other spells also by `mpShort` when the byte +0x9BD is set [C `0x141617830`].
3. Kinds 49 .. 51 (Cure) and the items 58, 59, 61, 62, 64, 68 open the party target list when there
   are at least two party members and +0x9BC is set; otherwise they execute at once on the default
   target [C `0x14161F300`, `0x1416236C0`].
4. Under water (+0x9EA set and character mode not 3) the kinds 29 .. 32 / 33 .. 36 / 37 .. 40 /
   45 .. 48 are shown and executed as 53 / 54 / 55 / 56 [C `0x14161DAB0`].
5. With Sneeze (+0xA59) every magic entry other than Cure executes kind 21 and the Magic list
   closes at once [C].
6. Situation commands: at most 6 list entries; which entry the top face button uses when none is
   selected: the oldest when +0x9C4 = 1 (default, `!(config byte 2 bit 0)`), the newest when 0
   ("Situation Commands: Prioritize Older / Newer") [C `0x141631110`]. An event button, when
   present, has priority over the list.
7. When a situation command is used while the pawn has ability 0xAC (vt+0x14E0), the used entry's
   remaining time is added to every other running entry (cap 99.9 s) [C `0x14161AB00`].
8. Item rows of the list are rebuilt from the pouch each time the menu enters MainSel and whenever a
   quantity changes; identical items in several pouch slots give one row [C `0x1416172B0`].
9. The slow motion of "Command Selection: Slow" is entered and left by the SubSel state only
   (`0x141615760`), never by the shortcut panel [C].

Still open for the display: the timed override slots (+0x9F8), states 7 .. 9 and 12 (not Sora in a
test level), the exact `simpleHud` byte (+0x6E0) source, and which sound (if any) the *game* side
plays when a situation command appears: the HUD part plays none.

## 6. Right HUD: `HudRight` (enemy gauge, level up, minimap)

Bound style: `UTresUIP_HudRight` creates sub parts from the movie's `parts` array.

- **Enemy HP** (`HudRightEnmHpPack`; update `0x14165F820` [C differs]): one bar = **200 HP**, shown
  HP capped at 24,000; for targets whose vfunc +0x17D0 is true 2,000 / 240,000. `stock = hp / 200`
  full bars under the top bar, top bar frame `F(hp % 200, 200, 500) + 1`; the top bar's base is
  `F(maxHP % 200, 200, 500)` and becomes a full bar when the stock drops. The stock shows as up to
  **20 squares**; above that a "+N" counter [A]. Red lag as for the player with scale 2. Armour
  gauge the same way with 500 frames.
  **Which enemy** (`0x14158A860(cockpit, pawn, locked, prevHP)` -> `0x14165A050`) [C; constants I]: the gauge
  needs the ability **LIBRA (21, "Scan")** on the player pawn (vt+0x14E0(0x15)) and a target whose
  vt+0x10B0 is true. It is pointed at (a) the pawn that owns a target cursor when an auto (type 0) or
  lock-on (type 1) cursor is added (section 7), (b) the enemy Sora just damaged (two hit paths,
  `0x141442DE0` / `0x141443DE0`, which pass the HP before the hit so the red lag starts there). A new
  pawn replaces the shown one unless the shown one was set with `locked` and the new one was not.
  **Hide**: when the last auto / lock-on cursor is removed the target is cleared with a hide timer of
  **0.5 s** (the gauge stays for half a second, then slot +0x288 hides it); it hides at once when the
  target pawn dies or the caller forces it.
- **Level up** (`HudRightLevelUp`) [A]: `setParam(level, character, icon, line1, line2, line3)`;
  window art by the number of lines; closing reported through `OnCallback`. An array of popups is
  owned (`m_LevelUp`), so several can queue.
- **Get bonus** (`HudRightGetBonus`), mission counter, timer: stubs.
- **Minimap** [A + C]: `NaviMapGra` is rendered natively into a render target and shown through
  `naviMapCanvas`; map definitions in `UI/Data/NaviMapData` (107 rows). Host-supplied in the port.

## 7. Left HUD and world-anchored HUD

- `HudLeft` [A]: information line (`open(text)` / `close()`; long text scrolls), obtained item
  (`setParam(icon, name, stockFull)`: the "stock full" colour), munny (`setParam(amount)`),
  **Next LV** (`setParam(n)`: EXP still needed for the next level), mission gauge, score, timer.
  Each popup reports its end through `OnCallback`.
- `HudTarget` [A]: the world-anchored layer (7.1 - 7.4): target cursors, shotlock scope and lock
  counter, EXP points, interaction prompts (`setupPushButtonMark`, `openButtonNotice`), additional
  input prompts. Update slot 90 `0x14167CEF0` [C `same`], projection slot 91 `0x141677E90`.

- **There are no damage numbers** in KH3's HUD [A]: no such clip or class exists in any HUD movie;
  the only floating numbers are EXP points.

### 7.1 Target cursors

Types and pools (AS3 `m_nLockonCursorTypeMax`) [A]: 0 **auto** target (`ALockonMark`, 10 clips,
labels off 1 / in 7 / loop 13 / out 21), 1 **lock-on** (`MLockonMark`, 2 clips: in 9, loop 17 .. 288,
frame 289 jumps back to `loop`), 2 shotlock mark (`FlowMarkAtk`, 32, sub label `shoot`), 3 focus-syphon
mark (same clips, sub label `aspil`), 4 airstep mark (`FlowMarkMov`, 10), 5 action (2), 6 athletic
flow (2), 7 attraction (6, with a gauge of 360 frames).

- **Add**: cockpit `0x1415759C0(cockpit, targetComponent, type, allowMulti, key)` ->
  `0x141668C60` [C]: takes a free clip of the type's pool and calls `addTargetCursor(index, type, 0,
  0)` (the clip plays `in`). Refused when the pool is full or the same component already has a
  cursor of that key and `allowMulti` is false. The controller's target search (`0x141404E50`, combat
  spec 7.2) adds type 0 for the current target and type 1 for the manual lock; the shotlock aim adds
  type 2 / 3 with `allowMulti` and the airstep search type 4 [I call arguments]. Which component is
  the current target is combat-spec logic; the HUD only follows.
- **Several marks on one target** (types 2, 3) [I constants]: the n-th extra mark is offset by a
  radius `rand * 25 + 10 + 2 n` px at the angle `((serial & 3) + n) * 1.7` rad and rotated by
  `-angle` (degrees); the offset is scaled with distance (below).
- **Remove**: cockpit `0x14157A7A0` -> `0x14166C730`: `deleteTargetCursor(index, type)` =
  `removeChild`, **no out animation** (attraction marks play `out` / `select` and report
  `OnCallback(1)`). A cursor whose component died is removed by the projection pass.
- **Sounds** [I]: adding a lock-on cursor plays **static SE 7**, removing one **static SE 8**; adding
  an airstep mark **static SE 19**. The auto cursor is silent.

### 7.2 Projection (every drawn frame)

The UI manager registers `0x1415A52F0` on a viewport draw delegate (`0x1415AB210`); it calls slot 91
of every visible part. `0x141677E90` builds the view block from the local player (controller
+0x448): view rectangle `(x0, y0, x1, y1)` at +0x420, view-projection matrix at +0x1B0, and the
factors `kx = 1920 / (x1 - x0)`, `ky = 1080 / (y1 - y0)` [I]. Then per cursor (`0x14167E1D0`)
[C; constants I]:

```
p  = component.vt+0xB88()                      // lock-on point of the target (or the stored position)
(cx, cy, cw, cz) = p * ViewProjection          // row-vector convention, 0x140ECF670
w  = cw (1e-6 when |cw| <= 1e-8);  sx = (cx / w + 1) * (x1 - x0) / 2 + x0
                                   sy = (1 - cy / w) * (y1 - y0) / 2 + y0;   depth = cw > 0 ? cz / w : 0
k  = min(2.0, depth * 50 + 0.25)               // scale of the multi-mark offset
x  = sx - x0 + offX * k;  y = sy - y0 + offY * k
onScreen = -100 <= x < (x1 - x0) + 100  and  -100 <= y < (y1 - y0) + 100  and  depth > 0
if onScreen: x *= kx; y *= ky
visible = onScreen and !component.vt+0xA68() and the cursor's HUD-layer flag still matches
updateTargetCursor(index, type, round(x), round(y), round(rotation), visible)   // round half away from zero
```

**There is no clamping to the screen edge and no off-screen arrow**: a target outside the view (with
a 100 px margin) or behind the camera simply has its cursor hidden. `depth` is the projected z / w
(with the engine's reversed infinite projection that is near plane / distance, so `k` = 2 closer
than about 2.9 m and falls towards 0.25 far away [derived, near plane 10 assumed]). The stage
position is exact only because the stage is stretched over the same 16:9 rectangle as the view
(section 11a).

### 7.3 Shotlock aim overlay (UI half; logic: magic spec part D, S2)

| Controller event (magic spec S2) | Cockpit call | HUD effect |
|---|---|---|
| Aim opens (hold > 0.2667 s) | `0x141587480(cockpit, aimTime, syphon, kind)` -> `0x14167A8F0(target, 0, aimTime, syphon)` | `setFlowScope(0, syphon, aimTime <= 0)`: clip `flowScope` (or `flowScopeC` for focus syphon) at the stage centre (960, 540) plays `in` -> `loop`; with no aim time the filter clips show `empty` and the gauge is at frame 1. Sound **static SE 18** (**25** for syphon or no time) [I], kept in `m_FocusModeAudio` and stopped on any later scope state. A full-screen colour filter is blended in natively (`0x1415AACB0`, colours and times from the UI data asset; optional for a port). The command menu enters state 6 (5.9) |
| Every frame while open (`0x14167CEF0`) | — | `updateFlowScope(Ff(1 - remaining / aimTime, 1, 316) + 1)` [I]: the ring gauge `fSGage` (317 frames) fills as the time runs out; at 0 `setFlowScope(11)` (line clip `empty`). The HUD counts the time down itself with the undilated frame time |
| A lock is added | `0x1415759C0(cockpit, component, 2 or 3, true, key)`; `0x1415895F0(cockpit, total, max)` -> `0x14167B630` | a new mark clip on the target (7.1); `setLockonCount(total, max)`: counter `flowScope.flowLock` (at 1246, 730) plays `in` the first time and shows `total`; **`total == max` plays `max` on `flowLockMax`**. Sound **static SE 19** per lock, **20** on the lock that reaches the maximum [I] |
| Can shoot / can airstep changes | `0x14158B420(cockpit, row, enabled)` | `setEnableCommandExText(row, enabled, false)` on the mode panel of the command menu (row 0 Shotlocks, 1 Airstep) |
| MAX reached: second shotlock | `0x14158C380(cockpit, kind)` | `setModeCommandText(0, name, true)`: row 0 is renamed (Kingdom Key: to the Ragnarok text) with the change flash |
| Released / cancelled | `0x141577520(cockpit, state)` -> `setFlowScope(state, false, true)` | 1 = out (cancel), 2 = `select` (shotlock fired), 4 = `select2` (airstep), 5 = `out2`, 6 = off; the lock counter is a child of the scope clip and leaves with it (`closeLockonCount` has no native caller), the marks are removed one by one, i.e. by one remove call per candidate in the same frame (magic spec A3.3 / A3.4): on a cancel and on an airstep with the scope; **on a shot only the airstep marks go with the scope and the shotlock marks stay on their targets until the shotlock's state has ended** (controller mode 2 cleaned up), when the scope gets state 3 (or 6), which the movie ignores |

The reticle art is authored for the 1920 x 1080 stage; the lock test circle of the controller is
150 px at 1280 x 720 = **225 px on the stage**, centred. The focus gauge of the status HUD shows the
pending cost while aiming (section 4, "consume mode").

### 7.4 Style-gauge full notice

`0x14158C410(cockpit, ratio)` (the form-change point ratio that also drives 5.7) sets a flag on the
target HUD when `ratio >= 1.0`; its next update plays **static SE 26** and `setStylePoinMax()`
(clip `styleMaxInfo` at (960, 886), 84 frames) [I].

## 8. EXP and levels

- Level table [A]: `DataTable/Player/LevelUp/p_ex001_LevelUpData`, 99 rows; `m_Exp` is the **total
  EXP** for the level (2: 80, 3: 280, 10: 5,410, 50: 376,030, 99: 1,527,640) with Strength, Magic,
  Defense, AP and the abilities learned (`data/levelup_p_ex001.json`).
- Shown in: the floating EXP points (HudTarget), "Next LV" (HudLeft), the level-up popup
  (HudRight), the camp menu top panels (level, EXP gauge) and the Stats screen (Level, Total EXP,
  Next Level, gauge).
- **Game side** (traced by the stats agent, `docs/sora-stats-spec.md` 4.2 - 4.4; reconciled, no
  conflict): `GiveExp` 0x140E9E830 adjusts the reward (difficulty, Zero EXP, EXP bargain, at most
  999 per award), adds it to the single total (`GW+0x7E00`, cap 9,999,999), runs `CheckLevelUp`
  0x140EBE400 for Sora and each party member, then calls the HUD.
- **HUD side** (this pass) [C]:
  1. **Floating EXP point**: `0x141575840(cockpit, worldPosition, added)`, only when a position is
     given and a next level existed before the award. It invokes `addExpPoint(added)`, keeps
     (clip index, position, lifetime **3.0 s**) in a list at HudTarget +0x2B8 and the target update
     re-projects each entry every frame (`updateExpPoint`). The clip animates 122 frames (2.03 s).
  2. **Next LV**: `0x141589570(cockpit, nextLevelExp - total)` after the level-up check, while a
     next level exists (nothing at level 99): `setParam(n)` + show on `HudLeft.m_NextLv`; visible
     146 frames (2.43 s).
  3. **Level up**: `CheckLevelUp` -> `0x141589330(cockpit, level, pawn, deltas)`: the first free
     popup of `HudRight.m_LevelUp` is filled (0x14165D770) and shown; a popup already on screen for
     the same character is updated in place (0x141659CD0). **One award that crosses several levels
     gives one popup** with the final level (stats spec 4.3); lines = non-zero deltas of Strength,
     Magic, Defense, AP and the ability. The clip runs 272 frames (4.53 s) without stopping.
  Order within one award: EXP point, level-up popup(s), Next LV.
- **Pickups** [C]: munny `0x141587530(cockpit, amount)` (from 0x140E9EFA0): the counter
  **accumulates while visible**, restarts otherwise, shows at most 9999, 2.43 s. Item
  `0x141587510(cockpit, itemId, stockFull)` (from 0x140E9E9D0): one popup at a time, further items
  are **queued** and shown as each closes, 1.9 s each.
- **Left stack** [C]: the left popups share at most **4 rows**; a part's row is the number of
  visible parts before it in HudLeft's list; rows slide when one closes (update 0x14164B2D0).

## 9. Sounds

- Menu sounds [C differs]: `UTresUIParts::PlaySystemSE(part, id)` `0x14176F7D0`, ids 0 Click,
  1 Decide, 2 Cancel, 3 Beep, 4 Tab, 5 GameHelpOpen, 6 GameHelpMessage (= the members of
  `TresUISystemSEDataSet`). With an override set (`TresUIManager.m_OverrideSystemSEDataSet`, used
  only by the gummi editor) its sounds play; otherwise the id maps to a **static SE** index
  0 / 1 / 3 / 4 / 2 / 2 / 5.
- Static SE [C `same`]: `0x141515B30(world, index, 1.0, 1.0, 0, flag)` -> table lookup
  `0x1415157D0`: `TresStaticSESet.m_Assets[43]` of `Load/Common/TresStaticSE` =
  `/Game/Sound/SE/common/00000/se00000_NNN` [A] (`data/ui_sounds.json`).
- Low-HP alarm [C]: `0x14158A340(cockpit, 1)` starts static SE 9 as a looping component kept in
  `m_HPAlertAudio`; `0x14158D800` [`same`] stops it.
- A part can also play a static SE by index: `0x14176F6B0(part, index)` (2D) and
  `0x14176F540(part, index, position)` (3D). **No movie contains a sound** [A]: no sound tag occurs
  in the 276 movies and no AS3 class uses the sound API, so every UI sound is one of these native
  calls. HUD cues identified in this pass [I: the index at each call site]:

| Event | Static SE | Sound | Call site |
|---|---|---|---|
| Command menu: cursor (main, sub list, situation switch) | 0 | se00000_001 | 0x141617D40, 0x141627CF0, 0x14162BAF0 |
| Command menu: sub list opens, party target list opens, shortcut page changes | 6 | se00000_007 | 0x1416220D0, 0x1416236C0, 0x14162CAE0 |
| Command menu: cancel / back | 3 | se00000_004 | 0x1416236C0 |
| Command menu: refused (disabled entry, empty shortcut, failed confirm) | 4 | se00000_005 | 0x1416220D0, 0x1416236C0, 0x14161D450, 0x14161E050, 0x141621720 |
| Lock-on acquired / released | 7 / 8 | se00000_008 / _009 | 0x141668C60 / 0x14166C730 |
| Airstep target marked; shotlock lock added | 19 | se00000_042 | 0x141668C60; 0x14167B630 |
| Shotlock lock count reaches MAX | 20 | se00000_043 | 0x14167B630 |
| Shotlock aim opens (normal / focus syphon or no aim time) | 18 / 25 | se00000_039 / _051 | 0x14167A8F0 |
| Style gauge (form-change points) full | 26 | se00000_054 | 0x14167CEF0 |
| Attraction mark chosen (3D, at the mark) | 28 | se00000_059 | 0x14166C730 |
| Low-HP alarm (loop) | 9 | se00000_010 | 0x14158A340 |
| Level-up popup opens; item popup opens; bonus popup opens | 12; 33; 38 | se00000_013; _067; _073 | 0x141658390 family, 0x141642070, 0x141658140 (audio spec 5.6) |

- **No HUD sound exists for**: executing a command from the menu or a shortcut, a situation command
  appearing / being used / expiring, MP Charge starting or ending, the focus gauge becoming full, EXP
  points, munny. All 116 callers of `0x14176F6B0`, the 16 direct callers of the static-SE player and
  the 6 callers of the manager wrapper were listed with their index; none belongs to these events
  [I]. Whatever is heard there comes from the action itself (effects, voices: audio spec).
- Static SE 33 at `0x141584180` belongs to a counter part (`HudRight.m_MissionCounter` /
  cockpit +0x100), not to the level-up popup [C]: the audio sheet's "level gained" label for that
  call site needs a second look.
- Requests for the audio agent: `build/sora_tmp/ui/asset_requests.json`.

## 10. Menus (pause screen and camp menu)

One movie, `15_campmenu/CampMenu`, holds every camp screen; the part is `UTresUIP_CampMenu`
(1,184 bytes), the logic object `UTresCampController` (`TresUIManager.m_CampController`, mgr+0x368).
The screens are sub parts (`m_SubMenus[8]`, +0x430): 0 Equipment (`equip`), 1 Abilities (`ability`),
2 Customize, 3 Stats (`status`), 4 Game Help, 5 Config, 6 Items (`backyard`), 7 Cuisine. The
in-battle pause screen is a separate movie (`13_pause/Pause`, `UTresUIP_Pause`, mgr+0x2C8).
Evidence in this section: [I] where a rule decides behaviour, [C] otherwise, [A] for the movies
and tables. Working notes: `build/sora_tmp/ui/notes/camp_native.md`.

### 10.1 From the Menu button to a paused game

1. **Request** (UI manager update `0x1415B0ED0`, gate repeated in `0x141591F70`) [C]: every frame
   after the UI pad poll, when all of these hold: the HUD actor (`m_ActiveHUD`) is absent or its
   vt+0x820 is true; the object at GI+0x630 is not alive (not identified); the player controller is
   not paused (`0x144436BE0`: `WorldSettings.Pauser`, +0x650) [U]; no entry in the manager list at
   +0x738; no full-screen video is playing (`0x141750C30`). Then **UI pad key 22 just pressed**
   (`0x141583B80(pad, 0x16, 0)`; key 22 = `Gamepad_Special_Right`, the Menu button:
   `sora-input-layout` row `pause`) or the game instance's forced request (GI vt+0x408 = GI+0x338
   bit 4) sets **bit 6 of world+0xA50** (`0x1441B6B80`) [I].
2. **Next world tick**: `UWorld::Tick` `0x1441BA050` tests the bit at its start, broadcasts the
   static delegate `0x1487562F0` and clears the bit [C]. The game instance registered
   `0x140EB0A30` on it (`0x140EA9810`): it repeats the gate of step 1 and calls
   `SetPause` = `0x1415A9430(mgr, pause = 1, withMenu = 1, pc = 0, force = 0)`. So the pause begins
   **one frame after the press**, at the start of the frame, before any actor ticks.
3. **`SetPause(true)`** `0x1415A9430` [I for the order]:
   - `MayPause` `0x140EA0680(GI)` must be true (skipped when forced): game phase GI+0x1D0 == 2; no
     screen fade running (three fade channels of the viewport client); GI+0x5F0 == 0 and GI+0x608
     bit 0 clear; **GI+0x338 bit 0 clear** (script `TresSetDisableGamePause` `0x142232C00` -> GI
     vt+0x468); no pause reason already set in the world (world+0xA54); the player pawn's request
     interface vt+0xD8 `0x14142C540` true: state and pending state neither DIE (59) nor GAMEOVER
     (118), pawn+0x634 bit 2 clear.
   - the Pause movie must be loaded;
   - `pc.SetPause(true)` (controller vt+0xB98): the engine pause [U by use]. World time stops and
     actors stop ticking; the UI manager keeps ticking (its time step is the undilated frame delta,
     11a);
   - every UI part gets vt+0x2B8(paused), and the parts of layer 0 / 1 (part+0xAC: the cockpit
     layers) have their movie player paused (`0x141772700`): **HUD timelines freeze**; subtitles too;
   - sound `0x141516030(pauseBgm, 1)`: sound state **`STATE_MENU_OPEN`** (audio spec: every SE
     category except system and LSI and all voices pause); `pauseBgm` (category `BGM` paused with a
     0.2 s fade) only when GI+0x880 bit 0 is set and GI+0x810 is clear, or in the Remy mini-game:
     in normal play **the music keeps playing**. Videos are paused;
   - with `withMenu`: the screen selector below.
4. **Screen selector** `0x1415A2E10(mgr, 0)` [I]: in this order
   - save flag GW+0x64 bit 0 set -> Pause screen;
   - GI+0x3C8 set (theater / memory archive / philharmonic playback) -> their pause variants;
   - GW+0x60 != 0 (set by `TresLevelScriptActor::TresSetPauseMenuType` `0x142703650`): types 1, 2
     -> `0x1417103C0`, 3, 4 -> `0x141711910`, 5 -> `0x1417149D0` (mini-game pause screens);
   - special player modes (`0x140EAB7B0`, gummi ship GI+0x810, hologram GI+0x84C) -> their screens;
   - **camp menu** when all hold, **else the Pause screen** (`0x1417102F0`):
     the unlock flag **`campmenu`** is set (`0x140EA1830(GI, "campmenu")`: a row of
     `TresUnlockFlagDataTable` tested against the game flags); **GI+0x8D4 bit 1 clear** (script
     `TresSetDisableCampMenu` `0x142232B80` -> `0x140EB7880`); the local controller is an
     `ATresPlayerControllerBase` with an `ATresPlayerPawnBase`; the pawn's request interface
     **slot 29 `0x14142BA80` ("may open the camp menu")** is true.
5. **Slot 29, the refuse rules** `0x14142BA80` [I, both switch tables decoded]: false when
   - **in battle**: `(GI+0x8D4 & 0x14) != 0` (bit 2 = enemies engaged, kept 1.0 s after the last
     one: actions spec; bit 4 = `TresSetSpecialBattleMode`), unless the pawn's character kind
     (vt+0xD80) is 11;
   - GI vt+0x278 (GI+0x880 bit 0, set during events) or interface vt+0x618 `0x141436AA0` (state
     SUPER_SLIDE 21, RAIL_SLIDE 22, WALL_KICK 23, POLE_TURN 24, ENEMY_FLOW 25, RAILSLIDE_MI 138, or
     JUMP with its byte +0xF8 set);
   - the pending state (`0x1405AB6E0`) is DAMAGE (19), WEAPONCHANGE (32) or DIE (59);
   - the current state is one of: FLOATING 5, GUARD 15, the attraction / link / team states
     26 - 28, 30, 73, 92, 95 - 97, 101, 103, 110, 112, 114, 115, 117, 119 - 121, 126 - 137,
     163 - 167, STYLECHANGE 31, WEAPONCHANGE 32, STYLEFINISH 33 - 39, SHOOTERMODE 40, SHOOTFLOW 41,
     ATHLETICFLOW 42, OPEN_TREASURE_BOX 74, TALK 104, CHECK 108, COVER 142, HOLD_CAMERA 144,
     FESTIVAL_DANCE 145; **JUMP (3) only while the jump state's field +0x78 is 7 or 8** (`0x14138FAC0`; which jump phases those are is not settled: 13);
     ONE_ACTION (150) unless its byte +0xC0 is 2;
   - the state is an Attack state and pawn vt+0x2748 is true, or the attack kind (pawn vt+0x1DC8)
     is 40 - 43, 53 - 57 or 63 - 67;
   - the controller is aiming a shotlock (ctrl vt+0x1908) or its current style command (vt+0x1A70)
     is 0x8B; pawn+0xE60 bit 17 (command lock, vt+0x2788) is set.

   **So: idle, running, falling, an ordinary jump, dodging, a finished combo on the ground: the
   camp menu opens. Mid-air is not refused as such. In battle the same button gives the Pause
   screen, never the camp menu.** (This answers open question 3 of the magic spec, part J: the
   pouch refill "when the menu closes" is in practice a refill after battle.)
6. **Camp open**: the selector binds the close callback (`0x1415945B0` = `SetPause(mgr, 0, 1)`),
   sets the UI gamma for HDR, caches the unlock flags `smartphone` (mgr+0x5FA0) and `spcamera`,
   and calls `0x1415435D0(campController, 0)` (controller state 1). Controller update
   `0x14154C710` [C]: state 1 -> 2: stop the player, build the party table (`m_PartyParams[5]`,
   0xA0 bytes each at controller+0xC8: +1 character id, +4 the status snapshot of stats spec 1.2,
   +0x70 the display name): slot 0 = the playable character GW+0x7E0E, then up to four friends ->
   3: `UTresUIP_CampMenu` slot 74 `0x1415C8DD0` shows the menu -> 4 .. 8: the character models
   (`TresCampPawnBase` actors spawned 100,000 units apart, one per party slot; 10.9).
7. **Unpause** `SetPause(mgr, 0, 1)`: `pc.SetPause(false)`, parts and videos resumed, sound state
   `STATE_MENU_CLOSE` (`0x141516360`; BGM category resumed with 0.2 s). The game instance tick
   then refills the auto-reload pouch slots on the first unpaused frame out of battle (magic spec
   J 7.5).

**Pause screen (stub)** `UTresUIP_Pause` [C + A]: `0x141712640(pause, type)`: static SE **2**,
root `loop`, `m_PauseTitle` / `m_PauseBG` `in`, hint clip `in` when hint texts are cached
(`TresUI_SetPauseHintText`), buttons from the table at `0x146B37820` (6 ids per type, 30 = none;
texts `kh3_menu.pause.*` through the key table at `0x1486F40C0`): type 0 **no button** (the plain
"PAUSE" of a battle), 1 Continue / Skip Scene, 6 Continue / Game Help (used instead of 0 when a
game-help entry is registered), 7 Continue / Exit, 8 Continue / Retry / Exit, 11 Exit / Retry
(game over). Input (slot 89 `0x141708B30`): the Menu key again (event 13) plays static SE **3**
and closes (title, background, hint and buttons `out`, then the Continue delegate ->
`SetPause(false)`); decide on a button runs its delegate; a disabled button plays static SE 4.

### 10.2 Menu events, navigation, cursor sounds

**Events.** The focus controller `0x141580C80` [C] turns the UI pad (12 bytes per key from pad+8:
state 1 = pressed this frame, 3 = held with the repeat byte set) into events (0x28-byte records:
+0x08 focused anchor proxy, +0x10 type, +0x14 anchor index, +0x18 the anchor's user id, +0x20
direction, +0x24 flags: bit 0 repeat, bit 1 handled), offered first to the active AS proxies of the
part (their slot 78) and then to the part (slot 89) (`0x141765160`):

**Key repeat** [I, added 2026-10-12]. The UI pad lives in the UI manager at +0x408 and is refreshed
by `0x14158E390` (called from the manager's update `0x1415B0ED0` with its frame time), which runs
the per-key update `0x14158EED0` on every 12-byte record (`+0` state, `+4` timer, `+8` repeat
byte, `+9` cleared): pressed this frame -> state 1, **timer = 0.6** (`3F19999Ah` at
`0x14158EF35`); released -> state 2; held -> state 3, repeat byte 0, `timer -= dt`, and when the
timer is below zero the repeat byte is 1 for this frame and **timer = 0.06** (`3D75C28Fh` at
`0x14158EFD6`); otherwise state 0. The overshoot is not carried, so at 60 fps a held direction
repeats first after 0.6 s and then every 4th frame. The decide / cancel keys with alternative
bindings go through `0x14158F000`, which writes the same two constants [C]. Sheet rows
`menu.repeat_first`, `menu.repeat_next`. Not read: whether the frame time passed is dilated.

| Type | Source | Meaning |
|---|---|---|
| 3 / 4 | AS3 `OnAnchorIn` / `OnAnchorOut` (`0x142C12D00` / `0x142C12E40`) | focus entered / left an anchor |
| 7, then 5 | key 27 (decide) | 7 to the focused anchor; 5 to the screen when the anchor did not take it |
| 8, then 6 | key 28 (cancel) | likewise |
| 16 | keys 10 / 11 / 13 / 12 (up / down / left / right; also keys 14 - 17, the stick) | sent only when `moveAnchor(direction, repeat)` did **not** move the focus; a successful move plays **system SE Click** (`0x14176F7D0(part, 0)` = static SE 0) and sends nothing |
| 9 / 10 / 11 / 12 | keys 4 / 5 / 6 / 7 | L1 / R1 / L2 / R2 (tab left / right, character previous / next) |
| 13 | key 22 | the Menu button again |
| 14 / 15 | keys 0 / 3 | top / left face button |
| 19, 17, 18 | keys 23 or 26, 24, 25 | View and the two extra keys (PC) |

**Navigation** [A, `tres.managers.FocusManager`, `tres.core.AnchorComponent`]: every focusable
clip has an `index` and the neighbour indices `toUp / toDown / toLeft / toRight` (-1 = none) with
`repeatStop*` flags, all authored in the movie. `moveAnchor(dir, repeat)`:
1. the focused anchor is asked first (`AnchorComponent.moveAnchor`): a list moves its own
   selection; a plain anchor answers "handled, not moved" when `repeat` is true and its
   `repeatStop<dir>` flag is set (**holding a direction stops at such an edge, a fresh press goes
   on**);
2. otherwise the neighbour index is followed; a neighbour that cannot take focus (itself or an
   ancestor invisible) is skipped in the same direction, at most 5 hops, and the walk stops there
   when `repeat` is true and that neighbour has the `repeatStop<dir>` flag;
3. when the result differs from the current anchor: `OnAnchorOut`, `OnAnchorIn` (events 4, 3) and
   the `Cursor` movie moves to the anchor rectangle.

Native classes hold one proxy per clip: `UTresASProxyTresControlsButton` / `...ListItem` /
`...List` (id set with `0x14154B7B0`, read with `0x141538940`; label `0x141548860`; enabled test
`0x14153EAE0`), all listed per screen below. Text is set with `0x1439093E0` (SetText), frames with
`0x1438F9470` (`gotoAndPlay(label)`) and `0x1438F9810` (`gotoAndStop(frame)`), visibility with
`0x1439099D0`.

**Sounds** [I, index at each call site]: every menu sound is a static SE (section 9): cursor move
**0** (Click), decide **1**, open a screen / tab **2**, cancel / close **3**, refused **4**. The
camp menu plays 2 on opening and 3 on closing (`0x1415C8DD0`, `0x1415C5270`, `0x1415C4870`).

### 10.3 Top screen

`0x1415C8DD0` (slot 74) [I for the entry order and tables]:

- **Entries**, top to bottom, button clips `topMenu.cmdBtnTopMenu.topMenu1 .. 9` at x 170,
  y 132 + 96 n (anchor indices 101 .. 109, `toUp` / `toDown` chained, 101 up -> 109 and 109 down ->
  101 with `repeatStop`: wraps on a fresh press only). Entry id (the anchor's user id) -> text
  `kh3_camp.top.menu_*`, help line `detail_*`, icon label:

  | id | Text | Shown when | Opens |
  |---|---|---|---|
  | 0 | Equipment | always | character list, then `m_SubMenus[0]` |
  | 1 | Abilities | always | character list, then `m_SubMenus[1]` |
  | 2 | Cuisine | unlock flag `cuisine` | `m_SubMenus[7]` |
  | 3 | Items | always | `m_SubMenus[6]` |
  | 4 | Customize | always | `m_SubMenus[2]` |
  | 5 | Stats | always | character list, then `m_SubMenus[3]` |
  | 6 | Gummiphone | unlock flag `smartphone` | delegate +0x2D0 (10.9) |
  | 7 | Game Help | always | `m_SubMenus[4]` |
  | 8 | Config | always | `m_SubMenus[5]` |

  Hidden entries leave no gap (the shown entries fill the clips from the top; unused clips are
  made invisible). **No entry is ever greyed**: the handler has no enabled test. Focus starts on
  entry 0. For a level-99 Sora in a test level with both flags clear: **Equipment, Abilities,
  Items, Customize, Stats, Game Help, Config** (7 rows). The order is the code's (table at
  `0x1486E0FF0`), not the numbering of the text keys.
- **Frame** (`stageFrame`, `tres.rsl.StageFrame`): play time `hours:minutes` from GW+0x7DFC
  (seconds, capped at 3,599,999; refreshed by the update slot 90 `0x1415D1450` when the minute
  changes), munny GW+0x7E04, world name (UI data by GW+0x7DF4) and area name
  (`TresAreaNameDataTable` / `TresMapNameDataTable`), title "MENU" (`kh3_camp.top.title_0001`).
- **Objective** `topMenu.nextGoal.nextGoalInfo`: the text of the UI data row for the world
  settings' byte +0x7C8 (`kh3_camp.top.purpose_*`); "—" rows exist per world.
- **Character panels** `topMenu.statusAnm.status.charaWindow01 .. 05` (`0x1415D3530` ->
  `0x1415D0020`) from `m_PartyParams`: name, **LV** `level`, **EXP gauge** `expGg` (151 frames):
  frame `F(exp - levelExp, nextLevelExp - levelExp, 150) + 1` with the helper of section 4 (at
  level 99 `nextLevelExp` is 0, so the gauge shows frame 1 = empty), **HP** `hp / maxHP`, **MP**
  `mp / maxMP` as plain integers, and an up-arrow icon text beside HP / MP while a meal bonus
  raises it. The panel layout label is `one .. five` by party size (`0x1486E0F50`).
- **Triangle** (event 14): toggles the status panels against the meal-bonus panel ("View Bonuses"
  / "View Team"), static SE 2; only with the unlock flag `cuisine` (`0x1415C3BB0`), so never in a test level without it.
- **Decide** (event 7) on an entry: Equipment / Abilities / Stats first show the **character
  list** (`topMenu` label `inChara`, buttons `charaList.btnChara1 .. 5`, help
  `kh3_camp.equip.char_0001` etc.; [C, added 2026-10-12] the three entries' handlers `0x1415CB3E0` Equipment / `0x1415C9D80` Abilities / `0x1415CB8D0` Stats set the top screen's help text object to `kh3_camp.equip.char_0001` / `kh3_camp.ability.detail_0010` / `kh3_camp.status.detail_0010` and `m_StageCategory2` (`topMenu.stageCategory2`) through `0x1415476B0(category, n, title)` to icon 2 / 3 / 5 with the title `kh3_camp.equip.title_0010` / `ability.title_0010` / `status.title_0010`; the list is shown for a party of one as well); decide on a character plays `outChara` and opens the sub menu
  for that party slot (sub menu vt+0x280). The other entries play `out` on `topMenu` and open
  their sub menu at once.
- **Cancel** (events 6 / 8) or **Menu** (13): from the character list back to the entries; from
  the entries: close (`0x1415C4870`): slot 75 `0x1415BD5F0` plays root `out`, static SE **3**, the
  close delegate unpauses. When the menu was opened by script with
  `TresUI_OpenCampMenuNeedCloseConfirm` (controller mode 2 or 4) the dialog
  `kh3_camp.top.dialog_0010` asks first.
- While a sub menu is open every event goes to it; the Menu key asks the sub menu to step back
  (its vt+0x2A8) or, when it is at its own top level (vt+0x2A0), closes the whole menu.

### 10.4 Abilities (`UTresUIP_CampMenuAbility`, clip `ability`)

Functions: open slot 80 `0x1415C9390`, list build `0x1415CD820` -> `0x1415CF300`, input slot 78
`0x1415C59B0`, toggle refresh `0x1415D2A00`, close slot 81 `0x1415BD6C0`; save-record helpers
`0x140E8B640` (owned copies), `0x140E8ECE0` (copy equipped), **`SetEquipped` `0x140E95E90`**.

**List** [C; row fields I]. For the chosen party slot the record is `GW+0x9640 + i * 0x9C0`
(`0x140E8AE20`). `0x140E8B640` lists every ability id **once per owned copy** (bits 0, 4, 8 of the
word). Rows are built per category in the order **Actions** (`ETresAbilityCategory` 0), **Mobility**
(1), **Support** (2) (`kh3_camp.ability.group_0010 / 0020 / 0030`); inside a category rows are
sorted by **`m_UIPriority` ascending**, equal priorities keep id order, so **the copies of one
ability are adjacent rows with the same name** (there is no "x2" counter). The first row of a
category carries the category title (`titleAction / titleMove / titleSupport`), the others a line
(`lineAction` ...). Row record (0x70 bytes): +0 ability id, +8 name (`m_NameTextID`), +0x18 help
(`m_HelpTextID`), +0x28 AP cost (`m_UseAP`), +0x30 NEW state, +0x34 equipped (`0x140E8ECE0`: the
copy's owned and equipped bits both set), +0x38 priority, +0x60 "may be toggled", +0x64 copy
index 0 .. 2. Abilities given by worn equipment or food are **not** in this list (they have no
owned bit); they are on the Equipment screen.

**AP display**: `m_NowUseApNum` = AP in use = sum of `m_UseAP` of the equipped rows;
`m_MaxApNum` = max AP = the status snapshot's 14th value (stats spec 2.7: level AP + AP boosts +
equipment AP); `m_ApGauge` (201 frames) = `F(used, max, 200) + 1`. Level-99 loadout: **180 / 180**,
gauge frame 201.

**Cursor on a row** (event 20) [C]: on an unequipped row the number shows the AP that **would** be
in use (`used + cost`), `m_NowUseApAnim` plays `equippable`, or `lack` when that exceeds the
maximum, and the preview gauge `m_ApIncreaseGauge` (or `m_ApLackGauge` when over) is set to
`F(used + cost, max, 200) + 1`; on an equipped row: `normal`, previews `off`. The help line shows
the ability's help text. A row whose NEW state is 1 becomes 2 and its NEW bit is cleared
(`0x1415A7250` -> `0x140E95F10(record, id, copy, 0)`).

**Toggle** (decide = event 7 on a row) [I `0x1415D2A00`, `0x140E95E90`]:
```
unequipped row:  allowed  <=>  cost <= maxAP - usedAP   and   equippedRows < maxRows
                              and not (id in {4, 5, 6} and another row with id in {4, 5, 6} is equipped)
                 allowed: SetEquipped(record, id, copy, 1); usedAP += cost; static SE 1
                 else   : static SE 4, nothing changes
equipped row:    always allowed: SetEquipped(record, id, copy, 0); usedAP -= cost; static SE 1
```
- `SetEquipped(record, id, copy, on)` needs the copy's owned bit (0 / 4 / 8) and writes its
  equipped bit (1 / 5 / 9). **It is the only writer of the equipped bits besides the record
  initialiser** (three call sites, all in this screen) [I, callers enumerated].
- **One reprisal**: ids 4, 5, 6 = Counter Slash, Counter Impact, Counter Kick (`GUARD_COUNTER`,
  `REVENGEIMPACT`, `REVENGEDIVE`): while one is equipped the other two cannot be equipped [I].
  No other ability is exclusive, and **no ability is locked on**: every equipped row can be
  removed, including the start set.
- `maxRows` = 1000 (no limit) [I]; 30 only with the Re Mind code-menu option A2 (`0x140EAB0E0(GI,
  2)`, the "limit" clip then shows `count / 30`). With code-menu option B1 (`0x140EAB160(GI, 1)`)
  every cost is 0.
- **Zero EXP** (id 26): equipping asks `kh3_camp.ability.dialog_0001` first (`0x1415C50C0`).
- **`m_MaxEquip` is never read**: none of the 16 functions that fetch a `TresAbilityData` row reads
  row+0x14 [I, displacement scan], and the toggle has no per-ability count limit. The real limit is
  the storage: three copies. The column is leftover data (stats spec open question 1).
- After a toggle every row's "may be toggled" flag is recomputed, the list redrawn, the gauge
  updated.

**When it takes effect**: at once. The toggle edits the save record word that the gameplay query
`Count(record, id)` (stats spec 5.1) reads; nothing is copied when the menu closes. (Max HP / MP do
not depend on abilities; values a state cached before the pause keep their old value until that
state recomputes them.)

**Other input**: L2 / R2 (events 11 / 12) previous / next party member (clip `left` / `right`,
static SE 2); Triangle (14) "Show Next List": for Sora it steps through read-only pages, one per
filled Keyblade slot, listing that Keyblade's **Form Actions** and **Shotlocks** (`group_0040`,
`group_0050`, from `TresKeybladeAbilityData`; `0x1415BC520`), then back to the ability list;
cancel (6) back to the top screen (clip `out`).

**What a newly granted ability gets** [I for the bit writes]: victory bonuses
(`0x140E86B30`), level abilities (`0x140E90E00`) and the recount (`0x140E98CF0`) set only **owned**
bits; the bonus path also sets the copy's bit 2 / 6 / 10, which is the **NEW mark** of the menu
(`0x140E8ED40` = owned and bit 2: read only by the new-mark builder `0x1415B2C20`; cleared when the
cursor first reaches the row). The record initialiser `0x140E93CA0` starts every word at 0x444 and
gives the start set `0xD` per owned copy plus the equipped bit for `m_EquipAbility` entries. The
player pawn's `AbilityAdd` / `AbilityRemove` virtuals (vt+0x1510 / +0x1518) are empty stubs
(`0x14041F340`). **So a newly learned ability is owned, marked NEW and not equipped: the player
must install it here.**

### 10.5 Items and Equipment

**Stock and pouch.** The *stock* is the inventory `GW+0x86B4` (count byte per item id, at most 99
of a battle item); the *pouch* is the eight item slots of the character record (`record+0x118`, 8
bytes each: id + flags; magic spec part J 7 has the slot rules, the flags and the refill). An item
moved into a pouch slot leaves the stock. The Items screen shows the stock; pouch slots are edited
on the Equipment screen.

**Items screen** (`UTresUIP_CampMenuBackyard`, clip `backyard`; open slot 80 `0x1415C9560`, input
slot 78 `0x1415C6150`) [C]:
- List: every inventory entry of the id ranges 0x101 - 0x10B (battle items), 0x201 - 0x205,
  0x300 - 0x364 (weapons), 0x401 - 0x434 (armour), 0x501 - 0x570 (accessories), 0x601 - 0x638,
  0x701 - 0x73C, 0x801 - 0x83B, 0x901 - 0x90D, 0xA01, sorted by the item rows' sort key; columns
  Name and Stock. Tabs (L2 / R2 = events 11 / 12, table at `0x149A958C0`): All, Items, Weapons,
  Materials, Key Items, Armor, Accessories (`kh3_camp.item.tab_*`).
- Decide on a row whose item category is 1 or 2 (and id != 0x10A): target mode: the five party
  buttons `m_TargetButtons` (each shows the member's HP / MP, 23 clip references); decide on a
  member tests the benefit (`0x1415C3C30`), applies the item (`0x140EBE1E0(GI, item, character,
  1)`: the stock count drops; the effect is that of the item command, magic spec part E), plays
  static SE **30** (31 for ids 0x202 - 0x205) and refreshes the panels; a refused use shows
  `kh3_camp.item.dialog_0002` ("cannot benefit") or `dialog_0003` ("cannot use any more"); with no
  copies left static SE 4.
- Triangle (event 14): Discard, after `dialog_0001`; items whose row forbids it (`0x141126C10`)
  show `dialog_0004`.
- Cancel: out of target mode, else back to the top screen.

**Equipment screen** (`UTresUIP_CampMenuEquipment`, clip `equip`; open slot 80 `0x1415E5540`, input
slot 78 `0x1415E30D0`, equipped-list builder `0x1415F0F40`, stock-list builder `0x1415E66D0`,
equip `0x1415EC620` -> commit `0x1415ECC00`) [C]:
- **Equipped list** (`m_EquippedList`), one row per **open** slot in this order: weapons
  (`record+0x80`, 3 slots for Sora), armour (`record+0x98`), accessories (`record+0xD8`), items
  (`record+0x118`); groups titled Weapons / Armor / Accessories / Items. Level-99 loadout: **3 + 3
  + 3 + 8 = 17 rows** (Kingdom Key and two empty weapon rows; armour and accessory rows empty; six
  filled item rows, two empty). Row record 0x38 bytes: +0 item id, +4 kind 0 .. 3, +8 slot index,
  +0x10 icon, +0x20 name, +0x30 auto-reload (slot flag bit 2), +0x31 "used, waiting for refill"
  (flags `(f & 0xD) == 0xD`).
- **Decide** on a slot opens the **stock list** (`m_BackyardEquipmentList`): the character's
  equippable stock (`0x140EA81D0`) filtered by item category (weapon 3, armour 4, accessory 5, item
  1: byte table `0x146A8E240`), sorted, with the slot's current item inserted as row 0. Decide on
  a row equips it and returns (static SE **29**); decide on row 0 with nothing to change just
  returns.
- **Equip rule** `0x1415EC620(newItem, row)`:
  1. weapons, armour, accessories: if the piece being removed gives AP (`m_AP` > 0) and the new
     one gives less: the abilities in use (sum of `m_UseAP` x equipped copies) must fit in
     `maxAP + delta`, else dialog `kh3_camp.equip.dialog_0003` ("first remove some abilities") and
     nothing changes (skipped with code-menu option B1);
  2. the removed piece must fit back into the stock (`dialog_0002` when the stock is at its
     maximum);
  3. commit: `0x140E98060` (weapon slot), `0x140E977D0` (armour), `0x140E94EF0` (accessory),
     `0x140E94F90` (item slot; a new item inherits the auto-reload setting of other slots with the
     same item), the old piece goes back to the stock (`0x140E86510`, not for a used item slot)
     and the new one leaves it (`0x140E8A250`); then the character's status is rebuilt
     (`0x140EA81D0`) and the new piece's assets are requested (`0x140EB2F40`; the list shows a
     loading mark until they arrive). **The change is in force at once.**
- **Square** (event 15) "Remove": equip of item 0, static SE **3**; refused with
  `dialog_0001` for the weapon slot that is the **current** Keyblade (`record+6`).
- **Triangle** (event 14): on an item row toggles **Auto-Reload** for that item kind on every slot
  holding it (`0x1415E8430` -> `0x140E94F60`, static SE 1; when another slot with the same item is
  waiting for a refill the dialog `dialog_0004` asks first); on other rows it opens the read-only
  **ability list** of the highlighted piece (`m_AbilityList`; only when the piece has abilities,
  `0x1415DE7D0`): these are the abilities counted through the equipment counter (stats spec 2.5);
  they cost no AP.
- **L2 / R2**: previous / next party member (static SE 2). **Cancel**: stock list -> equipped
  list -> top screen.
- **Status panes**: `m_EquippedStatusA` / `m_BackyardItemStatusA` (`0x1415A9CE0`) show the
  highlighted piece: name, help text, its Strength / Magic / Defense / AP and resist values, for a
  Keyblade the **upgraded** row (`0x14112EBD0` reads the override map at +0xDC0 before the base
  table: pointer only, stats spec open question 4) and its form-change and shotlock names.
  `m_BackyardItemStatusB` (`0x1415AA460`) is the **preview** while the stock list is open: for
  max HP, max MP, AP, Strength, Magic, Defense and the six resists it shows the character's
  current value (status snapshot) and the **difference** the highlighted piece would make:
  `new.m_AP - old.m_AP`, and likewise `m_AttackPlus`, `m_MagicPlus`, `m_DefensePlus`; the max
  HP / MP differences are always 0 (equipment has no HP / MP term); each resist difference is the
  resist recomputed with the piece swapped (multiplicative rule of stats spec 2.7) minus the
  current one. For a Keyblade in a slot that is not the current one, Strength and Magic are first
  re-based on that slot's Keyblade.
- **Keyblade slots and switching** [C]: three slots; the active one is `record+6`. In the field,
  D-pad right / left on the Attack command asks the request interface slots 234 / 235
  (`0x141446590` / `0x141446640`): next = the following **non-empty** slot in the order 0 -> 1 ->
  2 -> 0 (`0x141782070`), previous the other way (`0x1417820E0`); nothing happens with one
  Keyblade or while the weapon restriction (pawn vt+0x1ED8) is set; the switch itself is state
  WEAPONCHANGE (32). With the Kingdom Key alone there is no switch.
- **Slot counts** [stats spec 2.5, magic spec J 7.2]: armour 1, accessory 1, item 3 at the start;
  3 / 3 / 6 with the story bonuses, 8 item slots with Re Mind; the arrays hold 8.

### 10.6 Customize (`UTresUIP_CampMenuCustomize`, clip `customize`)

Functions: open slot 80 `0x1415E5400`, input slot 78 `0x1415E1FA0`, assignable lists
`0x1415EA9C0`, shortcut write `0x1415EED00`. Storage is the block `S = GW+0x13CE0` of the magic
spec (part J 3.1): `shortcut[3][4]` at S+0x00, `magicSlot[6]` at S+0x30, `linkSlot[5]` at S+0x48,
"tier chosen by hand" bits at S+0x5C. KH3 has no command deck; this screen edits what the command
menu and the shortcut panel show. Everything here is [C]; the storage accessors are [I] in the
magic spec.

Top layer (`m_TopLayerButtons`, help `kh3_camp.customize.detail_0001 ..`): **Magic**, **Links**,
**Shortcut Set A / B / C**, then one entry per friend in the party (Combat Style, Abilities,
Recovery: friend AI, 10.9). Entries without content are hidden (Links before a link is learned).

- **Magic** ("View, configure, and reorder magic commands"): the six slot buttons
  `m_MagicLinkButtons` show `magicSlot[0 .. 5]` in command-menu order.
  - **Decide** on a slot shows the three tier buttons `m_MagicStrengthButtons[3]` of that element
    (group of `UI/Data/CustomizeDataAsset.MagicCommands`); decide on a learned tier writes that
    kind into the slot (`0x140E970B0`) and sets the slot's bit at S+0x5C (`0x140E95E20`) when the
    kind changed. **So the player can put a lower tier (Fire instead of Firaga) into the command
    menu**; the bit stops a later tier bonus from replacing it. (The magic spec, part J 2.2, says
    lower tiers cannot be selected: true for the automatic fill only.)
  - **Triangle** (event 14, "Reorder") picks the focused slot up (static SE 1); while picked up,
    moving the focus to another slot **swaps the two slots** (kind and bit); Triangle again drops
    it.
- **Links**: the same list for `linkSlot[0 .. 4]` (`0x140E8B9F0` / `0x140E97000`), reorder only.
- **Shortcut Set A / B / C**: four buttons `m_ShortcutButtons[4]` with their pad icons
  (`m_ShortcutButtonPadText`: the face buttons right, top, left, bottom, as in the HUD panel 5.5)
  showing `shortcut[page][0 .. 3]`; an item entry shows the auto-reload mark when its pouch slot
  has it.
  - **Decide** on a button opens the list `m_ShortcutSettingList` with tabs (L2 / R2):
    **All, Magic, Links, Items** (only tabs that have entries). Entries, in this order: every
    **learned** magic kind of every group and tier (`0x140EA2000(GI, character, kind)`; asset
    order Fire, Fira, Firaga, Blizzard .., Thunder .., Water .., Aero .., Cure ..: each tier is its
    own entry), every learned link, every item command of
    `CustomizeDataAsset.ItemCommands` whose item has ever been obtained (`0x140E8F010`). Decide on
    an entry writes it (`0x140E97F10(S, page, slot, kind)`). **The same command may sit in several
    slots and sets; nothing is removed elsewhere.**
  - **Square** (event 15, "Remove") on a button clears the slot (kind 0).
  - An empty list shows `custom_0010 .. 0012` ("No items/commands.").
- **Cancel** steps back one layer; leaving the screen tells the HUD command menu to rebuild its
  lists (`0x141617530`), so the changes show as soon as the game resumes.

**Defaults** [magic spec J 3.1, 2.2]: a new game has all 12 shortcut slots empty; the magic slots
fill in the order the elements are learned, always with the highest tier, unless a tier was chosen
here. Level-99 reference loadout: magic slots Firaga, Waterga, Curaga, Blizzaga, Thundaga, Aeroga;
the shortcut sets are whatever the test configuration writes (`sora-loadout-99`), not a game rule.
With all six elements at tier 3 the Magic tab lists 18 entries.

### 10.7 Stats (`UTresUIP_CampMenuStatus`, clip `status`)

Open slot 80 `0x1415E5780`, filler `0x1415E8E00` (also slot 83), input slot 78 `0x1415E4340`
[C; `0x1415E4340` is `same`]. Every number comes from the party slot's **status snapshot** (the 27
ints of stats spec 1.2, filled by `0x1405AA250` through `0x140EA1BF0` when the menu opens and
again after an item or equipment change), so the screen shows the same values combat uses,
equipment and meals included. `s[n]` = snapshot value n.

| Field (clip member) | Value | Level-99 loadout (Balance, Standard, Kingdom Key) |
|---|---|---|
| Level `m_LevelNum` | `s[0]` | 99 |
| Total EXP `m_GetExpNum` | `s[1]` | 1527640 |
| Next Level `m_NextLevelExpNum` | `s[2] - s[1]`; **0 when the level is 99** | 0 |
| EXP gauge `m_ExpGauge` (101 frames) | `F(s[1] - s[3], s[2] - s[3], 100) + 1`; **frame 101 at level 99** | 101 |
| HP `m_NowHpNum` / `m_MaxHpNum` | `min(999, s[4])` / `min(999, s[5])` | 265 / 265 |
| MP `m_NowMpNum` / `m_MaxMpNum` | `min(999, s[6])` / `min(999, s[7])` | 169 / 169 |
| Focus `m_NowFocusNum` / `m_MaxFocusNum`, label `m_FocusName` | Sora only and only with game flag byte GW+0x95B0 bit 4: `min(999, s[8])` / `min(999, s[9])`, label "Focus", clip `m_FocusAnim` label `normal`; otherwise both numbers "—", label "? ? ?" (`kh3_camp.status.item_0040`), clip label `focus` | 100 / 100 |
| AP `m_ApNum` | `min(999, s[13])` = max AP | 180 |
| Strength `m_AttackNum` | `min(999, s[10])` | 66 |
| Magic `m_MagicNum` | `min(999, s[11])` | 67 |
| Defense `m_DefenceNum` | `min(999, s[12])` | 59 |
| Fire, Blizzard, Thunder, Water, Aero, Dark `m_FireNum ..` | `min(100, s[19 .. 24])` formatted "{0}%" (`kh3_system.general.window_0230`); `s[18]` (physical) and `s[25]` are not shown | 0% each |
| Name `m_CharaName`, help line | party name; "Viewing stats for {0}." | Sora |

`F` is the gauge helper of section 4. Note the two EXP gauges differ at level 99: the top-screen
panel shows an empty gauge (10.3), this screen a full one. Other difficulties and first choices:
HP / MP per the table of 11b. The HP / MP / Strength / Magic / Defense arrow clips
(`m_*ArrowIcon`, `m_*Anim`) are bound but not written by the filler: who drives them (the
meal-bonus display) was not found.

Input: L2 / R2 (events 11 / 12) previous / next party member (clip `right` / `left`, static SE 2,
only with more than one member: the hint `kh3_camp.status.button_0001` is then shown); cancel
(6) back. There is nothing to select on this screen.

**Character model (stub)**: the camp controller spawns one `TresCampPawnBase` per party slot
(states 5 - 8 of `0x14154C710`), far outside the map and 100,000 units apart, rendered into the
menu through the UI actor (`FTresUICampCharacterParam.UIActor`, data `TresUIDataAssetStatus`);
`TresUIP_CampMenuCharacterLoading` shows a loading mark until the model is ready. A port can show a
still image or nothing.

### 10.8 Config (`UTresUIP_CampMenuConfig` + `UTresASProxyRSLConfig`, library movie `90_rsl/configlibrary/Config`)

The 26 options are clips `configPack.setConfigValue1 .. 26` (class `...RSLConfig_SettingItem`, or
`..._SettingSliderItem` when the clip has a `slider`). Labels come from the key table at
`0x1486DE570` (`kh3_camp.config.config_*`), the choice texts from the table built by
`0x140101700`; choice order is the table's (it is **Off, On** for Link Cinematics, Attraction
Cinematics, Subtitles, Maps and Vibration).

**Storage** [I]: the user configuration block is 0x20 bytes at `GW+0x2E8` (= `GI+0xB58`), part
of the save data. The screen works on a copy (`rsl+0x1F0`): load slot 80 `0x14154F520` (block ->
choice index per item), store slot 79 `0x14154E660` (choice index -> block); both were read side
by side and agree. On leaving the screen (`0x1415C4E70`), if the copy differs from the block it
is written back, **applied** (`0x140EA6C90`: every player controller re-reads its input and camera
settings through vt+0xD30 / +0xD00 / +0xCF8, the UI manager refreshes the cockpit `0x141591CA0`
and flags the command menu to re-read its bits, the display-area cvars and the user settings are
pushed) and saved. Triangle = "Restore Defaults" after `kh3_camp.config.dialog_0010`: the block
constructor's values (`0x140E84340`).

Defaults: byte 0 = 0xC3, byte 1 = 0x1F, byte 2 = `(old & 0xE4) | 4`, bytes 3, 4 = 0, bytes 5, 6 =
50, byte 7 = 10, byte 8 = 0 [C `same`].

| # | Option | Stored in | Choices (index 0, 1, ..) | Default | Read by |
|---|---|---|---|---|---|
| 1 | Camera Work | byte 0 bit 1 (**set = Auto**) | Auto, Manual | Auto | camera spec (`0x1406086B0`) |
| 2 | Vertical Camera Control | byte 0 bit 3 | Normal, Flipped | Normal | `ATresCameraBase` input `0x1405CD3F0` |
| 3 | Horizontal Camera Control | byte 0 bit 2 | Normal, Flipped | Normal | `0x1405CD3F0` |
| 4 | Vertical Camera Sensitivity (Gamepad) | **byte 6**, slider 1 .. 100 | — | 50 | `0x1405EC530` (uses value - 1) |
| 5 | Horizontal Camera Sensitivity (Gamepad) | **byte 5**, slider 1 .. 100 | — | 50 | `0x1405EC530` |
| 6 | Vertical Reticle Control | byte 0 bit 5 | Normal, Flipped | Normal | `0x1405CD3F0`, controller aim input `0x141407920` |
| 7 | Horizontal Reticle Control | byte 0 bit 4 | Normal, Flipped | Normal | same |
| 8 | Vertical Ride Control | byte 2 bit 3 | Normal, Flipped | Normal | states Swim (`0x1413DA070`, `0x1413DAB60`), SmallAirplane, FRBaymaxRide: stick Y x -1 |
| 9 | Combo Cam | byte 3 (0 / 1 / 2) | Auto, On, Off | Auto | `UTresPlayerState_Attack` `0x141335120` (0 -> pawn vt+0x2790(1)); GI getters `0x140EAAF40`, `0x140EAB300` |
| 10 | Keyblade Cinematics | byte 1 bit 2 (**set = Normal**) | Normal, Basic | Normal | `UTresPlayerState_StyleChange` `0x1413CBA70`; `0x140EAAF40` |
| 11 | Link Cinematics | byte 1 bit 3 | Off, On | On | GI getter `0x140EAB070` (link pawns) |
| 12 | Attraction Cinematics | byte 1 bit 4 | Off, On | On | GI getter `0x140EAB060` (attraction pawns) |
| 13 | Command Menus | byte 1 bit 5 | By World, Classic | By World | `0x141616C10` -> HudCommand+0x9C0; skin choice `0x141670930` |
| 14 | Command Selection | byte 1 bit 6 | Active, Slow | Active | `0x141616C10` -> HudCommand+0x9C1 (magic spec J 6) |
| 15 | Cursor Position | byte 1 bit 7 | Reset, Lock | Reset | `0x141616C10` -> HudCommand+0x9C2 (Reset: after a command the cursor returns to Attack; Lock: it stays where it was, 5.4) |
| 16 | Situation Commands | byte 2 bit 0 | Prioritize Older, Prioritize Newer | Older | HudCommand (+0x9C4 = !bit, 5.10 rule 6) |
| 17 | Control Type | byte 2 bit 1 | Type A, Type B | Type A | GI getter `0x140E49A00`; HudCommand `setLegacyMode` (5.9); button-help icons `0x1415A24F0`. Decide on this row shows the controls diagram (`0x1415A2450`, picture 0x15 / 0x16, keyboard 0x89 / 0x8A) |
| 18 | Subtitles | byte 1 bit 0 | Off, On | On | UI manager subtitle path `0x1415A05C0` |
| 19 | Maps | byte 0 bit 6 | Off, On | On | cockpit `0x141575D00`, `0x141576420`, getter `0x141583D90` (11a) |
| 20 | Map Orientation | byte 0 bit 7 (**set = Fixed**) | Fixed, Match Camera | Fixed | cockpit `0x141575D00` -> cockpit+0x220 |
| 21 | Vibration | byte 0 bit 0 | Off, On | On | GI getter `0x140EAAF30` (force feedback `0x140E49A30`) |
| 22 | Master Volume | byte 7, slider 0 .. 10 (PC: mirrored in `UTresGameUserSettings`, `0x140E8BBF0` / `0x140E971B0`) | — | 10 | sound master volume |
| 23 | Graphics | not in the block (platform query `0x14041F1E0`) | Default Mode, Stable Mode | — | console only |
| 24 | Display Area | not in the block on PC (query stubbed to 0: the HUD rectangle is never shrunk, 11a). Decide opens the `ScreenOption` screen | — | 0 | `0x140E9FC70` |
| 25 | Screen Brightness | byte 4 (PC: `UTresGameUserSettings`, `0x140E8B7B0`). Decide opens the `BrightnessOption` screen | slider | 0 | gamma |
| 26 | Language | not in the block on PC (stub) | Japanese, English | — | voice language (console) |

Other bits: byte 1 bit 1 is the Theater's own toggle (`UTresUIP_Theater` `0x1417539B0`), byte 2
bit 2 is set by the constructor and has no reader in the scan. **The decide / jump button swap is
not here**: it is the engine setting `GamePadDecideType` / cvar `i.GamePadDecideType` (actions
spec), outside this menu, and there are no separate "field / battle camera speed" options: the two
sensitivity sliders are the only camera speeds. Subtitle and voice language are not Config options
on PC beyond rows 18 and 26.

This settles two open points of earlier passes: **`setLegacyMode` is Control Type B** (row 17),
and configuration byte 1 bit 7 ("the list stays open") is **Cursor Position: Lock** (row 15).

Input [C `0x141541A20`]: up / down walk the rows (anchor index = row number), left / right change
a choice or move a slider (the item proxies' own slot 78), cancel leaves and applies, Triangle
restores. Readers were found with a displacement scan of 0x140400000 .. 0x142D00000 for byte
operands at +0x2E8 .. +0x2F0 after a GW getter and at GI+0xB58 .. +0xB60 (lists in
`build/sora_tmp/ui/camp/cfgscan*.txt`).

### 10.9 Stubs (names and entry points only)

| Screen | Movie(s) | Native | Entry |
|---|---|---|---|
| **Gummiphone** | `42_mobilePortal/MobilePortal` + apps `MobileAlbum`, `MobileCode`, `MobileDictionary`, `MobileGlossary`, `MobileLsi`, `MobileLucky`, `MobileMix`, `MobileRecipe`, `MobileRecord`, `MobileStory`, `MobileTreasure` | `UTresMobilePortalController` (mgr+0x370, ctor `0x141531900`), `UTresUIP_MobilePortal` (ctor `0x1416D9FD0`), one `UTresUIP_Mobile*` per app | camp top entry 6 (unlock flag `smartphone`; script `TresDebugUnlockSmartphone` `0x14222F860`): `UTresUIP_CampMenu` delegate +0x2D0; the selfie camera needs the flag `spcamera` |
| **Friends (party AI)** | `CampMenu` clip `customize` (`m_NpcSecondLayerRoot`, `m_NpcSettingButtons[6]`, `m_NpcSettingListButtons[4]`) | part of `UTresUIP_CampMenuCustomize` (10.6); writer `0x1415EA860` | Customize -> a friend. Three settings per friend: Combat Style (Stick By Sora / Be Yourself / Hands Off / Finish the Job), Abilities (Go Wild / Watch MP / Conserve), Recovery (Use As Needed / In Emergencies / Like Crazy; On Sora, On Self, On Others = bits 0, 1, 2 of `record+0x15B`); the record bytes +0x15A .. +0x15C are these settings. Not needed for Sora alone |
| **Game Help** | `16_game_help/GameHelp`; camp clip `gameHelp` | `UTresUIP_GameHelp` (mgr+0x2E0; slots 71 `0x1415FAAC0`, 74 `0x141606D90`, 89 `0x1416024D0`), `UTresUIP_CampMenuGameHelp` (slots 71 `0x1415DB700`, 78 `0x1415E40D0`) | camp top entry 7; scripts `TresUI_OpenGameHelp` `0x1422384F0`, `TresUI_OpenGameHelpWithMessage` `0x142238640`; pause-screen button "Game Help" when a help entry is registered (`TresUI_SetPauseGameHelp`) |
| **Cuisine** | camp clip `cuisine` | `UTresUIP_CampMenuCuisine` (slots 71 `0x1415C2980`, 78 `0x1415C65E0`) + ConfirmNotice, Favorite, Parameters | camp top entry 2 (unlock flag `cuisine`) |
| **Save / Load** | `17_saveload/SaveLoad` | `UTresUIP_SaveLoad` (mgr+0x2F8; slots 71 `0x1416FF720`, 80 `0x141714F60`, 89 `0x141708F70`, 90 `0x141720F60`), list items `...SaveLoadListItem`, `...SaveDataDisplayInfo` | save points and the title screen (not from the camp menu); auto-save mark `UTresUIP_AutoSave` (mgr+0x2C0) |
| **Title** | `11_title/Title` | `UTresUIP_Title` (ctor `0x141749120`); settings menu `UTresUIP_SettingMenu` (`85_settingmenu/SettingMenu`, ctor `0x1416F60F0`, data `m_SettingMenuData`) | game start; `BP_GotoTitle` `0x14221FE50`; pause-screen "Exit" |
| **Tutorials** | `11_title/Tutorial`, library `90_rsl/tutoriallibrary/TutorialLibrary` | `UTresUIP_Tutorial` (ctor `0x1417616F0`), `UTresASProxyRSLTutorialMenu`, HUD actor `ATresHUDTutorial` (ctor `0x141573050`) | scripted; pause type "Exit Tutorial" (`kh3_menu.pause.menu_0101`) |
| **Premium (code) menu** | `83_codemenu/*` | opened by `0x1415A1970` from the camp top screen (View button, key 23, when the EZ / PRO codes exist) | Re Mind only; its option bits are the "code-menu options" A / B of the stats spec |
| **Dialogs** | `00_system/Dialog` (in `SystemLibrary`) | `0x1415A1C70(mgr, kind, text, ...)`: kind 1 = message with OK, 2 = Yes / No (default No when the 4th argument is 1) | every confirmation above |

## 11. Offline render

`render/hud_1080.png` (copy in `build/sora_tmp/ui/hud_1080.png`) is composed from the pack alone:
HudCommand at `loop` with the four slots (Attack selected), HudStatus at `loop` with max HP 250,
HP 180 after a hit from 215 (red lag), MP 110 / 180, focus 60 / 100, and Sora's portrait attached
in display order. It shows the blue command menu with the "COMMANDS" plate and the texts in the KH2
menu font bottom left, and bottom right the green HP ring that continues as a bar to the left with
a dark red lag section, the blue MP bar with its "MP" label, the orange focus arc labelled "FOCUS"
and the portrait inside the ring. Not reproduced by the reference renderer: filters and the 9-slice
of the MP base (drawn stretched).

## 11a. HUD modes, update order, screen adaptation

**Modes.** KH3 has **no field / battle slide** of the HUD: the status HUD and the command menu stay
in place; entering battle only changes the command colour (`setBattleMode`, 5.3), the portrait look
(section 4) and the number colour of items. What exists [C]:

- `0x1415849A0(cockpit, hide)` (argument 1 = **hide**; an earlier revision had it inverted), called
  through `0x14159F130` by `ATresCameraHoldCamera` (the first-person / photo camera
  `0x1405EB4B0`, `0x1405EE3D0`, `0x1405EE4C0`). Hide: command menu root `off` and state 11 (Wait),
  `setStatus(-1)` (off) on the status HUD, target plane invisible, left / right popups closed, the
  pinch alarm kept under its own reason bit (8). Show: command menu root `loop` and state 1,
  `setStatus(0)` (loop), target plane visible.
- The cockpit word +0x210 set by `0x141576420` is **not** a HUD mode: it names the part that hosts
  the minimap (0 = `HudRight.m_NaviMap`: normal play; 1 = none, used while the HUD is hidden; 2 =
  the Gigas HUD; 3 = the special-ship HUD). Changing it hides the old host's map and shows the new
  one when map data exists and the map option (configuration byte 0 bit 6) allows it. A Sora test
  level stays at 0.
- Short hides: the root label pairs `inShort` / `outShort` of HudCommand, HudStatus
  (`setStatus(1)` / `(2)`) and HudTarget are used around style changes and vehicles
  (`0x14162EDE0`, `0x14161E470`, `0x14165F370`, `0x141652B40`). `setStatus(12)` / `(13)` =
  `change2` / `change1`: Sora's slot switches between the solo and the party layout (`setSolo`).
- Target HUD "hide reasons": `0x1415817D0(cockpit, reason)` adds and `0x14158CD70` removes a reason
  from a set on the target HUD; while the set is non-empty one of its prompt clips (+0x2D8) is
  hidden. The command menu adds reasons 11 / 12 while a sub list or the shortcut panel is open and
  removes 7, 11, 12 in MainSel [C; which clip was not identified].

**Update order** [I for the group, C for the rest]: the UI manager owns a tick function
(`FTresUIManagerTickFunction`, at manager +0x100, execute `0x141594EB0`) whose tick group byte is
set to **5 = `TG_DuringPhysics`** in the constructor (`0x141591290`; this fork's enum is Early 0,
LevelScript 1, PrePrePhysics 2, **PrePhysics 3**, StartPhysics 4, DuringPhysics 5, ...). So per
frame: controller and pawn tick (PrePhysics) -> **UI manager update** (`0x1415B0ED0`: each part's
slot 90, then its proxies) -> remaining groups -> camera update (camera spec 1.3) -> viewport draw,
where slot 91 projects the world-anchored clips with this frame's view (7.2). The HUD therefore
shows this frame's pawn values and this frame's camera. **The time step of the UI update is not
the tick's**: `0x141594EB0` replaces it with `world+0x94C`, the frame's delta **before time
dilation** (written by `UWorld::Tick` `0x1441BA050`), so HUD timers ignore slow motion; the
situation-command timers alone multiply it by the world dilation (5.6). Movie timelines advance
with the Scaleform player (60 frames per second of real time).

**Screen adaptation** [I constants]: every part sets `SetViewScaleMode(2)` = **ExactFit** right
after its movie is loaded (`0x14176AAC7`), and no part sets a viewport, so the integration's
default viewport applies (`0x143906EE0` -> `0x1438F8E90`). For a window of W x H:

```
fw = cvar r.Upscale.SqexFitOffsetWidth, fh = r.Upscale.SqexFitOffsetHeight      (default 1.0)
left = trunc((W - W * fw) / 2);  top = trunc((H - H * fh) / 2);  w = W - 2 left;  h = H - 2 top
if W / H > 1.7777:  e = w - H * 1.7777;      left += trunc((e + 1) / 2);  w = trunc(w - (e - 0.5))   // pillarbox
if W / H < 1.7777:  e = h - W * 0.5625246;   top  += trunc((e + 1) / 2);  h = trunc(h - (e - 0.5))   // letterbox
stage (1920 x 1080) is stretched over (left, top, w, h)
```

So the HUD is **always a centred 16:9 rectangle**: at 21:9 it is pillarboxed (width = H * 1.7777),
at 16:10 and 4:3 letterboxed (height = W * 0.5625); nothing is anchored to the real screen edges
and nothing is stretched. The Config option "Display Area" is the two cvars: the game sets both
to `1 - n * 0.0005` (`0x140E9FC70`), which shrinks the rectangle symmetrically. The target
projection (7.2) maps the 3D view rectangle to the same stage, which is only consistent when the
3D view is that same rectangle [derived; the view rectangle itself was not traced].

## 11b. Implementation order and test list

Expected values use the frame helper of section 4 and `sheets/port/sora-loadout-99.json`
(level 99: max HP 280 / 265 / 250 by start choice on Standard, 139 / 131 / 124 on Critical;
max MP 159 / 169 / 179; max focus 100).

**Tier 1: status HUD and command menu display**
1. Gauge frames. Scripted check at level 99, max HP 250 / max MP 179: HP base frame 501, full HP
   fill 501, HP 125 -> 251, HP 1 -> 3 (never frame 1 while alive), HP 0 -> 1; MP base
   `trunc(179 * 1.8) + 1` = 323, full 323; focus 60 / 100 -> 61, 100 / 100 -> 101 with `isMax`.
   Max HP 280 -> base 561; max HP 139 -> base 279. Max HP 400 -> base stays 669.
2. Red lag: hit 250 -> 180 at t = 0: fill jumps to 361 at once, red stays at 501 until 0.8 s, then
   moves 14 frames per 33.3 ms tick and stops at 361 after 10 ticks (t = 1.13 s). Heal: red jumps
   with the fill. Second hit during the follow: red continues from where it is.
3. Pinch: max HP 250 -> pinch at HP <= 62 (frame 125): portrait look 2, alarm on; HP 63 -> off.
   Max HP 139 -> pinch at HP <= 34.
4. MP Charge: MP 0 -> charge mode on: pack label `charge_in`, pink gauge frame follows the
   charge value; leaving plays `charge_out`.
5. Command menu from a model: `initMainCommand("Attack","Magic","Items","Link")`, state in,
   selection 0..3 (slot plays `over`, others `up`), colour label by battle mode. Compare with
   `render/hud_1080.png`.

5a. Command display from a model (section 5): open Magic with 3 rows -> `initSubCommand(1, 3)`,
   `setSubCommandText`, `setSubCommand(0, 0)`, static SE 6; rows use clips `cmdSub1_6..8`; cursor down
   three times wraps to row 0 (SE 0 each); cancel -> `setSubCommand(2, 0)`, SE 3. MP Charge on -> all
   magic rows and the Magic slot `disabled`. Items: Potion x3 shows "x3", x1 shows nothing, x12 shows
   "x9". Shortcut held -> `setModeShortcut(0, page)`, D-pad down -> page + 1 with SE 6.
5b. Situation command with 12.34 s of 20 s: digits 12 and 3, counter `loop`, gauge frame
   `219 - trunc(0.617 * 214 + 0.5)` = 87; at 9.99 s the counter turns `red`; nothing counts during the
   first 0.5 s.

**Tier 2: target HUD, EXP, popups, shotlock overlay**
6. EXP award of 120 at level 1 (total 0): EXP point "120" for 2.03 s at the enemy; level-up popup
   "LV 2" with its delta lines for 4.53 s; Next LV shows 280 - 120 = 160 for 2.43 s. Award at
   level 99: no EXP point, no Next LV.
7. Munny 10 then 25 within 2.4 s -> counter shows 10 then 35. Three items at once -> three popups
   in sequence, 1.9 s each. Five left parts -> only four rows visible.
8. Enemy gauge: max HP 3000, HP 1234 -> 14 stock squares at full (15 bars), now 6 squares, top bar
   frame 86.
9. Target cursors (section 7): a target at the view centre gives (960, 540); 150 px outside the
   view edge -> `visible` false, no clamping; lock-on add -> SE 7, remove -> SE 8. Enemy gauge only
   with LIBRA equipped; it stays 0.5 s after the last cursor is removed.
9a. Shotlock aim of 6 s: scope `in`, SE 18; after 3 s `updateFlowScope(159)`; locks 1..24 ->
   `setLockonCount(n, 24)` with SE 19, the 24th with SE 20 and `max`. Window 2560 x 1080 -> HUD
   rectangle x 320..2240; 1920 x 1200 -> y 60..1140.

**Tier 3: pause screen and camp menu** (scripted walk; level-99 loadout of `sora-loadout-99`:
Balance, Standard, Kingdom Key, six pouch items of eight slots, unlock flag `campmenu` set, flags
`cuisine` and `smartphone` clear, party = Sora alone. Static SE numbers as in section 9.)

10. **Open / refuse.** Idle on the ground, no enemy engaged: Menu -> next frame the game is
    paused (world time stops, HUD timelines frozen, sound state `STATE_MENU_OPEN`, BGM still
    playing), static SE 2, camp root `in`, focus on entry 0. Same press with an enemy engaged, or
    within 1.0 s after the last one died: the **Pause** movie instead (title "PAUSE", no buttons,
    static SE 2); Menu again: static SE 3, unpaused. Menu while guarding, while aiming a shotlock or while talking, even with no enemy: Pause
    movie. Menu during an ordinary jump: camp menu.
    With `TresSetDisableGamePause(true)`: nothing happens.
11. **Top screen.** Rows top to bottom: Equipment, Abilities, Items, Customize, Stats, Game Help,
    Config (7 rows, none greyed). Down from Config (fresh press) wraps to Equipment; holding down
    stops at Config. Each move: static SE 0 and the help line `kh3_camp.top.detail_*`. Panel:
    "Sora", LV 99, HP 265 / 265, MP 169 / 169, EXP gauge frame 1 (empty at level 99). Frame: play
    time `h:mm`, munny, title "MENU". Cancel or Menu: root `out`, static SE 3, unpaused; the
    pouch's used auto-reload slots are refilled on the first unpaused frame.
12. **Abilities.** Decide on Abilities -> character list (one row, Sora) -> decide: list in the
    order Actions, Mobility, Support, sorted by `m_UIPriority`; first rows Aerial Recovery (AP 2),
    Payback Strike (5), Block (2), Counter Slash (4), Counter Impact (4), Counter Kick (4);
    stackable abilities appear as adjacent rows (Air Slide x 3, Superslide x 3, Combo Plus x 2,
    ...): 88 rows for 73 abilities. AP **180 / 180**, gauge frame 201. Cursor on Counter Impact
    (not installed): number shows 184, label `lack`, decide -> static SE 4, nothing changes (it
    is refused twice over: no AP and Counter Slash is installed). Uninstall Counter Slash: static
    SE 1, AP 176 / 180, gauge frame `F(176, 180, 200) + 1` = 197; Counter Impact now previews
    180 with `equippable`; install it: 180 / 180; Counter Kick: refused (one reprisal). The record
    word of id 4 changed from 0x..3 to 0x..1 on uninstall (bit 1), id 5 gained bit 1. Zero EXP
    asks for confirmation. Leaving and resuming: `AbilityCount(4)` = 0, `AbilityCount(5)` = 1 at
    once.
13. **Equipment.** 17 rows: 3 weapon rows (Kingdom Key, empty, empty), 3 armour, 3 accessory, 8
    item rows (Potion, Potion, Hi-Potion, Hi-Potion, Ether, Ether, empty, empty). Square on the
    Kingdom Key row: dialog "You cannot remove this equipment." Decide on an empty armour row:
    stock list (empty in the loadout: "No items."). With a test armour of Defense +2 in stock:
    preview shows Defense 59 with +2, max HP 265 with no change; decide: static SE 29, Stats
    Defense 61, stock count of that armour - 1. Removing an accessory that gives AP +5 (max AP 185): allowed with
    180 AP in use (180 <= 185 - 5), refused with 181 in use (`dialog_0003`).
    Triangle on a Potion row: auto-reload mark on both Potion rows, hint text switches to
    "Disable Auto-Reload", static SE 1.
14. **Items.** Tabs All / Items / ... with L2 / R2. Decide on Potion (stock 5) with HP 100 / 265:
    target buttons, decide on Sora: static SE 30, HP per the item rule (magic spec E), stock 4,
    top panel refreshed. At full HP: `dialog_0002`.
15. **Customize.** Magic: six slots Firaga, Waterga, Curaga, Blizzaga, Thundaga, Aeroga. Decide on
    slot 0 -> tiers Fire / Fira / Firaga; choose Fire: slot 0 = kind 29, bit 0 of S+0x5C set; the
    HUD Magic list shows "Fire" first after resuming. Triangle on slot 0, down, Triangle: slots 0
    and 1 swapped. Shortcut Set A, button 0 (right face button), decide: list tabs All / Magic /
    Items (no Links tab without a link), Magic tab 18 entries in asset order; choose Curaga: the
    HUD shortcut panel page A row 1 shows "Curaga". Square on the button: empty again.
16. **Stats.** Level 99, Total EXP 1527640, Next Level 0, EXP gauge frame 101, HP 265 / 265, MP
    169 / 169, Focus 100 / 100 (with game flag byte +0x95B0 bit 4; otherwise "—" and "? ? ?"),
    AP 180, Strength 66, Magic 67, Defense 59, six resists "0%". Vitality / Wisdom and Critical:
    the HP / MP of the table above (280 / 159, 250 / 179; 131 / 77 on Critical).
17. **Config.** 26 rows; defaults: Camera Work Auto, the five control rows Normal, both
    sensitivities 50, Combo Cam Auto, Keyblade Cinematics Normal, Link and Attraction Cinematics
    On, Command Menus By World, Command Selection Active, Cursor Position Reset, Situation Commands
    Prioritize Older, Control Type A, Subtitles On, Maps On, Map Orientation Fixed, Vibration On,
    Master Volume 10. Set Control Type to B and leave: block byte 2 bit 1 = 1, the command menu
    receives `setLegacyMode(true)` on its next update, D-pad up from Attack then walks into the
    situation commands. Set Command Selection to Slow: byte 1 bit 6 = 1, opening the Magic list
    slows the listed object types to 0.3 (magic spec J 6). Set Camera Work to Manual: byte 0 bit 1
    = 0. Triangle + Yes: block bytes 0, 1 back to C3 1F, byte 2 with bits 0, 1, 3 clear and bit 2
    set, bytes 3 .. 7 = 00 00 32 32 0A.

## 12. Addresses

| Address | What | Verdict | Read |
|---|---|---|---|
| 0x14164E140 | `UTresUIP_HudStatus` constructor | — | C |
| 0x1416608B0 | status HUD update (slot 90): pawn values -> frames | differs | C + I (constants, calls) |
| 0x141662640 | status HUD flush: AS3 calls | differs | C |
| 0x141598CA0 | gauge frame helper | same | I |
| 0x1405AD600 | pinch test (HP <= 25 %) | same | C |
| 0x14158A340 / 0x14158D800 | low-HP alarm start / stop | differs / same | C |
| 0x14165F820 | enemy HP pack update | differs | C |
| 0x1416147A0, 0x14162FF80, 0x14162D720 | command menu ctor, update, setState / initMainCommand | —, differs, differs | C |
| 0x14167CEF0 | target HUD update | same | C |
| 0x1415B0ED0, 0x141774710 | UI manager update, part tick | differs | C |
| 0x141582ED0, 0x141583020, 0x1415831C0, 0x141583870, 0x141582CF0 | invoke wrappers | same | C |
| 0x14176F7D0, 0x141515B30, 0x1415157D0 | system SE, static SE, table lookup | differs, same, same | C |
| 0x1415E4340 | Stats screen input | same | C |
| 0x140E84340 | configuration block constructor | same | C |
| pawn vtable +0x17B0 / +0x1820 / +0x17D8 (0x1405AB180) / +0x1828 / +0x17E0 (0x1405AD8F0) / +0x1990 (0x1405AD660) / +0x1998 (0x1405A4690) / +0x19D0 (0x1405B9330) | HP, max HP, MP, max MP, MP Charge, focus, max focus, focus charge | — | C (use) |

| 0x14161E540 | command menu state table (13 states) | — | C |
| 0x141632660, 0x141632AB0, 0x141633B60, 0x141632E40 | command rows: enable / colour, shortcut rows, numbers | differs | C |
| 0x1416158F0, 0x14161AB00, 0x141630AF0, 0x141630ED0, 0x141631110, 0x14162BAF0 | situation commands: add, delete, timers, display, mark, switch | differs | C + I (constants) |
| 0x14162DC40, 0x141631BC0, 0x14162EF30, 0x14161C3F0, 0x14162AB00 | style gauge, form gauge, style in / out, mode in | differs | C + I (constants) |
| 0x14162CAE0, 0x14162C120 | shortcut panel, shotlock panel | differs | C |
| 0x141598C50 | float gauge frame helper | — | I |
| 0x14176F6B0, 0x14176F540 | static SE by index (2D, 3D) | — | C |
| 0x141668C60, 0x14166C730, 0x141677E90, 0x14167E1D0, 0x140ECF670 | target cursor add, delete, view block, projection, project | —, —, —, —, — | C + I (constants) |
| 0x14167A8F0, 0x14167B630 | shotlock scope state, lock count | — | C + I (sound ids) |
| 0x14158A860, 0x14165A050 | enemy gauge target | — | C + I (ability id, 0.5 s) |
| 0x1415849A0, 0x141576420 | HUD hide / show, minimap host word | differs | C |
| 0x141590D80 (0x141591290), 0x141594EB0 | UI manager constructor (tick group 5), tick execute | — | I, C |
| 0x14176A830 (0x14176AAC7), 0x143906EE0, 0x1438F8E90 | part movie open (ExactFit), default viewport | — | I |

| 0x1441B6B80, 0x1441BA050, 0x140EB0A30, 0x141591F70 | pause request bit, world tick (delegate 0x1487562F0), game-instance handler, gate | same, —, —, — | I (bit), C |
| 0x1415A9430, 0x140EA0680, 0x1415A2E10 | `SetPause`, `MayPause`, screen selector (camp menu or Pause) | — | I (gates, order) |
| 0x14142BA80, 0x14142C540, 0x141436AA0 | request interface slot 29 (camp menu allowed), vt+0xD8, vt+0x618 | same-partial, —, — | I (switch tables decoded) |
| 0x141516030, 0x141516360 | sound state `STATE_MENU_OPEN` / `_CLOSE` | — | C |
| 0x1415435D0, 0x14154C710 | camp controller: open request, state machine (party table, models) | — | C |
| 0x1415C0470, 0x1415C8DD0, 0x1415C5270, 0x1415D1450, 0x1415BD5F0, 0x1415D3530, 0x1415D0020 | camp menu: bind, open, input, update, close, party panels | — | C + I (entry order, sound ids) |
| 0x141580C80, 0x141765160, 0x14176BF60 | menu focus controller (pad -> events), event dispatch, `moveAnchor` | — | C |
| 0x141712640, 0x141717190, 0x141708B30, 0x1417102F0 | Pause screen: open, buttons (table 0x146B37820), input, default type | — | C |
| 0x1415C9390, 0x1415CD820, 0x1415CF300, 0x1415C59B0, 0x1415D2A00, 0x1415C50C0 | Abilities: open, list build, rows, input, toggle rule, Zero EXP confirm | — | C + I (toggle rule) |
| 0x140E95E90, 0x140E8ECE0, 0x140E8B640, 0x140E8ED40, 0x140E95F10, 0x140E93CA0 | ability word: SetEquipped, copy equipped, owned copies, copy NEW, set / clear NEW, record initialiser | same (small ones) | I (SetEquipped), C |
| 0x1415C9560, 0x1415C6150, 0x1415D3B00 | Items: open, input, use on a target | — | C |
| 0x1415E5540, 0x1415F0F40, 0x1415E66D0, 0x1415E30D0, 0x1415EC620, 0x1415ECC00, 0x1415A9CE0, 0x1415AA460 | Equipment: open, equipped list, stock list, input, equip rule, commit, piece pane, preview | — | C |
| 0x141446590, 0x141446640, 0x141782070, 0x1417820E0 | Keyblade switch: next / previous request, slot search | — | C |
| 0x1415E5400, 0x1415E1FA0, 0x1415EA9C0, 0x1415EED00, 0x1415EA860 | Customize: open, input, assignable lists, shortcut write, friend setting | — | C |
| 0x1415E5780, 0x1415E8E00 | Stats: open, filler | — | C |
| 0x1415397A0, 0x14154F520, 0x14154E660, 0x141541A20, 0x1415C4E70, 0x140EA6C90 | Config: bind, load (block -> items), store, input, write back, apply | — | I (load / store bit map), C |
| 0x140EAAF30, 0x140EAB060, 0x140EAB070, 0x140E49A00, 0x140EAAF40, 0x1405CD3F0, 0x1405EC530, 0x141616C10, 0x141575D00 | configuration readers: vibration, attraction / link cinematics, control type, combo cam, camera and reticle flips, sensitivities, command menu bits, map bits | — | I (the bit at each site) |

`tools/lib/sora_ui_iface.json` lists, for 79 UI classes, their functions by vtable slot with the
AS3 names each references, and for the interface movies the native users of each method.

## 13. Open questions and probe requests

Open (the running game or more reading settles them):

1. **Screen adaptation**: established (11a). Left: the 3D view rectangle at non-16:9 sizes (assumed
   equal to the HUD rectangle) and the value range of "Display Area".
2. **Update order**: established (11a). Left: which viewport delegate carries the projection pass
   (begin or end of the draw).
3. **Command system**: the display contract is section 5. Left to the command-logic specs: list
   contents and order, the timed override slots (+0x9F8), states 7 - 9 and 12, the source of
   `simpleHud` (+0x6E0), the caller of the time-drain function `0x1416303E0`. (Configuration byte 2
   bit 1 = Control Type B and byte 1 bit 7 = Cursor Position Lock: settled in 10.8.)
4. **Who shows / hides**: established (11a). Left: the lock bit of the focus gauge, the clip
   behind the target HUD's hide reasons.

5. **EXP path**: established (section 8). Left open: whether a repeated Next LV / munny call
   restarts the clip, the sound of the level-up popup, the Stats screen gauge code (the stats spec
   gives the fraction as derived).
6. **Enemy gauge**: established (section 6). Left: the meaning of the target test vt+0x10B0 and of `locked` (one caller, `0x1414C01E0`).
7. **Target HUD**: established (section 7). Left: the component test vt+0xA68 that hides a cursor, the
   exact x / y assignment of the multi-mark offset (sine / cosine order), the colour filter of the
   shotlock aim (`0x1415AACB0`).
8. **Config**: established (10.8): every option, its bit, default and readers. Left: byte 2 bit 2
   (set by the constructor, no reader found), the console-only rows (Graphics, Display Area value,
   Language) and the curve of Screen Brightness.
9. **Sounds**: the HUD cues are identified (section 9). Left: static SE 33 at `0x141584180` (which
   counter), and the non-HUD sources of MP Charge / focus full / situation command sounds (audio spec).
10. **Camp menu**: established (10.1 - 10.9). Left: (a) the on-screen order of the top entries is
    the code's (Equipment first); one look at the running game confirms it; (b) which jump phases
    are the values 7 and 8 of the jump state's field +0x78 that refuse the menu; (c) GI+0x880 bit 0
    ("event running") and GI+0x630: who sets them; (d) save flag GW+0x64 bit 0 that forces the Pause
    screen; (e) the Stats arrow clips (`m_*ArrowIcon`): no writer found; (f) whether play time
    advances while the menu is open; (g) item-use benefit test `0x1415C3C30` and the discard rule
    `0x141126C10` were not read; (h) row visuals (clip labels of list rows) were taken from the
    proxies only, not compared with a capture.

Runtime-probe requests (object, offset, type):

| Object | Offset | Type | Why |
|---|---|---|---|
| `TresCockpit.m_pHudStatus` (cockpit +0x70) | +0x1D4, +0x1D8, +0x1DC, +0x1E0 | i32 x 4 | max HP, max MP, HP, MP as the HUD caches them |
| same | +0x1E8, +0x1EC, +0x1F0, +0x1F4, +0x1F8 | i32 x 5 | HP / MP base frames, HP / MP frames, last sent HP frame: checks the frame rule in play |
| same | +0x3DC, +0x3E0, +0x3E8, +0x3FC | i32, i32, i32, u32 | max focus, focus, focus frame, flag word (charge, consume, lock, parts visible) |
| same | +0x210 | u32 | slot 0 flags (MP Charge bit 0) |
| `TresCockpit` | +0x258, +0x259 | u8, u8 | alarm reason masks (pinch = bit 0) |
| `TresCockpit.m_pHudRight.m_EnmHpPack` | +0x110, +0x114, +0x118, +0x130, +0x134, +0x148 | i32 x 5, f32 | enemy max HP, HP, stock, HP per bar, cap, hide timer |
| user configuration block `(GameInstance + 0x870) + 0x2E8` | +0 .. +8 | u8 x 9 | confirm the option map of 10.8 (toggle Control Type, Cursor Position, Combo Cam once) |
| Sora save record `GW+0x9640 + i * 0x9C0` | +0x160 + 4 * id for ids 4, 5, 6, 23 | u32 | watch the words while installing / uninstalling in the Abilities screen: bit 1 / 5 / 9 toggles, bit 2 / 6 / 10 clears on the first cursor visit (10.4) |
| `GI` | +0x8D4 | u8 | bit 1 camp menu disabled, bit 2 battle, bit 4 special battle: decides camp menu or Pause screen (10.1) |
| `TresCampController` (UI manager +0x368) | +0x58, +0x5C, +0xC8 .. +0x3E8 | i32, i32, bytes | controller state, open mode, the five party records (snapshot at +4, name at +0x70) |
| `TresUIP_HudCommand` (cockpit +0x78) | +0x1D0 .. +0x310 | bytes | `m_mainCmdInfoAry`, `m_shortcutCmdInfoAry`: command lists as mirrored for the UI |


## Save and Load screens (2026-10-12)

The slot lists of the movie `17_saveload/SaveLoad` (save list, load list, the row, keys, dialogs, the title menu as a record) are specified in `docs/sora-saveload-spec.md`; the port is `crates/kh-ui/src/saveload.rs`.
