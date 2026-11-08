# Elden Ring boss defeat flags

Every boss defeat in Elden Ring and Shadow of the Erdtree, read from the game's own event scripts (exe 2.7.2.0, 2026-10-30): each event that calls the boss-defeat instruction and then sets its flag, with the boss's name from its boss bar.

- **Flag:** the event flag the game sets when the boss is defeated.
- **Map:** the event script it lives in.
- **Row:** the `boss.*` row of `sheets/port/sora-er-boss-rewards.json` that uses it, if any. A **reserved** row has no reward yet: the mod ignores its flag, so the boss still gives a small-boss item.

The flags come straight from the scripts. None of them has been seen set by a real kill in the game yet.

**Alabaster Lord has no entry:** the game has no boss bar or boss-defeat event for one. Donald's Gravity step from it moved to the second Onyx Lord (`onyx_lord_liurnia`).

| Flag | Map | Boss | Row |
|---|---|---|---|
| 10000800 | m10_00_00_00 | Godrick the Grafted | godrick |
| 10000850 | m10_00_00_00 | Margit, the Fell Omen | margit |
| 10010800 | m10_01_00_00 | Grafted Scion |  |
| 11000800 | m11_00_00_00 | Morgott, the Omen King | morgott |
| 11000850 | m11_00_00_00 | Godfrey, First Elden Lord | godfrey_shade |
| 11050800 | m11_05_00_00 | Godfrey, First Elden Lord / Hoarah Loux, Warrior | hoarah_loux |
| 11050850 | m11_05_00_00 | Sir Gideon Ofnir, the All-Knowing | gideon |
| 12010800 | m12_01_00_00 | Dragonkin Soldier of Nokstella | dragonkin_nokstella |
| 12010850 | m12_01_00_00 | Dragonkin Soldier |  |
| 12020800 | m12_02_00_00 | Valiant Gargoyle | valiant_gargoyles |
| 12020830 | m12_02_00_00 | Dragonkin Soldier |  |
| 12020850 | m12_02_00_00 | Mimic Tear | mimic_tear |
| 12030390 | m12_03_00_00 | Crucible Knight Siluria | crucible_knight_siluria (reserved) |
| 12030800 | m12_03_00_00 | Fia's Champion |  |
| 12030850 | m12_03_00_00 | Lichdragon Fortissax | fortissax |
| 12040800 | m12_04_00_00 | Astel, Naturalborn of the Void | astel |
| 12050800 | m12_05_00_00 | Mohg, Lord of Blood | mohg_lord |
| 12080800 | m12_08_00_00 | Ancestor Spirit | ancestor_spirit |
| 12090800 | m12_09_00_00 | Regal Ancestor Spirit | regal_ancestor |
| 13000800 | m13_00_00_00 | Maliketh, the Black Blade | maliketh |
| 13000830 | m13_00_00_00 | Dragonlord Placidusax | placidusax |
| 13000850 | m13_00_00_00 | Godskin Duo | godskin_duo |
| 14000800 | m14_00_00_00 | Rennala, Queen of the Full Moon | rennala |
| 14000850 | m14_00_00_00 | Red Wolf of Radagon | red_wolf |
| 15000800 | m15_00_00_00 | Malenia, Blade of Miquella / Malenia, Goddess of Rot | malenia |
| 15000850 | m15_00_00_00 | Loretta, Knight of the Haligtree | loretta_haligtree |
| 16000800 | m16_00_00_00 | Rykard, Lord of Blasphemy | rykard |
| 16000850 | m16_00_00_00 | Godskin Noble | godskin_noble |
| 16000860 | m16_00_00_00 | Abductor Virgin (Swinging Sickle) |  |
| 18000800 | m18_00_00_00 | Ulcerated Tree Spirit |  |
| 18000850 | m18_00_00_00 | Soldier of Godrick |  |
| 19000800 | m19_00_00_00 | Elden Beast | elden_beast |
| 20000800 | m20_00_00_00 | Divine Beast Dancing Lion | dancing_lion |
| 20010800 | m20_01_00_00 | Radahn, Consort of Miquella | consort_radahn |
| 20010850 | m20_01_00_00 | Needle Knight Leda |  |
| 21000850 | m21_00_00_00 | Golden Hippopotamus | golden_hippo |
| 21010800 | m21_01_00_00 | Base Serpent Messmer | messmer |
| 22000800 | m22_00_00_00 | Putrescent Knight | putrescent |
| 25000800 | m25_00_00_00 | Metyr, Mother of Fingers | metyr |
| 28000800 | m28_00_00_00 | Midra, Lord of Frenzied Flame | midra |
| 30000800 | m30_00_00_00 | Cemetery Shade |  |
| 30010800 | m30_01_00_00 | Erdtree Burial Watchdog |  |
| 30020800 | m30_02_00_00 | Erdtree Burial Watchdog |  |
| 30030800 | m30_03_00_00 | Spiritcaller Snail |  |
| 30040800 | m30_04_00_00 | Grave Warden Duelist |  |
| 30050800 | m30_05_00_00 | Cemetery Shade |  |
| 30050850 | m30_05_00_00 | Black Knife Assassin |  |
| 30060800 | m30_06_00_00 | Erdtree Burial Watchdog |  |
| 30070800 | m30_07_00_00 | Erdtree Burial Watchdog |  |
| 30080800 | m30_08_00_00 | Ancient Hero of Zamor |  |
| 30090800 | m30_09_00_00 | Red Wolf of the Champion |  |
| 30100800 | m30_10_00_00 | Crucible Knight Ordovis | crucible_knight_ordovis (reserved) |
| 30110800 | m30_11_00_00 | Black Knife Assassin |  |
| 30120800 | m30_12_00_00 | Misbegotten Warrior |  |
| 30130800 | m30_13_00_00 | Grave Warden Duelist |  |
| 30130810 | m30_13_00_00 | (no name on its bar) |  |
| 30140800 | m30_14_00_00 | Erdtree Burial Watchdog (Sword) |  |
| 30150800 | m30_15_00_00 | Cemetery Shade |  |
| 30160800 | m30_16_00_00 | Putrid Tree Spirit |  |
| 30170800 | m30_17_00_00 | Ancient Hero of Zamor |  |
| 30180800 | m30_18_00_00 | Ulcerated Tree Spirit |  |
| 30190800 | m30_19_00_00 | Putrid Grave Warden Duelist |  |
| 30200800 | m30_20_00_00 | Stray Mimic Tear |  |
| 31000800 | m31_00_00_00 | Patches |  |
| 31000850 | m31_00_00_00 | Patches |  |
| 31010800 | m31_01_00_00 | Runebear |  |
| 31020800 | m31_02_00_00 | Miranda Blossom |  |
| 31030800 | m31_03_00_00 | Beastman of Farum Azula |  |
| 31040800 | m31_04_00_00 | Cleanrot Knight |  |
| 31050800 | m31_05_00_00 | Bloodhound Knight |  |
| 31060800 | m31_06_00_00 | Crystalian (Staff) |  |
| 31070800 | m31_07_00_00 | Kindred of Rot |  |
| 31090800 | m31_09_00_00 | Demi-Human Queen Margot |  |
| 31100800 | m31_10_00_00 | Beastman of Farum Azula (Cleaver) |  |
| 31110800 | m31_11_00_00 | Putrid Crystalian (Spear) |  |
| 31120800 | m31_12_00_00 | Misbegotten Crusader |  |
| 31150800 | m31_15_00_00 | Demi-Human Chief |  |
| 31170800 | m31_17_00_00 | Guardian Golem |  |
| 31180800 | m31_18_00_00 | Miranda the Blighted Bloom |  |
| 31190800 | m31_19_00_00 | Black Knife Assassin |  |
| 31190850 | m31_19_00_00 | Necromancer Garris |  |
| 31200800 | m31_20_00_00 | Cleanrot Knight (Spear) |  |
| 31210800 | m31_21_00_00 | Frenzied Duelist |  |
| 31220800 | m31_22_00_00 | Spiritcaller Snail |  |
| 32000800 | m32_00_00_00 | Scaly Misbegotten |  |
| 32010800 | m32_01_00_00 | Stonedigger Troll |  |
| 32020800 | m32_02_00_00 | Crystalian |  |
| 32040800 | m32_04_00_00 | Stonedigger Troll |  |
| 32050800 | m32_05_00_00 | Crystalian (Ringblade) |  |
| 32070800 | m32_07_00_00 | Magma Wyrm |  |
| 32080800 | m32_08_00_00 | Fallingstar Beast |  |
| 32110800 | m32_11_00_00 | Astel, Stars of Darkness | astel_stars |
| 9280 | m34_10_00_00 | (no name on its bar) |  |
| 34110800 | m34_11_00_00 | (no name on its bar) |  |
| 34120800 | m34_12_00_00 | Onyx Lord | onyx_lord |
| 34130800 | m34_13_00_00 | Godskin Apostle |  |
| 34140850 | m34_14_00_00 | Fell Twin |  |
| 34150800 | m34_15_00_00 | (no name on its bar) |  |
| 35000800 | m35_00_00_00 | Mohg, the Omen | mohg_omen |
| 35000850 | m35_00_00_00 | Esgar, Priest of Blood |  |
| 39200800 | m39_20_00_00 | Magma Wyrm Makar | makar |
| 40000800 | m40_00_00_00 | Death Knight |  |
| 40010800 | m40_01_00_00 | Death Knight |  |
| 2052430800 | m40_02_00_00 | (no name on its bar) |  |
| 41000800 | m41_00_00_00 | Demi-Human Swordmaster Onze |  |
| 41010800 | m41_01_00_00 | Curseblade Labirith |  |
| 41020800 | m41_02_00_00 | Lamenter |  |
| 43000800 | m43_00_00_00 | Chief Bloodfiend |  |
| 43010800 | m43_01_00_00 | Ancient Dragon-Man |  |
| 1033420800 | m60_33_42_00 | Alecto, Black Knife Ringleader |  |
| 1033430800 | m60_33_43_00 | Erdtree Avatar | erdtree_avatar_liurnia_south (reserved) |
| 1033450800 | m60_33_45_00 | Bols, Carian Knight |  |
| 1034420800 | m60_34_42_00 | Glintstone Dragon Adula |  |
| 1034450800 | m60_34_45_00 | Glintstone Dragon Smarag | smarag |
| 1034480800 | m60_34_48_00 | Royal Revenant |  |
| 1034500800 | m60_34_50_00 | Glintstone Dragon Adula | adula |
| 1035420800 | m60_35_42_00 | Omenkiller |  |
| 1035500800 | m60_35_50_00 | Royal Knight Loretta | loretta_caria |
| 1035530800 | m60_35_53_00 | Magma Wyrm |  |
| 1036450800 | m60_36_45_00 | Death Rite Bird |  |
| 1036480800 | m60_36_48_00 | Night's Cavalry |  |
| 1036500800 | m60_36_50_00 | Onyx Lord | onyx_lord_liurnia |
| 1036540800 | m60_36_54_00 | Full-Grown Fallingstar Beast | fullgrown_fallingstar |
| 1037420800 | m60_37_42_00 | Deathbird |  |
| 1037460800 | m60_37_46_00 | Bell Bearing Hunter |  |
| 1037510800 | m60_37_51_00 | Ancient Dragon Lansseax | lansseax_west (reserved) |
| 1037530800 | m60_37_53_00 | Demi-Human Queen Maggie |  |
| 1037540810 | m60_37_54_00 | Ulcerated Tree Spirit |  |
| 1038410800 | m60_38_41_00 | Adan, Thief of Fire |  |
| 1038480800 | m60_38_48_00 | Erdtree Avatar | erdtree_avatar_liurnia_east (reserved) |
| 1038510800 | m60_38_51_00 | Demi-Human Queen Gilika |  |
| 1038520800 | m60_38_52_00 | Tibia Mariner |  |
| 1038540800 | m60_38_54_00 | Fallingstar Beast |  |
| 1039430800 | m60_39_43_00 | Night's Cavalry |  |
| 1039440800 | m60_39_44_00 | Tibia Mariner |  |
| 1039500800 | m60_39_50_00 | Godefroy the Grafted |  |
| 1039510800 | m60_39_51_00 | Night's Cavalry |  |
| 1039540800 | m60_39_54_00 | Elemer of the Briar | elemer |
| 1040520800 | m60_40_52_00 | Black Knife Assassin |  |
| 1040530800 | m60_40_53_00 | Sanguine Noble |  |
| 1041330800 | m60_41_33_00 | ?904133540 |  |
| 1041500800 | m60_41_50_00 | Fallingstar Beast | fallingstar_beast |
| 1041510800 | m60_41_51_00 | Tree Sentinel |  |
| 1041520800 | m60_41_52_00 | Ancient Dragon Lansseax | lansseax_east (reserved) |
| 1041530800 | m60_41_53_00 | Wormface |  |
| 1042330800 | m60_42_33_00 | Ancient Hero of Zamor |  |
| 1042360800 | m60_42_36_00 | Tree Sentinel | tree_sentinel |
| 1042370800 | m60_42_37_00 | Crucible Knight | crucible_knight_stormhill (reserved) |
| 1042380800 | m60_42_38_00 | Deathbird |  |
| 1042380850 | m60_42_38_00 | Bell Bearing Hunter |  |
| 1042550800 | m60_42_55_00 | Godskin Apostle |  |
| 1043300800 | m60_43_30_00 | Leonine Misbegotten | leonine |
| 1043330800 | m60_43_33_00 | Erdtree Avatar | erdtree_avatar_weeping (reserved) |
| 1043360800 | m60_43_36_00 | Flying Dragon Agheel | agheel |
| 1043370800 | m60_43_37_00 | Night's Cavalry |  |
| 1043530800 | m60_43_53_00 | Bell Bearing Hunter |  |
| 1044320800 | m60_44_32_00 | Deathbird |  |
| 1044320850 | m60_44_32_00 | Night's Cavalry |  |
| 1044350800 | m60_44_35_00 | Bloodhound Knight Darriwil |  |
| 1044360800 | m60_44_36_00 | Mad Pumpkin Head |  |
| 1044360800 | m60_44_36_10 | Mad Pumpkin Head |  |
| 1044530800 | m60_44_53_00 | Deathbird |  |
| 1044530800 | m60_44_53_10 | Deathbird |  |
| 1045390800 | m60_45_39_00 | Tibia Mariner |  |
| 1045390800 | m60_45_39_10 | Tibia Mariner |  |
| 1045520800 | m60_45_52_00 | Draconic Tree Sentinel | draconic_tree_sentinel |
| 1045520800 | m60_45_52_10 | Draconic Tree Sentinel | draconic_tree_sentinel |
| 1047400800 | m60_47_40_00 | Putrid Avatar |  |
| 1048370800 | m60_48_37_00 | Decaying Ekzykes |  |
| 1048400800 | m60_48_40_00 | Mad Pumpkin Head (Hammer) |  |
| 1048410800 | m60_48_41_00 | Bell Bearing Hunter |  |
| 1048510800 | m60_48_51_00 | Night's Cavalry |  |
| 1248550800 | m60_48_55_00 | Night's Cavalry (Glaive) |  |
| 1048570800 | m60_48_57_00 | Death Rite Bird |  |
| 1049370800 | m60_49_37_00 | Night's Cavalry |  |
| 1049370850 | m60_49_37_00 | Death Rite Bird |  |
| 1049380800 | m60_49_38_00 | Commander O'Neil |  |
| 1049390800 | m60_49_39_00 | Nox Monk |  |
| 1049390850 | m60_49_39_00 | Battlemage Hugues |  |
| 1049520800 | m60_49_52_00 | Black Blade Kindred |  |
| 1050560800 | m60_50_56_00 | Great Wyrm Theodorix |  |
| 1050570800 | m60_50_57_00 | Death Rite Bird |  |
| 1050570850 | m60_50_57_00 | Putrid Avatar |  |
| 1051360800 | m60_51_36_00 | Crucible Knight | crucible_knight_caelid (reserved) |
| 1051400800 | m60_51_40_00 | Putrid Avatar |  |
| 1051430800 | m60_51_43_00 | Black Blade Kindred |  |
| 1051570800 | m60_51_57_00 | Commander Niall | niall |
| 1252380800 | m60_52_38_00 | Starscourge Radahn | radahn |
| 1052410800 | m60_52_41_00 | Flying Dragon Greyll |  |
| 1052410850 | m60_52_41_00 | Night's Cavalry |  |
| 1252520800 | m60_52_52_00 | Fire Giant | fire_giant |
| 1052560800 | m60_52_56_00 | (no name on its bar) |  |
| 1053560800 | m60_53_56_00 | Roundtable Knight Vyke |  |
| 1254560800 | m60_54_56_00 | Borealis the Freezing Fog | borealis (reserved) |
| 2044450800 | m61_44_45_00 | Romina, Saint of the Bud | romina |
| 2044470800 | m61_44_47_00 | Rugalea the Great Red Bear |  |
| 2045440800 | m61_45_44_00 | Ghostflame Dragon |  |
| 2046380800 | m61_46_38_00 | Dancer of Ranah |  |
| 2046400800 | m61_46_40_00 | Demi-Human Queen Marigga |  |
| 2046410800 | m61_46_41_00 | Knight of the Solitary Gaol |  |
| 2046450800 | m61_46_45_00 | Red Bear |  |
| 2046460800 | m61_46_46_00 | Divine Beast Dancing Lion |  |
| 2047390800 | m61_47_39_00 | Death Rite Bird |  |
| 2047450800 | m61_47_45_00 | Black Knight Garrew |  |
| 2048380850 | m61_48_38_00 | Ghostflame Dragon |  |
| 2048440800 | m61_48_44_00 | Rellana, Twin Moon Knight | rellana |
| 2049410800 | m61_49_41_00 | Jagged Peak Drake |  |
| 2049430800 | m61_49_43_00 | Ghostflame Dragon |  |
| 2049430850 | m61_49_43_00 | Black Knight Edredd |  |
| 2049440800 | m61_49_44_00 | Dryleaf Dane |  |
| 2049450800 | m61_49_45_00 | Ralva the Great Red Bear |  |
| 2049480800 | m61_49_48_00 | Commander Gaius | gaius |
| 2050430800 | m61_50_43_00 | Dryleaf Dane |  |
| 2050480800 | m61_50_48_00 | (no name on its bar) | scadutree_avatar |
| 2051440800 | m61_51_44_00 | Rakshasa |  |
| 2052400800 | m61_52_40_00 | Jagged Peak Drake |  |
| 2052430800 | m61_52_43_00 | Jori, Elder Inquisitor |  |
| 2052480800 | m61_52_48_00 | Fallingstar Beast |  |
| 2054390800 | m61_54_39_00 | Bayle the Dread | bayle |
| 2054390850 | m61_54_39_00 | Ancient Dragon Senessax |  |
