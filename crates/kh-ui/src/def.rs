//! The UI pack as plain data: movies (`movies/<Name>.json`, format `kh3-ui-movie/1` written by
//! `tools/sora_ui_pack.js`), textures (PNG), fonts (`fonts/<font>.json`, glyph outlines) and the
//! English string tables. Nothing here plays anything: see `player.rs`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;

use crate::geom::{Cx, Matrix, Rect};

/// An RGBA8 texture, straight alpha.
#[derive(Debug)]
pub struct Texture {
    pub file: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// One edge of a fill: a line or a quadratic curve, in the shape's pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Edge {
    Line([f32; 4]),
    Curve([f32; 6]),
}

#[derive(Clone, Debug)]
pub struct BitmapFill {
    pub texture: Arc<Texture>,
    /// Bitmap pixels of the DECLARED size (sub-image offset already applied) -> shape pixels.
    pub matrix: Matrix,
    /// Declared size -> texture size (`FlashTextureScale_Mult4` rescales textures).
    pub scale: (f32, f32),
    pub repeat: bool,
    pub smooth: bool,
}

#[derive(Clone, Debug)]
pub enum Paint {
    None,
    Solid([f32; 4]),
    /// Stops: (ratio 0..1, straight RGBA 0..255). The matrix maps the gradient square
    /// (-16384..16384) to shape pixels.
    Gradient { radial: bool, matrix: Matrix, stops: Vec<(f32, [f32; 4])> },
    Bitmap(BitmapFill),
}

#[derive(Clone, Debug)]
pub struct Fill {
    pub paint: Paint,
    pub edges: Vec<Edge>,
}

#[derive(Clone, Debug)]
pub struct Stroke {
    pub width: f32,
    pub color: [f32; 4],
    pub edges: Vec<Edge>,
}

#[derive(Clone, Debug, Default)]
pub struct Shape {
    pub id: u32,
    pub bounds: Rect0,
    pub fills: Vec<Fill>,
    pub strokes: Vec<Stroke>,
    /// `DefineScalingGrid` of the pack (`scale9`), when this character has one.
    pub scale9: Option<Rect>,
}

/// A rectangle that defaults to all zeros (for `Default` derives).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect0 {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Rect0 {
    pub fn rect(&self) -> Rect {
        Rect::new(self.x0, self.y0, self.x1, self.y1)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
    Center,
}

#[derive(Clone, Debug)]
pub struct TextDef {
    pub id: u32,
    pub bounds: Rect0,
    pub font_class: Option<String>,
    pub size: f32,
    pub color: [f32; 4],
    pub align: Align,
    pub initial: String,
    /// `wordWrap` of the field and its line `leading` (used for wrapped fields only).
    pub word_wrap: bool,
    pub leading: f32,
}

/// A bitmap placed directly on a timeline: a declared-size rectangle at the origin.
#[derive(Clone, Debug)]
pub struct ImageChar {
    pub size: (f32, f32),
    pub fill: BitmapFill,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Frame(u32),
    Label(String),
}

/// One statement of a frame script (the only forms the packed HUD movies contain).
#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    Stop(Vec<String>),
    Play(Vec<String>),
    GotoPlay(Vec<String>, Target),
    GotoStop(Vec<String>, Target),
    Visible(Vec<String>, bool),
    /// `dispatchEvent(new AnimEvent(AnimEvent.END, Class.CONST))`: the constant's name and value.
    AnimEnd(String, i32),
    Unknown(String),
}

/// One timeline operation (PlaceObject2 / 3, RemoveObject2).
#[derive(Clone, Debug, Default)]
pub struct Op {
    pub depth: i32,
    pub remove: bool,
    pub is_move: bool,
    pub ch: Option<u32>,
    pub matrix: Option<Matrix>,
    pub cx: Option<Cx>,
    pub name: Option<Arc<str>>,
    pub clip: Option<i32>,
    pub ratio: Option<f32>,
    pub blend: Option<u8>,
    pub visible: Option<bool>,
}

/// (IMPL_ER23B) The text fields' localisation hook (`^KEY` links) is applied when a movie is read. Off = the authoring
/// placeholders show, as before (test app: KH_UI_LOCALISE=0; Elden Ring: `hud.localise = 0`). Set before the pack is read.
static LOCALISE_OFF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_localise(on: bool) {
    LOCALISE_OFF.store(!on, std::sync::atomic::Ordering::Relaxed);
}

pub fn localise_on() -> bool {
    !LOCALISE_OFF.load(std::sync::atomic::Ordering::Relaxed) && !std::env::var("KH_UI_LOCALISE").is_ok_and(|v| v == "0")
}

/// What the AS3 class of a sprite makes of it (the `tres.controls.*` base classes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Control {
    #[default]
    Plain,
    /// `tres.controls.LanguageObject`: stops on the label of `System.language` (or `none`).
    Language,
    /// `tres.controls.WorldObject`: stops on the label of `System.world` (or the first label).
    World,
    /// `tres.controls.TextFieldAdjust`: `text` goes to the child `textField`, squeezed to fit.
    TextAdjust,
    /// `tres.controls.BitmapNumber`: digits drawn from a bitmap strip.
    BitmapNumber,
    /// `tres.controls.Face`: hosts a face movie.
    Face,
    /// `tres.controls.Icon`: hosts an icon texture (host-supplied).
    Icon,
    /// (IMPL_ER23B) `tres.controls.ImageIcon`: one cell of a bitmap sheet, drawn by script (`controls::image_icon_set`);
    /// its authoring placeholder (a white rectangle) is removed by the constructor.
    ImageIcon,
}

#[derive(Clone, Debug, Default)]
pub struct SpriteDef {
    pub id: u32,
    pub frames: u32,
    pub labels: Vec<(String, u32)>,
    /// `ops[frame - 1]`.
    pub ops: Vec<Vec<Op>>,
    /// `scripts[frame - 1]`.
    pub scripts: Vec<Vec<Stmt>>,
    pub class: Option<String>,
    pub control: Control,
    /// (IMPL_ER21) `DefineScalingGrid` of this sprite (the pack's `scale9`: the grids of this pack all belong to
    /// sprites): the rectangle, in the sprite's own space, whose outside keeps its size when the sprite is scaled.
    pub scale9: Option<Rect>,
}

impl SpriteDef {
    pub fn label_frame(&self, label: &str) -> Option<u32> {
        self.labels.iter().find(|(l, _)| l == label).map(|(_, f)| *f)
    }

    /// The label at or before `frame` (AS3 `currentLabel`).
    pub fn label_at(&self, frame: u32) -> Option<&str> {
        self.labels.iter().filter(|(_, f)| *f <= frame).max_by_key(|(_, f)| *f).map(|(l, _)| l.as_str())
    }
}

#[derive(Clone, Debug)]
pub enum Char {
    Shape(Arc<Shape>),
    Morph(Arc<Shape>, Arc<Shape>),
    Text(Arc<TextDef>),
    Sprite(Arc<SpriteDef>),
    Image(Arc<ImageChar>),
    /// A symbol of a shared library movie (`90_rsl/*`), placed by its AS3 class name.
    Library(Arc<MovieDef>, Arc<SpriteDef>),
}

#[derive(Debug)]
pub struct MovieDef {
    pub name: String,
    pub stage: (f32, f32),
    pub fps: f32,
    pub chars: HashMap<u32, Char>,
    pub root: Arc<SpriteDef>,
    /// AS3 class name (`.` separated) -> character id (`SymbolClass`).
    pub symbols: HashMap<String, u32>,
    /// Bitmap symbol name -> the fill that draws it at 1:1 (for `BitmapManager.getBitmapData`).
    pub bitmaps: HashMap<String, BitmapFill>,
    /// Constants of the document class.
    pub consts: HashMap<String, f64>,
    /// Frame-script statements that were not understood (must be empty for the HUD movies).
    pub unknown_statements: Vec<String>,
}

impl MovieDef {
    pub fn sprite(&self, id: u32) -> Option<&Arc<SpriteDef>> {
        match self.chars.get(&id) {
            Some(Char::Sprite(s)) => Some(s),
            _ => None,
        }
    }
}

/// Character ids from here on are symbols of library movies (`Char::Library`).
pub const LIBRARY_CHAR_BASE: u32 = 0x4000_0000;

/// A quadratic outline segment of a glyph (font units, y down, origin on the baseline).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seg {
    Move(f32, f32),
    Line(f32, f32),
    Quad(f32, f32, f32, f32),
}

#[derive(Clone, Debug, Default)]
pub struct Glyph {
    pub advance: f32,
    pub contours: Vec<Vec<Seg>>,
}

#[derive(Debug, Default)]
pub struct Font {
    pub class_name: String,
    pub units_per_em: f32,
    pub ascent: f32,
    pub descent: f32,
    pub leading: f32,
    pub glyphs: HashMap<u32, Glyph>,
}

impl Font {
    /// The glyph of a code point, `?` when the font lacks it.
    pub fn glyph(&self, code: u32) -> Option<&Glyph> {
        self.glyphs.get(&code).or_else(|| self.glyphs.get(&63))
    }

    /// Advance width of `text` at `size` pixels per em.
    pub fn text_width(&self, text: &str, size: f32) -> f32 {
        let sc = size / self.units_per_em;
        text.chars().filter_map(|c| self.glyph(c as u32)).map(|g| g.advance * sc).sum()
    }
}

/// The pack on disk, with caches.
pub struct Pack {
    pub dir: PathBuf,
    textures: HashMap<String, Arc<Texture>>,
    movies: HashMap<String, Arc<MovieDef>>,
    fonts: HashMap<String, Arc<Font>>,
    strings: HashMap<String, Arc<HashMap<String, HashMap<String, String>>>>,
}

#[derive(Debug)]
pub struct PackError(pub String);

impl std::fmt::Display for PackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for PackError {}

fn err<T>(what: impl Into<String>) -> Result<T, PackError> {
    Err(PackError(what.into()))
}

fn f(v: &Value) -> f32 {
    v.as_f64().unwrap_or(0.0) as f32
}

fn matrix(v: &Value) -> Matrix {
    Matrix::new(f(&v[0]), f(&v[1]), f(&v[2]), f(&v[3]), f(&v[4]), f(&v[5]))
}

fn color(v: &Value) -> [f32; 4] {
    if v.is_array() {
        [f(&v[0]), f(&v[1]), f(&v[2]), f(&v[3])]
    } else {
        [0.0; 4]
    }
}

fn rect0(v: &Value) -> Rect0 {
    Rect0 { x0: f(&v[0]), y0: f(&v[1]), x1: f(&v[2]), y1: f(&v[3]) }
}

fn edges(v: &Value) -> Vec<Edge> {
    v.as_array()
        .map(|a| {
            a.iter()
                .map(|e| {
                    if e.as_array().map_or(0, |x| x.len()) == 4 {
                        Edge::Line([f(&e[0]), f(&e[1]), f(&e[2]), f(&e[3])])
                    } else {
                        Edge::Curve([f(&e[0]), f(&e[1]), f(&e[2]), f(&e[3]), f(&e[4]), f(&e[5])])
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Reads a PNG into RGBA8.
pub fn read_png(path: &Path) -> Result<(u32, u32, Vec<u8>), PackError> {
    let file = fs::File::open(path).map_err(|e| PackError(format!("{}: {e}", path.display())))?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| PackError(format!("{}: {e}", path.display())))?;
    let size = reader.output_buffer_size().ok_or_else(|| PackError(format!("{}: too large", path.display())))?;
    let mut buf = vec![0u8; size];
    let info = reader.next_frame(&mut buf).map_err(|e| PackError(format!("{}: {e}", path.display())))?;
    let (w, h) = (info.width, info.height);
    let n = (w * h) as usize;
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..n * 4].to_vec(),
        png::ColorType::Rgb => buf[..n * 3].chunks(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::Grayscale => buf[..n].iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf[..n * 2].chunks(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        other => return err(format!("{}: colour type {other:?} not handled", path.display())),
    };
    Ok((w, h, rgba))
}

/// Writes RGBA8 as a PNG (tests and comparisons).
pub fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), PackError> {
    let file = fs::File::create(path).map_err(|e| PackError(format!("{}: {e}", path.display())))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| PackError(format!("{}: {e}", path.display())))?;
    writer.write_image_data(rgba).map_err(|e| PackError(format!("{}: {e}", path.display())))
}

fn parse_script(lines: &Value, consts: &HashMap<String, f64>, unknown: &mut Vec<String>) -> Vec<Stmt> {
    let mut out = Vec::new();
    for line in lines.as_array().into_iter().flatten() {
        let text = line.as_str().unwrap_or("").trim().trim_end_matches(';').trim();
        if text.is_empty() {
            continue;
        }
        out.push(parse_stmt(text, consts).unwrap_or_else(|| {
            unknown.push(text.to_string());
            Stmt::Unknown(text.to_string())
        }));
    }
    out
}

fn parse_path(text: &str) -> Vec<String> {
    text.split('.').map(str::trim).filter(|p| !p.is_empty() && *p != "this").map(str::to_string).collect()
}

fn parse_stmt(text: &str, consts: &HashMap<String, f64>) -> Option<Stmt> {
    const EVENT: &str = "dispatchEvent(new AnimEvent(AnimEvent.END, ";
    if let Some(rest) = text.strip_prefix(EVENT) {
        let name = rest.trim_end_matches(')').rsplit('.').next()?.trim().to_string();
        let value = consts.get(&name).copied().unwrap_or(-1.0) as i32;
        return Some(Stmt::AnimEnd(name, value));
    }
    if let Some((lhs, rhs)) = text.split_once(" = ") {
        let path = lhs.strip_suffix(".visible")?;
        let value = match rhs.trim() {
            "true" => true,
            "false" => false,
            _ => return None,
        };
        return Some(Stmt::Visible(parse_path(path), value));
    }
    let open = text.find('(')?;
    let arg = text[open + 1..].strip_suffix(')')?.trim();
    let mut path = parse_path(&text[..open]);
    let method = path.pop()?;
    let target = || -> Option<Target> {
        if let Some(label) = arg.strip_prefix('"').and_then(|a| a.strip_suffix('"')) {
            Some(Target::Label(label.to_string()))
        } else {
            arg.parse::<u32>().ok().map(Target::Frame)
        }
    };
    match method.as_str() {
        "stop" if arg.is_empty() => Some(Stmt::Stop(path)),
        "play" if arg.is_empty() => Some(Stmt::Play(path)),
        "gotoAndPlay" => Some(Stmt::GotoPlay(path, target()?)),
        "gotoAndStop" => Some(Stmt::GotoStop(path, target()?)),
        _ => None,
    }
}

impl Pack {
    /// `dir` = `build/sora_pack/ui`.
    pub fn open(dir: impl Into<PathBuf>) -> Result<Pack, PackError> {
        let dir = dir.into();
        if !dir.join("manifest.json").is_file() {
            return err(format!("{}: no manifest.json (run node tools/sora_ui_pack.js)", dir.display()));
        }
        Ok(Pack { dir, textures: HashMap::new(), movies: HashMap::new(), fonts: HashMap::new(), strings: HashMap::new() })
    }

    /// The pack of this repository, found from the crate's own location (tests, examples).
    pub fn open_default() -> Result<Pack, PackError> {
        // KH_UI_PACK overrides the location (a build outside the tree).
        match std::env::var_os("KH_UI_PACK") {
            Some(dir) => Pack::open(PathBuf::from(dir)),
            None => Pack::open(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/ui")),
        }
    }

    fn json(&self, rel: &str) -> Result<Value, PackError> {
        let path = self.dir.join(rel);
        let text = fs::read_to_string(&path).map_err(|e| PackError(format!("{}: {e}", path.display())))?;
        serde_json::from_str(&text).map_err(|e| PackError(format!("{}: {e}", path.display())))
    }

    pub fn texture(&mut self, file: &str) -> Result<Arc<Texture>, PackError> {
        if let Some(t) = self.textures.get(file) {
            return Ok(t.clone());
        }
        let (width, height, rgba) = read_png(&self.dir.join(file))?;
        let t = Arc::new(Texture { file: file.to_string(), width, height, rgba });
        self.textures.insert(file.to_string(), t.clone());
        Ok(t)
    }

    /// A UI font by class name (`$KH2Menu`, `$sougei`, ...).
    pub fn font(&mut self, class_name: &str) -> Result<Arc<Font>, PackError> {
        if let Some(font) = self.fonts.get(class_name) {
            return Ok(font.clone());
        }
        let j = self.json(&format!("fonts/{}.json", class_name.trim_start_matches('$')))?;
        let mut font = Font { class_name: class_name.to_string(), units_per_em: f(&j["units_per_em"]), ascent: f(&j["ascent"]), descent: f(&j["descent"]), leading: f(&j["leading"]), glyphs: HashMap::new() };
        for (code, g) in j["glyphs"].as_object().into_iter().flatten() {
            let contours = g["path"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|contour| {
                    contour
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|s| match s[0].as_str() {
                            Some("M") => Seg::Move(f(&s[1]), f(&s[2])),
                            Some("L") => Seg::Line(f(&s[1]), f(&s[2])),
                            _ => Seg::Quad(f(&s[1]), f(&s[2]), f(&s[3]), f(&s[4])),
                        })
                        .collect()
                })
                .collect();
            font.glyphs.insert(code.parse().unwrap_or(0), Glyph { advance: f(&g["adv"]), contours });
        }
        let font = Arc::new(font);
        self.fonts.insert(class_name.to_string(), font.clone());
        Ok(font)
    }

    /// An English string by its full key, e.g. `kh3_system.command.attack` (table = the first
    /// segment, namespace = everything before the last dot).
    pub fn string(&mut self, key: &str) -> Option<String> {
        let table = key.split('.').next()?;
        if !self.strings.contains_key(table) {
            let mut map = HashMap::new();
            if let Ok(j) = self.json(&format!("strings/en/{table}.json")) {
                for (ns, entries) in j["namespaces"].as_object().into_iter().flatten() {
                    let m: HashMap<String, String> = entries.as_object().into_iter().flatten().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string()))).collect();
                    map.insert(ns.clone(), m);
                }
            }
            self.strings.insert(table.to_string(), Arc::new(map));
        }
        let (ns, name) = key.rsplit_once('.')?;
        self.strings.get(table)?.get(ns)?.get(name).cloned()
    }

    /// Every English string of a table by its full key (`kh3_camp.top.menu_0001`).
    pub fn string_table(&mut self, table: &str) -> HashMap<String, String> {
        self.string(&format!("{table}.-.-"));
        let mut out = HashMap::new();
        if let Some(namespaces) = self.strings.get(table) {
            for (ns, entries) in namespaces.iter() {
                for (key, text) in entries {
                    out.insert(format!("{ns}.{key}"), text.clone());
                }
            }
        }
        out
    }

    /// A data file of the pack (`data/<name>.json`).
    pub fn data(&self, name: &str) -> Result<Value, PackError> {
        self.json(&format!("data/{name}.json"))
    }

    pub fn movie(&mut self, name: &str) -> Result<Arc<MovieDef>, PackError> {
        self.movie_linked(name, &[])
    }

    /// A movie whose timelines place symbols of shared library movies by class name
    /// (`tres.rsl.TopMenuCmd` ...): `libraries` are searched in order after the movie itself.
    pub fn movie_linked(&mut self, name: &str, libraries: &[&str]) -> Result<Arc<MovieDef>, PackError> {
        if let Some(m) = self.movies.get(name) {
            return Ok(m.clone());
        }
        let mut libs = Vec::new();
        // A library may itself place symbols of the libraries listed before it (ConfigMenu holds
        // MainLibrary's HelpText and ItemListLibrary's ListScrollBar).
        for (i, library) in libraries.iter().enumerate() {
            libs.push(self.movie_linked(library, &libraries[..i])?);
        }
        // Class name -> character id of this movie (SymbolClass), for class placements.
        let own_symbols: HashMap<String, u32> = {
            let j = self.json(&format!("movies/{name}.json"))?;
            j["symbols"].as_object().into_iter().flatten().filter_map(|(id, n)| Some((n.as_str()?.replace("::", "."), id.parse().ok()?))).collect()
        };
        let library_chars: std::cell::RefCell<Vec<(Arc<MovieDef>, Arc<SpriteDef>)>> = std::cell::RefCell::new(Vec::new());
        let resolve_class = |class: &str| -> Option<u32> {
            let class = class.replace("::", ".");
            if let Some(id) = own_symbols.get(&class) {
                return Some(*id);
            }
            for lib in &libs {
                if let Some(def) = lib.symbols.get(&class).and_then(|id| lib.sprite(*id)) {
                    let mut list = library_chars.borrow_mut();
                    let at = list.iter().position(|(m, d)| Arc::ptr_eq(m, lib) && d.id == def.id).unwrap_or_else(|| {
                        list.push((lib.clone(), def.clone()));
                        list.len() - 1
                    });
                    return Some(LIBRARY_CHAR_BASE + at as u32);
                }
            }
            None
        };
        let j = self.json(&format!("movies/{name}.json"))?;
        if j["format"].as_str() != Some("kh3-ui-movie/1") {
            return err(format!("movies/{name}.json: format {:?} is not kh3-ui-movie/1", j["format"]));
        }
        let images = &j["images"];
        // image id -> (texture, declared size, sub-image offset)
        let resolve = |pack: &mut Pack, id: &str| -> Result<Option<(Arc<Texture>, (f32, f32), (f32, f32))>, PackError> {
            let mut img = &images[id];
            let mut off = (0.0, 0.0);
            if let Some(parent) = img.get("sub_image_of") {
                off = (f(&img["rect"][0]), f(&img["rect"][1]));
                img = &images[parent.to_string().trim_matches('"')];
            }
            let Some(file) = img["texture"].as_str() else { return Ok(None) };
            Ok(Some((pack.texture(file)?, (f(&img["declared"][0]), f(&img["declared"][1])), off)))
        };
        let consts: HashMap<String, f64> = j["as3"]["constants"].as_object().into_iter().flatten().filter_map(|(k, v)| Some((k.clone(), v.as_f64()?))).collect();
        let classes = &j["as3"]["classes"];
        let control_of = |class: Option<&str>| -> Control {
            let mut name = class.map(|c| c.replace("::", "."));
            for _ in 0..8 {
                let Some(n) = name else { break };
                match n.as_str() {
                    "tres.controls.LanguageObject" => return Control::Language,
                    "tres.controls.WorldObject" => return Control::World,
                    "tres.controls.TextFieldAdjust" => return Control::TextAdjust,
                    "tres.controls.BitmapNumber" => return Control::BitmapNumber,
                    "tres.controls.Face" => return Control::Face,
                    "tres.controls.Icon" => return Control::Icon,
                    "tres.controls.ImageIcon" => return Control::ImageIcon,
                    _ => {}
                }
                name = classes[&n]["super"].as_str().map(str::to_string);
            }
            Control::Plain
        };
        let mut unknown = Vec::new();
        let sprite = |id: u32, s: &Value, unknown: &mut Vec<String>| -> SpriteDef {
            let frames = s["frames"].as_u64().unwrap_or(1).max(1) as u32;
            let mut def = SpriteDef { id, frames, ops: vec![Vec::new(); frames as usize], scripts: vec![Vec::new(); frames as usize], class: s["class"].as_str().map(|c| c.replace("::", ".")), scale9: j["scale9"].get(id.to_string()).filter(|g| g.is_array()).map(|g| rect0(g).rect()), ..SpriteDef::default() };
            def.control = control_of(def.class.as_deref());
            for (label, frame) in s["labels"].as_object().into_iter().flatten() {
                def.labels.push((label.clone(), frame.as_u64().unwrap_or(1) as u32));
            }
            def.labels.sort_by_key(|(_, frame)| *frame);
            for (frame, ops) in s["timeline"].as_object().into_iter().flatten() {
                let Some(slot) = frame.parse::<usize>().ok().and_then(|n| def.ops.get_mut(n.wrapping_sub(1))) else { continue };
                for op in ops.as_array().into_iter().flatten() {
                    slot.push(Op {
                        depth: op["d"].as_i64().unwrap_or(0) as i32,
                        remove: op["remove"].as_bool().unwrap_or(false),
                        is_move: op["move"].as_bool().unwrap_or(false),
                        ch: op["ch"].as_u64().map(|c| c as u32).or_else(|| op["class"].as_str().and_then(&resolve_class)),
                        matrix: op.get("m").map(matrix),
                        cx: op.get("cx").map(|c| Cx { mul: [f(&c[0]), f(&c[1]), f(&c[2]), f(&c[3])], add: [f(&c[4]), f(&c[5]), f(&c[6]), f(&c[7])] }),
                        name: op["name"].as_str().map(Arc::from),
                        clip: op["clip"].as_i64().map(|c| c as i32),
                        ratio: op["ratio"].as_f64().map(|r| r as f32 / 65535.0),
                        blend: op["blend"].as_u64().map(|b| b as u8),
                        visible: op["visible"].as_bool(),
                    });
                }
            }
            for (frame, lines) in s["scripts"].as_object().into_iter().flatten() {
                if let Some(slot) = frame.parse::<usize>().ok().and_then(|n| def.scripts.get_mut(n.wrapping_sub(1))) {
                    *slot = parse_script(lines, &consts, unknown);
                }
            }
            def
        };
        let mut chars: HashMap<u32, Char> = HashMap::new();
        let scale9 = &j["scale9"];
        let shape = |pack: &mut Pack, id: u32, s: &Value| -> Result<Shape, PackError> {
            let mut out = Shape { id, bounds: rect0(&s["bounds"]), ..Shape::default() };
            if let Some(g) = scale9.get(id.to_string()) {
                out.scale9 = Some(rect0(g).rect());
            }
            for fill in s["fills"].as_array().into_iter().flatten() {
                let paint = match fill["kind"].as_str() {
                    Some("solid") => Paint::Solid(color(&fill["color"])),
                    Some("bitmap") => match resolve(pack, &fill["image"].to_string())? {
                        Some((texture, declared, off)) => {
                            let m = matrix(&fill["matrix"]).mul(&Matrix::translate(-off.0, -off.1));
                            Paint::Bitmap(BitmapFill { scale: (texture.width as f32 / declared.0, texture.height as f32 / declared.1), texture, matrix: m, repeat: fill["repeat"].as_bool().unwrap_or(false), smooth: fill["smooth"].as_bool().unwrap_or(true) })
                        }
                        None => Paint::Solid([255.0, 0.0, 255.0, 128.0]),
                    },
                    Some(kind @ ("linear" | "radial")) => Paint::Gradient {
                        radial: kind == "radial",
                        matrix: matrix(&fill["matrix"]),
                        stops: fill["stops"].as_array().into_iter().flatten().map(|s| (f(&s[0]) / 255.0, [f(&s[1]), f(&s[2]), f(&s[3]), f(&s[4])])).collect(),
                    },
                    _ => Paint::None,
                };
                out.fills.push(Fill { paint, edges: edges(&fill["edges"]) });
            }
            for stroke in s["strokes"].as_array().into_iter().flatten() {
                out.strokes.push(Stroke { width: f(&stroke["width"]), color: color(&stroke["color"]), edges: edges(&stroke["edges"]) });
            }
            Ok(out)
        };
        for (id, s) in j["shapes"].as_object().into_iter().flatten() {
            let id: u32 = id.parse().unwrap_or(0);
            chars.insert(id, Char::Shape(Arc::new(shape(self, id, s)?)));
        }
        for (id, s) in j["morphs"].as_object().into_iter().flatten() {
            let id: u32 = id.parse().unwrap_or(0);
            chars.insert(id, Char::Morph(Arc::new(shape(self, id, &s["start"])?), Arc::new(shape(self, id, &s["end"])?)));
        }
        for (id, t) in j["texts"].as_object().into_iter().flatten() {
            let id: u32 = id.parse().unwrap_or(0);
            let html = t["initial"].as_str().unwrap_or("");
            let attr = |name: &str| -> Option<&str> {
                let at = html.find(&format!("{name}=\""))? + name.len() + 2;
                Some(&html[at..at + html[at..].find('"')?])
            };
            let mut def = TextDef {
                id,
                bounds: rect0(&t["bounds"]),
                font_class: t["font_class"].as_str().map(str::to_string),
                size: t["size"].as_f64().unwrap_or(20.0) as f32,
                color: if t["color"].is_array() { color(&t["color"]) } else { [255.0; 4] },
                align: match t["align"].as_str() {
                    Some("right") => Align::Right,
                    Some("center") => Align::Center,
                    _ => Align::Left,
                },
                initial: html.to_string(),
                word_wrap: t["word_wrap"].as_bool().unwrap_or(false),
                leading: t["leading"].as_f64().unwrap_or(0.0) as f32,
            };
            if t["html"].as_bool().unwrap_or(false) {
                // The same attributes tools/lib/gfx_render.js reads from the initial HTML.
                match attr("align") {
                    Some("right") => def.align = Align::Right,
                    Some("center") => def.align = Align::Center,
                    Some(_) => def.align = Align::Left,
                    None => {}
                }
                if let Some(size) = attr("size").and_then(|s| s.parse().ok()) {
                    def.size = size;
                }
                if let Some(c) = attr("color").and_then(|c| u32::from_str_radix(c.trim_start_matches('#'), 16).ok()) {
                    def.color = [((c >> 16) & 255) as f32, ((c >> 8) & 255) as f32, (c & 255) as f32, 255.0];
                }
                if let Some(face) = attr("face") {
                    def.font_class = Some(face.to_string());
                }
                let mut plain = String::new();
                let mut in_tag = false;
                for c in html.chars() {
                    match c {
                        '<' => in_tag = true,
                        '>' => in_tag = false,
                        c if !in_tag => plain.push(c),
                        _ => {}
                    }
                }
                def.initial = plain.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&");
                // (IMPL_ER23B) THE MOVIE'S OWN LOCALISATION HOOK: a field whose authored text is a link `<a href="^KEY{fallback}">`
                // shows the string table's text of KEY in the game (Scaleform's translator; the visible authoring text is a
                // placeholder: "t" where the EXP popup's unit glyph goes, "AAAAA" for "EXP", "LV.up! " ...).
                if localise_on() {
                    if let Some(key) = html.split("href=\"^").nth(1).and_then(|rest| rest.split(['{', '"']).next()).filter(|k| !k.is_empty()) {
                        if let Some(text) = self.string(key) {
                            def.initial = text.replace("<br>", "
");
                        }
                    }
                }
            }
            chars.insert(id, Char::Text(Arc::new(def)));
        }
        for (id, s) in j["sprites"].as_object().into_iter().flatten() {
            let id: u32 = id.parse().unwrap_or(0);
            chars.insert(id, Char::Sprite(Arc::new(sprite(id, s, &mut unknown))));
        }
        let mut bitmaps = HashMap::new();
        for (id, img) in images.as_object().into_iter().flatten() {
            let Some((texture, declared, off)) = resolve(self, id)? else { continue };
            let size = if img.get("sub_image_of").is_some() { (f(&img["rect"][2]), f(&img["rect"][3])) } else { declared };
            let fill = BitmapFill { scale: (texture.width as f32 / declared.0, texture.height as f32 / declared.1), texture, matrix: Matrix::translate(-off.0, -off.1), repeat: false, smooth: true };
            if let Some(name) = img["export_name"].as_str() {
                bitmaps.insert(name.to_string(), fill.clone());
            }
            let id: u32 = id.parse().unwrap_or(0);
            chars.entry(id).or_insert_with(|| Char::Image(Arc::new(ImageChar { size, fill })));
        }
        // The root timeline is parsed before the library symbols are registered: it may be the
        // only user of one (CampMenu's `stageFrame`).
        let root = Arc::new(sprite(0, &j["root"], &mut unknown));
        for (at, (lib, def)) in library_chars.borrow().iter().enumerate() {
            chars.insert(LIBRARY_CHAR_BASE + at as u32, Char::Library(lib.clone(), def.clone()));
        }
        let symbols = j["symbols"].as_object().into_iter().flatten().filter_map(|(id, name)| Some((name.as_str()?.replace("::", "."), id.parse().ok()?))).collect();
        let movie = Arc::new(MovieDef { name: name.to_string(), stage: (f(&j["stage"][0]), f(&j["stage"][1])), fps: f(&j["fps"]), chars, root, symbols, bitmaps, consts, unknown_statements: unknown });
        self.movies.insert(name.to_string(), movie.clone());
        Ok(movie)
    }
}
