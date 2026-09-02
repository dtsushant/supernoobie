//! Drives the whole studio with a simulated hand: draws a figure, chooses it,
//! gives it something to do, and renders four moments of it running — with the
//! toolbar, exactly as the window paints it. For the eye; the claims are unit
//! tests.

use easel::bar::Bar;
use easel::{Action, Board};
use plotkit::{Canvas, Cx, View};
use shapes::Nib;
use std::f64::consts::TAU;

fn draw(b: &mut Board, path: &[Cx]) {
    for z in path {
        b.pointer(*z, true);
    }
    b.pointer(*path.last().expect("a path"), false);
}

/// A hand: the path you meant, plus a tremble you did not.
fn hand(path: Vec<Cx>) -> Vec<Cx> {
    path.into_iter()
        .enumerate()
        .map(|(k, z)| z + Cx::new(0.008 * (k as f64 * 2.7).sin(), 0.008 * (k as f64 * 4.4).sin()))
        .collect()
}

fn ring(r: f64, at: Cx) -> Vec<Cx> {
    hand((0..=70).map(|k| at + Cx::polar(r, k as f64 / 70.0 * TAU)).collect())
}

fn line(from: Cx, to: Cx) -> Vec<Cx> {
    hand((0..=30).map(|k| from + (to - from).scale(k as f64 / 30.0)).collect())
}

/// Pick up whatever is at this point.
fn choose(b: &mut Board, at: Cx) {
    b.selected.clear();
    b.pointer(at, true);
    b.pointer(at, false);
}

#[test]
fn studio() {
    let mut b = Board::new();
    b.nib = Nib::Quill { slow: 0.13, fast: 0.02, pace: 0.16 };
    b.taper = 0.12;

    // --- a little figure: head, body, two arms, two legs ------------------
    b.colour = 0xE0A44A;
    let head = Cx::new(-3.4, 1.6);
    draw(&mut b, &ring(0.42, head));
    draw(&mut b, &line(Cx::new(-3.4, 1.1), Cx::new(-3.4, 0.0)));
    draw(&mut b, &line(Cx::new(-3.4, 0.85), Cx::new(-3.9, 0.35)));
    draw(&mut b, &line(Cx::new(-3.4, 0.85), Cx::new(-2.9, 0.35)));
    draw(&mut b, &line(Cx::new(-3.4, 0.0), Cx::new(-3.75, -0.7)));
    draw(&mut b, &line(Cx::new(-3.4, 0.0), Cx::new(-3.05, -0.7)));

    // The figure walks, then jumps. Its six strokes are bound into one figure
    // first, so all of it is told at once -- otherwise the head walks off and
    // leaves the body standing.
    b.selected = (0..b.sheet.len()).collect();
    b.group();
    b.give(Action::Walk(Cx::new(1.1, 0.0)), Some(2.0));
    b.give(Action::Jump { height: 1.0, rate: 1.5 }, Some(2.0));
    b.selected.clear();

    // --- a ball that bounces, and a square that spins ---------------------
    b.colour = 0x4FBCD4;
    let ball = Cx::new(0.4, -1.5);
    draw(&mut b, &ring(0.45, ball));
    choose(&mut b, ball + Cx::new(0.45, 0.0));
    b.give(Action::Jump { height: 1.4, rate: 1.5 }, Some(2.0));

    b.colour = 0xE585AC;
    let box_at = Cx::new(2.6, -1.5);
    let corners: Vec<Cx> = [(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5), (-0.5, -0.5)]
        .iter()
        .flat_map(|(x, y)| {
            let c = box_at + Cx::new(*x, *y);
            (0..8).map(move |_| c)
        })
        .collect();
    // Walk the corners properly rather than teleporting between them.
    let square: Vec<Cx> = corners.windows(2).flat_map(|w| line(w[0], w[1])).collect();
    draw(&mut b, &square);
    choose(&mut b, box_at + Cx::new(0.5, 0.0));
    b.give(Action::Spin(0.5), Some(2.0));

    b.colour = 0x6FCF97;
    let blob = Cx::new(4.6, -1.5);
    draw(&mut b, &ring(0.5, blob));
    choose(&mut b, blob + Cx::new(0.5, 0.0));
    b.give(Action::Bob { height: 0.4, rate: 0.5 }, Some(2.0));
    b.give(Action::Pulse { amount: 0.25, rate: 0.5 }, Some(2.0));

    assert!(b.has_animation(), "something should move");

    // --- four moments, side by side ---------------------------------------
        let (w, h) = (1280, 860);
    let mut sheet = Canvas::new(w, h);
    sheet.clear(0x0B1017);

    for (k, t) in [0.0, 0.9, 2.3, 3.4].into_iter().enumerate() {
        b.clock = t;
        let mut tile = Canvas::new(w - 190, h / 4);
        tile.clear(if k % 2 == 0 { 0x0B1017 } else { 0x0D131B });
        let view = View::centred(w - 190, h / 4, 40.0);
        b.frame().draw(&mut tile, &view);
        // Copy the strip into place.
        let top = k * (h / 4);
        for y in 0..h / 4 {
            for x in 0..w - 190 {
                let p = tile.buf[y * (w - 190) + x];
                sheet.px((190 + x) as i32, (top + y) as i32, p);
            }
        }
        sheet.text(200, (top + 6) as i32, &format!("t = {t:.1}s"), 0x6B7987, 2);
    }

    // The toolbar, painted by the same code the window uses.
    let mut furniture = plotkit::Frame::new();
    b.playing = true;
    Bar::new(w as i32).paint(&mut furniture, &b, w as i32);
    easel::Tree::new(&b).paint(&mut furniture, &b, h as i32);
    furniture.draw(&mut sheet, &View::centred(w, h, 60.0));

    let out = std::env::temp_dir().join("studio.png");
    sheet.write_png(out.to_str().expect("a path")).expect("wrote the studio");
    println!("look at {}", out.display());
}
