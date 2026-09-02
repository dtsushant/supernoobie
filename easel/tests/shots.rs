//! Renders each sample so a human can see what they will get.
use easel::Board;
use plotkit::{Canvas, View};

#[test]
fn shots() {
    let dir = std::path::Path::new("../samples");
    if !dir.exists() {
        return;
    }
    let names = ["dials", "bouncing", "walker"];
    let (w, h) = (1240, 900);
    let strip = h / 3;
    let mut sheet = Canvas::new(w, h);
    sheet.clear(0x0B1017);

    for (k, name) in names.iter().enumerate() {
        let mut b = Board::new();
        if b.load(&format!("../samples/{name}.easel")).is_err() {
            continue;
        }
        for (j, t) in [0.0, 0.7, 1.4, 2.1].into_iter().enumerate() {
            b.clock = t;
            let tw = w / 4;
            let mut tile = Canvas::new(tw, strip);
            tile.clear(if (k + j) % 2 == 0 { 0x0B1017 } else { 0x0D131B });
            b.frame().draw(&mut tile, &View::centred(tw, strip, 22.0));
            for y in 0..strip {
                for x in 0..tw {
                    sheet.px((j * tw + x) as i32, (k * strip + y) as i32, tile.buf[y * tw + x]);
                }
            }
            sheet.text((j * tw + 8) as i32, (k * strip + 6) as i32, &format!("{name}  t={t:.1}"), 0x6B7987, 1);
        }
    }
    let out = std::env::temp_dir().join("shots.png");
    sheet.write_png(out.to_str().expect("a path")).expect("wrote it");
}
