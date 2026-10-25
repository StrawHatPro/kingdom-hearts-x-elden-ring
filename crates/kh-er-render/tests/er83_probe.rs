//! (ER83) Probes of the hands of KH1 Sora (retargeted KH3 clips) against KH3 Sora's own. Ignored tests, printing only.
use std::path::PathBuf;

use glam::Vec3;
use kh_anim::pose::component_space;
use kh_anim::{Animator, Pack, Settings, Trs};

fn pack_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack")
}

fn rel(p: Vec3, frame: &Trs) -> Vec3 {
    frame.rotation.inverse() * (p - frame.translation)
}

#[test]
#[ignore]
fn probe_hands() {
    let pack = pack_dir();
    let a = Animator::new(Pack::load(&pack).unwrap(), Settings::default()).unwrap();
    let sk = a.skeleton().clone();
    let r = kh_er_render::outfit::retarget_for(&pack, "kh1", &sk).unwrap().unwrap();
    let clips: Vec<String> = std::env::var("CLIPS").unwrap_or("1015,1018,1004,1014,1028".into()).split(',').map(str::to_string).collect();
    let fingers = ["te", "yubiA1", "yubiA2", "yubiA3", "yubiB1", "yubiB2", "yubiB3", "yubiC1", "yubiC2", "yubiC3", "yubiD1", "yubiD3", "yubiE1", "yubiE3", "buki"];
    for clip in &clips {
        let Some(info) = a.clip_info(clip) else { continue };
        let len = info.length;
        for t in [0.0, len * 0.5] {
            let local = a.sample(clip, t);
            let comp = component_space(&sk, &local);
            let (_, out) = r.pose_full(&local, &comp);
            let f3 = |n: &str| sk.index_of(n);
            let f1 = |n: &str| r.index_of(n);
            let list: [(&str, &Vec<Trs>, &dyn Fn(&str) -> Option<usize>); 2] = [("KH3", &comp, &f3), ("KH1", &out, &f1)];
            for (who, pose, idx) in list {
                let buki = pose[idx("R_buki").unwrap()];
                let koshi = pose[idx("koshi").unwrap()];
                for side in ["R", "L"] {
                    let s: Vec<String> = fingers.iter().filter_map(|f| { let i = idx(&format!("{side}_{f}"))?; let p = rel(pose[i].translation, &buki); Some(format!("{f}{:.1?}", p.to_array())) }).collect();
                    println!("{clip} t{t:.2} {who} {side} in R_buki: {}", s.join(" "));
                }
                let lte = pose[idx("L_te").unwrap()].translation;
                println!("{clip} t{t:.2} {who} L_te in koshi {:.1?}  R_te in koshi {:.1?}", rel(lte, &koshi).to_array(), rel(pose[idx("R_te").unwrap()].translation, &koshi).to_array());
            }
        }
    }
}

/// (ER83) EVERY CLIP of the pack: where KH3 Sora's hands touch his Keyblade's grip (off-hand) or his body, and KH1's
/// retargeted hands before / after the contact IK. `CLIPS=a,b` limits it; `OUT=file` writes the table (tsv).
#[test]
#[ignore]
fn scan_contacts() {
    let pack = pack_dir();
    let a = Animator::new(Pack::load(&pack).unwrap(), Settings::default()).unwrap();
    let sk = a.skeleton().clone();
    let r = kh_er_render::outfit::retarget_for(&pack, "kh1", &sk).unwrap().unwrap();
    let ik = r.hand_ik.clone().expect("kh1 has the hand IK");
    let mut names: Vec<String> = std::fs::read_dir(pack.join("anims")).unwrap().filter_map(|e| e.ok()?.file_name().to_str()?.strip_suffix(".glb").map(str::to_string)).collect();
    names.sort();
    if let Ok(list) = std::env::var("CLIPS") {
        names = list.split(',').map(str::to_string).collect();
    }
    let mut table = String::from("clip\tside\tkind\tframes\tt_from\tt_to\this_d_min\this_d_max\tbefore_mean\tbefore_max\tafter_mean\tafter_max\tgap_his\tgap_kh1_before\tgap_kh1_after\n");
    let side_name = ["R", "L"];
    for clip in &names {
        let Some(info) = a.clip_info(clip) else { continue };
        let n = ((info.length * 30.0).round() as usize).max(1);
        // per side, per kind (0 weapon, 1 body): (frames, t0, t1, d_min, d_max, before sum, before max, after sum, after max, gaps)
        let mut acc = [[(0usize, f32::MAX, 0f32, f32::MAX, 0f32, 0f32, 0f32, 0f32, 0f32, 0f32, 0f32, 0f32); 2]; 2];
        for f in 0..=n {
            let t = info.length * f as f32 / n as f32;
            let local = a.sample(clip, t);
            let comp = component_space(&sk, &local);
            kh_er_render::handik::set_enabled(false);
            let (_, before) = r.pose_full(&local, &comp);
            kh_er_render::handik::set_enabled(true);
            let (_, after) = r.pose_full(&local, &comp);
            let mb = ik.measure(&comp, &before);
            let ma = ik.measure(&comp, &after);
            for side in 0..2 {
                let c = mb[side].0;
                for (kind, w, d) in [(0, c.weapon, c.weapon_d), (1, c.body, c.body_d)] {
                    if w < 0.99 {
                        continue;
                    }
                    let e = &mut acc[side][kind];
                    e.0 += 1;
                    e.1 = e.1.min(t);
                    e.2 = e.2.max(t);
                    e.3 = e.3.min(d);
                    e.4 = e.4.max(d);
                    let (b, af) = if kind == 0 { (mb[side].2, ma[side].2) } else { (mb[side].1, ma[side].1) };
                    // weapon: the palm's distance to the grip vs his; body: distance to the target
                    let (b, af) = if kind == 0 { ((b - d).abs(), (af - d).abs()) } else { (b, af) };
                    e.5 += b;
                    e.6 = e.6.max(b);
                    e.7 += af;
                    e.8 = e.8.max(af);
                    if kind == 1 {
                        e.9 += ik.body_gap_his(&comp, side);
                        e.10 += ik.body_gap(&before, side);
                        e.11 += ik.body_gap(&after, side);
                    } else {
                        e.9 += d;
                        e.10 += mb[side].2;
                        e.11 += ma[side].2;
                    }
                }
            }
        }
        for side in 0..2 {
            for kind in 0..2 {
                let e = acc[side][kind];
                if e.0 == 0 {
                    continue;
                }
                let k = e.0 as f32;
                let line = format!("{clip}\t{}\t{}\t{}\t{:.2}\t{:.2}\t{:.1}\t{:.1}\t{:.1}\t{:.1}\t{:.2}\t{:.2}\t{:.1}\t{:.1}\t{:.1}\n", side_name[side], ["grip", "body"][kind], e.0, e.1, e.2, e.3, e.4, e.5 / k, e.6, e.7 / k, e.8, e.9 / k, e.10 / k, e.11 / k);
                print!("{line}");
                table.push_str(&line);
            }
        }
    }
    if let Ok(out) = std::env::var("OUT") {
        std::fs::write(out, table).unwrap();
    }
}

/// (ER83) KH1's OWN clips (walk, run, run start, jump: its default): the contact weights the solver finds there, and how far
/// the main hand's finger copy turns its own fingers (degrees, largest over the clip).
#[test]
#[ignore]
fn own_clips_untouched() {
    let pack = pack_dir();
    let mut a = Animator::new(Pack::load(&pack).unwrap(), Settings::default()).unwrap();
    let sk = a.skeleton().clone();
    let dir = pack.join("outfits").join("kh1");
    let own = a.set_own_clips_except(Some(&dir), &kh_anim::outfit_clips::kh3_clips(kh_anim::outfit_clips::default_for("kh1")));
    let r = kh_er_render::outfit::retarget_for(&pack, "kh1", &sk).unwrap().unwrap();
    let ik = r.hand_ik.clone().unwrap();
    // as kh-er gives it: the pose is the outfit's own clip (share 0); KH_SHARE=1 shows what the solver would do without it
    kh_er_render::handik::set_kh3_share(std::env::var("KH_SHARE").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0));
    let fingers: Vec<usize> = ["A", "B", "C", "D", "E"].iter().flat_map(|f| (1..3).map(move |j| format!("R_yubi{f}{j}"))).filter_map(|n| r.index_of(&n)).collect();
    for clip in own.iter().filter(|c| !c.ends_with('M')) {
        let Some(info) = a.clip_info(clip) else { continue };
        let n = ((info.length * 30.0).round() as usize).max(1);
        let (mut wmax, mut turn, mut moved): (f32, f32, f32) = (0.0, 0.0, 0.0);
        let hands = [r.index_of("R_te").unwrap(), r.index_of("L_te").unwrap()];
        for f in 0..=n {
            let t = info.length * f as f32 / n as f32;
            let local = a.sample(clip, t);
            let comp = component_space(&sk, &local);
            kh_er_render::handik::set_enabled(false);
            let (_, off) = r.pose_full(&local, &comp);
            kh_er_render::handik::set_enabled(true);
            let (_, on) = r.pose_full(&local, &comp);
            let c = ik.contacts(&comp, &off);
            wmax = wmax.max(c[0].weight()).max(c[1].weight());
            for &k in &hands {
                moved = moved.max((off[k].translation - on[k].translation).length());
            }
            for &k in &fingers {
                let d = (off[k].rotation.inverse() * on[k].rotation).normalize();
                turn = turn.max(2.0 * d.w.abs().min(1.0).acos().to_degrees());
            }
        }
        println!("own {clip}: contact weight max {wmax:.2}, wrists moved up to {moved:.1} cm, main-hand fingers turned up to {turn:.1} deg");
    }
}

/// (ER83) The main hand's fist on the grip in the battle stance: KH1's finger joints against KH3's in the R_buki (grip)
/// frame (mean, cm) and the fingertips' distance from the grip's axis (mean, cm), before / after.
#[test]
#[ignore]
fn main_fist() {
    let pack = pack_dir();
    let a = Animator::new(Pack::load(&pack).unwrap(), Settings::default()).unwrap();
    let sk = a.skeleton().clone();
    let r = kh_er_render::outfit::retarget_for(&pack, "kh1", &sk).unwrap().unwrap();
    let mut plain = r.clone();
    plain.hand_ik = None;
    let joints: Vec<String> = ["B", "C", "D", "E"].iter().flat_map(|f| (2..4).map(move |j| format!("R_yubi{f}{j}"))).collect();
    let tips: Vec<String> = ["B", "C", "D", "E"].iter().map(|f| format!("R_yubi{f}3")).collect();
    for (clip, t) in [("1015", 1.0), ("1018", 0.5), ("1004", 3.0), ("1014", 0.4), ("1028", 0.1)] {
        let local = a.sample(clip, t);
        let his = component_space(&sk, &local);
        let in_grip = |pose: &[Trs], idx: &dyn Fn(&str) -> Option<usize>, n: &str| rel(pose[idx(n).unwrap()].translation, &pose[idx("R_buki").unwrap()]);
        let fh = |n: &str| sk.index_of(n);
        let fo = |n: &str| r.index_of(n);
        let mut line = format!("{clip}@{t}:");
        let his_tip: f32 = tips.iter().map(|n| { let p = in_grip(&his, &fh, n); (p.y * p.y + p.z * p.z).sqrt() }).sum::<f32>() / 4.0;
        line += &format!(" KH3 tips {his_tip:.2} cm from the axis;");
        for (name, rt) in [("before", &plain), ("after", &r)] {
            let (_, pose) = rt.pose_full(&local, &his);
            let off: f32 = joints.iter().map(|n| (in_grip(&pose, &fo, n) - in_grip(&his, &fh, n)).length()).sum::<f32>() / joints.len() as f32;
            let tip: f32 = tips.iter().map(|n| { let p = in_grip(&pose, &fo, n); (p.y * p.y + p.z * p.z).sqrt() }).sum::<f32>() / 4.0;
            line += &format!(" KH1 {name}: joints {off:.2} cm off his, tips {tip:.2} cm from the axis;");
        }
        println!("{line}");
    }
}
