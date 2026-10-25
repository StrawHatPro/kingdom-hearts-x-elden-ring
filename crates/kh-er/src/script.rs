//! SCRIPTED PAD INPUT, STEP-EXACT (a testing aid; `in.script` of the control file, a one-shot: a
//! change of the value starts it): segments separated by `;`, each
//! `steps[:lx,ly[,rx,ry[,buttons]]]` = hold the left stick (lx, ly), the right stick (rx, ry) and
//! the buttons (`a b x y lb rb lt rt l3 r3 up down left right start back`, joined by `+`) for that many of
//! Sora's fixed 60 Hz steps. Example: `in.script = 120:0,1 ; 90:0,1,1,0 ; 6:0,0,0,0,b ; 60:`.
//! It replaces the pad and the keyboard path while it runs and is CANCELLED by any real pad input
//! (somebody picked the pad up: the instance is theirs).

use glam::Vec2;
use kh_core::{PadButton, PadButtons};

use crate::control::{Control, OneShot};
use crate::erlog;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seg {
    pub steps: u32,
    pub left: Vec2,
    pub right: Vec2,
    pub buttons: PadButtons,
}

pub fn parse(text: &str) -> Result<Vec<Seg>, String> {
    let mut segs = Vec::new();
    // (IMPL_ER14) '|' separates segments too (a tool that splits the control file's lines at ';' can carry a script)
    for part in text.split([';', '|']).map(str::trim).filter(|p| !p.is_empty()) {
        let (steps, rest) = part.split_once(':').unwrap_or((part, ""));
        let steps: u32 = steps.trim().parse().map_err(|_| format!("\"{part}\": the step count is not a number"))?;
        let fields: Vec<&str> = rest.split(',').map(str::trim).collect();
        let num = |k: usize| -> Result<f32, String> {
            match fields.get(k).filter(|f| !f.is_empty()) {
                Some(f) => f.parse::<f32>().map(|v| v.clamp(-1.0, 1.0)).map_err(|_| format!("\"{part}\": \"{f}\" is not a number")),
                None => Ok(0.0),
            }
        };
        let mut buttons = PadButtons::NONE;
        for name in fields.get(4).copied().unwrap_or("").split('+').map(str::trim).filter(|n| !n.is_empty()) {
            let button = match name {
                "a" => PadButton::FaceBottom,
                "b" => PadButton::FaceRight,
                "x" => PadButton::FaceLeft,
                "y" => PadButton::FaceTop,
                "lb" => PadButton::LeftShoulder,
                "rb" => PadButton::RightShoulder,
                "lt" => PadButton::LeftTrigger,
                "rt" => PadButton::RightTrigger,
                "l3" => PadButton::LeftThumb,
                "r3" => PadButton::RightThumb,
                "up" => PadButton::DPadUp,
                "down" => PadButton::DPadDown,
                "left" => PadButton::DPadLeft,
                "right" => PadButton::DPadRight,
                "start" => PadButton::SpecialRight,
                "back" => PadButton::SpecialLeft,
                other => return Err(format!("\"{part}\": no button \"{other}\"")),
            };
            buttons.set(button, true);
        }
        segs.push(Seg { steps: steps.min(36_000), left: Vec2::new(num(0)?, num(1)?), right: Vec2::new(num(2)?, num(3)?), buttons });
    }
    Ok(segs)
}

pub struct InScript {
    shot: OneShot,
    segs: Vec<Seg>,
    at: usize,
    done: u32,
}

impl InScript {
    pub fn new(control: &Control) -> InScript {
        InScript { shot: OneShot::new("in.script", control), segs: Vec::new(), at: 0, done: 0 }
    }

    pub fn active(&self) -> bool {
        self.at < self.segs.len()
    }

    /// Once a frame: a changed `in.script` starts it.
    pub fn poll(&mut self, control: &Control) {
        if let Some(text) = self.shot.fired(control) {
            match parse(&text) {
                Ok(segs) => {
                    erlog!("script: in.script started: {} segment(s), {} step(s)", segs.len(), segs.iter().map(|s| s.steps).sum::<u32>());
                    (self.segs, self.at, self.done) = (segs, 0, 0);
                }
                Err(e) => erlog!("script: in.script not understood: {e}"),
            }
        }
    }

    pub fn cancel(&mut self, why: &str) {
        if self.active() {
            erlog!("script: in.script CANCELLED after segment {} ({why})", self.at);
            self.segs.clear();
            self.at = 0;
        }
    }

    /// The input of the next step, while the script runs.
    pub fn next(&mut self) -> Option<Seg> {
        while self.at < self.segs.len() && self.done >= self.segs[self.at].steps {
            self.at += 1;
            self.done = 0;
            if self.at == self.segs.len() {
                erlog!("script: in.script finished");
            }
        }
        let seg = *self.segs.get(self.at)?;
        self.done += 1;
        Some(seg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_parse_and_play_in_order() {
        let segs = parse("2:0,1 ; 1:0,1,1,-0.5,a+rb ; 3:").unwrap();
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[1].right, Vec2::new(1.0, -0.5));
        assert!(segs[1].buttons.has(PadButton::FaceBottom) && segs[1].buttons.has(PadButton::RightShoulder));
        assert_eq!(segs[2].left, Vec2::ZERO);
        assert!(parse("x:1").is_err() && parse("3:0,0,0,0,q").is_err());
    }
}
