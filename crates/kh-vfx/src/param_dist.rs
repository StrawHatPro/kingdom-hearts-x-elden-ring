//! (VFX-RING) The stock INSTANCE-PARAMETER distributions, `DistributionFloatParticleParameter` and
//! `DistributionVectorParticleParameter` (pack `"type": "parameter"`, its `parameter` object: name, param_mode, min_input,
//! max_input, min_output, max_output, constant). They cannot be baked (lookup table `RDO_Uninitialized`), so the module
//! asks the distribution object each time, and stock UE 4.17 `UDistributionFloatParameterBase::GetValue` /
//! `UDistributionVectorParameterBase::GetValue` read the component's instance parameter of that name and, when the
//! component has none, take `Constant`; then `ValueFromParam` maps it:
//! - `DPM_Direct`: as it is;
//! - `DPM_Normal`: clamped to [MinInput, MaxInput] and mapped linearly onto [MinOutput, MaxOutput] (gradient 0 when
//!   MaxInput <= MinInput: MinOutput);
//! - `DPM_Abs`: the absolute value, then as Normal.
//!
//! The port's hosts set no instance parameters, so the value is the unset one: Constant through the mapping. Before, such a
//! property read as absent (`Dist::parse` found no table and no top-level constant) and its module took its default: the
//! spawn rate of vs_abn_cld0's rain (`RainNumber`, Direct, 400 / s) was 0 and the rain never fell.
//! The pack's users (2026-11-05): RainNumber (vs_abn_cld0), charge_opacity (vp_ant_rsk_crg_00 / 01), Vis_Keyblade
//! (vm_thb_blt0, vt_grv_bom03, vm_thw_blt0 .. 3), Boost_Alpha (vw_so172_bst00), BemLength (vg_kg07_pow_bem2), Edge Color and
//! AuraA_size (ve_ex711_tor2), SqExParentVelocityParam (vs_abn_brn0, vn_ex002_mcl_run0: constant 0 either way).

use glam::Vec3;
use serde_json::Value;

use crate::dist::Dist;

/// `ValueFromParam` (stock, EDistributionParamMode).
fn value_from_param(mode: &str, x: f32, min_in: f32, max_in: f32, min_out: f32, max_out: f32) -> f32 {
    let x = match mode {
        "DPM_Direct" => return x,
        "DPM_Abs" => x.abs(),
        _ => x,
    };
    let gradient = if max_in <= min_in { 0.0 } else { (max_out - min_out) / (max_in - min_in) };
    // FMath::Clamp(x, min, max) = min(max(x, min), max)
    min_out + (x.max(min_in).min(max_in) - min_in) * gradient
}

/// The distribution of a pack `parameter` distribution object `d` (None when it is not one).
pub fn parse(d: &Value) -> Option<Dist> {
    let p = d.get("parameter").filter(|_| d.get("type").and_then(Value::as_str) == Some("parameter"))?;
    let f = |v: &Value| v.as_f64().map(|x| x as f32);
    let arr = |k: &str| -> Option<Vec<f32>> { p.get(k)?.as_array()?.iter().map(f).collect() };
    let num = |k: &str, def: f32| p.get(k).and_then(f).unwrap_or(def);
    match p.get("constant") {
        Some(c) if c.is_number() => {
            let mode = p.get("param_mode").and_then(Value::as_str).unwrap_or("DPM_Normal");
            Some(Dist::constant(value_from_param(mode, f(c)?, num("min_input", 0.0), num("max_input", 1.0), num("min_output", 0.0), num("max_output", 1.0))))
        }
        Some(c) if c.is_array() => {
            let c: Vec<f32> = c.as_array()?.iter().map(f).collect::<Option<_>>()?;
            let modes: Vec<&str> = match p.get("param_mode") {
                Some(Value::Array(a)) => a.iter().map(|m| m.as_str().unwrap_or("DPM_Normal")).collect(),
                Some(Value::String(m)) => vec![m.as_str(); 3],
                _ => vec!["DPM_Normal"; 3],
            };
            let (min_in, max_in, min_out, max_out) = (arr("min_input")?, arr("max_input")?, arr("min_output")?, arr("max_output")?);
            let axis = |k: usize| -> Option<f32> { Some(value_from_param(modes.get(k).copied().unwrap_or("DPM_Normal"), *c.get(k)?, *min_in.get(k)?, *max_in.get(k)?, *min_out.get(k)?, *max_out.get(k)?)) };
            Some(Dist::constant_vec(Vec3::new(axis(0)?, axis(1)?, axis(2)?)))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;
    use serde_json::json;

    #[test]
    fn an_unset_parameter_is_its_constant_through_the_mapping() {
        let mut rng = Rng::new(1);
        let rain = json!({"class": "DistributionFloatParticleParameter", "type": "parameter", "parameter": {"name": "RainNumber", "param_mode": "DPM_Direct", "min_input": 0, "max_input": 1, "min_output": 0, "max_output": 1, "constant": 400}});
        assert_eq!(parse(&rain).map(|d| d.float(0.0, &mut rng)), Some(400.0), "Direct: as it is");
        let bem = json!({"type": "parameter", "parameter": {"param_mode": "DPM_Normal", "min_input": 0, "max_input": 10, "min_output": 0, "max_output": 5, "constant": 2}});
        assert_eq!(parse(&bem).map(|d| d.float(0.0, &mut rng)), Some(1.0), "Normal: 2 of 0..10 onto 0..5");
        let clamp = json!({"type": "parameter", "parameter": {"param_mode": "DPM_Abs", "min_input": 0, "max_input": 1, "min_output": 0, "max_output": 4, "constant": -3}});
        assert_eq!(parse(&clamp).map(|d| d.float(0.0, &mut rng)), Some(4.0), "Abs then clamped to the input range");
        let flat = json!({"type": "parameter", "parameter": {"param_mode": "DPM_Normal", "min_input": 0, "max_input": 0, "min_output": 7, "max_output": 9, "constant": 5}});
        assert_eq!(parse(&flat).map(|d| d.float(0.0, &mut rng)), Some(7.0), "no input range: MinOutput");
    }

    #[test]
    fn a_vector_parameter_maps_each_axis_with_its_own_mode() {
        let mut rng = Rng::new(1);
        let v = json!({"type": "parameter", "parameter": {"name": "BemLength", "param_mode": ["DPM_Normal", "DPM_Direct", "DPM_Normal"], "min_input": [0, 0, 0], "max_input": [10, 1, 1], "min_output": [0, 1, 1], "max_output": [100, 1, 1], "constant": [1, 3, 1]}});
        assert_eq!(parse(&v).map(|d| d.vec3(0.0, &mut rng)), Some(Vec3::new(10.0, 3.0, 1.0)));
        assert!(parse(&json!({"type": "constant", "constant": 1})).is_none(), "only parameter distributions");
    }

    #[test]
    fn dist_parse_reads_a_parameter_property() {
        let mut rng = Rng::new(1);
        let prop = json!({"distribution": {"class": "DistributionFloatParticleParameter", "type": "parameter", "parameter": {"param_mode": "DPM_Direct", "constant": 400}, "lookup_table": {"op": 0, "entry_count": 0, "values": [50]}}});
        assert_eq!(Dist::parse(&prop).map(|d| d.float(0.0, &mut rng)), Some(400.0));
    }
}
