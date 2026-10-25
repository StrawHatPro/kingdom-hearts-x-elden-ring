# Sora port: Kingdom Hearts III's Save and Load screens (and the title menu, as a record)

2026-10-12. From the game's data and code. Marks: **[A]** read from an asset (movie, AS3 listing, string
table, data asset); **[C]** decompiled C only (not checked at instruction level); **[D]** derived from the
two; **[L]** LOCAL choice of the port; **[?]** not determined. Nothing here was seen in the running game:
no capture of KH3's save screen exists in this project.

Sources: movie `/Game/UI/Scaleform/17_saveload/SaveLoad` (pack `ui/movies/SaveLoad.json`, AS3 listing
`ui/as3/SaveLoad.as3.txt`); native part `UTresUIP_SaveLoad` (constructor 0x1416F5C30, size 0x740) with
`UTresUIP_SaveLoadListItem`, `UTresUIP_SaveLoadAutoSaveButton`, `UTresUIP_SaveLoadSaveDataDisplayInfo`;
string tables `kh3_menu.save`, `kh3_menu.savepoint`, `kh3_menu.title`, `kh3_system.general`,
`kh3_settings.top`. The names of the part's string globals (`DAT_1486F3xxx`) were resolved from their
static initialisers (list: `build/sora_tmp/er/saveload/fnames.txt`).

## 1. The movie [A]

One movie holds every screen of the save point and of loading. Root labels off / in / loop / inTutorial /
outTutorial / out; children:

| clip | what |
|---|---|
| `menuSavePoint` | the save-point menu (six `TopMenuCmd`; docs/sora-savepoint-spec.md 5) |
| `menuSaveData` | THE SAVE LIST: labels off, in (5), inNext (33), loop (47), outBack (51) |
| `menuLoadData` | THE LOAD LIST: `autoBtn` (the autosave row) above a four-row list; off / in / loop |
| `menuLoadClearData` | a five-row load list without the autosave row ("Select clear data.": New Game carry-over, Re Mind) |
| `menuAreaSelect`, `menuTutorial` | Select an Area, Tutorials |
| `stageFrame` (`tres.rsl.StageFrame`) | play time, munny, the area's world title and name, the button guide |
| `stageBg` (+ `bgColor` color1 / color2) | a dark full-screen background |

Each list clip has a `stageCategory` (the title plate at the left edge, text field
`category.categoryName.txName`) and an object with `coList` (a `tres.controls.List`), `helpText` (the help
line at the bottom, x 395 y 892) and a `scrollbar`.

**Lists.** `CoSaveDataList`: `rowNum` 5, one column, `enableScrollLoop` true, `enablePaging` true,
`repeatStop` on every side. `CoLoadDataList` (in `menuLoadData`): `rowNum` 4, no scroll loop, paging,
`repeatStopDown`; its anchors are chained with the autosave button (index 41 above, the list 42).
`CoLoadDataList_R5` (in `menuLoadClearData`): 5 rows. The list's first row sits at (476, 202) on the
1920 x 1080 stage (the four-row load list at y 322, the autosave button at y 172); a row's `rect` is
968 x 120 px (the pitch), its focus area 968 x 110.

**A row** (`CoSaveDataListItem` / `CoLoadDataListItem`, labels normal / select / decide / disable): 

| part | where (first row, stage px) | what |
|---|---|---|
| `txNum.textField` | 504, 247, size 24, centred | the slot's number (authored text "00") |
| `cursor` (`tres.rsl.ListItemBgSave`) | the row | the light bar: normal / selectIn / select / decide / selectOut; parts base / grd / anm / crown with colour variants color1 .. color3 |
| `dataLabel` | | labels **info** (everything), **info2** (level and munny hidden), **empty** (only "NO DATA") |
| `dataLabel.txDataEmpty` | 741, 249 | `kh3_menu.save.manual_0001` "NO DATA" |
| `dataInfo.worldTitleImage.coImage` (`CoImgWorldTitle`) | 568, 192; 164 x 128 | the world's title image, an external texture |
| `dataInfo.txLv` + `txNumLv` | 742 / 774, 226 | "LV" (`kh3_menu.save.level`) and the level |
| `dataInfo.txNumMunny` + `txMunny` | 835 (right-aligned) / 949 | munny and "MUNNY" (`kh3_menu.save.munny`) |
| `dataInfo.txDifficulty` | 1031, 224, size 23 | the difficulty's name |
| `dataInfo.txPlayTime` | 1182, 226, right-aligned, authored "999:99" | play time |
| `dataInfo.txAreaName` | 741, 267, size 20 | the area's name |
| `dataInfo.charaImage.coImage` (`CoImgChara`) | 1189, 142; 256 x 256 shown through the clip rectangle y 60 .. 169 | the face image, an external texture; `charaImage` has normal / select |
| `dataInfo.clearMark` | 506, 212 | the crown of clear data (off / loop) |
| `dataInfo.dlcMark` | | the DLC mark |

The two images are `tres.controls.Icon` clips: the constructor removes the authored placeholder
(`removeChildren`) and a loaded bitmap is drawn stretched over the clip's authored size, through its `rect`
when it has one [A `Icon.onLoaderLoadComplete`].

## 2. What a row shows [C + A]

The list-refresh handler 0x1416FDB00 (slot number, order number, row data) fills twelve fields from the
slot's summary (records of 0x308 bytes from the save manager 0x1487E5180; 0x1416FE2F0 finds the record of a
slot or returns -1):

| field | from the record | shown as |
|---|---|---|
| order number | the argument | the slot number [?: formatting; the field is authored "00"] |
| has data | found / -1 | `info` or `empty` |
| "full info" | byte +0x22 (the played character) not in 8 .. 10 / 0x10 .. 0x11 | `info` or `info2` (no level / munny for the non-Sora episodes) [D] |
| clear mark | byte +0x29 bit 0 | the crown |
| DLC mark | flags +0 bit 26 | the DLC mark |
| world title | world id +8 through table `/Game/UI/Data/WorldIconName` (0x14159A440) | `/Game/UI/Icon/MenuWorldTitle/MenuWorldTitle<XX>` (18 worlds; the world's name key is `kh3_map.wm.world_<xx>`) |
| level | byte +0x1C | the number |
| munny | +0x18 | the number |
| difficulty | +4 (4 is shown as 0) | `kh3_menu.save.difficulty_0 .. 3`: Beginner / Standard / Proud / Critical |
| play time | +0x10 through 0x1415B26B0 (seconds) | `kh3_system.general.system_0009` "{0}:{1}": hours unpadded, minutes `%02d` |
| area name | area id +0x44 (or +0x54 for a DLC episode) through 0x140E8C810 | the text |
| face | (episode, played character +0x22, world id) through 0x14155EA50 = table `/Game/UI/Data/SaveMenuFaceIconData` (rows by world and game progress: lists of character ids, a "lottery") | `/Game/UI/Icon/MenuFace/MenuFace<World><Chara>` (37 images; Sora: Ex, Ex2, Ca, Mi, Ts, Bx) |

No name is shown (KH3 has one hero) and no thumbnail of the scene.

**How many slots.** The save list is created with **9** rows of data (`0x141548CB0(list, 9)`) [C]; slot
numbers are 1 .. 9 (list index + 1). Slot **0** is the autosave: the load screen's `autoBtn`
(`UTresUIP_SaveLoadAutoSaveButton`, event component 9, record of slot 0). So: nine manual slots and one
autosave; five rows are visible on the save list, the autosave row plus four on the load list.
An autosave row does not exist on the save list.

## 3. Titles, help lines, button guide [C + A]

| screen | title (`stageCategory`) | help line | stage-frame guide |
|---|---|---|---|
| save-point menu | `kh3_menu.savepoint.title_0001` "SAVE" (icon 0) | per command | |
| save list | `kh3_menu.save.title_0001` "SAVE" (icon 0x14) | `kh3_menu.save.detail_0001` "Select a slot to save to." (`_dlc` variant in a DLC episode) | `kh3_menu.save.button_0001` "(Triangle) Delete" |
| load list | `kh3_menu.save.title_0002` "LOAD" (icon 0) | `detail_0002` "Select a slot to load from." (`_dlc`, and `detail_0004` with "Use (Square) to continue from the main story") | `button_0001` / `button_0003` |
| clear-data list | "LOAD" | `detail_0003` "Select clear data." | |

The help line is set when the cursor moves (0x141727670, event 0x14).

## 4. Keys and flow [C: 0x141708F70, the part's event handler; components 7 = save list, 8 = load list, 9 = autosave button, 10 = clear-data list]

- **Open (save).** The save-point menu's Save (component 0, decide): `menuSavePoint` plays `outNext`,
  `menuSaveData` plays `inNext`, the list gets 9 rows, **the cursor goes to the slot of the last save**
  (game instance +0x42C, minus one; scroll +0x430), the stage frame shows the Delete guide. The list takes
  keys when its way in ends (AS3 `ON_END_IN_SAVE_LIST` -> 0x141705A60).
- **Decide on a save row.** Empty slot: **the save starts at once** (0x141715DE0), no question. Used slot:
  the question dialog (0x1415A1C70, kind 2) with `kh3_system.general.dialog_0002` **"Overwrite the save data
  in this slot?"**, entries `choice_0001` **Yes**, `choice_0002` **No**, the cursor on **No** (default index
  1). Yes (index 0) saves (0x141708080); No gives the row its focus back. (When the game has clear data to
  protect, `kh3_menu.save.dialog_0004` is shown first as an information dialog.)
- **Saving.** 0x141715DE0 hands the slot to the UI manager's save (0x1415A6970), which shows the access
  message 0x1415A3620: `kh3_system.general.dialog_0001_xb1` **"Saving... / Don't turn off your PC at this
  time."** (the PC build's `_xb1` strings). When it ends (0x141707000, result 1): **static SE 14**
  (`se00000_015`), the row is refreshed and focused, keys count again. **There is no "saved" message.** At a
  save point the player stays on the list; cancel leads back. Results other than 1 raise a message dialog
  [C; its text was not traced].
- **Decide on a load row.** Empty slot: the refuse sound (static SE 4). Used slot: **the load starts at
  once** (0x1417043C0): **the game asks nothing** ("Load this data?" does not exist in its tables). The
  access message is `dialog_0006_xb1` "Loading... / Don't turn off your PC at this time.".
- **Cancel.** Save list: `menuSaveData` `outBack`, `menuSavePoint` `inBack` (0x1416F7F00), the stage frame
  loses the guide. Load lists: the part closes (0x1416F91C0). Save-point menu: the part closes.
- **Triangle** (events 0xE, lists 7 / 8, a used slot): "Delete the save data in this slot?"
  (`kh3_system.general.dialog_0003`; `dialog_0013` names the photos freed), then "Deleting..."
  (`dialog_0004_xb1`). **Square** (event 0xF, load list): continue the main story of a DLC save.
- **L1 / R1** (event 0x15): the list's page function (0x141543240; AS3 `enablePaging`).
- **Up / down**: the list's `moveAnchor`; the save list wraps on a fresh press and stops while the key
  repeats (`enableScrollLoop` + `repeatStop`); the load list does not wrap, up from its first row goes to
  the autosave button. Cursor sound: the menus' static SE 0; decide 1; open 2; close 3 [D: the shared
  focus controller; not traced for this part].
- **Start / the Menu button**: not handled by the part [C].
- The part's tick (0x141720F60) updates the stage frame's play time each minute and runs a small state
  machine (+0x63C: wait for the save, 0.3 s, ...) for the "save, then leave" entry points (clear data after
  the battle report: `kh3_menu.save.dialog_0005` / `dialog_0006` "Please save your progress up to this point.").

**Failure texts** [A, their use here is [?]]: `kh3_settings.top.system_0030` "Failed to save game data.",
`system_0040` "Save data is corrupt. Failed to load game data.", `system_0010` "Save data is corrupt and
will be deleted. / Creating new save data.", `kh3_menu.title.dialog_0015` "The save data is not compatible
with this version of the game. ...".

**Title variant against save-point variant** [D]: the same lists. From the title the part is opened on the
load list directly (root `in`, `menuLoadData` `in`, no world behind it: `stageBg`); at a save point the save
list comes out of the save-point menu over the paused world. The game-over menu's Load opens the same load list.

## 5. The title menu (RECORD ONLY; nothing is built)

Movie `/Game/UI/Scaleform/11_title/Title` (`UTresUIP_Title`, size 0x720; data asset
`/Game/UI/Data/UIDataAssetTitle`: textures TitleBg00 .. 02, TitleBgNami*, TitleBgSora, TitleLogo,
TitleMenu01, NewBtn*, ...). It is NOT in the UI pack. Entries and help lines, `kh3_menu.title` [A]:

| key | entry | help |
|---|---|---|
| menu_0001 | CONTINUE | Continue the game from your most recent save data. |
| menu_0002 | NEW GAME | Start the game from the beginning. |
| menu_0003 | LOAD | Resume from the save data of your choice. |
| menu_0004 / 0007 | CONFIG | Configure game settings. |
| menu_0005 | THEATER | Revisit cutscenes you have seen in the game. |
| menu_0006 | TUTORIALS | Go hands-on and learn how to play the game. |
| menu_0008 | MEMORY ARCHIVE | Look back on the events of the past. |
| menu_0009 | SPECIAL | Look back on your journey so far. |
| menu_0010 | Downloadable Content | Play downloadable content (DLC). |
| menu_0012 .. 0017 | Re Mind, LIMITCUT EPISODE, SECRET EPISODE, DATA GREETING, SLIDESHOW, ORCHESTRA | (the DLC menu) |

NEW GAME: window "NEW GAME" / "Select a game mode." with Beginner / Standard / Proud / Critical Mode and
their descriptions (`window_0003 .. 0006`), "Begin the game with this difficulty?" (Yes / No), the autosave
notice `dialog_0001_xb1`, the carry-over question (`window_0020 .. 0022`). [?]: the order and the conditions
of the entries (CONTINUE needs a save; THEATER / MEMORY ARCHIVE / SPECIAL / DLC unlock), the part's key
handler, the layout: none of it was read.

## 6. The port

**Pack.** `tools/sora_saveload_pack.js` (adds only) -> `build/sora_pack/saveload/`: `saveload.json`, the six
Sora faces, the 18 world titles and `/Game/UI/Icon/Hologram/HologramArea/MenuWorldTitleNone` (the game's
own world title for "no world": a dark blue oval with a question mark); 26 rows `saveload/...` in
`sheets/port/sora-assets.json`.

**`crates/kh-ui/src/saveload.rs`**: `SaveLoadScreen` (`SaveScreen` / `LoadScreen` are the same type) over
`SlotSummary { index, state, name, level, munny, play_time_seconds, location, world, mode, clear,
is_current, portrait }`. It follows section 4: the save list is `menuSaveData` as the game drives it; an
empty slot saves at once; a used one asks with the cursor on No; an empty load row refuses; a used one
loads without a question; "Saving..." / "Loading..." while the host works; static SE 14 after a save and
the list again; cancel plays `outBack`. Tests `crates/kh-ui/tests/saveload.rs` (4).

What is COMPOSED or LOCAL there:
- **the load screen is the movie's five-row list without the autosave button (`menuLoadClearData`)** with
  the load screen's title and help: the host (ten equal slots of one playthrough) has no autosave slot. The
  game's own load screen (`menuLoadData`) would show the autosave row and four rows;
- the list has as many rows as the host gives (ten for Elden Ring; the game: nine);
- a slot whose place is no KH world shows `MenuWorldTitleNone`; the location text is the host's string in
  the area-name field; `name` is not drawn (the row has no field for it);
- `mode` goes into the difficulty field (a difficulty's name, or the host's own word);
- the face is one of Sora's six images, chosen by the host (the game picks by world and progress);
- the access message is the Dialog window with the game's text and no button, at least 45 frames; a
  failure shows `kh3_settings.top.system_0030` / `system_0040` with OK;
- the Dialog window keeps its authored size (the buttons of the overwrite question sit low in it);
- the stage frame (play time, munny, area, the Delete guide) is not shown; Delete (Triangle) and the DLC
  keys are not built; slot numbers are two digits; the row cursor's colour variant is as authored; the 12
  frames of a row's decide animation before the choice counts.

**`kh_host::hud::Hud`**: `open_save`, `open_load`, `saveload_frame` (no world needed), `set_saveload_busy`,
`set_saveload_slots`, `take_saveload_result`, `saveload_open`, `saveload_state`, `close_saveload`,
`saveload_background`; drawn on the layers "NewGameText" (list) and "NewGameDialog" (message window), which
are never up together with it: `LAYERS` stays at 14. A cancelled Save screen that came out of the
save-point menu brings that menu back (`SavePointMenu::reopen_back`, the clip's `inBack`); a cancelled Load
screen that came out of the game-over menu brings that menu back. `kh_host::save`: `slot_file`,
`slot_summary`, `slot_summary_of`, `slot_summaries`.

**Test app** (`crates/kh-app/src/saveload.rs`): ten `.khsave` files in `KH_SAVE_DIR` (default
`build/khgame_saves`); the save-point menu's Save, the game-over menu's Load, and the script word
`title-load` (the Load screen over black with the world held).

**(SV, 2026-10-17, the user: "Have a menu to load save data at save points, and have the ability to delete save data in
the save menu (without saving)")** KH3's Delete is built (`SaveLoadScreen::delete_enabled`): Triangle / Y on a used row of the
save or the load list asks `kh3_system.general.dialog_0003` "Delete the save data in this slot?" (Yes / No, the cursor on No),
Yes shows "Deleting..." (`dialog_0004_xb1`) and tells the host `Delete(slot)`; done = static SE 14 [L: the save's access-done
sound] and the row shows NO DATA; the guide `kh3_menu.save.button_0001` "<icon(Triangle)> Delete" is a second help line [L: the
stage frame is not shown]; an empty row refuses (static SE 4); a row the host protects (`delete_current` off: the running
game's slot) refuses with "This save data is in use and cannot be deleted." [COMPOSED]. The save-point menu has a **Load**
entry (`SavePointCommand::Load`, the title menu's LOAD words, icon cell `shopGm2` [L]); its Load screen asks "Load the save data
in this slot? / All unsaved game progress will be lost." first (`confirm_load`, [COMPOSED]: KH3 asks nothing); its cancel
brings the menu back with the cursor on Load. Notes: build/sora_tmp/party/IMPL_SV_STATE.md.

## 7. What only the running game can settle

1. The look of a filled row (cursor colour variants, the face's `select` state, how the slot number is written).
2. The message dialog's exact look while saving, and what a failed save says.
3. Whether the save list's rows 6 .. 9 are reached by scrolling one row or by pages when the stick is held.
4. The title menu's order, conditions and layout.
