//! (ZETTA) The event effects of Donald's Zettaflare (KH3 kg702b, packed in build/sora_tmp/party/ZETTA/pack/vfx): the glows
//! on his bones (`SQEX_BoneAttach`, fed by `World::set_bones`), the sparks on the circle's ring and the dust thrown outward
//! (the stock `LocationPrimitiveCylinder`), the dust's unbaked Square Enix alpha (0.1 .. 0.5, not 1).

use glam::{Quat, Vec3};
use kh_vfx::{Camera, DrawList, EffectDef, SpawnOptions, World, Xform};
use std::collections::HashMap;
use std::sync::Arc;

const DT: f32 = 1.0 / 60.0;

fn zetta_effect(name: &str) -> Option<EffectDef> {
    let file = format!("build/sora_tmp/party/ZETTA/pack/vfx/effects/{name}.json");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().map(|a| a.join(&file)).find(|p| p.is_file())?;
    EffectDef::parse(&std::fs::read_to_string(path).ok()?).ok()
}

fn quads(w: &World) -> DrawList {
    let mut out = DrawList::default();
    w.draw(1.0, &Camera::DEFAULT, &mut out);
    out
}

fn centre(c: &[Vec3; 4]) -> Vec3 {
    (c[0] + c[1] + c[2] + c[3]) * 0.25
}

#[test]
fn the_aura_glows_sit_on_his_bones() {
    let name = "vd_kg702_86_zfr_03";
    let Some(def) = zetta_effect(name) else {
        eprintln!("(no ZETTA pack effect {name}: skipped)");
        return;
    };
    let mut w = World::new();
    w.register(def);
    let id = w.spawn(name, Xform::default(), SpawnOptions { seed: Some(1), ..SpawnOptions::default() }).unwrap();
    assert!(w.wants_bones(id));
    // his hat 160 cm up, his hips 70 cm up, far from the effect's origin; the other bones missing
    let bones: HashMap<String, (Vec3, Quat)> = [("bousi", Vec3::new(500.0, 0.0, 160.0)), ("koshi", Vec3::new(500.0, 0.0, 70.0))].into_iter().map(|(n, p)| (n.to_string(), (p, Quat::IDENTITY))).collect();
    w.set_bones(id, Some(Arc::new(bones)));
    for _ in 0..6 {
        w.step(DT);
    }
    let out = quads(&w);
    let at: Vec<Vec3> = out.quads.iter().map(|q| centre(&q.corners)).collect();
    // (ZETTA round 5) each glow is drawn 50 cm BEHIND its bone along the view (SQEX_DepthOffset -50, the event's own value
    // for this aura in kg702c: his body hides it, the glow shows round him): on the camera's ray through the bone, past it
    let cam = Camera::DEFAULT.position;
    let on_bone = |c: Vec3, bone: Vec3, tol: f32| {
        let dir = (bone - cam).normalize();
        let d = c - bone;
        let along = d.dot(dir);
        (d - dir * along).length() < tol && along > 30.0 && along < 75.0
    };
    assert!(at.iter().any(|c| on_bone(*c, Vec3::new(500.0, 10.0, 160.0), 2.0)), "a glow on the hat (its offset y 10), behind it: {at:?}");
    assert!(at.iter().any(|c| on_bone(*c, Vec3::new(500.0, 0.0, 70.0), 12.0)), "a glow on the hips, behind them: {at:?}");
}

#[test]
fn the_circles_sparks_rise_from_its_ring_and_its_dust_is_light() {
    let name = "vd_kg702_86_zfr_01";
    let Some(def) = zetta_effect(name) else {
        eprintln!("(no ZETTA pack effect {name}: skipped)");
        return;
    };
    let dust = def.emitters.iter().find(|e| e.name == "smk_rate").expect("smk_rate");
    assert!(dust.unknown.iter().all(|u| !u.contains("LocationPrimitive")), "the cylinder is read: {:?}", dust.unknown);
    let mut w = World::new();
    w.register(def);
    w.spawn(name, Xform::default(), SpawnOptions { seed: Some(2), ..SpawnOptions::default() }).unwrap();
    let mut far = 0;
    let mut seen = 0;
    // the circle's rings are drawn by 2.1 s (its controls start their children then)
    for _ in 0..170 {
        w.step(DT);
    }
    {
        for q in quads(&w).quads {
            let c = centre(&q.corners);
            seen += 1;
            if Vec3::new(c.x, c.y, 0.0).length() > 200.0 {
                far += 1;
            }
            // a particle colour's alpha above 0.5 x its fade curve's 1 would be the default alpha 1 of an unread distribution
            assert!(q.color[3] <= 1.0 + 1e-4);
        }
    }
    assert!(seen > 0 && far * 4 > seen, "most of the circle's sprites stand out on its ring / dust ring, not at its centre: {far} of {seen}");
}

