//! The routes wired in the second sound pass, against the real audio pack (skipped, saying so,
//! when it is not there): voice group numbers carried on the seam and `group:N` names, effect
//! names without case, spell / shot actor classes (fire SE, the sounds of their fire and body
//! effects, hit and map-hit SE), loops that end with their effect, prize sounds, the full static
//! SE table, the sockets table.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use glam::Vec3;
use kh_audio::routes::{Context, Resolved};
use kh_audio::{Engine, Language, Listener, Pack, SeamPlay, VoiceCommand};

const DT: f32 = 1.0 / 60.0;

fn pack() -> Option<Arc<Pack>> {
    static PACK: OnceLock<Option<Arc<Pack>>> = OnceLock::new();
    PACK.get_or_init(|| {
        let dir = Path::new(env!("KH_AUDIO_PACK_DIR"));
        if !dir.join("manifest.json").is_file() {
            eprintln!("SKIPPED: no audio pack at {}", dir.display());
            return None;
        }
        Some(Arc::new(Pack::load(dir).expect("the audio pack loads")))
    })
    .clone()
}

fn seam(name: &str) -> SeamPlay<'_> {
    SeamPlay { name, position: Vec3::ZERO, volume: 1.0, pitch: 1.0, looping: false, spatial: true, anim: None }
}

/// One engine step: the files of the voices it started.
fn step(engine: &mut Engine) -> Vec<String> {
    let commands = engine.update(DT, &Listener::default(), &|_| None);
    let pack = engine.pack().clone();
    commands.iter().filter_map(|c| if let VoiceCommand::Start { bank, material, .. } = c { Some(pack.bank(*bank).materials[material].file.clone()) } else { None }).collect()
}

fn causes(engine: &Engine) -> Vec<String> {
    engine.active().iter().map(|a| format!("{} = {}#{}", a.cause, a.bank.rsplit('/').next().unwrap_or(""), a.index)).collect()
}

fn asset_name(resolved: &Resolved) -> String {
    match resolved {
        Resolved::Asset { path, .. } => path.rsplit('/').next().unwrap_or("").to_string(),
        other => format!("{other:?}"),
    }
}

#[test]
fn a_voice_group_plays_the_unit_of_the_number_on_the_seam() {
    // Spec 5.2: the cast voice of Fire is group sora_magic unit 20 (magic-tujyo-1), its
    // finisher unit 2 (Fire_Finish); no unit 0 exists in that group.
    let Some(pack) = pack() else { return };
    let mut engine = Engine::new(pack.clone(), 3, Language::En);
    let cx = |no: i32| Context { surface: 0, language: Language::En, voice_group_no: no, anim: None };
    assert!(matches!(engine.routes.resolve(&pack, "sora_magic", &cx(0)), Resolved::Silent(_)));
    assert_eq!(asset_name(&engine.routes.resolve(&pack, "sora_magic", &cx(20))), "magic-tujyo-1");
    assert_eq!(asset_name(&engine.routes.resolve(&pack, "sora_magic:20", &cx(0))), "magic-tujyo-1");
    assert_eq!(asset_name(&engine.routes.resolve(&pack, "sora_magic:2", &cx(0))), "Fire_Finish");
    assert!(matches!(engine.routes.resolve(&pack, "sora_magic:999", &cx(0)), Resolved::Silent(_)));
    assert_eq!(asset_name(&engine.routes.resolve(&pack, "damage_dead", &cx(1))), asset_name(&engine.routes.resolve(&pack, "damage_dead:1", &cx(0))));
    // Through the engine: silent with number 0, the unit once the seam carried the number.
    assert!(engine.seam_play(1, &seam("sora_magic")).is_none());
    engine.set_voice_group_no(20);
    assert!(engine.seam_play(2, &seam("sora_magic")).is_some());
    assert_eq!(step(&mut engine).len(), 1);
    // The magic state's own "sora_magic:20" of the same notify is the same line: SEAD's battle
    // voice macro cancels the copy (spec 2.1, macro 106).
    assert!(engine.seam_play(3, &seam("sora_magic:20")).is_none());
    assert!(engine.unknown.is_empty(), "{:?}", engine.unknown);
    let log = engine.take_log();
    assert!(log.iter().any(|l| l.kind == "voice-group" && l.note.contains("20")), "{log:?}");
}

#[test]
fn an_aux_table_notify_plays_the_record_of_its_animation() {
    // Spec 4.4: aux `slide` in SlopeSlide, `step` in HopR, `railslide` in the rail-slide loops;
    // the seam name is only the source type, the animation tells the record. Surface 0 = rock.
    let Some(pack) = pack() else { return };
    let routes = kh_audio::Routes::new(&pack);
    let at = |anim: &'static str, surface: u32| routes.resolve(&pack, "AuxSurfaceTable", &Context { surface, language: Language::En, voice_group_no: 0, anim: Some((anim, 0.0)) });
    let (slide, step) = (asset_name(&at("SlopeSlide", 0)), asset_name(&at("HopR", 0)));
    assert!(slide.starts_with("se10010") && step.starts_with("se10010") && slide != step, "{slide} {step}");
    assert!(asset_name(&at("SlopeSlide", 13)).starts_with("se10013"), "the wood bank on wood");
    assert!(matches!(at("IdleN", 0), Resolved::Unknown));
}

#[test]
fn effect_names_match_without_case() {
    // The Keyblade's appear effect arrives as "VW_SO010_APP0" (spec 5.3: -> se02001_010).
    let Some(pack) = pack() else { return };
    let mut engine = Engine::new(pack, 3, Language::En);
    let names = |engine: &Engine, effect: &str| engine.routes.effect_sounds(effect).iter().map(|s| s.asset.rsplit('/').next().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(names(&engine, "VW_SO010_APP0"), ["se02001_010"]);
    assert_eq!(names(&engine, "vw_so010_app0"), ["se02001_010"]);
    engine.effect_spawned(1, "VW_SO010_APP0", Vec3::ZERO);
    assert!(step(&mut engine).iter().any(|f| f.contains("se02001")), "the appear sound starts");
}

#[test]
fn a_spell_actor_sounds_when_spawned_and_when_it_hits() {
    // TresProj_MagFireNml: fire SE se00002_015 (code.projectile), body effect vm_fir_blt0 ->
    // se00002_002 attached to the actor, pawn hit se00002_012, map hit se00002_013 (code.hit_magic,
    // from TresProj_MagFireBase).
    let Some(pack) = pack() else { return };
    let actor = pack.actor("TresProj_MagFireNml_C").expect("the class is in tables.json");
    assert_eq!(actor.command.as_deref(), Some("TRES_CMD_KIND_FIRE"));
    assert!(pack.actor("tresproj_magfirenml").is_some(), "any case, with or without _C");
    let mut engine = Engine::new(pack.clone(), 3, Language::En);
    engine.effect_spawned(40, "TresProj_MagFireNml_C", Vec3::new(200.0, 0.0, 100.0));
    step(&mut engine);
    let live = causes(&engine);
    assert!(live.iter().any(|c| c.starts_with("actor TresProj_MagFireNml_C fire = se00002_015")), "{live:?}");
    assert!(live.iter().any(|c| c.contains("body effect vm_fir_blt0 = se00002_002")), "{live:?}");
    // The body sound is attached to the actor (it follows the effect handle) and ends with it.
    let body = engine.active().into_iter().find(|a| a.cause.contains("body effect")).expect("body sound");
    engine.effect_stopped(40);
    for _ in 0..90 {
        step(&mut engine);
    }
    assert!(!engine.driver.is_playing(body.key), "the actor's body sound ended with the actor");
    // Hits.
    assert!(engine.actor_hit(None, "TresProj_MagFireNml_C", false, Vec3::ZERO).is_some());
    step(&mut engine);
    assert!(causes(&engine).iter().any(|c| c == "actor TresProj_MagFireNml_C hit = se00002_012#0"), "{:?}", causes(&engine));
    assert!(engine.actor_hit(Some(77), "TresProj_MagFireNml_C", true, Vec3::ZERO).is_some());
    step(&mut engine);
    assert!(causes(&engine).iter().any(|c| c == "actor TresProj_MagFireNml_C map hit = se00002_013#0"), "{:?}", causes(&engine));
    assert!(engine.seam_live(77), "a hit sound that came under a seam handle is tracked by it");
    // A class without a hit SE of its own (the burn stage) is silent, an unknown class is said once.
    assert!(engine.actor_hit(None, "TresProj_FiraNmlBurn_C", false, Vec3::ZERO).is_none());
    assert!(engine.actor_hit(None, "NoSuchActor_C", false, Vec3::ZERO).is_none());
    assert_eq!(engine.unknown, ["NoSuchActor_C"]);
    // Thunder has a pawn hit SE and no map hit SE; Aero's own hit effect carries the same SE as
    // its class (se00002_410): the copy within 0.067 s is cancelled by start macro 1.
    let thunder = pack.actor("TresProj_MagThunderNml_C").unwrap();
    assert!(thunder.pawn_hit_se.as_deref().is_some_and(|a| a.ends_with("se00002_206")) && thunder.map_hit_se.is_none());
    assert!(pack.actor("TresProj_MagAeroNml_C").unwrap().pawn_hit_se.as_deref().is_some_and(|a| a.ends_with("se00002_410")));
}

#[test]
fn a_looping_effect_sound_ends_with_its_effect() {
    // Every looping effect-attached sound of the pack stops when its effect is stopped, also
    // those flagged bNoSeFadeAtTheEffectEnd (a loop cannot play out).
    let Some(pack) = pack() else { return };
    let mut engine = Engine::new(pack.clone(), 3, Language::En);
    let mut loops: Vec<String> = engine.routes.effects.iter().filter(|(_, list)| list.iter().any(|se| !se.at_end && pack.asset(&se.asset).is_some_and(|a| a.looping))).map(|(name, _)| name.clone()).collect();
    loops.sort();
    assert!(!loops.is_empty(), "the pack has looping effect sounds");
    let mut checked = 0;
    for (k, name) in loops.iter().enumerate() {
        let handle = 100 + k as u32;
        engine.effect_spawned(handle, name, Vec3::ZERO);
        step(&mut engine);
        let keys: Vec<_> = engine.active().into_iter().filter(|a| a.cause == format!("effect {name}")).map(|a| a.key).collect();
        engine.effect_stopped(handle);
        for _ in 0..240 {
            step(&mut engine);
        }
        for key in &keys {
            let looping = engine.active().iter().any(|a| a.key == *key && pack.assets.values().any(|asset| asset.looping && pack.bank(asset.bank).name == a.bank && asset.index == a.index));
            assert!(!looping, "the loop of effect {name} is still playing 4 s after the effect ended");
        }
        checked += keys.len();
    }
    assert!(checked > 0);
}

#[test]
fn prizes_static_ses_and_sockets_are_in_the_tables() {
    let Some(pack) = pack() else { return };
    let engine = Engine::new(pack.clone(), 3, Language::En);
    let cx = Context { surface: 0, language: Language::En, voice_group_no: 0, anim: None };
    // Spec 5.6 `code.prize`: get / bounce = HP 022 / 021, MP 028 / 027, munny 024 / 023, focus 041 / 040.
    for (name, asset) in [("PrizeHpS.get", "se00000_022"), ("PrizeHpL.bound", "se00000_021"), ("PrizeMpL.get", "se00000_028"), ("PrizeMpS.bound", "se00000_027"), ("PrizeMunnyM.get", "se00000_024"), ("PrizeMunnyS.bound", "se00000_023"), ("PrizeFocusL.get", "se00000_041"), ("PrizeFocusS.bound", "se00000_040")] {
        assert_eq!(asset_name(&engine.routes.resolve(&pack, name, &cx)), asset, "{name}");
    }
    // The whole ETresStaticSE table is packed now (43 ids), each with its bank sound.
    assert_eq!(pack.static_se.len(), 43);
    for (id, path) in pack.static_se.iter().enumerate() {
        let asset = pack.asset(path.as_deref().unwrap_or_else(|| panic!("static SE {id} is not in the pack"))).unwrap();
        assert!(pack.sound(asset.bank, asset.index).is_some(), "static SE {id}: its bank sound is packed");
    }
    // Sockets: the weapon socket hangs on the hand bone R_buki, turned by roll -90.
    let weapon = pack.sockets.get("R_wep_skt").expect("R_wep_skt");
    assert_eq!((weapon.bone.as_str(), weapon.location, weapon.rotation), ("R_buki", [0.0, 0.0, 0.0], [0.0, 0.0, -90.0]));
    assert!(pack.sockets.contains_key("vfx_root") && pack.sockets.len() >= 40);
}

/// (IMPL_ER19) The user's report "no SFX when you enter the save point": the heal burst of the save point
/// (vg_com_svp_in00) carries se00000_016; a host that draws the effect itself asks for it as "effect:<name>". It is in
/// the pack with its bank sound, is 3D, starts a voice, and the save point's other four effects carry nothing.
#[test]
fn the_save_points_heal_effect_carries_a_sound_that_plays() {
    let Some(pack) = pack() else { return };
    let mut engine = Engine::new(pack.clone(), 3, Language::En);
    let cx = Context { surface: 0, language: Language::En, voice_group_no: 0, anim: None };
    assert_eq!(asset_name(&engine.routes.resolve(&pack, "effect:vg_com_svp_in00", &cx)), "se00000_016");
    assert_eq!(asset_name(&engine.routes.resolve(&pack, "effect:VG_COM_SVP_IN00", &cx)), "se00000_016");
    for silent in ["effect:vg_com_svp_nml00", "effect:vg_com_svp_nml01", "effect:vg_com_svp_lmt00", "effect:vg_com_svp_lmt01", "effect:vs_plz_hpb0"] {
        assert!(matches!(engine.routes.resolve(&pack, silent, &cx), Resolved::Silent(_)), "{silent}");
    }
    let asset = pack.asset("/Game/Sound/SE/common/00000/se00000_016").expect("the asset is packed");
    assert!(asset.attenuation && !asset.looping && pack.sound(asset.bank, asset.index).is_some());
    let key = engine.seam_play(900, &SeamPlay { position: Vec3::new(150.0, 0.0, 0.0), ..seam("effect:vg_com_svp_in00") });
    assert!(key.is_some(), "the seam sound starts");
    let files = step(&mut engine);
    assert_eq!(files.len(), 1, "one voice: {files:?}");
    assert!(std::path::Path::new(env!("KH_AUDIO_PACK_DIR")).join(&files[0]).is_file(), "its material file is in the pack: {}", files[0]);
}

/// (IMPL_ER19) THE ORBS' SOUNDS in the user's session ("seam sounds .. nothing played for N" rose with every kill): every
/// prize route starts its sound; what is NOT played is the second of two equal sounds asked for within the bank's own
/// start macro window (two orbs of a kind bouncing or arriving together): the game's data, not a missing route.
#[test]
fn every_prize_sound_starts_and_only_an_equal_sound_just_before_cancels_it() {
    let Some(pack) = pack() else { return };
    let mut engine = Engine::new(pack.clone(), 3, Language::En);
    let mut handle = 100;
    for class in ["PrizeHpS", "PrizeHpL", "PrizeMpS", "PrizeMpL", "PrizeMunnyS", "PrizeMunnyM", "PrizeMunnyL", "PrizeFocusS", "PrizeFocusL"] {
        for what in ["bound", "get"] {
            let name = format!("{class}.{what}");
            handle += 1;
            let first = engine.seam_play(handle, &SeamPlay { position: Vec3::new(300.0, 0.0, 0.0), ..seam(&name) });
            assert!(first.is_some(), "{name} starts");
            handle += 1;
            let second = engine.seam_play(handle, &SeamPlay { position: Vec3::new(300.0, 0.0, 0.0), ..seam(&name) });
            assert!(second.is_none(), "{name}: the same sound in the same step is cancelled by the bank's start macro");
            assert_eq!(step(&mut engine).len(), 1, "{name}: one voice");
            // (the sizes of a kind share their sound: let the macro's window pass)
            for _ in 0..30 {
                step(&mut engine);
            }
        }
    }
    assert!(engine.unknown.is_empty(), "{:?}", engine.unknown);
}
