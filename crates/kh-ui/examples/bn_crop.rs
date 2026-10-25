//! (BN) Development aid: crop + nearest-neighbour zoom of a PNG (to compare the harness against KH3's reference).
//!   cargo run -p kh-ui --release --example bn_crop -- IN OUT X Y W H ZOOM
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let n = |i: usize| a[i].parse::<usize>().unwrap();
    let (w, _h, rgba) = kh_ui::def::read_png(std::path::Path::new(&a[0])).unwrap();
    let (x0, y0, cw, ch, z) = (n(2), n(3), n(4), n(5), n(6));
    let mut out = vec![0u8; cw * z * ch * z * 4];
    for y in 0..ch * z {
        for x in 0..cw * z {
            let (sx, sy) = (x0 + x / z, y0 + y / z);
            let o = (y * cw * z + x) * 4;
            out[o..o + 4].copy_from_slice(&rgba[(sy * w as usize + sx) * 4..][..4]);
        }
    }
    kh_ui::def::write_png(std::path::Path::new(&a[1]), (cw * z) as u32, (ch * z) as u32, &out).unwrap();
}
