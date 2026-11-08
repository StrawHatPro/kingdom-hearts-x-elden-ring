//! (MIMIC-FX, THE USER 2026-11-06: "all of the mimic tears (this goes for bosses and summons) need to have their VFX visible
//! and working" and "sounds made by the copy MUST be played from it's position and not the player's") A SORA COPY'S EFFECTS
//! AND SOUNDS ARE HIS OWN, through the shared route of every Sora copy (the Mimic Tear boss's copy 10, its summoned Mimic 11,
//! the player's Mimic Sora 12: kh-er copyfx.rs `SoraCopyFx` / `SoraCopyAudio`, re-made here with the same rule): two real
//! cores of the SAME character (the player at the origin, his copy 8 m to his left), each with his own animator, the same
//! inputs (a ground combo, a Firaga, a form change into Second Form), each with his own training dummy at the same place
//! relative to him. The effect side gets the copy's step as a Sora INSTANCE (`EffectHost::step_instance`, his own scene),
//! the audio side his sounds on his own member (`Attach::Party`, kh-audio's own state of member 12: Sora's routes, his own
//! voice line, his own motion-sound detector on his bones). Checked:
//! - the copy's Keyblade trail runs on HIS blade; his spell's body flies with HIS spell actor; his form change's effects
//!   spawn on HIS sockets; none of his effects is near the player, none of the player's near him;
//! - the copy's sounds (swings, voices, the spell, the form change, his footsteps) play at HIS place, the player's at his;
//! - THE PLAYER UNCHANGED: with no copy and with the copy, the player's own command streams (effects, sounds) are the same,
//!   byte for byte, and the effect side places every effect of his at exactly the same point.
//!
//! Skipped, saying so, without the pack.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use glam::Vec3;
use kh_audio::{Language, Listener};
use kh_core::host::party::{handle_member, PartyMember, PARTY_HANDLE_BASE, PARTY_HANDLE_SPAN};
use kh_core::{Attach, EffectHandle, EffectSpawn, Input, PadButton, PadButtons, PadState, Pose, SoundHandle, SoundPlay, StateId};
use kh_host::audio::{HostAudio, Output as AudioOut, PoseTap, Step};
use kh_host::rig::{component_to_world, Rig};
use kh_host::seam::{AudioQueue, EffectCommand, EffectQueue};
use kh_host::vfx::{EffectHost, Scene};

/// The copy's member id (kh_audio::copies::MIMIC_SORA: the player's Mimic Sora; the boss's are 10 / 11 by the same rule).
const COPY: u8 = kh_audio::copies::MIMIC_SORA;
/// The copy stands this far to the player's left (cm): farther than the widest effect reaches (a dash finisher's vp_cmb_tuk1
/// sweeps a 6 m arc around whoever plays it).
const APART: f32 = 2000.0;
const STEP: f32 = 1.0 / 60.0;

fn project() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// A copy's own handle for his core's (kh-er copyfx.rs `own_handle`: his member's range, his core's own number kept).
fn own(raw: u32) -> u32 {
    PARTY_HANDLE_BASE + u32::from(COPY) * PARTY_HANDLE_SPAN + (raw & kh_host::vfx::CORE_BITS)
}

/// kh-er copyfx.rs `SoraCopyFx`: his own handles, `Attach::Sora` = on HIM (his instance scene).
struct CopyFx<'a>(&'a mut EffectQueue);

impl kh_core::Effects for CopyFx<'_> {
    fn spawn(&mut self, h: EffectHandle, s: &EffectSpawn) {
        self.0.spawn(EffectHandle(own(h.0)), s);
    }
    fn timing(&mut self, h: EffectHandle, t: kh_core::host::EffectTiming) {
        self.0.timing(EffectHandle(own(h.0)), t);
    }
    fn tint(&mut self, h: EffectHandle, t: kh_core::host::Tint) {
        self.0.tint(EffectHandle(own(h.0)), t);
    }
    fn color(&mut self, h: EffectHandle, rgba: [f32; 4]) {
        self.0.color(EffectHandle(own(h.0)), rgba);
    }
    fn place(&mut self, h: EffectHandle, at: Vec3) {
        self.0.place(EffectHandle(own(h.0)), at);
    }
    fn transform(&mut self, h: EffectHandle, at: Vec3, r: Vec3, s: Vec3) {
        self.0.transform(EffectHandle(own(h.0)), at, r, s);
    }
    fn stop(&mut self, h: EffectHandle, immediate: bool) {
        self.0.stop(EffectHandle(own(h.0)), immediate);
    }
}

/// kh-er copyfx.rs `SoraCopyAudio`: his own handles, on him = on HIS taps (`Attach::Party { member }`), his voice group his.
struct CopyAudio<'a>(&'a mut AudioQueue);

fn on_him(at: Attach) -> Attach {
    match at {
        Attach::Sora { bone, offset } => Attach::Party { member: PartyMember(COPY), bone, offset },
        other => other,
    }
}

impl kh_core::Audio for CopyAudio<'_> {
    fn play(&mut self, h: SoundHandle, p: &SoundPlay) {
        self.0.play(SoundHandle(own(h.0)), &SoundPlay { at: on_him(p.at), ..*p });
    }
    fn stop(&mut self, h: SoundHandle, fade: f32) {
        self.0.stop(SoundHandle(own(h.0)), fade);
    }
    fn voice_group_no(&mut self, number: i32) {
        self.0.party_voice_group_no(PartyMember(COPY), number);
    }
}

/// The input of step `k` (the same for both): a ground combo, Firaga, the form gauge filled and Second Form taken, a combo in
/// it. `state` is the player's state (the form's waits).
struct Plan {
    held: PadButtons,
    phase: u32,
    wait: u32,
}

impl Plan {
    fn input(&mut self, k: usize, state: StateId, cores: &mut [&mut Rig]) -> Input {
        let mut now = PadButtons::NONE;
        match k {
            // the ground combo: attack every other step
            10..=110 if k.is_multiple_of(2) => now = now.with(PadButton::ATTACK),
            150 => {
                let kind = kh_core::magic::spells::command_by_name("firaga").expect("Firaga");
                for r in cores.iter_mut() {
                    r.sora.command_request(kh_core::command::CommandRequest::Execute(kind));
                }
            }
            300 | 302 => {
                for r in cores.iter_mut() {
                    r.sora.debug_add_style(if k == 300 { 60.0 } else { 40.0 });
                }
            }
            _ => {}
        }
        // the form offer taken (Y) once the cast is over, until STYLECHANGE begins
        if k >= 320 && self.phase == 0 {
            self.wait += 1;
            if state == StateId::Stylechange {
                self.phase = 1;
            } else if self.wait.is_multiple_of(4) {
                now = now.with(PadButton::SITUATION);
            }
        }
        if self.phase == 1 && state != StateId::Stylechange {
            self.phase = 2;
            self.wait = 0;
        }
        // a combo in the form
        if self.phase == 2 {
            self.wait += 1;
            if (30..150).contains(&self.wait) && self.wait.is_multiple_of(2) {
                now = now.with(PadButton::ATTACK);
            }
        }
        let pad = PadState::from_held(now, self.held);
        self.held = now;
        // a run at the end (his footsteps)
        let stick = if (780..880).contains(&k) { glam::Vec2::new(0.0, 1.0) } else { glam::Vec2::ZERO };
        Input { move_stick: stick, camera_yaw: 0.0, ..Input::default() }.with_pad(pad)
    }
}

/// One effect of the player as the effect side placed it: (handle, effect, position bits, stopped).
type Placed = (u32, String, [u32; 3], bool);

/// What one run left.
#[derive(Default)]
struct Trace {
    /// Per step: the player's own effect and sound commands (Debug text).
    player_cmds: Vec<String>,
    /// Per step: the effect side's places of the player's effects (handle, effect, position bits).
    player_places: Vec<Vec<Placed>>,
    /// The copy's effects seen (effect, its distance from him, from the player, the step, the state he was in).
    copy_fx: Vec<(String, f32, f32, usize, StateId)>,
    /// The player's effects seen (effect, distance from him, from the copy).
    player_fx: Vec<(String, f32, f32)>,
    /// The copy's trail heads against his blade's tip and the player's (cm), and the player's trail against his own.
    copy_trail: Vec<(f32, f32)>,
    /// The copy's spell bodies against his spell actor (cm).
    copy_spell: Vec<f32>,
    /// The copy's sounds as the audio side placed them (distance from him, from the player) and the player's.
    copy_sounds: Vec<(f32, f32)>,
    player_sounds: Vec<(f32, f32)>,
    /// Live sounds of the copy's own audio state (cause, distance from him, from the player).
    copy_live: Vec<(String, f32, f32)>,
    /// The copy reached STYLECHANGE and his form.
    copy_form: i32,
}

fn rigs(pack: &std::path::Path, with_copy: bool) -> (Rig, Option<Rig>) {
    let loadout = project().join("sheets").join("port").join("sora-loadout-99.json");
    let player = Rig::new(pack, &loadout, Some(Vec3::new(300.0, 0.0, 0.0))).expect("the player's rig");
    let copy = with_copy.then(|| {
        let mut c = Rig::new(pack, &loadout, Some(Vec3::new(300.0, APART, 0.0))).expect("the copy's rig");
        let z = c.sora.output().position.z;
        c.sora.teleport(Pose { position: Vec3::new(0.0, APART, z), yaw: 0.0 });
        c.out = c.sora.output();
        c
    });
    (player, copy)
}

fn run(steps: usize, with_copy: bool) -> Option<Trace> {
    let pack = project().join("build").join("sora_pack");
    if !pack.join("vfx").join("effects").is_dir() || !pack.join("audio").join("manifest.json").is_file() {
        eprintln!("SKIPPED: no effect / audio pack under {}", pack.display());
        return None;
    }
    let (mut player, mut copy) = rigs(&pack, with_copy);
    let mut fx = EffectHost::load(&pack).expect("the effect pack");
    let apack = Arc::new(kh_audio::Pack::load(&pack.join("audio")).expect("the audio pack"));
    let mut audio = HostAudio::new(apack, AudioOut::null(), 1, Language::En);
    let mut plan = Plan { held: PadButtons::NONE, phase: 0, wait: 0 };
    let mut t = Trace::default();
    let mut copy_tap = PoseTap::default();
    let mut copy_steps = 0u64;
    for k in 0..steps {
        let state = player.out.state;
        let input = {
            let mut cores: Vec<&mut Rig> = std::iter::once(&mut player).chain(copy.as_mut()).collect();
            plan.input(k, state, &mut cores)
        };
        // the player's step: his own seams
        let mut pq = EffectQueue::default();
        player.step_with(&input, &mut audio.queue, &mut pq);
        t.player_cmds.push(format!("{:?} | {:?}", pq.0, audio.queue.0));
        // the copy's step: his own handles, on him
        let mut cq = EffectQueue::default();
        if let Some(c) = copy.as_mut() {
            c.step_with(&input, &mut CopyAudio(&mut audio.queue), &mut CopyFx(&mut cq));
            copy_steps += 1;
            copy_tap.record(copy_steps, &c.out, c.animator.skeleton(), &c.pose, &component_to_world(&c.out));
            copy_tap.samples.clear();
            t.copy_form = t.copy_form.max(c.sora.combat().form.cmd);
        }
        let mut player_tap = PoseTap::default();
        player_tap.record(k as u64 + 1, &player.out, player.animator.skeleton(), &player.pose, &component_to_world(&player.out));
        player_tap.samples.clear();
        // the effect side: the copy's step on HIS scene first (kh-er copyfx.rs `sora_copy_step`), then the main step
        let eye = player.out.position + Vec3::new(-500.0, 0.0, 200.0);
        if let Some(c) = copy.as_ref() {
            let dummy = c.dummy;
            let target = move |_: kh_core::TargetId| dummy.map(|d| d.info.center);
            let scene = Scene { out: &c.out, sora: &c.sora, tap: &copy_tap, target: &target, ground: Some(0.0), eye };
            fx.step_instance(COPY, &cq.0, &scene);
        }
        let pdummy = player.dummy;
        let ptarget = move |_: kh_core::TargetId| pdummy.map(|d| d.info.center);
        fx.step(STEP, &pq.0, &Scene { out: &player.out, sora: &player.sora, tap: &player_tap, target: &ptarget, ground: Some(0.0), eye });
        // where the effect side placed everything
        let copy_at = copy.as_ref().map(|c| c.out.position);
        let mut places = Vec::new();
        for p in fx.places() {
            let from_player = p.position.distance(player.out.position);
            match (p.member, copy_at) {
                (Some(m), Some(at)) => {
                    assert_eq!(m, COPY);
                    t.copy_fx.push((p.effect.clone(), p.position.distance(at), from_player, k, copy.as_ref().map_or(StateId::Idle, |c| c.out.state)));
                }
                (None, _) => {
                    places.push((p.handle, p.effect.clone(), p.position.to_array().map(f32::to_bits), p.stopped));
                    t.player_fx.push((p.effect.clone(), from_player, copy_at.map_or(f32::MAX, |c| p.position.distance(c))));
                }
                _ => {}
            }
        }
        t.player_places.push(places);
        if let Some(c) = copy.as_ref() {
            let dummy = c.dummy;
            let target = move |_: kh_core::TargetId| dummy.map(|d| d.info.center);
            let scene = Scene { out: &c.out, sora: &c.sora, tap: &copy_tap, target: &target, ground: Some(0.0), eye };
            let sockets: HashMap<String, Vec3> = fx.instance_sockets(COPY, &scene, &[]).into_iter().map(|(n, p, _)| (n, p)).collect();
            let player_tip = fx.socket_point(&Scene { out: &player.out, sora: &player.sora, tap: &player_tap, target: &ptarget, ground: Some(0.0), eye }, "vfx_top");
            for p in fx.places().into_iter().filter(|p| p.member == Some(COPY) && p.trail && !p.stopped) {
                if let Some(head) = fx.trail_head_of(p.handle) {
                    t.copy_trail.push((head.distance(sockets["vfx_top"]), head.distance(player_tip)));
                }
            }
            for p in c.sora.magic().projectile_views() {
                if let Some(e) = p.effect {
                    for at in fx.places_of(own(e.0)) {
                        t.copy_spell.push(at.distance(p.position));
                    }
                }
            }
        }
        // the audio side: the copy's sounds on HIS taps (fx.rs `set_party`'s feed of his tap), his motion sounds from his bones
        let effects_for_audio: Vec<EffectCommand> = pq
            .0
            .iter()
            .copied()
            .chain(cq.0.iter().map(|c| match *c {
                EffectCommand::Spawn(h, s) => EffectCommand::Spawn(h, EffectSpawn { at: on_him(s.at), ..s }),
                other => other,
            }))
            .collect();
        let to_world = component_to_world(&player.out);
        // (the listener between the two: both within KH3's 20 m motion-sound clip distance)
        let listener = Listener { position: player.out.position + Vec3::new(-400.0, APART * 0.5, 150.0), forward: Vec3::X, right: Vec3::Y };
        let copy_ref = copy.as_ref();
        let tap_ref = &copy_tap;
        let place = |at: &Attach| match at {
            Attach::Party { member, bone, offset } if member.0 == COPY => match (copy_ref, bone.and_then(|b| tap_ref.bone_world(b))) {
                (_, Some(b)) => b.translation + b.rotation * *offset,
                (Some(c), None) => kh_host::seam::on_actor(c.out.position, c.out.yaw, bone.is_some(), *offset),
                (None, None) => player.place(at),
            },
            other => player.place(other),
        };
        if let Some(c) = copy.as_ref() {
            audio.engine.set_party_anim(COPY, c.out.anim_ticked.name.name(), c.out.anim_ticked.time);
            let status = c.sora.status();
            audio.engine.set_party_sora_weapons(COPY, if status.attack_model.is_empty() { "w_so010" } else { status.attack_model }, status.left_attack_model);
        }
        audio.step_with(&Step { out: &player.out, dt: STEP, skeleton: player.animator.skeleton(), pose: &player.pose, component_to_world: &to_world, effects: &effects_for_audio, surface: 0, listener, hp: None, place: &place }, &mut |engine| {
            let Some(c) = copy_ref else { return };
            let positions: [Vec3; 14] = std::array::from_fn(|i| tap_ref.bone(kh_audio::autose::BONES[i]).unwrap_or(c.out.position));
            let frame = kh_audio::BoneFrame::from_positions(&positions, c.out.on_floor);
            let _ = engine.party_auto_se(COPY, kh_audio::AutoSeTick { dt: STEP, frame: &frame, surface: 0, anim: Some((c.out.anim_ticked.name.name(), c.out.anim_ticked.time)), listener: &listener, bone: &|b| tap_ref.bone(b) });
        });
        for (h, at) in &audio.placed {
            let from_player = at.distance(player.out.position);
            match (handle_member(*h).map(|m| m.0), copy_at) {
                (Some(COPY), Some(c)) => t.copy_sounds.push((at.distance(c), from_player)),
                (None, _) if *h > 0 && *h < PARTY_HANDLE_BASE => t.player_sounds.push((from_player, copy_at.map_or(f32::MAX, |c| at.distance(c)))),
                _ => {}
            }
        }
        if let Some(c) = copy_at {
            for l in audio.engine.active() {
                if let (true, Some(p)) = (l.cause.starts_with(kh_audio::copies::name(COPY)), l.position) {
                    t.copy_live.push((l.cause.clone(), p.distance(c), p.distance(player.out.position)));
                }
            }
        }
    }
    Some(t)
}

#[test]
fn mimic_fx_a_sora_copys_effects_and_sounds_are_his_own_and_the_players_unchanged() {
    let steps = 900;
    let Some(alone) = run(steps, false) else { return };
    let with = run(steps, true).expect("the second run");
    // THE COPY'S EFFECTS: on him
    let names: std::collections::BTreeSet<&str> = with.copy_fx.iter().map(|f| f.0.as_str()).collect();
    eprintln!("the copy's effects ({}): {names:?}", names.len());
    assert!(!with.copy_fx.is_empty(), "the copy's effects were drawn");
    for (e, from_him, from_player, k, state) in &with.copy_fx {
        assert!(*from_him < 900.0 && from_him < from_player, "step {k} ({state:?}): the copy's {e} stands {from_him:.0} cm from him, {from_player:.0} cm from the player");
    }
    for (e, from_player, from_copy) in &with.player_fx {
        assert!(from_player < from_copy, "the player's {e} stands {from_player:.0} cm from him, {from_copy:.0} cm from the copy");
    }
    // and each of his effects reaches as far from him as the player's same effect from the player (the same rules on his own
    // sockets: the two cores play the same moves)
    let reach = |list: &mut dyn Iterator<Item = (&String, f32)>| {
        let mut m: HashMap<String, f32> = HashMap::new();
        for (e, d) in list {
            let r = m.entry(e.clone()).or_insert(0.0);
            *r = r.max(d);
        }
        m
    };
    let his = reach(&mut with.copy_fx.iter().map(|f| (&f.0, f.1)));
    let players = reach(&mut with.player_fx.iter().map(|f| (&f.0, f.1)));
    for (e, r) in &his {
        if let Some(p) = players.get(e) {
            assert!((r - p).abs() < 0.25 * p.max(*r) + 60.0, "{e}: reaches {r:.0} cm from the copy, {p:.0} cm from the player");
        }
    }
    let shared = his.keys().filter(|e| players.contains_key(*e)).count();
    eprintln!("{shared} of his {} effects reach as far from him as the player's same effect from the player", his.len());
    assert!(shared * 10 >= his.len() * 9, "the copy plays the player's effects");
    // his Keyblade trail on HIS blade
    assert!(!with.copy_trail.is_empty(), "the copy's Keyblade trail ran");
    let worst = with.copy_trail.iter().map(|t| t.0).fold(0.0, f32::max);
    eprintln!("the copy's trail: {} samples, its head at most {worst:.1} cm from HIS blade's tip", with.copy_trail.len());
    assert!(with.copy_trail.iter().all(|(his, players)| his < players), "his trail follows his blade, not the player's");
    // his spell's body on HIS spell actor
    assert!(!with.copy_spell.is_empty(), "the copy's Firaga body was drawn");
    assert!(with.copy_spell.iter().all(|d| *d < 1.0), "his Firaga's body flies with his own actor: {:?}", with.copy_spell.iter().fold(0.0f32, |a, b| a.max(*b)));
    // his form change: Second Form, its effects on him
    assert!(with.copy_form > 0, "the copy changed form");
    let form_fx: Vec<&String> = with.copy_fx.iter().filter(|f| f.4 == StateId::Stylechange).map(|f| &f.0).collect();
    eprintln!("the copy's STYLECHANGE effects: {:?}", form_fx.iter().collect::<std::collections::BTreeSet<_>>());
    assert!(!form_fx.is_empty(), "the copy's transformation effects were drawn on him");
    // HIS SOUNDS: at his place
    eprintln!("sounds: the copy's {} placed, the player's {}; the copy's live sounds {}", with.copy_sounds.len(), with.player_sounds.len(), with.copy_live.len());
    assert!(!with.copy_sounds.is_empty() && !with.player_sounds.is_empty());
    for (his, players) in &with.copy_sounds {
        assert!(his < players && *his < 300.0, "a copy's sound {his:.0} cm from him, {players:.0} cm from the player");
    }
    for (players, his) in &with.player_sounds {
        assert!(players < his, "a player's sound {players:.0} cm from him, {his:.0} from the copy");
    }
    assert!(with.copy_live.iter().any(|l| l.0.contains("autose")), "the copy's own footsteps / motion sounds: {:?}", with.copy_live.iter().map(|l| &l.0).collect::<std::collections::BTreeSet<_>>());
    for (cause, his, players) in &with.copy_live {
        assert!(his < players, "the copy's live sound {cause}: {his:.0} cm from him, {players:.0} from the player");
    }
    // THE PLAYER UNCHANGED
    assert_eq!(alone.player_cmds.len(), with.player_cmds.len());
    for (k, (a, b)) in alone.player_cmds.iter().zip(&with.player_cmds).enumerate() {
        assert_eq!(a, b, "step {k}: the player's command stream");
    }
    // every effect of his alive in both runs at exactly the same point (the evaluator seeds a system's randomness by its spawn
    // number, kh-vfx sim.rs, so a particle's life may differ by the copy's spawns between his, as with Donald's and Goofy's)
    let mut compared = 0;
    for (k, (a, b)) in alone.player_places.iter().zip(&with.player_places).enumerate() {
        for x in a {
            if let Some(y) = b.iter().find(|y| y.0 == x.0 && y.1 == x.1) {
                assert_eq!(x, y, "step {k}: the player's effect as the effect side places it");
                compared += 1;
            }
        }
        let (na, nb) = (a.len(), b.len());
        assert!(na.abs_diff(nb) <= 2, "step {k}: the player's live effects {na} alone, {nb} with the copy");
    }
    eprintln!("the player's effects: {compared} places compared bit for bit, all the same");
    assert!(compared > 1000);
}
