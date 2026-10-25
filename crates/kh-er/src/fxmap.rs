//! (fx) The pure numbers of Sora's SOUND AND EFFECTS in the Elden Ring host: sheet
//! `sheets/port/sora-er-fx.json` (rows are host choices, as sora-er-host's): Elden Ring's ground
//! material under the player -> KH3's footstep surface, the output level, the effects' scale
//! into Elden Ring's scene. No game access here: unit-tested.
//! State note: build/sora_tmp/er/IMPL_ER_FX_STATE.md.

/// Elden Ring's `hit_material` is `class * 100 + material` (the SDK: class 1 = never slippery,
/// 4 / 5 = always sliding); the material is the HitMtrlParam row.
pub const MATERIAL_MODULUS: i32 = 100;

#[derive(Clone, Debug, PartialEq)]
pub struct FxParams {
    /// (Elden Ring material, KH3 `SurfaceType`).
    pub surfaces: Vec<(i32, u32)>,
    /// The surface of a material without a row (0 = FsTable_Sora's default: the rock bank).
    pub surface_default: u32,
    /// Sora's output level under Elden Ring's own audio, dB (`snd.vol` overrides).
    pub volume_db: f32,
    /// (IMPL_ER14) `sound.ui_gain_db`: the menus' own sounds are played this much louder than authored (`snd.ui_gain` overrides).
    pub ui_gain_db: f32,
    /// What KH3's scene-linear 1.0 of an EFFECT becomes in Elden Ring's HDR scene target
    /// (`vfx.hdr` overrides).
    pub vfx_hdr_scale: f32,
    /// (IMPL_ER75) `vfx.slash_follow`: the melee swing arcs follow Sora (kh-host `EffectHost::slash_follow`).
    pub slash_follow: bool,
    /// (IMPL_ER39) The light pass: the effects' particle lights and the hero light (rows `fx.light*`, `hero.*`;
    /// control keys `fx.light`, `r.hero_light` override the switches while the game runs).
    pub light: kh_er_render::pointlight::LightSettings,
}

/// The built-in value of sheet row `sound.ui_gain_db` (the title mode has no sheet read yet).
pub const UI_GAIN_DB: f32 = 12.0;

impl Default for FxParams {
    fn default() -> FxParams {
        FxParams { surfaces: Vec::new(), surface_default: 0, volume_db: -6.0, ui_gain_db: UI_GAIN_DB, vfx_hdr_scale: 1.0, slash_follow: true, light: kh_er_render::pointlight::LightSettings::default() }
    }
}

impl FxParams {
    /// The sheet's rows over the defaults; the second value lists rows that were not understood.
    /// A row whose value is null is UNCHECKED: it maps nothing (the default surface is used).
    pub fn from_sheet(text: &str) -> Result<(FxParams, Vec<String>), String> {
        let json: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let rows = json.get("rows").and_then(|r| r.as_array()).ok_or("no rows")?;
        let mut p = FxParams::default();
        let mut problems = Vec::new();
        for row in rows {
            let (Some(id), Some(value)) = (row.get(0).and_then(|v| v.as_str()), row.get(1)) else { continue };
            if value.is_null() {
                continue;
            }
            let number = value.as_f64().map(|v| v as f32).filter(|v| v.is_finite());
            match (id, number) {
                ("surface.default", Some(v)) if v >= 0.0 => p.surface_default = v as u32,
                ("sound.volume_db", Some(v)) => p.volume_db = v,
                ("sound.ui_gain_db", Some(v)) if (-24.0..=24.0).contains(&v) => p.ui_gain_db = v,
                ("vfx.hdr_scale", Some(v)) if v > 0.0 => p.vfx_hdr_scale = v,
                ("vfx.slash_follow", Some(v)) => p.slash_follow = v != 0.0,
                // (IMPL_ER39) the light pass
                ("fx.light", Some(v)) => p.light.fx = v != 0.0,
                ("fx.light_gain", Some(v)) if v >= 0.0 => p.light.fx_gain = v,
                ("fx.light_max", Some(v)) if (0.0..=16.0).contains(&v) => p.light.fx_max = v as usize,
                ("fx.light_on_sora", Some(v)) if (0.0..=1.0).contains(&v) => p.light.fx_on_sora = v,
                ("light.occlusion_steps", Some(v)) if (0.0..=16.0).contains(&v) => p.light.occlusion_steps = v as u32,
                ("hero.light", Some(v)) if (0.0..=2.0).contains(&v) => p.light.hero = v as u8,
                ("hero.dark_on", Some(v)) if v >= 0.0 => p.light.hero_dark_on = v,
                ("hero.dark_off", Some(v)) if v >= 0.0 => p.light.hero_dark_off = v,
                ("hero.fade_s", Some(v)) if v >= 0.0 => p.light.hero_fade = v,
                ("hero.radius_cm", Some(v)) if v > 0.0 => p.light.hero_radius = v,
                ("hero.height_cm", Some(v)) => p.light.hero_height = v,
                ("hero.intensity", Some(v)) if v >= 0.0 => p.light.hero_intensity = v,
                ("hero.color_r", Some(v)) if v >= 0.0 => p.light.hero_color.x = v,
                ("hero.color_g", Some(v)) if v >= 0.0 => p.light.hero_color.y = v,
                ("hero.color_b", Some(v)) if v >= 0.0 => p.light.hero_color.z = v,
                ("hero.exponent", Some(v)) if v > 0.0 => p.light.hero_exponent = v,
                ("hero.wrap", Some(v)) if (0.0..=1.0).contains(&v) => p.light.hero_wrap = v,
                ("hero.on_sora", Some(v)) if (0.0..=1.0).contains(&v) => p.light.hero_on_sora = v,
                // (IMPL_ER44) the lights on Sora's own materials
                ("hero.sora_light", Some(v)) if (0.0..=4.0).contains(&v) => p.light.hero_sora = v,
                ("hero.sora_ambient", Some(v)) if (0.0..=1.0).contains(&v) => p.light.hero_sora_ambient = v,
                ("hero.sora_height_cm", Some(v)) => p.light.hero_sora_height = v,
                ("hero.sora_toward_cm", Some(v)) if v >= 0.0 => p.light.hero_sora_toward = v,
                ("fx.sora_light", Some(v)) if (0.0..=4.0).contains(&v) => p.light.fx_sora = v,
                ("fx.sora_ambient", Some(v)) if (0.0..=1.0).contains(&v) => p.light.fx_sora_ambient = v,
                // (IMPL_ER44 part 2) the Keyblade light
                ("hero.keyblade", Some(v)) => p.light.hero_keyblade = v != 0.0,
                ("hero.blade_cm", Some(v)) if (-20.0..=90.0).contains(&v) => p.light.hero_blade = v,
                ("hero.shadow", Some(v)) if (0.0..=1.0).contains(&v) => p.light.hero_shadow = v,
                ("hero.sparkle_s", Some(v)) if v >= 0.1 => p.light.hero_sparkle = v,
                // (IMPL_ER53) the steady Keyblade glow
                ("hero.glow", Some(v)) => p.light.hero_glow = v != 0.0,
                // (IMPL_ER56) the ambient effects' lights on Sora
                ("fx.ambient_sora", Some(v)) if (0.0..=1.0).contains(&v) => p.light.fx_ambient_sora = v,
                // (IMPL_ER64) the hero light on Sora as a character light; the world's lights on him
                ("hero.sora_placed", Some(v)) => p.light.hero_sora_placed = v != 0.0,
                ("world.light", Some(v)) => p.light.world = v != 0.0,
                ("world.max_level", Some(v)) if v >= 0.0 => p.light.world_max_level = v,
                ("world.gain", Some(v)) if (0.0..=8.0).contains(&v) => p.light.world_gain = v,
                ("world.min_contrast", Some(v)) if (0.0..=1.0).contains(&v) => p.light.world_min_contrast = v,
                ("hero.glow_hold_s", Some(v)) if (0.0..=2.0).contains(&v) => p.light.hero_glow_hold = v,
                (_, Some(v)) if id.starts_with("surface.er.") && v >= 0.0 => match id["surface.er.".len()..].parse::<i32>() {
                    Ok(material) => p.surfaces.push((material, v as u32)),
                    Err(_) => problems.push(id.to_string()),
                },
                _ => problems.push(id.to_string()),
            }
        }
        Ok((p, problems))
    }

    /// KH3's surface for the game's `hit_material` (the whole number first, then its material
    /// part); `None`: no row (the caller uses `surface_default` and may log the number once).
    pub fn surface(&self, hit_material: i32) -> Option<u32> {
        let find = |m: i32| self.surfaces.iter().find(|(k, _)| *k == m).map(|(_, s)| *s);
        find(hit_material).or_else(|| find(hit_material.rem_euclid(MATERIAL_MODULUS)))
    }

    /// (ER party S5) A PARTY MEMBER'S FOOTSTEP SURFACE: Elden Ring's ground material under HIM when it is known and has
    /// a row; otherwise the leader's surface (the material under the hidden player body, which stands where the leader
    /// is: plan, "the leader's material when the material under an AI ally is not known"). `snd.surface` forces both.
    pub fn member_surface(&self, own_material: Option<i32>, leader_surface: u32) -> u32 {
        own_material.filter(|m| *m >= 0).and_then(|m| self.surface(m)).unwrap_or(leader_surface)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sheet_maps_materials_and_null_rows_map_nothing() {
        let text = r#"{"rows":[["surface.default",0],["surface.er.3",4],["surface.er.13",null],["surface.er.21",33],["sound.volume_db",-9],["vfx.hdr_scale",0.5],["surface.er.x",1],["other",2]]}"#;
        let (p, problems) = FxParams::from_sheet(text).unwrap();
        assert_eq!(problems, ["surface.er.x", "other"]);
        assert_eq!((p.surface(3), p.surface(103), p.surface(521), p.surface(13), p.surface(-1)), (Some(4), Some(4), Some(33), None, None));
        assert_eq!((p.volume_db, p.vfx_hdr_scale, p.surface_default), (-9.0, 0.5, 0));
        // (IMPL_ER39) the light rows
        let (l, problems) = FxParams::from_sheet(r#"{"rows":[["fx.light",0],["hero.light",2],["hero.intensity",0.5],["hero.color_b",0.9],["hero.dark_on",null],["fx.light_max",40],["hero.sora_light",0.5],["fx.sora_ambient",0.2],["hero.sora_toward_cm",-1]]}"#).unwrap();
        assert_eq!(problems, ["fx.light_max", "hero.sora_toward_cm"]);
        // (IMPL_ER44) the lights on Sora
        assert_eq!((l.light.hero_sora, l.light.fx_sora_ambient, l.light.fx_sora), (0.5, 0.2, 1.0));
        let (k, problems) = FxParams::from_sheet(r#"{"rows":[["hero.keyblade",0],["hero.blade_cm",60],["hero.shadow",0.5],["hero.sparkle_s",0.01]]}"#).unwrap();
        assert_eq!(problems, ["hero.sparkle_s"]);
        assert_eq!((k.light.hero_keyblade, k.light.hero_blade, k.light.hero_shadow), (false, 60.0, 0.5));
        assert_eq!((l.light.fx, l.light.hero, l.light.hero_intensity, l.light.hero_color.z), (false, 2, 0.5, 0.9));
        assert_eq!(l.light.hero_dark_on, kh_er_render::pointlight::LightSettings::default().hero_dark_on);
    }

    #[test]
    fn the_real_sheet_reads_without_problems() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sheets/port/sora-er-fx.json");
        let Ok(text) = std::fs::read_to_string(path) else {
            eprintln!("SKIPPED: no {path}");
            return;
        };
        let (p, problems) = FxParams::from_sheet(&text).unwrap();
        assert!(problems.is_empty(), "{problems:?}");
        assert!(p.surfaces.len() >= 5 && p.volume_db <= 0.0);
        // Sand is sand, water is one of KH3's water surfaces.
        assert_eq!(p.surface(3), Some(4));
        assert!(matches!(p.surface(21), Some(32..=37)));
        // (IMPL_ER53) the steady glow and the stronger, warmer hero light, as the defaults
        let d = kh_er_render::pointlight::LightSettings::default();
        assert!(p.light.hero_glow && (p.light.hero_glow_hold - 0.25).abs() < 1e-6);
        assert_eq!((p.light.hero_intensity, p.light.hero_radius, p.light.hero_color, p.light.hero_sora_ambient), (d.hero_intensity, d.hero_radius, d.hero_color, d.hero_sora_ambient));
        let (g, problems) = FxParams::from_sheet(r#"{"rows":[["hero.glow",0],["hero.glow_hold_s",3]]}"#).unwrap();
        assert_eq!(problems, ["hero.glow_hold_s"]);
        assert!(!g.light.hero_glow);
    }

    /// (ER party S5) a member's footsteps: his own ground when it is known and mapped, else the leader's
    #[test]
    fn a_members_surface_is_his_own_ground_else_the_leaders() {
        let (p, _) = FxParams::from_sheet(r#"{"rows":[["surface.er.3",4],["surface.er.21",33]]}"#).unwrap();
        // known and mapped: his own (sand), also through the class digit (103 = class 1, material 3)
        assert_eq!((p.member_surface(Some(3), 10), p.member_surface(Some(103), 10)), (4, 4));
        // unknown (no reading, or the game's -1), or a material without a row: the leader's
        assert_eq!((p.member_surface(None, 10), p.member_surface(Some(-1), 10), p.member_surface(Some(77), 10)), (10, 10, 10));
    }
}
