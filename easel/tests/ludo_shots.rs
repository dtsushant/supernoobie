//! Four moments of a hot-seat game, so it can be seen rather than trusted.
use easel::Board;
use plotkit::{Canvas, View};

#[test]
fn shots() {
    let mut b = Board::new();
    if b.load("../samples/ludogame.easel").is_err() {
        return;
    }
    b.playing_game = true;
    b.watching = true;

    // One throw, watched. The die whirls, eases and stops.
    let (w, h) = (1280, 300);
    let tw = w / 5;
    let mut sheet = Canvas::new(w, h);
    sheet.clear(0x0B1017);
    b.play_tap(9);

    for (k, t) in [0.02, 0.14, 0.4, 0.9, 3.0].into_iter().enumerate() {
        b.clock = t;
        let mut tile = Canvas::new(tw, h);
        tile.clear(if k % 2 == 0 { 0x0B1017 } else { 0x0D131B });
        // Framed on the die and the turn marker rather than the whole board.
        let view = View::centred(tw, h, 30.0).with_origin(tw as f64 * 0.5 - 250.0, h as f64 * 0.5 + 95.0);
        b.frame().draw(&mut tile, &view);
        for y in 0..h {
            for x in 0..tw {
                sheet.px((k * tw + x) as i32, y as i32, tile.buf[y * tw + x]);
            }
        }
        let settled = b.written().vars.iter().find(|(n, _)| n == "settled").map(|(_, v)| v.re).unwrap_or(0.0);
        sheet.text(
            (k * tw + 8) as i32,
            8,
            &format!("{t:.2}s  {}", if settled > 0.5 { "settled" } else { "rolling" }),
            0x6B7987,
            1,
        );
    }

    let out = std::env::temp_dir().join("ludoplay.png");
    sheet.write_png(out.to_str().expect("a path")).expect("wrote it");
}
