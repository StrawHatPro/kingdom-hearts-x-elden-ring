# KH3 weapon assets: every keyblade, its formchanges, Donald's and Goofy's weapons, the Moogle shop, the form colours on KH1 / KH2 Sora

Assets only (2026-11-03). The game code that equips them is another wave; it reads `build/sora_pack/keyblades/manifest.json`.
Everything is read from the user's own KH3 install (base game + Re Mind, both installed). Nothing here was launched in a game.

## 1. What is where

| Folder / file | What | Size |
|---|---|---|
| `keyblades/manifest.json` | THE INDEX (`kh3-sora-keyblades/1`): `keyblades.<w_soNNN>` (name, item rows, forms, models, sounds, effects with the root that holds each, `dlc`), `models.<model>`, `party_weapons`, `npcs`, `sora_forms`, `vfx.where`, `pack_inputs_folders` | 0.4 MB |
| `keyblades/<model>/` | `weapon.glb` (LOD 0, one skin, Kingdom Key conventions), `textures/*.png` (base colour, normal), `skeleton.json` (kh_anim `Skeleton::from_manifest`, as `xsi/weapons/<model>/skeleton_xw.json`), `anims/<clip>.glb` (every row of the model's own anim sets), `model.json` (fingerprint, clips by logical name, notifies) | 68 models, 325 MB |
| `keyblades/materials/` | the models' TRANSLATED materials (tools/sora_material_pack.js outfit mode): 138 slots, 55 shader owners, all 55 translated, owner ids 500+, meshes `kb:<model>`; shaders `materials/er` (kh-shaderc, see gaps) | 143 MB |
| `keyblades/sora_forms/anims/*.glb` + `keyblades/sora_forms_manifest.json` | Sora's own clips of every form anim set (p_ex001_AnimSet2..23 + Rage's AnimSet13) the main pack / XS overlay lack: 1208 clips, 104 blend spaces described, 272 root-motion tracks; an overlay in the `xsi_manifest.json` format that kh-anim `Pack::load` now reads | 188 MB |
| `keyblades/vfx/` | the keyblades' and the shop's particle systems the main / party / XS roots lack: 826 listed (838 packed with children), 4730+ emitters, its own shading tables (`vfx/shaders`) and DXIL (`vfx/er`, 826 of 1027 pixel shaders translated, 1328 shaders built) | 326 MB + 16 MB |
| `keyblades/outfit_forms.json` | per (outfit, wear form): the KH3 form palette, the region mapping, the textures (section 6) | 0.1 MB |
| `npc/n_ex014/`, `npc/n_ex028/`, `npc/shop/materials/` | the Moogle shop NPCs (model.glb, textures, skeleton, clips) and their translated materials (owner ids 600+, meshes `npc:<id>`) | 10 MB |
| `audio/` (rebuilt) | + every keyblade weapon's swing / hit / alias sets, the keyblades' effect-attached SE, the form clips' voice / SE (banks 02002..02019, DLC 02020..02022 ...): 646 banks (was 622), 4050 triggers | +~155 MB |
| `outfits/kh1|kh2/materials/textures/*_kbf<N>.png` + `wear_extra.json` | the KH1 / KH2 form-colour variants | 15 MB |

Pack size added: about 1.2 GB (keyblades 1.03 GB, audio 0.15 GB, outfits 15 MB, npc 10 MB).

## 2. Keyblades (21 TresEquipKeybladeData; 17 have an item row)

Ids are the equip data (`Blueprints/Player/p_ex001/w_soNNN_Equip`). Names from TresEquipItemData `m_EquipSubclass` -> TresItemData
`m_NameTextID` -> `kh3_item.locres`. Forms: `form/grow (wear form: Sora's anim sets)`. All meshes, form weapons, transform units,
finisher pawns load in the renderer's loader (picture test). FX = effects of the keyblade found in a root / named.
Sounds = hit / swing / alias sets of the base blueprint (every entry resolves: kh-audio test).

| Id | Name | Base | Forms (Sora's anim sets) | Models | Mesh | Forms | FX | Sounds |
|---|---|---|---|---|---|---|---|---|
| w_so010 | Kingdom Key | w_so010 | 01/LV2 Second Form (wear 05) | 1 | ok | ok | 29/29 | hit/swing/alias |
| w_so020 | Shooting Star | w_so020 | 02 (W02: AnimSet2), 05 (W02: 5) | 5 | ok | ok | 71/71 | hit/swing/alias |
| w_so030 | Hero's Origin | w_so030 | 0B LV1..3 (W03: 11) + Special w_so037, pawn w_so036 | 7 | ok | ok | 65/65 | hit/swing/alias |
| w_so040 | Hunny Spout | w_so040 | 10 (W03: 2+16), 11 (W03: 5+17) | 5 | ok | ok | 66/66 | hit/swing |
| w_so050 | Favorite Deputy | w_so050 | 03 (W01: 3), 06 (W01: 6) | 4 | ok | ok | 82/82 | hit/swing |
| w_so060 | Happy Gear | w_so060 | 0C (W01: 12), 04 (W01: 4) | 6 | ok | ok | 103/103 | hit/swing |
| w_so070 | Ever After | w_so070 | 0A (W02: 10), pawn w_so073 | 4 | ok | ok | 75/75 | hit/swing |
| w_so080 | Crystal Snow | w_so080 | 12 (W04: 12), 07 (W04: 7), pawn w_so088 | 9 | ok | ok | 95/95 | hit/swing |
| w_so090 | Wheel of Fate | w_so090 | 0E (W01: 14), 0F (W01: 15), pawn w_so094 | 5 | ok | ok | 71/71 | hit/swing |
| w_so100 | (no item row) | - | no m_EquipMap at all | 0 | - | - | - | - |
| w_so110 | Classic Tone | w_so110 | 14 (W03: 3+20), 15 (W03: 6+21), pawn w_so114 | 5 | ok | ok | 108/108 | hit/swing |
| w_so120 | (no item row) | w_so120 | 01/LV2 (W05) | 1 | ok | ok | 8/8 | hit/swing |
| w_so121 | Dead of Night | w_so121 | 01/LV2 (W05) | 1 | ok | ok | 9/9 | hit/swing |
| w_so140 | Grand Chef | w_so140 | 13 LV1..3 (W04: 11+19), pawn w_so145 | 6 | ok | ok | 80/80 | hit/swing |
| w_so150 | Starlight | w_so150 | 01/LV2 (W05) | 1 | ok | ok | 34/34 | hit/swing |
| w_so160 | (no item row; spawns w_so010) | w_so010 | 01/LV2 (W05) | 1 | ok | ok | 29/29 | KK's |
| w_so170 | Nano Gear | w_so170 | 08 (W04: 8), pawn w_so174 | 3 | ok | ok | 67/67 | hit/swing |
| w_so180 | (no item row; spawns w_so010) | w_so010 | 01/LV2 (W05) | 1 | ok | ok | 29/29 | KK's |
| w_so190 | Ultima Weapon | w_so190 | 09 (W06: 9) | 3 | ok | ok | 68/68 | hit/swing |
| w_so200 | Oathkeeper (DLC) | w_so200 | 16 Light (W08: 22), 18 Double (W0A: 23) | 2 | ok | ok | 82/82 | hit/swing (Sound/_DLC 02020) |
| w_so210 | Oblivion (DLC) | w_so210 | 17 Dark (W09: 22), 18 Double (W0A: 23) | 2 | ok | ok | 82/82 | hit/swing (Sound/_DLC 02021) |

Per form, `keyblades.<id>.forms[]` gives the weapons on Sora's sockets (socket, bone, socket location / rotation / scale),
transform units (the "Deform" model), magic / projectile sets, UI data and finisher pawns. Every socket the equip data names is on
Sora's skeleton. A model's clips are keyed by the logical names its anim set gives (the weapon plays the row of the same name
as Sora).

DLC: Re Mind is installed: `Effects/dlc01` (w_so200, w_so210, w_so220 = the Double Form finisher effects), `Effects/dlc02`
(Re Mind's other characters), `Sound/_DLC/SE/weapon/02020..02022`, `Character/pc/p_ex001/mdl/mat0/FormChangeTex/dlc01`
(Light / Dark / Double costume). The DLC keyblades are Oathkeeper and Oblivion with Light, Dark and Double Form. No other
keyblade item exists in the install (item rows SO_15 / SO_16 are absent).

## 3. Donald's and Goofy's weapons

Already packed by tools/party_pack.js (all 14 + 14: `party/weapons/<model>/weapon.glb`, materials in each member's library,
swing / hit sets in the audio pack, 7 entries each). Indexed in `keyblades/manifest.json party_weapons` (item, name, glb,
socket, sets). Nothing missing.

## 4. The Moogle

| NPC | Shop blueprint | Class | Clips | Use |
|---|---|---|---|---|
| n_ex014 | g_ex_mogli_01 | TresShopActor | M_001 / Idle_GM, M_002, n_ex014_pub01 (holo set) | the standing Moogle: the Roundtable synthesis shop |
| n_ex028 | g_ex_mogli_02 | TresShopActorMogli | M_001, M_010 | the balloon Moogle; its arrival effect vg_com_mgl_shp00 is in keyblades/vfx |

Model, textures, skeleton, clips: `npc/<id>/`; translated materials `npc/shop/materials` (5 slots, 5 owners, all translated,
fur shader included). Sounds: the Moogle has no voice assets and no sound notifies in its clips; the shop's open / buy sounds
are played from native code (TresShopActor*), not data: a gap for the gameplay wave (pick system SE from PACK/audio).

## 5. Tools

- `tools/sora_keyblades_pack.js` (new): phases models, materials, forms, audio, vfx, npc, manifest; `--only`, `--force`, `--list`.
  INCREMENTAL: per model / per clip / per library fingerprints (tool version + sha256 of every cooked source): a run with nothing
  changed writes nothing (35 s); an added keyblade writes only its files.
- `tools/sora_outfit_formcolours.js` (new): section 6; `tools/lib/second_form_swap.js` (new): KH2 Second Form (6a); `tools/lib/rage_bake.js` (new): Rage Form (6b).
- `tools/packgen.js`: steps `keyblades` and `outfit-kb-forms` before `audio`.
- `tools/packgen/recipes/shaders.json` + `tools/packgen_recipe.js`: char dirs `keyblades`, `npc/shop` (built as the party's),
  effect root `build/sora_pack/keyblades`.
- `tools/sora_audio_pack.js`: reads `audit/sora_port/audio/keyblade_rows.json` (weapon rows of Sora per model, effect SE, requests).
- `tools/lib/bcdec.js`: PF_FloatRGBA decode (a keyblade texture).
- `crates/kh-anim/src/pack.rs`: the keyblade overlay is read after the XS one (lazy clips).
- Tests / pictures: `crates/kh-er-render/examples/kb_pics.rs`, `crates/kh-vfx/tests/keyblade_fx.rs`,
  `crates/kh-audio/tests/keyblades.rs`, `crates/kh-anim/tests/keyblade_forms.rs`; `crates/kh-audio/tests/pack.rs` counts Sora's own
  rows without the keyblade rows.
- `sheets/port/sora-outfit-voice.json`: KH1's voice table covers the 3 new form voice lines (bt1010710 -> finisher,
  bt1010770 / bt1010840 -> combo_heavy, by the KH3 numbering next to each row's own).

Times (this PC): keyblade pack from scratch 4.1 min + effect tables 40 s + kh-shaderc effects 15 s; audio pack 35 s; outfit
colours 50 s; a no-change run 35 s. On a player's PC add the extraction / conversion of the new folders.

## 6. Form colours on KH1 and KH2 Sora

KH3 recolours Sora by WEAR FORM (p_ex001_WearformSet m_Forms[N]: the base colour textures of p_ex001_mat0/1/2_Inst swap to
FormChangeTex/p_ex001_<N>d00..02 while FormChangeSlide runs 0 -> 1). Each keyblade form names its wear form:

| Wear form | KH3 name | Keyblade forms |
|---|---|---|
| 1 | Brave | Favorite Deputy 03 / 06, Happy Gear 04 / 0C, Wheel of Fate 0E / 0F |
| 2 | Wisdom | Shooting Star 02 / 05, Ever After 0A |
| 3 | Solid | Hero's Origin 0B, Hunny Spout 10 / 11, Classic Tone 14 / 15 |
| 4 | Grace | Crystal Snow 07 / 12, Grand Chef 13, Nano Gear 08 |
| 5 | Limit | Second Form of Kingdom Key, w_so120, Dead of Night, Starlight (w_so160 / 180) |
| 6 | Final | Ultima Weapon 09 |
| 7 | Anti | Rage Form (no texture change in KH3; its point pattern p_ex001_7f00) |
| 8 / 9 / 10 | Light / Dark / TWO-KB | Oathkeeper 16, Oblivion 17, Double Form 18 (Re Mind) |

The variants are generated from the outfits' own textures: KH3's form palette per region of KH3 Sora's default costume (the
DOMINANT colour the form puts where the default has red / yellow / black / grey / white; `accent` = the form's dominant vivid
cloth colour; `contrast` = its dominant dark cloth colour; `hue_*` = its dominant colour per hue family), each KH1 / KH2 cloth
texel classed by hue / saturation / value and given its region's KH3 colour with its own shading kept (a light target compresses
the highlights instead of clipping, so texture detail stays). Mapping:

- KH1: red jumpsuit = accent; yellow shoes = KH3 yellow trims (Final: silver); blue pockets / belts = KH3 black (Final: the form's
  dark contrast); white / cream = white; grey = grey.
- KH2: black / navy jacket and shorts = KH3 black; red flaps and blue details = accent (Final: contrast); yellow straps = trims
  (Final: silver); grey hood = grey; white = white.
- KH2 Second Form: a MIRROR SWAP from data (section 6a). The earlier variants (sora_outfit_forms.js's colour table, the palette
  versions) are dropped from KH2's wear_extra.json (no other form uses them).
- KH1 Second Form: unchanged (tools/sora_outfit_forms.js's styled look the user chose in ER69 / ER72).
- Rage (7): KH3's real Rage look (section 6b), not a palette.
- Double (10): KH3's TWO-KB palette equals Light's; its trims take Dark's violet so the two differ (a rule of ours).

Each (outfit, form) is a `direct["<N>"]` entry of `outfits/<id>/materials/wear_extra.json` (kh-look `Library::step_wear` puts it on
when wear form N is applied) and an entry of `keyblades/outfit_forms.json`. KH3 Sora's own forms 1..4, 6..10 are not in the main
library yet (only 0 and 5 are): see the gaps.

Pictures (build/sora_tmp/keyblades/pics): `outfit_forms.png` (rows KH3 / KH2 / KH1, columns wear forms 0..10, legend .txt),
`second_form_swap.png` / `second_form_swap_large.png` (KH3 default, KH3 Second Form, KH2 default, KH2 Second Form), `second_form_closeup.png` (KH3 default vs KH2 Second Form, jacket and shorts), `rage_side_by_side.png` (KH3 / KH1 / KH2
in Rage Form; KH3's is a mock-up of its anti mode), `rage_before.png` (the earlier palette Rage), `fix_compare.png` (Final Form
before / after), `outfit_forms_before_fix.png`, `outfit_forms_before_rage.png`.

## 6a. KH2's Second Form = KH2 Sora wearing KH3's default colours (the user, 2026-11-03)

KH3's Second Form costume (p_ex001_5d0N) repaints KH3 Sora with KH2-style colours; KH2's Second Form inverts it
(tools/lib/second_form_swap.js):
1. THE TABLE (KH3's own texel pairs, Second Form colour -> default colour, cloth textures 0d00 / 0d01 / 0d02, entries >= 0.3 % of
   the cloth; agreement = share of the entry's texels whose default colour is the chosen one):

   | KH3 Second Form colour | KH3 default colour | Share | Agreement |
   |---|---|---|---|
   | 50, 49, 51 (black) | 34, 33, 33 (black) | 21.3 % | 31 % |
   | 54, 57, 76 (navy) | 56, 67, 59 (dark grey-green) | 14.4 % | 45 % |
   | 120, 120, 120 (grey) | 120, 120, 120 (grey) | 12.2 % | 97 % |
   | 69, 69, 77 (grey-navy) | 35, 34, 34 (black) | 10.9 % | 35 % |
   | 14, 14, 14 | 14, 14, 14 | 8.1 % | 89 % |
   | 244, 187, 34 (gold) | 56, 66, 59 (dark grey-green) | 5.3 % | 54 % |
   | 244, 240, 227 (white) | 47, 56, 51 (dark grey-green) | 3.9 % | 33 % |
   | 238, 234, 220 (cream) | 160, 61, 60 (red) | 3.3 % | 23 % |
   | 147, 26, 1 (red) | 79, 82, 77 (grey) | 2.7 % | 39 % |
   | 187, 197, 208 | 188, 198, 209 | 1.9 % | 82 % |
   | 151, 156, 165 (silver) | 46, 46, 46 | 1.8 % | 99 % |
   | 61, 67, 132 (blue) | 57, 59, 62 | 1.8 % | 53 % |
   | 252, 209, 6 (yellow band) | 163, 32, 27 (red band) | 1.0 % | 46 % |
   | 246, 193, 37 (yellow) | 254, 218, 0 (yellow) | 0.9 % | 63 % |
   | 111, 24, 11 (dark red) | 71, 75, 69 | 0.8 % | 40 % |

   (all 24 entries: keyblades/outfit_forms.json `second_form_swap.table`). One colour often stands for several default colours
   (Second Form's black is the default's black top AND its grey-green bottoms; agreement < 50 %), so:
2. THE PLACE decides: each KH2 cloth texel gets its 3D position (bind pose); KH3 Sora's cloth triangles within 7 cm whose Second
   Form colour is close to the texel's KH2 colour (Oklab < 0.11) vote for the default colour at that place; the vote is taken per
   ISLAND (connected texels of one KH2 colour class: a garment piece), so a piece takes one colour. Without votes: the table's
   nearest entry. Shading kept: the texel's luminance against its class colour's.
3. Result on KH2 (largest islands; KH2 colour -> KH3 default colour, by):

   | KH2 colour | KH3 default colour | By | Cells |
   |---|---|---|---|
   | 36, 45, 49 (black shorts) | 56, 64, 56 (dark grey-green) | place | 84998 |
   | 241, 206, 49 (yellow straps) | 160, 32, 24 (red) | table | 49218 |
   | 36, 45, 49 (black jacket) | 32, 32, 32 (black) | place | 36082 |
   | 22, 60, 86 (navy) | 56, 64, 56 (dark grey-green) | place | 30674 |
   | 36, 45, 49 (upper shorts) | 80, 80, 80 (grey) | place | 29790 |
   | 36, 45, 49 (hood) | 168, 64, 64 (red) | place | 28251 |
   | 22, 60, 86 (navy) | 32, 32, 32 (black) | place | 26301 |
   | 145, 40, 22 (red flaps) | 80, 80, 80 (grey) | place | 18387 |
   | 202, 209, 212 (grey hood lining) | 40, 48, 40 (dark grey-green) | place | 16681 |
   | 241, 206, 49 (yellow, waist) | 152, 24, 24 (red band) | place | 10765 |
   | 241, 206, 49 (wrist bands) | 248, 216, 0 (yellow) | place | 9987 |

   Every KH2 colour class found a counterpart (by place or the table); the texture's black padding (0, 0, 0) takes the table's
   black. The look: black jacket with red hood and sleeve trims, grey upper shorts over dark grey-green bottoms, a red waist band,
   grey pouches, yellow wrist bands, dark green shoes: KH3's default scheme (`second_form_swap_large.png`,
   `second_form_closeup.png`: KH3 default next to KH2 Second Form).

## 6b. Rage Form (wear form 7) on KH1 / KH2, and what the renderer must do (F-RAGE, KH3_WEAPONS_PLAN.md 6e)

How KH3 draws it [A, read from the translated shaders, build/sora_pack/materials/materials.wgsl KH_C8 / KH_C10 / KH_C2]: Rage Form
changes NO costume texture (WearformSet m_Forms[7] keeps p_ex001_0d0N). Every material of Sora (cloth p_ex001_mat0/1/2, skin
mat3, face F_mat0, hair, eyes) has the ANTI MODE (static switch UseAntiMode) driven by one run-time scalar, `AntiVolume`
(native code fades it 0 -> 1; the other parameters are the instances' / defaults'):
1. Darkness: `lum = dot(albedo, (0.3, 0.59, 0.11))` (linear), `albedo' = lerp(albedo, lum * AntiDarknessColorMulti, grade)` with
   AntiDarknessColorMulti = (0.04, 0.5, 1.0) (a dark blue monochrome). `grade` is a front that spreads from the head point
   (bone `atama` + AntiVolumePositionOffsetUD 5 / MU -10 cm) over AntiVolumeGradeLength 170 cm, AntiVolumeGradeReverse 1, contrast
   AntiVolumeGradeContrast 2: `d = |p - head| / 170; g = 1 - d; grade = saturate((g - (1 - 2 AntiVolume)) * (AntiVolume * 2 + 5))`,
   then `grade = grade * (1 - grade) + grade`, times `saturate(2 AntiVolume)`: all of the body at AntiVolume 1.
2. Face (F_mat0: UseAntiFaceDark 1): `lum` pulled to 0.008 by a gradient from the head point (AntiFaceDarkOffset 0.3,
   AntiFaceDarkVolume 15 cm) before step 1: a black face.
3. Eyes (Eyelens: AntiEyeMaterial 1): emissive AntiEyeEmissiv_L (10, 0.2, 0) red / _H (5, 1, -0.5) orange; AntiEyeWPO 0.4 pushes
   the lens out.
4. Pattern: the alpha of AntiPointMaskTex = p_ex001_7f00 at the mesh's SECOND UV set, only where that UV lies in tile (1, 0), x
   ceil(AntiVolume), emitted as AntiMaskEmissiv (cloth mat0 / mat1: (10, 15, 30), HDR blue-white), scrolled by
   AntiMaskEmissivSpeed 0.2.
5. The aura: the pawn's m_AntiFormEffect = ead_p_ant_com (vp_ant_bdy_ora00, vp_ant_eye_ora00, vp_ant_rsk_crg_00 / 01,
   vp_ant_rsk_sps01, vp_ant_rsk_vts01): in PACK/vfx (31 vp_ant_* effects there).

Packed for KH1 / KH2 (tools/lib/rage_bake.js, called by tools/sora_outfit_formcolours.js): `direct["7"]` of each outfit's
wear_extra.json replaces the base colour of EVERY body slot (cloth, skin, face, lashes) with `<name>_kbf7.png` = step 1 at
AntiVolume 1 on the outfit's own texel, step 2 on the face texture (as one value 0.85: a rule of ours, the shader grades it per
pixel), and step 4's pattern carried onto the outfit's own UVs (each outfit vertex takes the second-UV coordinate of the nearest
KH3 Sora cloth vertex within 6 cm, bind poses; triangles that map into tile (1, 0) without a seam are rasterised in the outfit's
first UV space and sample 7f00's alpha), baked in the AntiMaskEmissiv hue. The same pattern is written as `<name>_kbf7_glow.png`
(red = the mask) for an emissive pass. KH1: 15674 pattern texels on its jumpsuit texture; KH2: 47454 on its body texture.

F-RAGE, the renderer:
- KH3 Sora: set `AntiVolume` on all of Sora's slots (fade 0 -> 1 on entering, 1 -> 0 on leaving; the translated shaders do
  steps 1..4 themselves; their uniform layout is in materials.json owners[*].sca / vec). Nothing else.
- KH1 / KH2 Sora: apply wear form 7 (the textures above; kh-look already switches them). For the full look also: draw the glow
  mask as emission `AntiMaskEmissiv * mask` (scrolling by AntiMaskEmissivSpeed along V is our reading; KH3 scrolls the emissive
  term), tint the eyes with AntiEyeEmissiv (their iris slots: SQEX_IrisTexEyeMaterial, procedural, so a colour override), darken
  the hair (its colour parameters HairColor / HairPointEndColor x the step-1 formula: the hair shader has no base texture to swap),
  and fade with the same AntiVolume curve (cross-fade the direct textures, or snap at 0.5).
- All three: spawn ead_p_ant_com's effects on the pawn (PACK/vfx).
The mock-up (`rage_side_by_side.png`) bakes all of this into textures and paints the irises red; the real KH3 shader also runs an
HSV step after step 1 whose parameters were not traced (the in-game shade of KH3 Sora may differ slightly from the mock-up).

## 7. Verification

- `kb_pics` (the renderer's `Model::load`): all 68 keyblade models and both Moogles load and draw (`keyblades.png`, `moogle.png`).
- `kh-vfx keyblade_fx`: 1036 effects (826 of keyblades/vfx + 199 the manifest places in vfx / party / xsi roots + children)
  parse, spawn and run 2 s: 0 failed (129 spawn nothing in 2 s: children, event-driven, mesh-only).
- `kh-audio keyblades`: 56 weapon models with sets, 897 set entries resolve to packed sounds, 237 effect SE rows all packed.
  The whole kh-audio suite passes.
- `kh-anim keyblade_forms`: 1208 form clips registered as lazy clips and loaded, 1660 weapon clips loaded, 0 failed.
- Clippy 0 warnings: kh-anim, kh-audio, kh-vfx, kh-er-render (all targets). `node tools/preflight.js`: 0 errors.

## 8. Gaps

1. KEYBLADE / MOOGLE MATERIAL SHADERS NOT BUILT: kh-shaderc fails in its `probe` step on every character folder (also party/team):
   `er_kh_texel` is defined inside `#ifdef ER_PROBE_RING` (crates/kh-er-render/shaders/er_char.wgsl lines 554..611) but used at
   line 771 outside it. Not this task's file; once fixed, `kh-shaderc --pack build/sora_pack/keyblades --party --main-pack
   build/sora_pack` (and `npc/shop`) builds them (the shader phase runs both).
2. The renderer / host do not yet LOAD the new roots: keyblade models + library (`kb:` meshes, like xsiweapon.rs does for `xsi:`),
   the fourth effect root `keyblades/vfx` (its tables use owner ids of their own), the Moogle. Gameplay / render wave.
3. KH3 Sora's own wear forms 1..4, 6..10 (the KH3 outfit's form costumes): the data is there (WearformSet, FormChangeTex), the main
   library carries only 0 and 5. Enabling them = tools/sora_material_pack.js `wearForms([0, 5])` -> all forms (kh-shaderc copies their
   BC7 textures); main-pack change, so left to the wave that switches forms.
4. Not translatable: one slot (w_so037's hand, an unlit effect material) and 201 of 1027 effect pixel shaders (reasons in
   keyblades/vfx/shaders/shading.json); 2 effect textures not decodable (BC6H sky, a cube map).
5. w_so036's row M_B425 is a blend space on the pawn's skeleton: listed, not packed.
6. The form point textures of forms 1..6, 8..10 (KH3's camo / stars / flames patterns, p_ex001_<N>f00) are not mapped onto KH1 / KH2
   (Rage's pattern is: section 6b). Rage's glow, red eyes, dark hair and aura need the renderer (6b, F-RAGE).
7. Light (8) and Double (10) are close on the outfits because KH3's own palettes are (Double's trims differ by rule).
8. Moogle shop sounds come from native code (section 4).
9. VRAM: the outfit form variants are PNG (no cooked DDS exists for a generated texture): KH2's are 2048 x 2048; kh-look loads
   every wear form's textures with the outfit.

(KBFX, 2026-11-03) Gap 1 for the keyblades is closed differently: their materials are drawn in SORA's library, so they are
built in his vertex layout (recipe `kb_dirs`: tools/splice_kb_wgsl.js + tools/kb_shader_jobs.js, kh-shaderc --rebuild, --only
tex), not `--party`. The effect root's table is now its own (`sora_vfx_shaders.js --root`, names reserved against the main root
by `sora_vfx_pack.js --reserve`, engine textures by `sora_vfx_extra_textures.js --pack`); details and release steps in
build/sora_tmp/weapons/KBFX/STATE.md.

## 9. Release notes

- PACK_EPOCH must be bumped (setup/setup.js): new pack tools and inputs.
- setup/pack_inputs.json needs the new KH3 folders: `build/sora_tmp/keyblades/pack_inputs_new_folders.json` (208 folders: the
  weapons' mdl / anm folders, Effects/wep / dlc01 / dlc02 folders, the form anim set clips, the Moogle, FormChangeTex(/dlc01),
  Sound/_DLC). The effect materials' / textures' own folders are listed in keyblades/vfx/manifest.json inputs.
- Linux parity: no platform-specific code was added (Node tools run by packgen.js on both; the material / effect translators
  use the same disassembly path the existing tools use). setup-linux / setup.sh / play.sh need no change beyond the same
  pack_inputs.json and PACK_EPOCH.
