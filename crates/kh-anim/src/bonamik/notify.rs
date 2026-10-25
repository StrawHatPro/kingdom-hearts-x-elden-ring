//! `tools/lib/bonamik/notify.js`: the effects of the Bonamik anim-notify states on a running
//! evaluator.
//!
//!   set_group_param / restore_group_param   TresAnimNotifyState_Bonamik_ChangeGroupParams begin / end
//!   set_chara_wind / set_chara_wind_scale / clear_chara_wind   TresAnimNotifyState_Bonamik_CharaWind
//!
//! Source: instance functions 0x143B064B0 (set) / 0x143B018B0 (restore), branch inst+0x33E8 == 0;
//! chara wind: 0x143B05F10. A group name that matches no group of the asset is ignored (as in the
//! game). The set is immediate and absolute; the restore writes the default, not the value
//! before the set.
//!
//! ASSUMPTION (the JS's): the defaults restored (per-joint table core+0x37B0, per-cone table
//! core+0x37A0) are the values the records have right after setup. Who fills those tables was
//! not read. This port snapshots them at the end of setup (the JS: on first use; nothing but
//! these functions changes the values in between).

use crate::json::Json;

use super::records::Core;

/// One row of a notify's `m_GroupParams`.
#[derive(Clone, Debug, PartialEq)]
pub struct GroupParam {
    /// `m_Groups`
    pub groups: Vec<String>,
    /// `m_Parameter` without the `ESQEX_Bonamik_Parameters_` prefix
    pub parameter: String,
    /// `m_Value`
    pub value: f64,
}

/// The properties of a `TresAnimNotifyState_Bonamik_CharaWind`.
#[derive(Clone, Debug, PartialEq)]
pub struct CharaWind {
    /// `m_DirectionSpace` (ESQEX_Bonamik_EmissiveDirection: UpVec 0, ModelSpace 1, WorldSpace 2)
    pub direction_space: i32,
    pub direction_vec: [f64; 3],
    pub base_strength: f64,
    pub wave_amplitude: f64,
    pub wave_period: f64,
    /// `m_Groups` (empty = all)
    pub groups: Vec<String>,
}

const PARAMETERS: [&str; 13] = ["Mass", "InnerConeStr", "OuterConeStr", "Friction", "Damping", "Bounciness", "WindDrag", "WindLift", "LFCoeff", "ParkRate", "MaxLF", "LFDrag", "LFLift"];

fn names(j: &Json) -> Vec<String> {
    j.items().iter().filter_map(|n| n.as_str()).map(str::to_string).collect()
}

/// The rows of a `m_GroupParams` array (`[{ m_Groups, m_Parameter, m_Value }]`, the plain form
/// of `data/bonamik.json` -> `notify_states`).
pub fn group_params_from_json(rows: &Json) -> Result<Vec<GroupParam>, String> {
    let mut out = Vec::new();
    for row in rows.items() {
        let parameter = row.get("m_Parameter").as_str().unwrap_or("").trim_start_matches("ESQEX_Bonamik_Parameters_").to_string();
        if !PARAMETERS.contains(&parameter.as_str()) {
            return Err(format!("Bonamik parameter not handled: {parameter}"));
        }
        out.push(GroupParam { groups: names(row.get("m_Groups")), parameter, value: row.get("m_Value").as_f64().ok_or("Bonamik group parameter without m_Value")? });
    }
    Ok(out)
}

/// The properties of a chara-wind notify (`props` of `notify_states`).
pub fn chara_wind_from_json(props: &Json) -> Result<CharaWind, String> {
    let space = props.get("m_DirectionSpace");
    let direction_space = match (space.as_f64(), space.as_str()) {
        (Some(n), _) => n as i32,
        (_, Some(s)) if s.contains("ModelSpace") => 1,
        (_, Some(s)) if s.contains("WorldSpace") => 2,
        _ => 0,
    };
    let dv = props.get("m_DirectionVec");
    let direction_vec = if dv.items().len() >= 3 {
        [dv.at(0).as_f64().unwrap_or(0.0), dv.at(1).as_f64().unwrap_or(0.0), dv.at(2).as_f64().unwrap_or(0.0)]
    } else {
        [dv.get("x").as_f64().unwrap_or(0.0), dv.get("y").as_f64().unwrap_or(0.0), dv.get("z").as_f64().unwrap_or(0.0)]
    };
    let number = |key: &str| props.get(key).as_f64().ok_or_else(|| format!("Bonamik chara wind without {key}"));
    Ok(CharaWind { direction_space, direction_vec, base_strength: number("m_BaseStrength")?, wave_amplitude: number("m_WaveAmplitude")?, wave_period: number("m_WavePeriod")?, groups: names(props.get("m_Groups")) })
}

/// The values of one joint right after setup.
#[derive(Clone, Copy, Debug)]
struct Defaults {
    f250: f32,
    f254: f32,
    f268: f32,
    f260: f32,
    f264: f32,
    f26c: f32,
    f270: f32,
    f288: f32,
    f28c: f32,
    f290: f32,
    f294: f32,
    f298: f32,
    /// (f28, f2c) of the cone record the joint owns
    cone: Option<(f32, f32)>,
}

/// What notify.js keeps on the `sim` object.
#[derive(Clone, Debug, Default)]
pub struct NotifyState {
    /// `sim._notifyDefaults`
    defaults: Vec<Defaults>,
    /// `sim._charaWindGroups`
    chara_wind_groups: Vec<usize>,
}

/// `Math.hypot` as V8 computes it (scaled by the largest magnitude, Kahan summation).
pub fn js_hypot(values: &[f64]) -> f64 {
    let mut max = 0.0f64;
    for v in values {
        if v.is_nan() {
            return f64::NAN;
        }
        if v.abs() > max {
            max = v.abs();
        }
    }
    if max == 0.0 {
        return 0.0;
    }
    if max == f64::INFINITY {
        return f64::INFINITY;
    }
    let (mut sum, mut compensation) = (0.0f64, 0.0f64);
    for v in values {
        let n = v.abs() / max;
        let summand = n * n - compensation;
        let preliminary = sum + summand;
        compensation = (preliminary - sum) - summand;
        sum = preliminary;
    }
    sum.sqrt() * max
}

impl NotifyState {
    /// `defaultsOf(sim)`, taken at the end of setup.
    pub fn new(core: &Core) -> NotifyState {
        let sc = &core.f3778;
        let defaults = sc.f0.iter().map(|p| Defaults { f250: p.f250, f254: p.f254, f268: p.f268, f260: p.f260, f264: p.f264, f26c: p.f26c, f270: p.f270, f288: p.f288, f28c: p.f28c, f290: p.f290, f294: p.f294, f298: p.f298, cone: p.f2a8.map(|c| (sc.f30[c].f28, sc.f30[c].f2c)) }).collect();
        NotifyState { defaults, chara_wind_groups: Vec::new() }
    }

    fn apply(&self, core: &mut Core, rows: &[GroupParam], restore: bool) {
        for row in rows {
            for i in joints_of(core, &row.groups) {
                let d = self.defaults[i];
                let cone = core.f3778.f0[i].f2a8;
                let j = &mut core.f3778.f0[i];
                let value = row.value as f32;
                let pick = |default: f32| if restore { default } else { value };
                match row.parameter.as_str() {
                    "Mass" => {
                        let v = pick(d.f250);
                        if restore || (v as f64) > 1e-4 {
                            j.f250 = v;
                            j.f254 = if restore { d.f254 } else { (1.0 / (v as f64)) as f32 };
                        }
                    }
                    "InnerConeStr" => {
                        if let (Some(c), Some(default)) = (cone, d.cone) {
                            core.f3778.f30[c].f28 = pick(default.0);
                        }
                    }
                    "OuterConeStr" => {
                        if let (Some(c), Some(default)) = (cone, d.cone) {
                            core.f3778.f30[c].f2c = pick(default.1);
                        }
                    }
                    "Friction" => j.f268 = pick(d.f268),
                    "Damping" => j.f260 = pick(d.f260),
                    "Bounciness" => j.f264 = pick(d.f264),
                    "WindDrag" => j.f26c = pick(d.f26c),
                    "WindLift" => j.f270 = pick(d.f270),
                    "LFCoeff" => j.f288 = pick(d.f288),
                    "ParkRate" => j.f28c = pick(d.f28c),
                    "MaxLF" => j.f290 = pick(d.f290),
                    "LFDrag" => j.f294 = pick(d.f294),
                    "LFLift" => j.f298 = pick(d.f298),
                    // (ER69) a parameter not handled: left as it is
                    _ => super::unported(15),
                }
            }
        }
    }

    /// TresAnimNotifyState_Bonamik_ChangeGroupParams begin
    pub fn set_group_param(&self, core: &mut Core, rows: &[GroupParam]) {
        self.apply(core, rows, false);
    }

    /// TresAnimNotifyState_Bonamik_ChangeGroupParams end
    pub fn restore_group_param(&self, core: &mut Core, rows: &[GroupParam]) {
        self.apply(core, rows, true);
    }

    /// TresAnimNotifyState_Bonamik_CharaWind begin
    pub fn set_chara_wind(&mut self, core: &mut Core, props: &CharaWind) {
        let v = props.direction_vec;
        let mut l = js_hypot(&v);
        if l == 0.0 || l.is_nan() {
            l = 1.0;
        }
        core.f3720 = props.direction_space;
        core.f3724 = [(v[0] / l) as f32, (v[1] / l) as f32, (v[2] / l) as f32];
        // core.f3730 = core: the reference core is this core
        core.f3738 = props.base_strength as f32;
        core.f373c = props.wave_amplitude as f32;
        core.f3740 = if props.wave_period > 0.0 { (1.0 / props.wave_period) as f32 } else { 0.0 };
        core.f3744 = 1;
        self.chara_wind_groups = if !props.groups.is_empty() {
            let mut groups: Vec<usize> = Vec::new();
            for i in joints_of(core, &props.groups) {
                let g = core.f3778.f0[i].f2b8;
                if !groups.contains(&g) {
                    groups.push(g);
                }
            }
            groups
        } else {
            (0..core.f410).collect()
        };
        self.set_chara_wind_scale(core, 1.0);
    }

    /// TresAnimNotifyState_Bonamik_CharaWind tick: the scale (the clip's `Wind` float curve)
    pub fn set_chara_wind_scale(&self, core: &mut Core, scale: f32) {
        for &g in &self.chara_wind_groups {
            core.f954[g] = scale;
        }
    }

    /// The groups the active character wind was set on (empty: none active).
    pub fn chara_wind_groups(&self) -> &[usize] {
        &self.chara_wind_groups
    }

    /// TresAnimNotifyState_Bonamik_CharaWind end
    pub fn clear_chara_wind(&mut self, core: &mut Core) {
        self.set_chara_wind_scale(core, 0.0);
        core.f3744 = 0;
        self.chara_wind_groups = Vec::new();
    }
}

/// `jointsOf(sim, groups)`: the particles of the named groups (no name = all particles).
fn joints_of(core: &Core, groups: &[String]) -> Vec<usize> {
    let ps = &core.f3778.f0;
    if groups.is_empty() {
        return (0..ps.len()).collect();
    }
    let mut out = Vec::new();
    for n in groups {
        let Some(g) = core.f4d8[..core.f410].iter().position(|x| x.to_lowercase() == n.to_lowercase()) else { continue };
        for (i, p) in ps.iter().enumerate() {
            if p.f2b8 == g {
                out.push(i);
            }
        }
    }
    out
}
