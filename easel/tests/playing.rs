//! Four moments of the addition game being played, so a human can see it.
use easel::Board;
use plotkit::{Canvas, Cx, View};

#[test]
fn playing() {
    let mut b = Board::new();
    if b.load("../samples/adding.easel").is_err() {
        return;
    }
    b.playing_game = true;

    let (w, h) = (1240, 800);
    let (tw, th) = (w / 2, h / 2);
    let mut sheet = Canvas::new(w, h);
    sheet.clear(0x0B1017);

    // Right, right, wrong, wrong -- so the smile and the ghost both show.
    let taps = [0.0f64, 0.0, -3.4, -3.4];
    for k in 0..4 {
        let mut tile = Canvas::new(tw, th);
        tile.clear(if k % 2 == 0 { 0x0B1017 } else { 0x0D131B });
        b.frame().draw(&mut tile, &View::centred(tw, th, 48.0));
        let (ox, oy) = ((k % 2) * tw, (k / 2) * th);
        for y in 0..th {
            for x in 0..tw {
                sheet.px((ox + x) as i32, (oy + y) as i32, tile.buf[y * tw + x]);
            }
        }
        let score = b.written().vars.iter().find(|(n, _)| n == "score").map(|(_, v)| v.re).unwrap_or(0.0);
        sheet.text((ox + 10) as i32, (oy + 8) as i32, &format!("tap {k}: score {score}"), 0x6B7987, 1);
        let at = Cx::new(taps[k], -2.2);
        b.pointer(at, true);
        b.pointer(at, false);
        // A moment later, so the face is up but not yet faded.
        b.clock += 0.25;
    }

    let out = std::env::temp_dir().join("playing.png");
    sheet.write_png(out.to_str().expect("a path")).expect("wrote it");
}
