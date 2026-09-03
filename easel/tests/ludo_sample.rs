//! The Ludo sample at four moments, as the browser would show it.
use easel::Board;
use plotkit::{Canvas, View};

#[test]
fn ludo_sample() {
    let mut b = Board::new();
    if b.load("../samples/ludo.easel").is_err() {
        return;
    }
    let (w, h) = (1240, 320);
    let mut sheet = Canvas::new(w, h);
    sheet.clear(0x0B1017);
    let tw = w / 4;

    for (k, t) in [0.0, 2.5, 5.5, 9.0].into_iter().enumerate() {
        b.clock = t;
        let mut tile = Canvas::new(tw, h);
        tile.clear(if k % 2 == 0 { 0x0B1017 } else { 0x0D131B });
        b.frame().draw(&mut tile, &View::centred(tw, h, 19.0));
        for y in 0..h {
            for x in 0..tw {
                sheet.px((k * tw + x) as i32, y as i32, tile.buf[y * tw + x]);
            }
        }
        sheet.text((k * tw + 8) as i32, 8, &format!("t = {t:.1}s"), 0x6B7987, 1);
    }
    let out = std::env::temp_dir().join("ludosample.png");
    sheet.write_png(out.to_str().expect("a path")).expect("wrote it");
}
