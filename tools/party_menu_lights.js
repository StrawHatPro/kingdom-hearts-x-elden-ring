#!/usr/bin/env node
// tools/party_menu_lights.js (party I2): the friends' CAMP-MENU PICTURE RIG -> build/sora_pack/party/n_ex00x/data/ui_actor.json
// (kh-app menu_model.rs lights Donald's / Goofy's menu copies with it, in the format of PACK/menu/menu.json "lights").
//
//   node tools/party_menu_lights.js
//
// Read from /Game/UI/Blueprints/CampTop/n_ex00x_UIActor (decoded package): its SceneCaptureComponent2D (location,
// rotation, FOV, post-process overrides) and its two SpotLightComponent templates (SpotLight = the key light,
// SpotLight1 = the fill). Members a template does not serialize take the stock UE 4.17 class defaults (the same rule
// and values as tools/sora_material_pack.js uses for p_ex001_UIActor): Intensity 5000, OuterConeAngle 44,
// InnerConeAngle 0, AttenuationRadius 1000, MinRoughness 0.08, white. Local build output, never distributed.
'use strict';
const fs = require('fs'), path = require('path');
const ROOT = path.resolve(__dirname, '..');
const CONTENT = path.join(ROOT, 'assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content');
const OUT = path.join(ROOT, 'build', 'sora_pack', 'party');
const DEFAULTS = { Intensity: 5000, OuterConeAngle: 44, InnerConeAngle: 0, AttenuationRadius: 1000, MinRoughness: 0.08 };

function conv(v) {
  if (Array.isArray(v)) return props(v);
  if (v && typeof v === 'object' && Array.isArray(v.props)) return props(v.props);
  return v;
}
function props(list) { const o = {}; for (const p of list || []) if (p && 'name' in p) o[p.name] = conv(p.value); return o; }
function vec(v) { return v ? { x: v.x, y: v.y, z: v.z } : { x: 0, y: 0, z: 0 }; }
function rot(v) { return v ? { pitch: v.pitch, yaw: v.yaw, roll: v.roll } : { pitch: 0, yaw: 0, roll: 0 }; }

for (const chara of ['n_ex001', 'n_ex002']) {
  const rel = `UI/Blueprints/CampTop/${chara}_UIActor`;
  const pkg = JSON.parse(fs.readFileSync(path.join(CONTENT, rel + '.uasset.json'), 'utf8'));
  const byName = {};
  for (const e of pkg.exports || []) byName[e.objectName || e.name] = e;
  const exp = name => {
    const e = (pkg.exports || []).find(x => (x.objectName || x.name) === name);
    if (!e) return null;
    return props((e.data && e.data.props) || []);
  };
  const lights = [];
  for (const name of ['SpotLight1', 'SpotLight']) {
    const p = exp(`${name}_GEN_VARIABLE`);
    if (!p) throw new Error(`${chara}: no ${name}_GEN_VARIABLE`);
    lights.push({
      name,
      location_cm: vec(p.RelativeLocation),
      rotation_deg: rot(p.RelativeRotation),
      intensity: p.Intensity ?? DEFAULTS.Intensity,
      inner_cone_deg: p.InnerConeAngle ?? DEFAULTS.InnerConeAngle,
      outer_cone_deg: p.OuterConeAngle ?? DEFAULTS.OuterConeAngle,
      attenuation_radius_cm: p.AttenuationRadius ?? DEFAULTS.AttenuationRadius,
      min_roughness: p.MinRoughness ?? DEFAULTS.MinRoughness,
      cast_shadows: p.CastShadows ?? true,
      color: [1, 1, 1],
    });
  }
  const cap = exp('SceneCaptureComponent2D') || {};
  const pp = cap.PostProcessSettings || {};
  const out = {
    format: 'kh3-party-ui-actor/1',
    source: `/Game/${rel}, written by tools/party_menu_lights.js (party I2)`,
    capture: { location_cm: vec(cap.RelativeLocation), rotation_deg: rot(cap.RelativeRotation), fov_deg: cap.FOVAngle ?? 90 },
    lights,
    post_process: { AmbientCubemapIntensity: pp.AmbientCubemapIntensity, FilmContrast: pp.FilmContrast, FilmHealAmount: pp.FilmHealAmount, FilmDynamicRange: pp.FilmDynamicRange },
    defaults_note: 'stock 4.17 SpotLightComponent defaults for members the template does not serialize: ' + JSON.stringify(DEFAULTS),
  };
  const dir = path.join(OUT, chara, 'data');
  if (!fs.existsSync(dir)) throw new Error(`${dir}: no party pack`);
  fs.writeFileSync(path.join(dir, 'ui_actor.json'), JSON.stringify(out, null, 1) + '\n');
  console.log(`${chara}: data/ui_actor.json capture ${JSON.stringify(out.capture.location_cm)} fov ${out.capture.fov_deg}, lights ${lights.map(l => `${l.name} ${l.intensity} @ ${Math.round(l.location_cm.x)},${Math.round(l.location_cm.y)},${Math.round(l.location_cm.z)}`).join('; ')}`);
}
