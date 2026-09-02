//! Keyframes driven through the real board: draw a figure, wind the clock,
//! drag it where it should be, and render the result with the onion skin. For
//! the eye; the claims are unit tests.

use easel::bar::Bar;
use easel::Board;
use plotkit::{Canvas, Cx, View};
use shapes::Nib;
use std::f64::consts::TAU;

fn draw(b: &mut Board, path: &[Cx]) {
    for z in path {
        b.pointer(*z, true);
    }
    b.pointer(*path.last().expect("a path"), false);
}

fn ring(r: f64, at: Cx) -> Vec<Cx> {
    (0..=70).map(|k| at + Cx::polar(r, k as f64 / 70.0 * TAU)).collect()
}

fn line(from: Cx, to: Cx) -> Vec<Cx> {
    (0..=30).map(|k| from + (to - from).scale(k as f64 / 30.0)).collect()
}

/// Drag from one point to another, in steps, as a hand would.
fn drag(b: &mut Board, from: Cx, by: Cx) {
    b.pointer(from, true);
    for k in 1..=12 {
        b.pointer(from + by.scale(k as f64 / 12.0), true);
    }
    b.pointer(from + by, false);
}

#[test]
fn keys() {
    let mut b = Board::new();
    b.nib = Nib::Quill { slow: 0.14, fast: 0.02, pace: 0.16 };
    b.taper = 0.12;

    // --- a figure of five strokes, bound into one ------------------------
    b.colour = 0xE0A44A;
    let head = Cx::new(-4.0, 1.2);
    draw(&mut b, &ring(0.4, head));
    draw(&mut b, &line(Cx::new(-4.0, 0.75), Cx::new(-4.0, -0.3)));
    draw(&mut b, &line(Cx::new(-4.0, 0.5), Cx::new(-4.5, 0.05)));
    draw(&mut b, &line(Cx::new(-4.0, 0.5), Cx::new(-3.5, 0.05)));
    draw(&mut b, &line(Cx::new(-4.0, -0.3), Cx::new(-4.0, -1.1)));
    b.selected = (0..b.sheet.len()).collect();
    b.group();

    // --- three keys: across, up, and down again --------------------------
    // Exactly what a person does: wind the clock, drag it where it belongs.
    b.tool = easel::Tool::Pick;
    b.sheet.marks[0].track.looping = false;
    // Grab it where it can be SEEN, which is where the pose has put it -- not
    // where its points happen to still be.
    let grab = |b: &Board| head + Cx::new(0.4, 0.0) + b.sheet.marks[0].pose_at(b.clock).b;
    for (at, by) in [(1.0, Cx::new(2.6, 1.6)), (2.0, Cx::new(2.6, -1.6)), (3.0, Cx::new(2.6, 1.2))] {
        b.clock = at;
        for m in b.sheet.marks.iter_mut() {
            m.track.looping = false;
        }
        let from = grab(&b);
        drag(&mut b, from, by);
    }

    assert_eq!(b.sheet.marks[0].track.len(), 4, "three drags and the key at zero");

    // --- and a shape that turns, to show the interpolation ---------------
    b.colour = 0x9B7BD4;
    let flag = Cx::new(-1.0, -2.6);
    let pole: Vec<Cx> = [(0.0, -0.6), (0.0, 0.6), (0.9, 0.35), (0.0, 0.1)]
        .windows(2)
        .flat_map(|w| line(flag + Cx::new(w[0].0, w[0].1), flag + Cx::new(w[1].0, w[1].1)))
        .collect();
    draw(&mut b, &pole);
    let flag_mark = b.sheet.len() - 1;
    b.selected = vec![flag_mark];
    // A half turn, in two steps so it is unambiguous which way round.
    for (k, at) in [1.0, 2.0].into_iter().enumerate() {
        b.clock = at;
        b.key();
        let m = &mut b.sheet.marks[flag_mark];
        let turn = Cx::polar(1.0, (k + 1) as f64 * TAU / 4.0);
        m.track.set(at, shapes::Pose::new(turn, Cx::new(2.4 * (k + 1) as f64, 0.0)), easel::Ease::Smooth);
    }

    // --- render five moments, with the onion skin on the figure ----------
    b.selected = (0..5).collect();
    let bar = Bar::new();
    let (w, h) = (1280, 900);
    let strip = h / 5;
    let mut sheet = Canvas::new(w, h);
    sheet.clear(0x0B1017);

    for (k, t) in [0.0, 0.75, 1.5, 2.25, 3.0].into_iter().enumerate() {
        b.clock = t;
        let mut tile = Canvas::new(w - 190, strip);
        tile.clear(if k % 2 == 0 { 0x0B1017 } else { 0x0D131B });
        let view = View::centred(w - 190, strip, 46.0);

        let mut f = plotkit::Frame::new();
        for ghost in b.ghosts() {
            f.add(ghost).color(0x2A3542).width(1);
        }
        b.frame().draw(&mut tile, &view);
        f.draw(&mut tile, &view);
        // The onion skin under the drawing, so it reads as a hint.
        let mut under = Canvas::new(w - 190, strip);
        under.clear(if k % 2 == 0 { 0x0B1017 } else { 0x0D131B });
        f.draw(&mut under, &view);
        b.frame().draw(&mut under, &view);

        for y in 0..strip {
            for x in 0..w - 190 {
                sheet.px((190 + x) as i32, (k * strip + y) as i32, under.buf[y * (w - 190) + x]);
            }
        }
        sheet.text(200, (k * strip + 6) as i32, &format!("t = {t:.2}s"), 0x6B7987, 2);
    }

    let mut furniture = plotkit::Frame::new();
    bar.paint(&mut furniture, &b, h as i32);
    furniture.draw(&mut sheet, &View::centred(w, h, 60.0));

    let out = std::env::temp_dir().join("keys.png");
    sheet.write_png(out.to_str().expect("a path")).expect("wrote it");
    println!("look at {}", out.display());
}
