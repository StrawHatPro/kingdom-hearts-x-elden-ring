//! (INTERACT-MARK, the user 2026-11-05: "do option 1") THE SAVE POINTS WHOSE AREA IS THE USER'S CHOICE, NOT KH3'S: the rows of
//! `sheets/port/kh-savepoint-areas.json` (written by tools/kh_savepoint_areas.js from the map data). One row today: the
//! Roundtable Hold's Table of Lost Grace, whose grace stands on the floor inside the round table. Its save point keeps
//! KH3's capsule sizes (reactor radius 110 / height 180, recover 50 / 140) with their BASE on the table top and their
//! centre on the table between the grace and its edge (toward the grace's player point): the Save on the table top at
//! its edge. THEN THE USER'S CORRECTION (2026-11-05: "you should be able to access round table hold save point from the
//! floor next to the table"): a BAND around the table (kh_host::savepoint::Ring): the table's edge measured all around
//! (every 5 degrees, on the navmesh), the Save while the leader stands within KH3's reactor radius (110 cm) of that edge
//! at floor height with KH3's capsule height (the heal: KH3's recover radius, 50 cm); not further out. Every other save
//! point: KH3's own capsules (unchanged). The inside-only rule (savepoint_rules.rs `y_rule`) is the same for all; Elden
//! Ring's own grace rows (6100 .. 6103) stay switched off (gracelock.rs).

use std::sync::OnceLock;

use glam::Vec3;

const SHEET: &str = include_str!("../../../sheets/port/kh-savepoint-areas.json");

/// One save point's area (Elden Ring metres from the grace's character, the place savepoint.rs builds the save point at).
#[derive(Clone, Debug, PartialEq)]
pub struct Area {
    pub grace: u32,
    /// (radius, height) in cm, KH3's.
    pub reactor: (f32, f32),
    pub recover: (f32, f32),
    /// The capsules' centre: Elden Ring x, z (m) from the grace character.
    pub centre: (f32, f32),
    /// The capsules' bottom over the grace character (m).
    pub base: f32,
    /// (the user's correction) the band: the table's edge (m) by KH3 yaw from the grace character (every 360 / len
    /// degrees), the floor over the character (m), the reach from the edge (cm: Save, heal), the slab's height (cm).
    pub edge: Vec<f32>,
    pub floor: f32,
    pub reach: (f32, f32),
    pub band_height: f32,
}

impl Area {
    /// (reactor, recover) as kh_host::savepoint::SavePointData capsules: (radius, height, offset of the centre from the
    /// save point's root, KH3 cm), for a root `root_cm` over the grace character (savepoint.rs `savepoint.height`).
    pub fn capsules(&self, root_cm: f32) -> ((f32, f32, Vec3), (f32, f32, Vec3)) {
        let one = |(radius, height): (f32, f32)| {
            let centre_er = Vec3::new(self.centre.0, self.base + height / 200.0, self.centre.1);
            (radius, height, crate::coords::dir_er_to_kh(centre_er) - Vec3::Z * root_cm)
        };
        (one(self.reactor), one(self.recover))
    }

    /// (the user's correction) The band as kh_host's [`kh_host::savepoint::Ring`] for a root `root_cm` over the grace character.
    pub fn ring(&self, root_cm: f32) -> kh_host::savepoint::Ring {
        let cm = crate::coords::CM_PER_M;
        kh_host::savepoint::Ring { centre: -Vec3::Z * root_cm, edge: self.edge.iter().map(|e| e * cm).collect(), floor: self.floor * cm - root_cm, height: self.band_height, reach: self.reach }
    }
}

/// (pure) The sheet's rows.
pub fn parse(text: &str) -> Result<Vec<Area>, String> {
    let j: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in j["rows"].as_array().ok_or("no rows")? {
        let v = &row[1];
        let f = |x: &serde_json::Value| x.as_f64().map(|n| n as f32).ok_or_else(|| format!("{}: a number is missing", row[0]));
        let pair = |o: &serde_json::Value| -> Result<(f32, f32), String> { Ok((f(&o["radius_cm"])?, f(&o["height_cm"])?)) };
        let ring = &v["ring"];
        out.push(Area {
            grace: v["grace"].as_u64().ok_or("no grace")? as u32,
            reactor: pair(&v["reactor"])?,
            recover: pair(&v["recover"])?,
            centre: (f(&v["centre_from_grace_character_m"][0])?, f(&v["centre_from_grace_character_m"][1])?),
            base: f(&v["base_over_grace_character_m"])?,
            edge: ring["edge_m_by_kh_yaw"].as_array().ok_or("no ring edge")?.iter().map(f).collect::<Result<Vec<f32>, String>>()?,
            floor: f(&ring["floor_over_grace_character_m"])?,
            reach: (f(&ring["reach_cm"]["reactor"])?, f(&ring["reach_cm"]["recover"])?),
            band_height: f(&ring["height_cm"])?,
        });
    }
    Ok(out)
}

static AREAS: OnceLock<Vec<Area>> = OnceLock::new();

/// The area chosen for this grace, if any (None: KH3's own capsules).
pub fn of(grace: u32) -> Option<Area> {
    AREAS.get_or_init(|| parse(SHEET).unwrap_or_default()).iter().find(|a| a.grace == grace).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_host::savepoint::{SavePoint, SavePointData};

    fn row() -> serde_json::Value {
        let j: serde_json::Value = serde_json::from_str(SHEET).unwrap();
        j["rows"][0][1].clone()
    }

    fn f(v: &serde_json::Value) -> f32 {
        v.as_f64().unwrap() as f32
    }

    const ROOT_CM: f32 = 92.0;
    const LEADERS: [(&str, f32, f32); 3] = [("sora", 60.0, 89.0), ("goofy", 40.0, 89.0), ("donald", 30.0, 49.0)];

    /// The Roundtable save point's data as savepoint.rs makes it (the area's capsules and band over KH3's).
    fn roundtable() -> (Area, SavePointData) {
        let area = of(11_101_950).expect("the Roundtable row");
        let mut data = SavePointData::default();
        (data.reactor, data.recover) = area.capsules(ROOT_CM);
        data.ring = Some(area.ring(ROOT_CM));
        (area, data)
    }

    /// Is a leader (KH3 capsule radius, half height) whose feet stand at Elden Ring `feet` (m from the grace character)
    /// inside the save point's reactor (kh_host's own step)?
    fn inside(data: &SavePointData, feet: Vec3, radius: f32, half: f32) -> bool {
        let mut sp = SavePoint::new(data.clone(), Vec3::Z * ROOT_CM); // the grace character at the KH origin
        let mut sora = kh_core::Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 });
        let centre = crate::coords::dir_er_to_kh(feet) + Vec3::Z * half;
        sp.step_checked(1.0 / 60.0, &mut sora, (centre, radius, half), false).in_reactor
    }

    /// A point (Elden Ring m from the grace character) `d` out along KH3 yaw `deg`, `up` over the character.
    fn at(deg: f32, d: f32, up: f32) -> Vec3 {
        let a = deg.to_radians();
        Vec3::new(a.sin() * d, up, a.cos() * d)
    }

    /// (the user's correction) ON THE FLOOR RIGHT NEXT TO THE TABLE, ALL AROUND IT: offered, whoever leads; 1.5 m out from
    /// the edge and beyond: not; on the table top at its edge: offered.
    #[test]
    fn interact_mark_the_roundtable_save_point_from_the_floor_next_to_the_table() {
        let (area, data) = roundtable();
        let m = &row()["measured"];
        let body = f(&m["body_m"]);
        assert_eq!(area.edge.len(), 72);
        assert!(area.edge.iter().all(|e| (3.9..4.4).contains(e)), "the table's edge all around: {:?}", area.edge);
        assert!((area.floor - 0.32).abs() < 0.01 && (area.base - 1.324).abs() < 0.01);
        assert_eq!((area.reach, area.band_height), ((110.0, 50.0), 180.0), "KH3's radii and height");
        let ring = data.ring.clone().expect("the band");
        for deg in (0..360).step_by(15).map(|d| d as f32 + 2.5) {
            let edge = ring.edge_at(deg.to_radians()) / 100.0;
            for (who, radius, half) in LEADERS {
                assert!(inside(&data, at(deg, edge + body, area.floor), radius, half), "{who} at {deg} deg: on the floor against the table");
                assert!(inside(&data, at(deg, edge + 1.0, area.floor), radius, half), "{who} at {deg} deg: on the floor 1 m out");
                assert!(!inside(&data, at(deg, edge + 1.5, area.floor), radius, half), "{who} at {deg} deg: 1.5 m out: not offered");
                assert!(!inside(&data, at(deg, edge + 3.0, area.floor), radius, half), "{who} at {deg} deg: farther out");
                assert!(inside(&data, at(deg, edge - body, area.base), radius, half), "{who} at {deg} deg: on the table top at its edge");
            }
        }
        // the player's side: the band's edge there is the one measured on that line
        let dir = Vec3::new(f(&m["toward_player_point"][0]), 0.0, f(&m["toward_player_point"][1]));
        assert!((ring.edge_at(dir.x.atan2(dir.z)) / 100.0 - f(&m["table_top_edge_m"])).abs() < 0.1, "the band's edge on the player's side");
        // KH3's own capsule alone: none of those spots (why the user chose this)
        let kh3 = SavePointData::default();
        assert!(!inside(&kh3, at(30.0, 4.3, area.floor), 60.0, 89.0) && !inside(&kh3, at(30.0, 3.7, area.base), 60.0, 89.0));
    }

    /// Every other save point keeps KH3's capsules, no band.
    #[test]
    fn interact_mark_other_save_points_are_kh3s() {
        assert!(of(10_001_950).is_none() && of(0).is_none());
        assert_eq!(parse(SHEET).unwrap().len(), 1);
        assert_eq!(SavePointData::default().ring, None);
    }
}
