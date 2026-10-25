//! (HUD2) KH3'S "OBTAINED" POPUP (movie KeyItem) WITH THE PORT'S THREE KINDS OF PICTURE, OFFLINE: an Elden Ring key item
//! (its own menu icon from the pack's `er_icons`, its caption), a KH3 key item (its KH3 icon and text), a new party
//! member (his KH3 menu portrait). Each is opened through `kh_ui::key_item::KeyItemPopup` and drawn when its `in`
//! has finished (and at a few frames of the `in`, for the VFX); the reference picture is written beside it.
//!
//!   cargo run -p kh-ui --release --example hud2_obtained -- OUT_DIR
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kh_ui::camp_menu::LIBRARIES;
use kh_ui::def::{read_png, write_png, Texture};
use kh_ui::key_item::{KeyItemPopup, Obtained, Stage};
use kh_ui::{Canvas, Fonts, Matrix, Pack, Renderer};

fn texture(path: &Path) -> Option<Arc<Texture>> {
    let (width, height, rgba) = read_png(path).ok()?;
    Some(Arc::new(Texture { file: path.display().to_string(), width, height, rgba }))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = PathBuf::from(args.first().cloned().unwrap_or_else(|| root.join("build/sora_tmp/party/HUD2/obtained").display().to_string()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut pack = Pack::open_default().expect("the UI pack");
    let fonts = Fonts::load(&mut pack).unwrap();
    let ui = root.join("build/sora_pack/ui");
    let s = |pack: &mut Pack, k: &str| pack.string(k).unwrap_or_default();
    let cases: Vec<(&str, Obtained)> = vec![
        // AN ELDEN RING KEY ITEM: Stonesword Key, iconId 3080 (EquipParamGoods 8000), its caption as the message tables give it
        ("er_key_item", Obtained { picture: texture(&ui.join("er_icons/03080.png")), name: "Stonesword Key".into(), caption: "Used to break imp statue seals.".into() }),
        // A KH3 KEY ITEM: KEY_ITEM02 (Gummiphone), TresItemData's icon item302 and kh3_item.key texts
        ("kh_key_item", Obtained { picture: texture(&ui.join("textures/Icon/Item/item302.png")), name: s(&mut pack, "kh3_item.key.key_02"), caption: s(&mut pack, "kh3_item.key.detail_key_02") }),
        // A KEYBLADE (the hook): WEP_KEYBLADE_SO_01 Hero's Origin
        ("keyblade", Obtained { picture: texture(&ui.join("textures/Icon/Item/weapon002.png")), name: s(&mut pack, "kh3_item.weapon.kb_02"), caption: s(&mut pack, "kh3_item.weapon.detail_kb_02") }),
        // A NEW PARTY MEMBER: Donald, KH3's menu portrait (UI/Icon/MenuFace/MenuFaceExDonald)
        ("character_donald", Obtained { picture: texture(&root.join("build/sora_pack/saveload/textures/MenuFaceExDonald.png")), name: s(&mut pack, "kh3_char_name.character.main_0020"), caption: "The royal magician of Disney Castle.<br>He fights with magic and heals the party.".into() /* the unlock sheet's default text.obtained.donald */ }),
    ];
    // (HUD2 round 2) the user's overflow case and the box's extremes: `-- OUT_DIR round2`
    let cases = if args.get(1).map(String::as_str) == Some("round2") {
        let limgrave = "Map of the western region of Limgrave. The demesne of Limgrave, far south of the Erdtree, is bordered to the northwest by the cliffs of Stormveil, and under its ever-clouded skies roll the great steppes of the pale lands.".to_string();
        let long = std::iter::repeat("A very long caption that keeps going so the box has to reach its tallest and the text has to step down in size.").take(9).collect::<Vec<_>>().join(" ");
        vec![
            ("er_map_limgrave_5_lines", Obtained { picture: texture(&ui.join("er_icons/00261.png")), name: "Map: Limgrave, West".into(), caption: limgrave }),
            ("one_line", Obtained { picture: texture(&ui.join("er_icons/00228.png")), name: "Stonesword Key".into(), caption: "Breaks the seal on imp statues.".into() }),
            ("very_long_step_down", Obtained { picture: texture(&ui.join("er_icons/00228.png")), name: "A long caption".into(), caption: long }),
        ]
    } else {
        cases
    };
    let reference = read_png(&root.join("build/sora_tmp/party/ui2/ref/kh3_obtained_popup.png")).ok();
    for (name, what) in cases {
        let def = pack.movie_linked(kh_ui::key_item::MOVIE, &LIBRARIES).expect("KeyItem");
        let mut popup = KeyItemPopup::new(def, fonts.clone());
        let mut sounds = Vec::new();
        popup.show(what, &mut sounds);
        let mut renderer = Renderer::new(fonts.clone());
        let mut frame = 0;
        let mut shots = Vec::new();
        while popup.stage() != Stage::Wait && frame < 400 {
            popup.tick(&mut sounds);
            frame += 1;
            if [10, 30, 50, 60, 70].contains(&frame) {
                shots.push((frame, draw(&mut renderer, &popup)));
            }
        }
        let full = draw(&mut renderer, &popup);
        write_png(&dir.join(format!("{name}.png")), 1920, 1080, &full).unwrap();
        // the VFX: frames of the `in`, each the stage's centre at half size, side by side
        let (cw, ch) = (960usize, 540usize);
        let mut strip = vec![0u8; cw * shots.len() * ch * 4];
        for (k, (_, rgba)) in shots.iter().enumerate() {
            for y in 0..ch {
                for x in 0..cw {
                    let src = ((y * 2) * 1920 + x * 2) * 4;
                    let dst = (y * cw * shots.len() + k * cw + x) * 4;
                    strip[dst..dst + 4].copy_from_slice(&rgba[src..src + 4]);
                }
            }
        }
        write_png(&dir.join(format!("{name}_in_frames.png")), (cw * shots.len()) as u32, ch as u32, &strip).unwrap();
        // beside the reference: the reference is a screenshot crop at 0.6 of the stage (its picture panel, 420 stage px
        // wide, is 253 px there): the stage from (392, 210), sampled at 1 / 0.6
        if let Some((rw, rh, rgba)) = &reference {
            let (rw, rh) = (*rw as usize, *rh as usize);
            let (ox, oy, k) = (392.0f32, 210.0f32, 1.0f32 / 0.6);
            let mut side = vec![0u8; rw * 2 * rh * 4];
            for y in 0..rh {
                for x in 0..rw {
                    side[(y * rw * 2 + x) * 4..][..4].copy_from_slice(&rgba[(y * rw + x) * 4..][..4]);
                    // a box of the stage pixels under this one
                    let (x0, y0) = (ox + x as f32 * k, oy + y as f32 * k);
                    let mut acc = [0u32; 4];
                    let mut n = 0;
                    for yy in (y0 as usize)..((y0 + k).ceil() as usize).min(1080) {
                        for xx in (x0 as usize)..((x0 + k).ceil() as usize).min(1920) {
                            for c in 0..4 {
                                acc[c] += full[(yy * 1920 + xx) * 4 + c] as u32;
                            }
                            n += 1;
                        }
                    }
                    for c in 0..4 {
                        side[(y * rw * 2 + rw + x) * 4 + c] = (acc[c] / n.max(1)) as u8;
                    }
                }
            }
            write_png(&dir.join(format!("{name}_vs_ref.png")), (rw * 2) as u32, rh as u32, &side).unwrap();
        }
        println!("{name}: in done after {frame} frames; sounds {sounds:?}; caption {:?}", popup.caption_layout());
        // A closes it: the out plays and the popup goes
        assert!(popup.decide(&mut sounds));
        let mut out = 0;
        while popup.is_open() && out < 200 {
            popup.tick(&mut sounds);
            out += 1;
        }
        println!("  closed by A after {out} frames; sounds {sounds:?}");
    }
}

/// The popup over a dim scene (the reference's dark forest stands in as a flat colour).
fn draw(renderer: &mut Renderer, popup: &KeyItemPopup) -> Vec<u8> {
    let mut canvas = Canvas::new(1920, 1080);
    renderer.draw(&mut canvas, &popup.movie, &Matrix::IDENTITY);
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, _| [0.10, 0.12, 0.09], &mut rgba);
    rgba
}
