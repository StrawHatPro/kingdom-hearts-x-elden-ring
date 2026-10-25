//! (ER party S8) SHEET `sheets/port/party-er-unlocks.json` read into plain values (game-free). Every Elden Ring id and
//! place of a scene row was parsed from the user's install (the row's evidence names the file and record); the words
//! are OUR TEXT for the user to review.

use glam::Vec3;
use kh_host::party::PartyChara;
use serde_json::Value;

/// A scene's place: an Elden Ring map-local position and yaw (degrees, the MSB's own).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Place {
    pub pos: Vec3,
    pub rot_y: f32,
}

/// A fight scene (Goofy vs Leonine, Sora vs Margit).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FightScene {
    /// The row id (`scene.<id>`).
    pub id: String,
    pub chara: Option<PartyChara>,
    pub boss_name: String,
    pub map: String,
    pub boss_entity: u32,
    pub boss_chr: i32,
    /// The boss's own NpcParam and NpcThinkParam (its double is made from them).
    pub boss_npc_param: i32,
    pub boss_think: i32,
    pub boss: Place,
    pub defeat_flag: u32,
    pub fog: Place,
    pub fog_passed_flag: u32,
    pub approach: Vec3,
    /// The first-encounter flag set to skip the intro (Margit's 10000851), None = no intro.
    pub intro_flag: Option<u32>,
}

/// The talk scene (Donald at the crone).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TalkScene {
    pub map: String,
    pub crone_entity: u32,
    pub crone: Place,
    pub stand_off: f32,
    pub talk_range: f32,
    pub show_range: f32,
    /// (CV3, the user 2026-10-17: "put him where Goofy is in the picture") HIS SPOT RELATIVE TO HER: `spot_dist` m from
    /// her, in the direction `spot_angle` degrees off her facing (KH3 yaw sense; negative = toward the walkway's approach
    /// from the Limgrave road), so it holds for her live facing and her MSB one alike. `crone_reach`: how far her own
    /// talk prompt reaches (m, measured in the user's session: up at <= 2.35 m, never at 2.95 m): his spot is kept out
    /// of it (at least `crone_reach` + 0.4 m from her).
    pub spot_dist: f32,
    pub spot_angle: f32,
    pub crone_reach: f32,
}

impl TalkScene {
    /// (CV3) His distance from her (m), never inside her own talk reach.
    pub fn spot_from_her(&self) -> f32 {
        self.spot_dist.max(self.crone_reach + 0.4)
    }
}

/// One of Donald's lines.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Line {
    pub speaker: String,
    pub text: String,
    pub anim: Option<String>,
    pub voice: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UnlockSheet {
    pub on: bool,
    /// THE STAGED BOSS FIGHTS (the user's universal rule): every `scene.<id>` row of kind `fight`.
    pub fights: Vec<FightScene>,
    pub donald: TalkScene,
    pub stage_range: f32,
    pub friend_offset: f32,
    pub anchor_offset: f32,
    pub lost_after: f32,
    /// The friend's hits on the staged boss: fired by his body (a non-player owner), this attack value (the HP given
    /// back at once), stamina, and the poise damage per KH3 reaction power.
    pub bullet_phys: u16,
    pub bullet_stamina: u16,
    pub bullet_poise: f32,
    pub angry: Vec<String>,
    pub unlock_rows: [Vec<String>; 3],
    pub fx_effect: String,
    pub fx_se: u32,
    pub fx_voice: String,
    pub banner_s: f32,
    pub total_s: f32,
    pub select_question: String,
    pub select_options: [String; 3],
    pub select_describe: [String; 3],
    pub select_confirm: String,
    pub joined: String,
    /// (HUD2) The description in the OBTAINED popup of a new party member (Sora, Donald, Goofy): OUR TEXT (KH3 has no
    /// character captions in its UI strings), sheet rows `text.obtained.sora|donald|goofy`.
    pub obtained: [String; 3],
    pub prompt: String,
    pub lines: Vec<Line>,
}

impl Default for UnlockSheet {
    fn default() -> UnlockSheet {
        UnlockSheet {
            on: true,
            fights: Vec::new(),
            donald: TalkScene { stand_off: 1.6, talk_range: 2.5, show_range: 80.0, spot_dist: 3.0, spot_angle: -35.0, crone_reach: 2.35, ..TalkScene::default() },
            stage_range: 400.0,
            friend_offset: 5.0,
            anchor_offset: 2.5,
            lost_after: 4.0,
            bullet_phys: 1,
            bullet_stamina: 1,
            bullet_poise: 10.0,
            angry: vec!["M_0908".into(), "IdleN".into()],
            unlock_rows: [vec!["SelfiePoseChange1".into()], vec!["BattleEnd".into(), "ODanceIdle".into()], vec!["BattleEnd".into(), "ODanceIdle".into()]],
            fx_effect: "vs_com_lv1".into(),
            fx_se: 38,
            fx_voice: "BattleStart".into(),
            banner_s: 3.0,
            total_s: 5.0,
            select_question: "Whose journey will you begin?".into(),
            select_options: ["Sora".into(), "Donald".into(), "Goofy".into()],
            select_describe: [String::new(), String::new(), String::new()],
            select_confirm: "Is this who you will be?".into(),
            joined: "{name} joined the party!".into(),
            obtained: [
                "The Keyblade's chosen wielder.<br>He fights with Keyblade combos, magic and formchanges.".into(),
                "The royal magician of Disney Castle.<br>He fights with magic and heals the party.".into(),
                "The captain of the royal knights of Disney Castle.<br>He guards the party with his shield.".into(),
            ],
            prompt: "Talk".into(),
            lines: Vec::new(),
        }
    }
}

fn v3(v: &Value) -> Option<Vec3> {
    let a = v.as_array()?;
    Some(Vec3::new(a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32))
}

fn place(v: &Value) -> Place {
    Place { pos: v3(&v["pos"]).unwrap_or(Vec3::ZERO), rot_y: v["rot_y"].as_f64().unwrap_or(0.0) as f32 }
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().into_iter().flatten().filter_map(|s| s.as_str().map(str::to_string)).collect()
}

fn three(v: &Value, fallback: &[String; 3]) -> [String; 3] {
    let s = strings(v);
    if s.len() == 3 {
        [s[0].clone(), s[1].clone(), s[2].clone()]
    } else {
        fallback.clone()
    }
}

fn fight(id: &str, v: &Value) -> FightScene {
    FightScene {
        id: id.to_string(),
        chara: v["chara"].as_str().and_then(kh_host::formation::chara_of),
        boss_name: v["boss_name"].as_str().unwrap_or("").to_string(),
        map: v["map"].as_str().unwrap_or("").to_string(),
        boss_entity: v["boss"]["entity"].as_u64().unwrap_or(0) as u32,
        boss_chr: v["boss"]["chr"].as_i64().unwrap_or(0) as i32,
        boss_npc_param: v["boss"]["npc_param"].as_i64().unwrap_or(-1) as i32,
        boss_think: v["boss"]["think"].as_i64().unwrap_or(-1) as i32,
        boss: place(&v["boss"]),
        defeat_flag: v["defeat_flag"].as_u64().unwrap_or(0) as u32,
        fog: place(&v["fog"]),
        fog_passed_flag: v["fog_passed_flag"].as_u64().unwrap_or(0) as u32,
        approach: v3(&v["approach"]["pos"]).unwrap_or(Vec3::ZERO),
        intro_flag: v["intro_flag"].as_u64().map(|f| f as u32),
    }
}

impl UnlockSheet {
    /// From the sheet's text: the values and the ids not understood (said in the log).
    pub fn from_text(text: &str) -> Result<(UnlockSheet, Vec<String>), String> {
        let json: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let rows = json["rows"].as_array().ok_or("no rows")?;
        let mut s = UnlockSheet::default();
        let mut unknown = Vec::new();
        let mut lines: Vec<(usize, Line)> = Vec::new();
        for row in rows {
            let id = row[0].as_str().unwrap_or("");
            let v = &row[1];
            let f = |d: f32| v.as_f64().map_or(d, |x| x as f32);
            match id {
                "unlock.on" => s.on = v.as_f64().is_none_or(|x| x != 0.0),
                _ if id.starts_with("scene.") && v["kind"].as_str() == Some("fight") => s.fights.push(fight(&id["scene.".len()..], v)),
                "scene.donald" => {
                    s.donald = TalkScene {
                        map: v["map"].as_str().unwrap_or("").to_string(),
                        crone_entity: v["crone"]["entity"].as_u64().unwrap_or(0) as u32,
                        crone: place(&v["crone"]),
                        stand_off: v["stand_off"].as_f64().unwrap_or(1.6) as f32,
                        talk_range: v["talk_range"].as_f64().unwrap_or(2.5) as f32,
                        show_range: v["show_range"].as_f64().unwrap_or(80.0) as f32,
                        spot_dist: v["spot_dist"].as_f64().unwrap_or(3.0) as f32,
                        spot_angle: v["spot_angle"].as_f64().unwrap_or(-35.0) as f32,
                        crone_reach: v["crone_reach"].as_f64().unwrap_or(2.35) as f32,
                    }
                }
                "stage.range" => s.stage_range = f(s.stage_range),
                "stage.friend_offset" => s.friend_offset = f(s.friend_offset),
                "stage.anchor_offset" => s.anchor_offset = f(s.anchor_offset),
                "stage.lost_after" => s.lost_after = f(s.lost_after),
                "stage.bullet" => {
                    s.bullet_phys = v["phys"].as_u64().unwrap_or(1).clamp(1, 60000) as u16;
                    s.bullet_stamina = v["stamina"].as_u64().unwrap_or(1).clamp(1, 60000) as u16;
                    s.bullet_poise = v["poise"].as_f64().unwrap_or(10.0) as f32;
                }
                "anim.donald.angry" => s.angry = strings(v),
                "anim.unlock.sora" => s.unlock_rows[0] = strings(v),
                "anim.unlock.donald" => s.unlock_rows[1] = strings(v),
                "anim.unlock.goofy" => s.unlock_rows[2] = strings(v),
                "fx.unlock" => {
                    s.fx_effect = v["effect"].as_str().unwrap_or("vs_com_lv1").to_string();
                    s.fx_se = v["se"].as_u64().unwrap_or(38) as u32;
                    s.fx_voice = v["voice"].as_str().unwrap_or("BattleStart").to_string();
                    s.banner_s = v["banner_s"].as_f64().unwrap_or(3.0) as f32;
                    s.total_s = v["total_s"].as_f64().unwrap_or(5.0) as f32;
                }
                "text.select.question" => s.select_question = v.as_str().unwrap_or("").to_string(),
                "text.select.options" => s.select_options = three(v, &s.select_options),
                "text.select.describe" => s.select_describe = three(v, &s.select_describe),
                "text.select.confirm" => s.select_confirm = v.as_str().unwrap_or("").to_string(),
                "text.joined" => s.joined = v.as_str().unwrap_or("").to_string(),
                "text.obtained.sora" => s.obtained[0] = v.as_str().unwrap_or("").to_string(), // (HUD2)
                "text.obtained.donald" => s.obtained[1] = v.as_str().unwrap_or("").to_string(),
                "text.obtained.goofy" => s.obtained[2] = v.as_str().unwrap_or("").to_string(),
                "text.talk.prompt" => s.prompt = v.as_str().unwrap_or("Talk").to_string(),
                _ if id.starts_with("talk.donald.") => {
                    let n: usize = id["talk.donald.".len()..].parse().unwrap_or(999);
                    lines.push((n, Line { speaker: v["speaker"].as_str().unwrap_or("Donald").to_string(), text: v["text"].as_str().unwrap_or("").to_string(), anim: v["anim"].as_str().map(str::to_string), voice: v["voice"].as_str().map(str::to_string) }));
                }
                "anim.donald.talk" => {}
                _ => unknown.push(id.to_string()),
            }
        }
        lines.sort_by_key(|l| l.0);
        s.lines = lines.into_iter().map(|l| l.1).collect();
        Ok((s, unknown))
    }

    /// The fight scene of a character (Goofy, Sora).
    pub fn fight(&self, c: PartyChara) -> Option<&FightScene> {
        self.fights.iter().find(|f| f.chara == Some(c))
    }

    /// `{name}` filled.
    pub fn joined_text(&self, c: PartyChara) -> String {
        self.joined.replace("{name}", c.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_sheet_reads_whole() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sheets/port/party-er-unlocks.json");
        let text = std::fs::read_to_string(path).expect("the sheet");
        let (s, unknown) = UnlockSheet::from_text(&text).expect("parses");
        assert!(unknown.is_empty(), "rows not understood: {unknown:?}");
        assert!(s.on);
        assert_eq!(s.fights.len(), 2, "every scene.* row of kind fight");
        let (goofy, sora) = (s.fight(PartyChara::Goofy).unwrap(), s.fight(PartyChara::Sora).unwrap());
        assert_eq!((goofy.boss_entity, goofy.defeat_flag, goofy.fog_passed_flag, goofy.intro_flag), (1043300800, 1043300800, 1043302805, None));
        assert_eq!((goofy.boss_chr, goofy.boss_npc_param, goofy.boss_think), (3460, 34600913, 34601000));
        assert_eq!((sora.boss_entity, sora.defeat_flag, sora.fog_passed_flag, sora.intro_flag), (10000850, 10000850, 10002855, Some(10000851)));
        assert_eq!((sora.boss_chr, sora.boss_npc_param, sora.boss_think), (2130, 21300014, 21300000));
        assert_eq!(sora.map, "m10_00_00_00");
        assert_eq!(s.bullet_phys, 1);
        assert_eq!(s.donald.crone_entity, 1041390700);
        assert!((s.donald.crone.pos - Vec3::new(-104.363, 253.534, 21.189)).length() < 1e-3);
        // (CV3) his spot: 3 m from her, 35 degrees off her facing toward the approach, outside her own talk reach
        assert_eq!((s.donald.spot_dist, s.donald.spot_angle, s.donald.crone_reach), (3.0, -35.0, 2.35));
        assert!(s.donald.spot_from_her() > s.donald.crone_reach);
        assert_eq!(s.lines.len(), 6);
        assert!(s.lines[5].text.contains("{leader}"));
        assert_eq!(s.select_options, ["Sora".to_string(), "Donald".to_string(), "Goofy".to_string()]);
        assert_eq!(s.joined_text(PartyChara::Donald), "Donald joined the party!");
    }
}
