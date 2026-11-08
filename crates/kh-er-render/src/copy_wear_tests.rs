//! (MIMIC-BOSS hotfix 3, MB3-FORMLEAK) A copy of Sora changes form only when HE does: engine-free, on the real KH3 library of
//! `build/sora_pack` (skipped without it) stepped by the renderer's own rules (kh-look `step_wear`, rage.rs).

use super::*;
use kh_look::library::Library;

const STEP: f32 = 1.0 / 60.0;

/// The KH3 library as the renderer loads it (rage.rs `prepare` included), or None without the pack.
fn kh3() -> Option<Library> {
    let pack = kh_host::audit_a::project().join("build").join("sora_pack");
    let dir = crate::outfit::look_dir(&pack, "kh3");
    let mut lib = Library::load(&dir).ok()?;
    crate::rage::prepare(&dir, &mut lib);
    Some(lib)
}

/// A wear form with a costume of its own (not Rage's), and the library's slots it changes.
fn a_form(lib: &Library) -> u8 {
    lib.wear.forms.keys().copied().filter(|f| *f != 0 && *f != crate::rage::RAGE_WEAR_FORM).min().expect("a form costume in KH3's WearformSet")
}

/// The wear slots' textures of a library stepped at `form` long enough to wear it whole.
fn worn_at(form: u8) -> Vec<Vec<Option<TextureRef>>> {
    let mut l = kh3().expect("the pack");
    for _ in 0..90 {
        l.step_wear(form, STEP);
    }
    wear_slots(&l).iter().map(|k| l.slots[*k].textures.clone()).collect()
}

fn textures_of(w: &CopyWear, slots: &[usize]) -> Vec<Vec<Option<TextureRef>>> {
    slots.iter().map(|k| w.textures(*k).expect("his slot").to_vec()).collect()
}

/// THE PLAYER CHANGES FORM WHILE A COPY IS IN HIS BASE FORM: the copy's slots, parameters and models stay base, through the
/// player's slide, his costume, and his Rage Form; the player's own library wears the player's form exactly as before.
#[test]
fn the_players_form_never_reaches_a_copy_in_his_base_form() {
    let Some(mut lib) = kh3() else {
        eprintln!("SKIPPED: no kh3 library under build/sora_pack");
        return;
    };
    let form = a_form(&lib);
    let slots = wear_slots(&lib);
    assert!(!slots.is_empty(), "KH3's costume changes some of Sora's slots");
    let base = worn_at(0);
    let in_form = worn_at(form);
    assert_ne!(base, in_form, "form {form}'s costume is other textures");
    let mut wears = CopyWears::new(&lib);
    lib.step_wear(0, STEP); // (the player's first frame in his base form: his form then slides in)
    let mut slid = false;
    for frame in 0..120u64 {
        let clock = frame as f32 * STEP;
        lib.step_wear(form, STEP); // (the player's, gfx.rs)
        let w = wears.step(&lib, 9, 0, clock, frame);
        // his parameters: no slide, whatever the player's is
        for k in &slots {
            if let Some(player) = lib.slots[*k].scalars.get("FormChangeSlide").copied() {
                slid |= player > 0.0;
                let mut his = lib.slots[*k].clone();
                w.apply(*k, &mut his);
                assert_eq!(his.scalars.get("FormChangeSlide").copied(), Some(0.0), "frame {frame}: the player's slide {player} is not his");
            }
        }
        assert_eq!(textures_of(w, &slots), base, "frame {frame}: his costume is his base one");
        assert_eq!(w.worn(), (0, 0));
    }
    assert!(slid, "the player's costume slid in");
    let w = wears.get(9).expect("his look");
    // the player wears the form; the copy needs tables of his own for exactly the slots that differ
    let player: Vec<Vec<Option<TextureRef>>> = slots.iter().map(|k| lib.slots[*k].textures.clone()).collect();
    assert_eq!(player, in_form, "the player's own library wears the player's form as before");
    assert!(slots.iter().any(|k| w.differs(&lib, *k)), "his own tables are wanted");
    assert!((0..lib.slots.len()).filter(|k| w.differs(&lib, *k)).all(|k| slots.contains(&k)), "only wear slots differ");
    // THE PLAYER'S RAGE FORM (rage.rs `apply` on the library, its twins): none of it on the copy
    crate::rage::apply(&mut lib, 1.0);
    for (k, slot) in lib.slots.iter().enumerate() {
        assert!(w.needs(&lib, k, slot, 1.0), "slot {k}: the player's Rage parameters are taken off his");
        let mut his = slot.clone();
        w.apply(k, &mut his);
        if slot.scalars.contains_key("AntiVolume") {
            assert_eq!(his.scalars.get("AntiVolume").copied(), Some(0.0), "slot {} ({}): no AntiVolume of the player's", k, slot.slot);
        }
        assert_eq!(w.draw_slot(k), k, "his own slots, never the player's Rage twins");
    }
    let mut clean = kh3().expect("the pack");
    crate::rage::apply(&mut clean, 0.0);
    for (k, slot) in lib.slots.iter().enumerate() {
        let mut his = slot.clone();
        w.apply(k, &mut his);
        assert_eq!(his.vectors.get("AOChannel"), clean.slots[k].vectors.get("AOChannel"), "slot {k}: his occlusion is his base one");
    }
}

/// THE COPY CHANGES FORM WHILE THE PLAYER STAYS IN HIS BASE FORM: the copy wears his form (his costume slid in, then whole;
/// Rage Form's look by HIS fade), the player's library does not.
#[test]
fn a_copy_wears_his_own_form_and_the_player_does_not() {
    let Some(mut lib) = kh3() else {
        eprintln!("SKIPPED: no kh3 library under build/sora_pack");
        return;
    };
    let form = a_form(&lib);
    let slots = wear_slots(&lib);
    let (base, in_form) = (worn_at(0), worn_at(form));
    let mut wears = CopyWears::new(&lib);
    let mut his_slide = 0.0f32;
    for frame in 0..90u64 {
        let clock = frame as f32 * STEP;
        lib.step_wear(0, STEP);
        // (a base-form start, then his form change at frame 10)
        let w = wears.step(&lib, 10, if frame < 10 { 0 } else { form }, clock, frame);
        for k in &slots {
            if let Some(s) = w.slide(*k) {
                his_slide = his_slide.max(s);
            }
        }
    }
    let w = wears.get(10).expect("his look");
    assert!(his_slide > 0.0, "his costume slid in (FormChangeSlide on his slot copies)");
    assert_eq!(w.worn(), (form, form), "he wears his form");
    assert_eq!(textures_of(w, &slots), in_form, "his costume is form {form}'s");
    let player: Vec<Vec<Option<TextureRef>>> = slots.iter().map(|k| lib.slots[*k].textures.clone()).collect();
    assert_eq!(player, base, "the player's library stays in his base form");
    assert!(lib.slots.iter().all(|s| s.scalars.get("FormChangeSlide").is_none_or(|v| *v == 0.0)), "no slide on the player's slots");
    // he needs his own tables for his costume; a copy in the player's form needs none
    assert!(slots.iter().any(|k| w.differs(&lib, *k)));
    let mut same = CopyWears::new(&lib);
    assert!((0..lib.slots.len()).all(|k| !same.step(&lib, 11, 0, 0.0, 1).differs(&lib, k)), "a base copy beside a base player: the player's tables");
    // HIS RAGE FORM: his fade, his AntiVolume; the player's slots untouched
    for frame in 90..160u64 {
        wears.step(&lib, 10, crate::rage::RAGE_WEAR_FORM, frame as f32 * STEP, frame);
    }
    let w = wears.get(10).expect("his look");
    assert_eq!(w.volume(), 1.0, "his Rage look is on (1 s in)");
    if lib.wear.forms.contains_key(&crate::rage::RAGE_WEAR_FORM) {
        assert_eq!(w.worn().1, crate::rage::RAGE_WEAR_FORM, "his Rage costume (past the fade's SNAP)");
    }
    let dark = (0..lib.slots.len()).filter(|k| {
        let mut his = lib.slots[*k].clone();
        w.apply(*k, &mut his);
        his.scalars.get("AntiVolume") == Some(&1.0)
    });
    assert!(dark.count() > 0, "his slots take his AntiVolume 1");
    assert!(lib.slots.iter().all(|s| s.scalars.get("AntiVolume").is_none_or(|v| *v == 0.0)), "the player's slots carry no Rage");
    // and back to base: his costume goes back, at once out of Rage (EndStyle's 0 s)
    for frame in 160..200u64 {
        wears.step(&lib, 10, 0, frame as f32 * STEP, frame);
    }
    let w = wears.get(10).expect("his look");
    assert_eq!((w.volume(), w.worn()), (0.0, (0, 0)));
    assert_eq!(textures_of(w, &slots), base);
}

/// The renderer steps a copy once a frame however many passes ask; a copy not drawn for a while is forgotten.
#[test]
fn a_copy_is_stepped_once_a_frame_and_forgotten_when_gone() {
    let Some(lib) = kh3() else {
        eprintln!("SKIPPED: no kh3 library under build/sora_pack");
        return;
    };
    let form = a_form(&lib);
    let mut wears = CopyWears::new(&lib);
    wears.step(&lib, 9, 0, 0.0, 1);
    wears.step(&lib, 9, form, 0.5, 2);
    let once = wears.get(9).cloned().expect("his look");
    wears.step(&lib, 9, form, 0.9, 2);
    assert_eq!(wears.get(9).map(CopyWear::worn), Some(once.worn()));
    assert_eq!(wears.get(9).and_then(|w| w.slide(wear_slots(&lib)[0])), once.slide(wear_slots(&lib)[0]), "the same frame: not stepped again");
    wears.forget_gone(2 + GONE_AFTER_FRAMES);
    assert!(wears.get(9).is_some());
    wears.forget_gone(3 + GONE_AFTER_FRAMES);
    assert!(wears.get(9).is_none(), "gone: a new copy of that member starts in his own form afresh");
}

/// THE TABLE POOL: a table is reused while its textures are wanted, written only once no frame in flight can read it.
#[test]
fn the_copies_tables_are_reused_and_never_rewritten_in_flight() {
    let files = |f: &str| vec![Some(f.to_string()), None];
    let mut pool = TablePool::new(2, 4);
    assert_eq!(pool.choose(5, &files("a"), 10), Choice::Write(0));
    assert_eq!(pool.choose(5, &files("a"), 11), Choice::Hit(0), "the same textures: the same table, nothing written");
    assert_eq!(pool.choose(6, &files("a"), 11), Choice::Write(1), "another slot: another table");
    assert_eq!(pool.choose(5, &files("b"), 12), Choice::Full, "both bound within the frames in flight");
    assert_eq!(pool.choose(5, &files("b"), 16), Choice::Write(0), "the older one, once no frame in flight reads it");
    assert_eq!(pool.choose(6, &files("a"), 16), Choice::Hit(1));
    assert_eq!(TablePool::new(0, 4).choose(1, &files("a"), 1), Choice::Full);
    assert_eq!(files_of(&[Some(TextureRef { file: "x.png".into(), srgb: true, mips: true }), None]), vec![Some("x.png".to_string()), None]);
    if let Some(lib) = kh3() {
        let wear = wear_slots(&lib);
        assert_eq!(TablePool::count_for(&lib), 2 * MAX_LOOKS * wear.len());
        eprintln!("KH3: {} wear slot(s) of {} ({}), {} copy table(s)", wear.len(), lib.slots.len(), wear.iter().map(|k| lib.slots[*k].slot.as_str()).collect::<Vec<_>>().join(", "), TablePool::count_for(&lib));
    }
}
