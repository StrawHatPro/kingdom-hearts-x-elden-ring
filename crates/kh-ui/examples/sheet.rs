//! Contact sheet for looking at screenshots: crops the same rectangle out of every file into a grid.
//! cargo run -p kh-ui --release --example sheet -- OUT.png COLS X Y W H [SCALE] FILE...
use kh_ui::def::{read_png, write_png};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let n = |i: usize| args[i].parse::<usize>().expect("number");
    let (out, cols, x, y, w, h) = (&args[0], n(1), n(2), n(3), n(4), n(5));
    let (scale, files) = match args[6].parse::<usize>() {
        Ok(s) => (s.max(1), &args[7..]),
        Err(_) => (1, &args[6..]),
    };
    let (cw, ch) = (w * scale, h * scale);
    let rows = files.len().div_ceil(cols);
    let mut sheet = vec![255u8; cols * cw * rows * ch * 4];
    for (i, file) in files.iter().enumerate() {
        let (iw, ih, rgba) = read_png(Path::new(file)).expect("png");
        let (ox, oy) = ((i % cols) * cw, (i / cols) * ch);
        for yy in 0..ch {
            for xx in 0..cw {
                let (sx, sy) = ((x + xx / scale).min(iw as usize - 1), (y + yy / scale).min(ih as usize - 1));
                let (k, q) = ((sy * iw as usize + sx) * 4, ((oy + yy) * cols * cw + ox + xx) * 4);
                let border = xx == 0 || yy == 0;
                for c in 0..3 {
                    sheet[q + c] = if border { 255 } else { rgba[k + c] };
                }
            }
        }
    }
    write_png(Path::new(out), (cols * cw) as u32, (rows * ch) as u32, &sheet).expect("write");
}
