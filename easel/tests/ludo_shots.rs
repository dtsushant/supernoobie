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

    let (w, h) = (1280, 340);
    let tw = w / 4;
    let mut sheet = Canvas::new(w, h);
    sheet.clear(0x0B1017);

    // Four turns, each: roll, then move the first token of whoever's turn it is.
    let mut note = String::new();
    for k in 0..4 {
        let mut tile = Canvas::new(tw, h);
        tile.clear(if k % 2 == 0 { 0x0B1017 } else { 0x0D131B });
        b.frame().draw(&mut tile, &View::centred(tw, h, 15.0));
        for y in 0..h {
            for x in 0..tw {
                sheet.px((k * tw + x) as i32, y as i32, tile.buf[y * tw + x]);
            }
        }
        sheet.text((k * tw + 8) as i32, 8, &note, 0x6B7987, 1);

        b.play_tap(9);
        let die = b.written().vars.iter().find(|(n, _)| n == "die").map(|(_, v)| v.re).unwrap_or(0.0);
        let turn = b.written().vars.iter().find(|(n, _)| n == "turn").map(|(_, v)| v.re).unwrap_or(0.0);
        note = format!("seat {} rolled {}", turn as i64 + 1, die as i64);
        // Try both of that seat's tokens; one of them may be able to move.
        b.play_tap(turn as u32 * 2 + 1);
        b.play_tap(turn as u32 * 2 + 2);
        b.tally.values.insert("rolled".into(), 0.0);
    }

    let out = std::env::temp_dir().join("ludoplay.png");
    sheet.write_png(out.to_str().expect("a path")).expect("wrote it");
}
