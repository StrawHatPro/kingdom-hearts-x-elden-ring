//! The Bonamik port (crates/kh-anim/src/bonamik) against the table of
//! docs/sora-animation-spec-changes.md, section "B4. Bonamik: recipe", item 8 ("row: ..."), i.e.
//! the numbers of tools/lib/bonamik/validate.js and of tools/sora_pack.js --check, on the real
//! pack (build/sora_pack). Without a pack on disk the tests say so and pass.
//!
//! The cross-check against the JS evaluator reads build/sora_tmp/bnm_crosscheck/idle_crosscheck.bin,
//! written by `node build/sora_tmp/bnm_crosscheck/dump.js`; without that file it says so and passes.

use std::path::PathBuf;

use glam::{Quat, Vec3};
use kh_anim::bonamik::records::{CollObj, Core};
use kh_anim::bonamik::{self, Bonamik, BonamikHost, BonamikOptions, Jitter, DEFAULT_SEED};
use kh_anim::json::Json;
use kh_anim::pose::component_space;
use kh_anim::{Animator, BodySlot, Frame, NoIkWorld, Pack, Settings, Trs};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn pack() -> Option<Pack> {
    let dir = root().join("build/sora_pack");
    if !dir.join("manifest.json").is_file() {
        eprintln!("SKIPPED: no pack at {}", dir.display());
        return None;
    }
    let pack = Pack::load(&dir).expect("the pack loads");
    if pack.bonamik.is_none() {
        eprintln!("SKIPPED: the pack has no data/bonamik.json");
        return None;
    }
    Some(pack)
}

fn data(pack: &Pack) -> &Json {
    pack.bonamik.as_ref().expect("checked by pack()")
}

fn solver(pack: &Pack, options: BonamikOptions) -> Bonamik {
    Bonamik::new(data(pack), &pack.skeleton, options).expect("Bonamik sets up")
}

fn zero() -> BonamikOptions {
    BonamikOptions::default()
}

fn seeded() -> BonamikOptions {
    BonamikOptions { jitter: Jitter::Seeded(DEFAULT_SEED), ..BonamikOptions::default() }
}

/// An animator used only as the game's sampler (`Animator::sample`).
fn sampler(pack: &Pack) -> Animator {
    Animator::new(pack.clone(), Settings { leg_ik: false, kine_driver: false, face: false, bonamik: false, ..Settings::default() }).expect("the animator sets up")
}

/// Component-space pose of a sequence at a time.
fn clip_pose(animator: &Animator, sequence: &str, time: f32) -> Vec<Trs> {
    component_space(animator.skeleton(), &animator.sample(sequence, time))
}

fn frame(sim: &mut Bonamik, dt: f32, pose: &[Trs]) -> (Vec<Trs>, bonamik::FrameInfo) {
    let mut out = pose.to_vec();
    let info = sim.frame(dt, &mut out, &Trs::IDENTITY, &BonamikHost::default());
    (out, info)
}

fn dist(a: Vec3, b: Vec3) -> f64 {
    let (x, y, z) = ((a.x - b.x) as f64, (a.y - b.y) as f64, (a.z - b.z) as f64);
    (x * x + y * y + z * z).sqrt()
}

fn bits(t: &Trs) -> [u32; 10] {
    [t.rotation.x.to_bits(), t.rotation.y.to_bits(), t.rotation.z.to_bits(), t.rotation.w.to_bits(), t.translation.x.to_bits(), t.translation.y.to_bits(), t.translation.z.to_bits(), t.scale.x.to_bits(), t.scale.y.to_bits(), t.scale.z.to_bits()]
}

fn pose_bits(pose: &[Trs]) -> Vec<[u32; 10]> {
    pose.iter().map(bits).collect()
}

fn dynamic(sim: &Bonamik) -> Vec<usize> {
    sim.core().f3778.f0.iter().enumerate().filter(|(_, p)| p.f276 == 0).map(|(i, _)| i).collect()
}

struct Rest {
    sim: Bonamik,
    /// `hist.findIndex(x => x < 1e-3) + 1` of validate.js / sora_pack.js.
    settle: usize,
    /// Largest distance of a settled dynamic node from the position its animated bone gives it.
    max_node: f64,
    /// Largest translation offset of a weighted bone from the animation in the shown pose.
    max_bone: f64,
    driven: usize,
    unchanged: usize,
    changed_undriven: usize,
}

/// `restRun` of validate.js / `rest` of sora_pack.js: a pose held for `n` evaluations at 30 fps.
fn rest(pack: &Pack, pose: &[Trs], options: BonamikOptions, n: usize) -> Rest {
    let mut sim = solver(pack, options);
    let dynamic = dynamic(&sim);
    let driven_set = sim.driven_bones();
    let input = pose_bits(pose);
    let (mut previous, mut history): (Option<Vec<Vec3>>, Vec<f64>) = (None, Vec::new());
    let mut out = pose.to_vec();
    let mut changed_undriven = 0;
    for _ in 0..n {
        let (shown, info) = frame(&mut sim, 1.0 / 30.0, pose);
        assert!(info.did_eval, "at 30 fps every frame evaluates");
        out = shown;
        let positions = sim.particle_positions();
        let current: Vec<Vec3> = dynamic.iter().map(|i| positions[*i]).collect();
        if let Some(previous) = &previous {
            history.push(current.iter().zip(previous).map(|(a, b)| dist(*a, *b)).fold(0.0, f64::max));
        }
        previous = Some(current);
        // row "driven set": the non-driven bones are bit-identical to the input pose on every frame
        let shown_bits = pose_bits(&out);
        changed_undriven += (0..pose.len()).filter(|i| !driven_set.contains(i) && shown_bits[*i] != input[*i]).count();
    }
    let core = sim.core();
    let mut max_node = 0.0f64;
    for &i in &dynamic {
        let p = &core.f3778.f0[i];
        let bone = &pose[p.f248];
        let local = Vec3::new(p.f170[0], p.f170[1], p.f170[2]);
        let rest = bone.translation.as_dvec3() + bone.rotation.as_dquat() * (bone.scale.as_dvec3() * local.as_dvec3());
        let at = Vec3::new(p.f0[0], p.f0[1], p.f0[2]).as_dvec3();
        max_node = max_node.max((at - rest).length());
    }
    let (mut driven, mut unchanged, mut max_bone) = (0, 0, 0.0f64);
    for i in 0..pose.len() {
        if core.f3810[i] > 0.0 {
            driven += 1;
            max_bone = max_bone.max(dist(out[i].translation, pose[i].translation));
        } else if bits(&out[i]) == input[i] {
            unchanged += 1;
        }
    }
    let settle = history.iter().position(|x| *x < 1e-3).map_or(0, |i| i + 1);
    Rest { sim, settle, max_node, max_bone, driven, unchanged, changed_undriven }
}

// ------------------------------------------------------------------------------------------------
// row: setup (recipe item 2): the built records equal data/bonamik2.json -> runtime2

struct Compare {
    checked: usize,
    bad: Vec<String>,
    worst: f64,
}

impl Compare {
    fn number(&mut self, what: &str, got: f64, want: &Json) {
        self.checked += 1;
        match want.as_f64() {
            // the file has float32 values printed as doubles (-0 as 0)
            Some(w) if (w as f32) as f64 == got || (w == got) => {}
            Some(w) => {
                self.worst = self.worst.max((w - got).abs());
                if self.bad.len() < 40 {
                    self.bad.push(format!("{what}: {got:e} (bits {:08x}), runtime2 has {w:e} (bits {:08x})", (got as f32).to_bits(), (w as f32).to_bits()));
                }
            }
            None => self.bad.push(format!("{what}: runtime2 has no number")),
        }
    }

    fn floats(&mut self, what: &str, got: &[f32], want: &Json) {
        if want.items().len() != got.len() {
            self.bad.push(format!("{what}: {} values, runtime2 has {}", got.len(), want.items().len()));
            return;
        }
        for (k, g) in got.iter().enumerate() {
            self.number(&format!("{what}[{k}]"), *g as f64, want.at(k));
        }
    }

    fn index(&mut self, what: &str, got: Option<usize>, want: &Json) {
        self.number(what, got.map_or(-1.0, |i| i as f64), want);
    }

    fn list(&mut self, what: &str, got: &[f64], want: &Json) {
        if want.items().len() != got.len() {
            self.bad.push(format!("{what}: {got:?}, runtime2 has {} entries", want.items().len()));
            return;
        }
        for (k, g) in got.iter().enumerate() {
            self.number(&format!("{what}[{k}]"), *g, want.at(k));
        }
    }

    fn object(&mut self, what: &str, o: &CollObj, want: &Json) {
        self.number(&format!("{what}.type"), o.f18 as f64, want.get("type"));
        self.number(&format!("{what}.a"), o.f0 as f64, want.get("a"));
        self.index(&format!("{what}.b"), o.f8, want.get("b"));
        self.index(&format!("{what}.c"), o.f10, want.get("c"));
        self.number(&format!("{what}.batch"), o.f1c as f64, want.get("batch"));
        self.number(&format!("{what}.group"), o.f38 as f64, want.get("group"));
        self.number(&format!("{what}.concave"), o.f64 as f64, want.get("concave"));
        self.number(&format!("{what}.boundRadius"), o.f80 as f64, want.get("boundRadius"));
        self.number(&format!("{what}.thickness"), o.f5c as f64, want.get("thickness"));
    }
}

#[test]
fn setup_records_equal_runtime2() {
    let Some(pack) = pack() else { return };
    let path = root().join("build/sora_pack/data/bonamik2.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("SKIPPED: no {}", path.display());
        return;
    };
    let file = Json::parse(&text).expect("bonamik2.json parses");
    let want = file.get("runtime2");
    let sim = solver(&pack, zero());
    let core: &Core = sim.core();
    let sc = &core.f3778;
    let mut c = Compare { checked: 0, bad: Vec::new(), worst: 0.0 };

    let g = sim.gravity();
    c.floats("gravityWorld", &[g.x, g.y, g.z], want.get("gravityWorld"));
    let it = want.get("iterations");
    c.number("iterations.substeps", core.f674 as f64, it.get("substeps"));
    c.number("iterations.outer", core.f678 as f64, it.get("outer"));
    c.number("iterations.link", core.f67c as f64, it.get("link"));
    c.number("iterations.collision", core.f680 as f64, it.get("collision"));
    let ts = want.get("timeStep");
    c.number("timeStep.bucket", core.f3824 as f64, ts.get("bucket"));
    c.number("timeStep.step", core.f3830 as f64, ts.get("step"));
    c.number("timeStep.kAnimSim", core.f3838 as f64, ts.get("kAnimSim"));
    c.number("timeStep.kFast", core.f383c as f64, ts.get("kFast"));
    c.number("timeStep.teleportDistSq", core.f371c as f64, ts.get("teleportDistSq"));
    let flags = want.get("flags");
    c.number("flags.fastMotionMode", core.f660 as f64, flags.get("fastMotionMode"));
    c.number("flags.jointLinkMode", core.f664 as f64, flags.get("jointLinkMode"));
    c.number("flags.groundCollision", core.f65d as f64, flags.get("groundCollision"));
    c.number("flags.resetPending", core.f33f4 as f64, flags.get("resetPending"));
    c.number("flags.resetHard", core.f33f5 as f64, flags.get("resetHard"));

    assert_eq!(core.f410, want.get("groups").items().len(), "groups");
    for (i, w) in want.get("groups").items().iter().enumerate() {
        let n = format!("group {i}");
        assert_eq!(w.get("name").as_str(), Some(core.f4d8[i].as_str()), "{n} name");
        c.number(&format!("{n}.state"), core.f804[i] as f64, w.get("state"));
        c.number(&format!("{n}.animWeight"), core.f3578[i] as f64, w.get("animWeight"));
        c.floats(&format!("{n}.refUp"), &core.f1c0[i], w.get("refUp"));
        c.number(&format!("{n}.tag"), core.f684[i] as f64, w.get("tag"));
        c.number(&format!("{n}.collisionTag"), core.f744[i] as f64, w.get("collisionTag"));
        c.number(&format!("{n}.enableGround"), core.f924[i] as f64, w.get("enableGround"));
        c.number(&format!("{n}.windPresetSlot"), core.fad4[i] as f64, w.get("windPresetSlot"));
        c.number(&format!("{n}.charaWindScale"), core.f954[i] as f64, w.get("charaWindScale"));
        c.number(&format!("{n}.presetWindScale"), core.fa14[i] as f64, w.get("presetWindScale"));
        c.number(&format!("{n}.innerConeScale"), core.fb94[i] as f64, w.get("innerConeScale"));
        c.number(&format!("{n}.outerConeScale"), core.fc54[i] as f64, w.get("outerConeScale"));
        c.number(&format!("{n}.localForceScale"), core.fd14[i] as f64, w.get("localForceScale"));
        c.number(&format!("{n}.worldWindScale"), core.fdd4[i] as f64, w.get("worldWindScale"));
        c.number(&format!("{n}.massScale"), core.fe94[i] as f64, w.get("massScale"));
        c.number(&format!("{n}.aeroEnable"), core.f8f4[i] as f64, w.get("aeroEnable"));
    }

    assert_eq!(sc.f0.len(), want.get("particles").items().len(), "particles");
    for (i, w) in want.get("particles").items().iter().enumerate() {
        let (p, n) = (&sc.f0[i], format!("particle {i}"));
        assert_eq!(w.get("joint").as_str(), Some(p.name.as_str()), "{n} joint");
        c.number(&format!("{n}.bone"), p.f248 as f64, w.get("bone"));
        c.floats(&format!("{n}.pos"), &p.f0, w.get("pos"));
        c.floats(&format!("{n}.quat"), &p.f20, w.get("quat"));
        c.index(&format!("{n}.parent"), p.f190, w.get("parent"));
        c.index(&format!("{n}.child"), p.f198, w.get("child"));
        c.list(&format!("{n}.pcLinks"), &p.f1b0[..p.f1a8].iter().map(|l| *l as f64).collect::<Vec<_>>(), w.get("pcLinks"));
        c.number(&format!("{n}.isChainChild"), p.fbc as f64, w.get("isChainChild"));
        c.number(&format!("{n}.hasAnchor"), p.fbd as f64, w.get("hasAnchor"));
        c.number(&format!("{n}.kinematic"), p.f276 as f64, w.get("kinematic"));
        c.number(&format!("{n}.collision"), p.f277 as f64, w.get("collision"));
        c.number(&format!("{n}.multiCollision"), p.f278 as f64, w.get("multiCollision"));
        c.list(&format!("{n}.twistNeighbours"), &p.f2e0[..p.f310].iter().map(|l| *l as f64).collect::<Vec<_>>(), w.get("twistNeighbours"));
        c.list(&format!("{n}.twistSigns"), &p.f300[..p.f310].iter().map(|l| *l as f64).collect::<Vec<_>>(), w.get("twistSigns"));
        c.number(&format!("{n}.twistRest"), p.f318 as f64, w.get("twistRest"));
        c.number(&format!("{n}.pcTwist"), p.f314 as f64, w.get("pcTwist"));
        c.floats(&format!("{n}.dir1DC"), &p.f1dc, w.get("dir1DC"));
        c.number(&format!("{n}.kdiDriven"), p.f240.is_some() as u8 as f64, w.get("kdiDriven"));
        let driver = if p.f278 != 0 && core.f400[p.f248] != 0 && core.f400[p.f248] != 9999 { Some(core.f400[p.f248] - 1) } else { None };
        c.index(&format!("{n}.driverOfMultiCollision"), driver, w.get("driverOfMultiCollision"));
    }

    assert_eq!(sc.f40.len(), want.get("links").items().len(), "links");
    for (i, w) in want.get("links").items().iter().enumerate() {
        let (l, n) = (&sc.f40[i], format!("link {i}"));
        c.number(&format!("{n}.parent"), l.f0 as f64, w.get("parent"));
        c.number(&format!("{n}.child"), l.f8 as f64, w.get("child"));
        c.number(&format!("{n}.batch"), l.f10 as f64, w.get("batch"));
        c.number(&format!("{n}.str"), l.f14 as f64, w.get("str"));
        c.number(&format!("{n}.parentStr"), l.f18 as f64, w.get("parentStr"));
        c.number(&format!("{n}.rest"), l.f20 as f64, w.get("rest"));
        c.number(&format!("{n}.isParentChild"), l.f24 as f64, w.get("isParentChild"));
        c.number(&format!("{n}.hardLength"), l.f26 as f64, w.get("hardLength"));
        c.index(&format!("{n}.cone"), l.f28, w.get("cone"));
        // the position of the link's collision object in the sorted dynamic-side list
        c.index(&format!("{n}.collisionObject"), l.f30.and_then(|o| sc.f50.iter().position(|x| *x == o)), w.get("collisionObject"));
    }

    assert_eq!(sc.f30.len(), want.get("cones").items().len(), "cones");
    for (i, w) in want.get("cones").items().iter().enumerate() {
        let (k, n) = (&sc.f30[i], format!("cone {i}"));
        c.number(&format!("{n}.parent"), k.f0 as f64, w.get("parent"));
        c.number(&format!("{n}.child"), k.f8 as f64, w.get("child"));
        c.number(&format!("{n}.batch"), k.f10 as f64, w.get("batch"));
        c.number(&format!("{n}.outerStr"), k.f14 as f64, w.get("outerStr"));
        c.number(&format!("{n}.parentShare"), k.f18 as f64, w.get("parentShare"));
        c.number(&format!("{n}.animation"), k.f20 as f64, w.get("animation"));
        c.number(&format!("{n}.innerStr"), k.f28 as f64, w.get("innerStr"));
        c.number(&format!("{n}.limitY"), k.f30 as f64, w.get("limitY"));
        c.number(&format!("{n}.limitZ"), k.f34 as f64, w.get("limitZ"));
        c.number(&format!("{n}.restLength"), k.f40 as f64, w.get("restLength"));
        c.number(&format!("{n}.length"), k.f44 as f64, w.get("length"));
        c.floats(&format!("{n}.q50"), &k.f50, w.get("q50"));
        c.floats(&format!("{n}.q60"), &k.f60, w.get("q60"));
        c.floats(&format!("{n}.q70"), &k.f70, w.get("q70"));
        c.floats(&format!("{n}.q80"), &k.f80, w.get("q80"));
    }

    assert_eq!(sc.f10.len(), want.get("anchors").items().len(), "anchors");
    for (i, w) in want.get("anchors").items().iter().enumerate() {
        let (a, n) = (&sc.f10[i], format!("anchor {i}"));
        c.number(&format!("{n}.particle"), a.f0 as f64, w.get("particle"));
        c.number(&format!("{n}.type"), a.f4 as f64, w.get("type"));
        c.number(&format!("{n}.strength"), a.f8 as f64, w.get("strength"));
        c.floats(&format!("{n}.anchor"), &a.fc, w.get("anchor"));
        c.number(&format!("{n}.radius"), a.f24 as f64, w.get("radius"));
    }

    c.list("chainRoots", &sc.f748.iter().map(|i| *i as f64).collect::<Vec<_>>(), want.get("chainRoots"));

    assert_eq!(sc.f50.len(), want.get("dynObjects").items().len(), "dynamic-side collision objects");
    for (i, w) in want.get("dynObjects").items().iter().enumerate() {
        c.object(&format!("dynObject {i}"), &sc.objs[sc.f50[i]], w);
    }
    let mut lists = 0;
    for (key, w) in want.get("colliderLists").members() {
        let slot: usize = key.parse().expect("a collider list index");
        let got = sc.f70.get(slot).cloned().unwrap_or_default();
        assert_eq!(got.len(), w.items().len(), "collider list {slot}");
        for (i, o) in w.items().iter().enumerate() {
            c.object(&format!("colliderList {slot}[{i}]"), &sc.objs[got[i]], o);
        }
        lists += 1;
    }
    assert_eq!(sc.f70.iter().filter(|l| !l.is_empty()).count(), lists, "non-empty collider lists");
    assert_eq!(sc.f60.len(), want.get("selfPairs").items().len(), "self pairs");
    for (i, w) in want.get("selfPairs").items().iter().enumerate() {
        c.object(&format!("selfPair {i}"), &sc.objs[sc.f60[i]], w);
    }

    assert_eq!(sc.f768.len(), want.get("kdi").items().len(), "KDI records");
    for (i, w) in want.get("kdi").items().iter().enumerate() {
        let (r, n) = (&sc.f768[i], format!("kdi {i}"));
        c.number(&format!("{n}.driven"), r.f0 as f64, w.get("driven"));
        assert_eq!(r.f8.len(), w.get("drivers").items().len(), "{n} drivers");
        for (k, d) in w.get("drivers").items().iter().enumerate() {
            let e = &r.f8[k];
            c.number(&format!("{n}.driver {k}"), e.f0 as f64, d.get("driver"));
            c.floats(&format!("{n}.driver {k}.local"), &e.f8, d.get("local"));
            c.floats(&format!("{n}.driver {k}.rel"), &e.f20, d.get("rel"));
            c.number(&format!("{n}.driver {k}.wTrans"), e.f30 as f64, d.get("wTrans"));
            c.number(&format!("{n}.driver {k}.wRot"), e.f34 as f64, d.get("wRot"));
        }
    }

    let counts = want.get("counts");
    let hard = sc.f40.iter().filter(|l| l.f26 != 0).count();
    for (name, got) in [("particles", sc.f0.len()), ("links", sc.f40.len()), ("cones", sc.f30.len()), ("anchors", sc.f10.len()), ("chainRoots", sc.f748.len()), ("dynObjects", sc.f50.len()), ("selfPairs", sc.f60.len()), ("kdi", sc.f768.len()), ("pin", sc.f758.len()), ("hardLengthLinks", hard)] {
        c.number(&format!("counts.{name}"), got as f64, counts.get(name));
    }
    println!(
        "row setup: {} particles, {} links ({} hard-length), {} cones, {} anchors, {} chain roots, {} dynamic-side collision objects, {} self pairs, {} KDI record(s); {} values compared with runtime2, {} differ (largest difference {:e})",
        sc.f0.len(),
        sc.f40.len(),
        hard,
        sc.f30.len(),
        sc.f10.len(),
        sc.f748.len(),
        sc.f50.len(),
        sc.f60.len(),
        sc.f768.len(),
        c.checked,
        c.bad.len(),
        c.worst
    );
    assert_eq!((sc.f0.len(), sc.f40.len(), sc.f30.len(), sc.f10.len(), sc.f748.len(), sc.f50.len(), sc.f60.len(), sc.f768.len()), (252, 164, 49, 18, 37, 57, 24, 1));
    assert!(c.bad.is_empty(), "setup records differ from runtime2:\n{}", c.bad.join("\n"));
}

// ------------------------------------------------------------------------------------------------
// row: reference pose held for 120 evaluations; row: driven set

#[test]
fn reference_pose_held_settles_at_35() {
    let Some(pack) = pack() else { return };
    let pose = bonamik::reference_pose(&pack.skeleton);
    let r = rest(&pack, &pose, zero(), 120);
    println!(
        "row reference pose: max node move < 0.001 cm from evaluation {} (expected 35); settled nodes at most {:.3} cm from the animated position (expected 2.974); shown pose: weighted bones at most {:.3} cm from the animation",
        r.settle, r.max_node, r.max_bone
    );
    println!("row driven set: {} weighted bones (expected 86), {} other bones bit-identical at the end (expected 188), {} non-driven bone-frames changed over 120 frames (expected 0)", r.driven, r.unchanged, r.changed_undriven);
    assert_eq!(r.settle, 35);
    assert!((r.max_node - 2.974).abs() < 0.0006, "max node distance {}", r.max_node);
    assert_eq!((r.driven, r.unchanged, r.changed_undriven), (86, 188, 0));
    assert_eq!(r.sim.driven_bones().len(), 86);
    // the driven set of the manifest
    let listed: Vec<usize> = pack.manifest.get("runtime").get("bonamik").get("driven_bones").items().iter().filter_map(|b| b.get("bone").as_usize()).collect();
    if !listed.is_empty() {
        let mut listed = listed;
        listed.sort_unstable();
        assert_eq!(r.sim.driven_bones(), listed, "manifest runtime.bonamik.driven_bones");
    }
}

// ------------------------------------------------------------------------------------------------
// row: IdleN (0001) frame 0 held, soft start; row: same with hard reset at start; row: random
// stream at rest; row: driven set

#[test]
fn idle_frame_0_held_settles_at_70_and_hard_reset_at_40() {
    let Some(pack) = pack() else { return };
    let animator = sampler(&pack);
    let pose = clip_pose(&animator, "0001", 0.0);
    let soft = rest(&pack, &pose, zero(), 120);
    println!("row IdleN frame 0, soft start: settles from evaluation {} (expected 70); at most {:.3} cm (expected 2.722)", soft.settle, soft.max_node);
    let hard = rest(&pack, &pose, BonamikOptions { hard_reset_at_start: true, ..zero() }, 120);
    let (a, b) = (soft.sim.particle_positions(), hard.sim.particle_positions());
    let apart = dynamic(&soft.sim).iter().map(|i| dist(a[*i], b[*i])).fold(0.0, f64::max);
    // The 4e-4 cm of the table is the number of validate.js at its default of 240 evaluations
    // (3.90e-4); at 120 evaluations validate.js itself prints 5.40e-4.
    let (soft_long, hard_long) = (rest(&pack, &pose, zero(), 240), rest(&pack, &pose, BonamikOptions { hard_reset_at_start: true, ..zero() }, 240));
    let (a, b) = (soft_long.sim.particle_positions(), hard_long.sim.particle_positions());
    let apart_long = dynamic(&soft_long.sim).iter().map(|i| dist(a[*i], b[*i])).fold(0.0, f64::max);
    println!("row IdleN frame 0, hard reset at start: settles from evaluation {} (expected 40); final configuration {:.2e} cm from the soft start after 240 evaluations (expected within 4e-4; validate.js: 3.90e-4), {:.2e} cm after 120 evaluations (validate.js: 5.40e-4)", hard.settle, apart_long, apart);
    let random = rest(&pack, &pose, seeded(), 120);
    println!("row random stream at rest: state hash zero-jitter {:08x}, seeded {:08x}", soft.sim.state_hash(), random.sim.state_hash());
    println!("row driven set (IdleN frame 0): {} weighted bones, {} others bit-identical, {} non-driven bone-frames changed", soft.driven, soft.unchanged, soft.changed_undriven + hard.changed_undriven + random.changed_undriven);
    assert_eq!(soft.settle, 70);
    assert!((soft.max_node - 2.722).abs() < 0.0006, "max node distance {}", soft.max_node);
    assert_eq!(hard.settle, 40);
    assert_eq!((soft_long.settle, hard_long.settle), (70, 40));
    assert!(apart_long <= 4e-4, "soft start and hard reset end {apart_long} cm apart after 240 evaluations");
    assert!(apart <= 6e-4, "soft start and hard reset are {apart} cm apart after 120 evaluations");
    assert_eq!(soft.sim.state_hash(), random.sim.state_hash(), "the rest state depends on the random stream");
    assert_eq!((soft.driven, soft.unchanged), (86, 188));
    assert_eq!(soft.changed_undriven + hard.changed_undriven + random.changed_undriven, 0);
}

// ------------------------------------------------------------------------------------------------
// row: determinism

#[test]
fn two_runs_of_the_idle_clip_are_identical() {
    let Some(pack) = pack() else { return };
    let animator = sampler(&pack);
    let length = animator.clip_info("0001").expect("IdleN").length;
    let run = || {
        let mut sim = solver(&pack, seeded());
        let mut states = Vec::new();
        for i in 0..240 {
            let pose = clip_pose(&animator, "0001", (i as f32 / 60.0) % length);
            let (out, _) = frame(&mut sim, 1.0 / 60.0, &pose);
            states.push((sim.state_hash(), pose_bits(&out)));
        }
        (states, sim.stats())
    };
    let ((a, stats), (b, _)) = (run(), run());
    let same = a.iter().zip(&b).filter(|(x, y)| x == y).count();
    println!("row determinism: IdleN clip, 240 frames at 60 fps, two runs (seeded stream): {same} of 240 frames with identical particle states and poses; {} evaluations", stats.steps);
    assert_eq!(same, 240);
}

// ------------------------------------------------------------------------------------------------
// row: frame rate

#[test]
fn evaluation_states_do_not_depend_on_the_frame_rate() {
    let Some(pack) = pack() else { return };
    let animator = sampler(&pack);
    let length = animator.clip_info("0001").expect("IdleN").length;
    // the same animated pose per EVALUATION index: IdleN sampled at k / 30 s
    let poses: Vec<Vec<Trs>> = (0..120).map(|k| clip_pose(&animator, "0001", (k as f32 / 30.0) % length)).collect();
    let run = |fps: f32| {
        let mut sim = solver(&pack, zero());
        let (mut hashes, mut frames) = (Vec::new(), 0);
        while hashes.len() < 120 {
            let (_, info) = frame(&mut sim, 1.0 / fps, &poses[hashes.len()]);
            frames += 1;
            if info.did_eval {
                hashes.push(sim.state_hash());
            }
            assert!(frames < 100_000, "no evaluation");
        }
        (hashes, frames, sim.core().f3830, sim.core().f3824)
    };
    let base = run(30.0);
    for fps in [45.0, 60.0, 120.0, 144.0] {
        let r = run(fps);
        let first = r.0.iter().zip(&base.0).position(|(a, b)| a != b);
        println!("row frame rate: {fps} fps against 30 fps: {}; frames used {} (30 fps: {}); solver step {:.6}, bucket {}", first.map_or("all 120 evaluation states bit-identical".to_string(), |i| format!("first difference at evaluation {i}")), r.1, base.1, r.2, r.3);
        assert_eq!(first, None, "{fps} fps");
    }
    let slow = run(20.0);
    let first = slow.0.iter().zip(&base.0).position(|(a, b)| a != b);
    println!("row frame rate: 20 fps against 30 fps: states differ from evaluation {first:?} (expected 15: bucket change; step {:.6}, bucket {})", slow.2, slow.3);
    assert_eq!(first, Some(15));
}

// ------------------------------------------------------------------------------------------------
// row: 60 fps frame pattern

#[test]
fn frame_pattern_at_60_fps() {
    let Some(pack) = pack() else { return };
    let animator = sampler(&pack);
    let pose = clip_pose(&animator, "0001", 0.0);
    let mut sim = solver(&pack, zero());
    let mut pattern = Vec::new();
    for _ in 0..12 {
        let (_, info) = frame(&mut sim, 1.0 / 60.0, &pose);
        pattern.push((info.did_eval, info.alpha));
    }
    println!("row 60 fps frame pattern: {}", pattern.iter().enumerate().map(|(i, (e, a))| format!("{i}:{} a={a:.3}", if *e { 'E' } else { '-' })).collect::<Vec<_>>().join("  "));
    for (i, (evaluated, alpha)) in pattern.iter().enumerate() {
        // evaluation on frames 0, 1, 3, 5, ...; alphas 1, 0, 0.375, 0, 0.375, ...
        let expected = (i == 0 || i % 2 == 1, if i == 0 { 1.0 } else if i % 2 == 1 { 0.0 } else { 0.375 });
        assert_eq!(*evaluated, expected.0, "frame {i}");
        assert!((alpha - expected.1).abs() < 5e-4, "frame {i}: alpha {alpha}");
    }
}

// ------------------------------------------------------------------------------------------------
// row: fall-loop notifies

#[test]
fn fall_loop_notifies_move_the_cloth_and_restore() {
    let Some(pack) = pack() else { return };
    let animator = sampler(&pack);
    let pose = clip_pose(&animator, "0033", 0.0);
    let (mut plain, mut windy) = (solver(&pack, zero()), solver(&pack, zero()));
    let driven = plain.driven_bones();
    let (mut a, mut b) = (pose.clone(), pose.clone());
    for i in 0..100 {
        if i == 50 {
            windy.notify_begin_0033();
        }
        if i >= 50 {
            // the begin's update ticks too: the second tick (i = 51) sets the wind scale 2
            windy.notify_tick_0033();
        }
        a = frame(&mut plain, 1.0 / 30.0, &pose).0;
        b = frame(&mut windy, 1.0 / 30.0, &pose).0;
    }
    let apart = |a: &[Trs], b: &[Trs]| driven.iter().map(|i| dist(a[*i].translation, b[*i].translation)).fold(0.0, f64::max);
    let moved = apart(&a, &b);
    let others = (0..pose.len()).filter(|i| !driven.contains(i) && bits(&a[*i]) != bits(&b[*i])).count();
    windy.notify_end_0033();
    for _ in 0..150 {
        a = frame(&mut plain, 1.0 / 30.0, &pose).0;
        b = frame(&mut windy, 1.0 / 30.0, &pose).0;
    }
    let after = apart(&a, &b);
    println!("row fall-loop notifies: 0033 frame 0, 50 evaluations after the notify effects the driven bones are up to {moved:.3} cm from the un-notified run (12.679 in the JS evaluator); after restore + 150 evaluations {after:.2e} cm (expected within 2e-3)");
    assert!(moved > 1.0, "the notify effects moved the driven bones by only {moved} cm");
    assert!((moved - 12.679).abs() < 0.5, "the notify effects moved the driven bones by {moved} cm; the JS evaluator: 12.679");
    assert_eq!(others, 0, "non-driven bones differ");
    assert!(after <= 2e-3, "after the restore the runs are {after} cm apart");
}

/// The Animator fires the notify states of clip 0033 through its notify windows.
#[test]
fn animator_runs_bonamik_and_the_0033_notifies() {
    let Some(pack) = pack() else { return };
    let mut animator = Animator::new(pack.clone(), Settings { leg_ik: false, face: false, ..Settings::default() }).expect("the animator sets up");
    assert!(animator.warnings.iter().all(|w| !w.contains("Bonamik")), "{:?}", animator.warnings);
    assert_eq!(animator.bonamik_bones().len(), 86);
    let mut without = Animator::new(pack, Settings { leg_ik: false, face: false, bonamik: false, ..Settings::default() }).expect("the animator sets up");
    let dt = 1.0 / 60.0;
    let length = animator.clip_info("0033").expect("the fall loop").length;
    let sequences = ["0033"];
    let (mut time, mut first) = (0.0f32, true);
    let driven = animator.bonamik_bones().to_vec();
    let (mut wind_seen, mut changed_others, mut moved) = (false, 0usize, 0.0f64);
    for step in 0..120 {
        let from = if first { -1.0 } else { time };
        if !first {
            time += dt;
        }
        let wrapped = time >= length;
        if wrapped {
            time -= length;
        }
        let body = BodySlot { samples: [("0033", 1.0), ("0033", 0.0)], blend_with: None, notify_sequences: &sequences, time, weight: 1.0, active: true, notify_window: (from, time) };
        first = false;
        let f = Frame { dt, body, component_to_world: Trs::IDENTITY };
        let with = animator.step(&f, &NoIkWorld).clone();
        let plain = without.step(&f, &NoIkWorld).clone();
        let core = animator.bonamik().expect("Bonamik").core();
        if step == 0 {
            assert_eq!(core.f3744, 1, "the chara wind begins with the clip");
            assert!(core.f954[..core.f410].iter().all(|s| *s == 1.0), "scale 1 on the first tick");
        }
        if step == 1 {
            assert!(core.f954[..core.f410].iter().all(|s| *s == 2.0), "scale 2 from the second tick");
            wind_seen = true;
        }
        assert_eq!(core.f3744, 1, "the looping clip keeps its notify states active (step {step})");
        changed_others += (0..with.component.len()).filter(|i| !driven.contains(i) && bits(&with.component[*i]) != bits(&plain.component[*i])).count();
        moved = moved.max(driven.iter().map(|i| dist(with.component[*i].translation, plain.component[*i].translation)).fold(0.0, f64::max));
        for &bone in &driven {
            let parent = animator.skeleton().bones[bone].parent.expect("a driven bone has a parent");
            let again = with.local[bone].then(&with.component[parent]);
            assert!(dist(again.translation, with.component[bone].translation) < 1e-3, "local transform of bone {bone}");
        }
    }
    println!("Animator: 120 steps of 0033 at 60 fps: driven bones up to {moved:.2} cm from the pose without Bonamik; {changed_others} non-driven bone-frames differ");
    assert!(wind_seen && moved > 1.0);
    assert_eq!(changed_others, 0);
    // another clip: the states end, the asset defaults are back
    let sequences = ["0001"];
    let body = BodySlot { samples: [("0001", 1.0), ("0001", 0.0)], blend_with: None, notify_sequences: &sequences, time: 0.0, weight: 1.0, active: true, notify_window: (-1.0, 0.0) };
    animator.step(&Frame { dt, body, component_to_world: Trs::IDENTITY }, &NoIkWorld);
    let core = animator.bonamik().expect("Bonamik").core();
    assert_eq!(core.f3744, 0);
    assert!(core.f954[..core.f410].iter().all(|s| *s == 0.0));
    let fresh = Bonamik::new(data(&pack_again()), animator.skeleton(), zero()).expect("Bonamik sets up");
    for (p, q) in core.f3778.f0.iter().zip(&fresh.core().f3778.f0) {
        assert_eq!(p.f270.to_bits(), q.f270.to_bits(), "WindLift of {} restored", p.name);
    }
    for (c, d) in core.f3778.f30.iter().zip(&fresh.core().f3778.f30) {
        assert_eq!(c.f28.to_bits(), d.f28.to_bits(), "InnerConeStr restored");
    }
}

/// The branches the JS does not transcribe are `unreachable!` here: no body clip of the pack
/// reaches one (leg IK off: it needs a level; KineDriver and Bonamik on).
#[test]
fn no_clip_of_the_pack_reaches_an_untranscribed_branch() {
    let Some(pack) = pack() else { return };
    let mut clips: Vec<(String, f32)> = Vec::new();
    for item in pack.manifest.get("items").items() {
        if item.get("kind").as_str() == Some("animation") {
            if let (Some(name), Some(length)) = (item.get("animation_name").as_str(), item.get("duration_s").as_f32()) {
                clips.push((name.to_string(), length));
            }
        }
    }
    let mut animator = Animator::new(pack, Settings { leg_ik: false, ..Settings::default() }).expect("the animator sets up");
    let dt = 1.0 / 60.0;
    let (mut steps, mut farthest) = (0usize, 0.0f64);
    for (name, length) in &clips {
        let sequences = [name.as_str()];
        let (mut time, mut first) = (0.0f32, true);
        // each clip twice through (as a loop), cut at 4 s
        let frames = ((length * 2.0).min(4.0) / dt).ceil() as usize + 2;
        for _ in 0..frames {
            let from = if first { -1.0 } else { time };
            if !first {
                time += dt;
            }
            if time >= *length && *length > 0.0 {
                time -= *length;
            }
            first = false;
            let body = BodySlot { samples: [(name.as_str(), 1.0), (name.as_str(), 0.0)], blend_with: None, notify_sequences: &sequences, time, weight: 1.0, active: true, notify_window: (from, time) };
            let pose = animator.step(&Frame { dt, body, component_to_world: Trs::IDENTITY }, &NoIkWorld).clone();
            steps += 1;
            for &bone in animator.bonamik_bones() {
                let t = pose.component[bone].translation;
                assert!(t.is_finite() && pose.component[bone].rotation.is_finite(), "clip {name}: bone {bone} is not finite");
                // (from the hips: since pack 1.5.0 the pack has clips that carry the whole body far from the component origin)
                let hips = animator.skeleton().index_of("koshi").map_or(glam::Vec3::ZERO, |b| pose.component[b].translation);
                farthest = farthest.max((t - hips).length() as f64);
            }
        }
    }
    println!("{} body clips, {steps} steps at 60 fps with KineDriver and Bonamik: no untranscribed branch reached; the farthest driven bone is {farthest:.1} cm from the hips", clips.len());
    assert!(farthest < 400.0, "a driven bone flew {farthest} cm away");
}

fn pack_again() -> Pack {
    pack().expect("the pack was there a moment ago")
}

// ------------------------------------------------------------------------------------------------
// row: cross-check against the JS

struct Dump {
    bytes: Vec<u8>,
    at: usize,
}

impl Dump {
    fn u32(&mut self) -> u32 {
        let b = &self.bytes[self.at..self.at + 4];
        self.at += 4;
        u32::from_le_bytes([b[0], b[1], b[2], b[3]])
    }

    fn f32(&mut self) -> f32 {
        f32::from_bits(self.u32())
    }

    fn pose(&mut self, bones: usize) -> Vec<Trs> {
        (0..bones)
            .map(|_| {
                let v: Vec<f32> = (0..10).map(|_| self.f32()).collect();
                Trs { rotation: Quat::from_xyzw(v[0], v[1], v[2], v[3]), translation: Vec3::new(v[4], v[5], v[6]), scale: Vec3::new(v[7], v[8], v[9]) }
            })
            .collect()
    }

    fn floats(&mut self, n: usize) -> Vec<f32> {
        (0..n).map(|_| self.f32()).collect()
    }
}

fn particle_state(sim: &Bonamik) -> Vec<f32> {
    sim.core().f3778.f0.iter().flat_map(|p| p.f0.iter().chain(&p.f40).chain(&p.f20).copied().collect::<Vec<f32>>()).collect()
}

/// (number of differing values, largest absolute difference, index of the first differing value)
fn compare(a: &[f32], b: &[f32]) -> (usize, f64, Option<usize>) {
    let mut out = (0, 0.0f64, None);
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        // -0 and +0 are different bit patterns and are counted
        if x.to_bits() != y.to_bits() {
            out.0 += 1;
            out.1 = out.1.max((*x as f64 - *y as f64).abs());
            out.2 = out.2.or(Some(i));
        }
    }
    out
}

fn pose_floats(pose: &[Trs]) -> Vec<f32> {
    pose.iter().flat_map(|t| [t.rotation.x, t.rotation.y, t.rotation.z, t.rotation.w, t.translation.x, t.translation.y, t.translation.z, t.scale.x, t.scale.y, t.scale.z]).collect()
}

#[test]
fn cross_check_against_the_js() {
    let Some(pack) = pack() else { return };
    let path = root().join("build/sora_tmp/bnm_crosscheck/idle_crosscheck.bin");
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("SKIPPED: no {} (write it with: node build/sora_tmp/bnm_crosscheck/dump.js)", path.display());
        return;
    };
    assert_eq!(&bytes[0..4], b"BNMX");
    let mut d = Dump { bytes, at: 4 };
    let (bones, particles, frames) = (d.u32() as usize, d.u32() as usize, d.u32() as usize);
    assert_eq!(bones, pack.skeleton.len());
    // the reference pose: this crate's double-precision composition against anim.js's
    let js_reference = d.pose(bones);
    let reference = bonamik::reference_pose(&pack.skeleton);
    let r = compare(&pose_floats(&reference), &pose_floats(&js_reference));
    println!("cross-check: reference pose: {} of {} values differ from anim.js (largest difference {:e})", r.0, bones * 10, r.1);
    assert_eq!(r.0, 0, "the reference pose differs from anim.js's");

    // section 1: zero jitter. Both sides get the SAME input poses (the JS's).
    let mut sim = Bonamik::with_reference_pose(data(&pack), &pack.skeleton, &js_reference, zero()).expect("Bonamik sets up");
    assert_eq!(sim.core().f3778.f0.len(), particles);
    let mut inputs = Vec::new();
    let (mut evaluation, mut first_state, mut first_pose) = (0usize, None, None);
    let (mut state_bad, mut state_worst, mut pose_bad, mut pose_worst, mut hash_bad, mut timing_bad) = (0usize, 0.0f64, 0usize, 0.0f64, 0usize, 0usize);
    for f in 0..frames {
        let (dt, did_eval, eval_dt, alpha, hash) = (d.f32(), d.u32() != 0, d.f32(), d.f32(), d.u32());
        let input = d.pose(bones);
        let js_out = d.pose(bones);
        let js_state = d.floats(particles * 10);
        let (out, info) = frame(&mut sim, dt, &input);
        if info.did_eval != did_eval || info.eval_dt.to_bits() != eval_dt.to_bits() || info.alpha.to_bits() != alpha.to_bits() {
            timing_bad += 1;
        }
        let s = compare(&particle_state(&sim), &js_state);
        if s.0 > 0 && first_state.is_none() {
            let i = s.2.expect("a difference");
            first_state = Some(format!("frame {f} (evaluation {evaluation}), particle {} ({}), value {} of (pos3 vel3 quat4)", i / 10, sim.core().f3778.f0[i / 10].name, i % 10));
        }
        state_bad += s.0;
        state_worst = state_worst.max(s.1);
        let p = compare(&pose_floats(&out), &pose_floats(&js_out));
        if p.0 > 0 && first_pose.is_none() {
            let i = p.2.expect("a difference");
            first_pose = Some(format!("frame {f}, bone {} ({}), value {} of (quat4 pos3 scale3)", i / 10, pack.skeleton.bones[i / 10].name, i % 10));
        }
        pose_bad += p.0;
        pose_worst = pose_worst.max(p.1);
        if sim.state_hash() != hash as u64 {
            hash_bad += 1;
        }
        if did_eval {
            evaluation += 1;
        }
        inputs.push(input);
    }
    println!(
        "cross-check, zero jitter: IdleN clip, {frames} frames at 60 fps, {evaluation} evaluations, {particles} particles: particle states (pos, vel, quat): {state_bad} float32 values differ (largest difference {state_worst:e}), first: {}; shown poses: {pose_bad} values differ (largest {pose_worst:e}), first: {}; state hashes differing on {hash_bad} frames; timer / alpha differing on {timing_bad} frames",
        first_state.as_deref().unwrap_or("none"),
        first_pose.as_deref().unwrap_or("none")
    );

    // section 2: the JS's seeded xorshift stream (the aero jitter is live)
    let mut sim = Bonamik::with_reference_pose(data(&pack), &pack.skeleton, &js_reference, seeded()).expect("Bonamik sets up");
    let (mut seeded_state_bad, mut seeded_worst, mut seeded_pose_bad, mut seeded_first, mut seeded_hash_bad) = (0usize, 0.0f64, 0usize, None, 0usize);
    for (f, input) in inputs.iter().enumerate() {
        let (did_eval, hash) = (d.u32() != 0, d.u32());
        let js_out = d.pose(bones);
        let js_state = d.floats(particles * 10);
        let (out, info) = frame(&mut sim, 1.0 / 60.0, input);
        assert_eq!(info.did_eval, did_eval, "frame {f}");
        let s = compare(&particle_state(&sim), &js_state);
        if s.0 > 0 && seeded_first.is_none() {
            let i = s.2.expect("a difference");
            seeded_first = Some(format!("frame {f}, particle {} ({}), value {}", i / 10, sim.core().f3778.f0[i / 10].name, i % 10));
        }
        seeded_state_bad += s.0;
        seeded_worst = seeded_worst.max(s.1);
        seeded_pose_bad += compare(&pose_floats(&out), &pose_floats(&js_out)).0;
        if sim.state_hash() != hash as u64 {
            seeded_hash_bad += 1;
        }
    }
    assert_eq!(d.at, d.bytes.len(), "the dump was read to its end");
    println!(
        "cross-check, seeded stream: particle states: {seeded_state_bad} values differ (largest {seeded_worst:e}), first: {}; shown poses: {seeded_pose_bad} values differ; state hashes differing on {seeded_hash_bad} frames",
        seeded_first.as_deref().unwrap_or("none")
    );
    assert_eq!(timing_bad, 0, "timer / alpha");
    assert_eq!((state_bad, hash_bad), (0, 0), "zero jitter: particle states differ from the JS evaluator's");
    assert_eq!(pose_bad, 0, "zero jitter: shown poses differ from the JS evaluator's");
    assert_eq!((seeded_state_bad, seeded_hash_bad, seeded_pose_bad), (0, 0, 0), "seeded stream: states differ from the JS evaluator's");
}
