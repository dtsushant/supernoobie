//! Drives the editor with a simulated hand and renders the page, so a human
//! can look at what the pen would actually produce. Not an assertion — the
//! claims are unit tests; this is for the eye.

use easel::Board;
use plotkit::{Canvas, Cx, View};
use shapes::Nib;
use std::f64::consts::TAU;

/// A hand: the path you meant, plus a tremble you did not.
fn hand(path: impl Iterator<Item = Cx>, shake: f64) -> Vec<Cx> {
    path.enumerate()
        .map(|(k, z)| {
            let n = k as f64;
            // Fast and small, which is what a tremble is — and irrational
            // ratios so it never repeats.
            z + Cx::new(shake * (n * 2.7).sin(), shake * (n * 3.1 * 1.618).sin())
        })
        .collect()
}

fn draw(b: &mut Board, path: &[Cx]) {
    for z in path {
        b.pointer(*z, true);
    }
    b.pointer(*path.last().expect("a path"), false);
}

#[test]
fn page() {
    let mut b = Board::new();

    // --- a quill signature, drawn quickly ---------------------------------
    b.nib = Nib::Quill { slow: 0.16, fast: 0.02, pace: 0.2 };
    b.taper = 0.15;
    b.colour = 0xE3E9EF;
    let flourish: Vec<Cx> = (0..=160)
        .map(|k| {
            let s = k as f64 / 160.0;
            // Speeds up through the middle, so the quill should thin there.
            let x = -4.0 + 8.0 * (s - 0.25 * (s * TAU).sin() / TAU);
            Cx::new(x, 2.6 + 0.9 * (x * 1.4).sin())
        })
        .collect();
    draw(&mut b, &hand(flourish.into_iter(), 0.012));

    // --- the same line with a round nib, for comparison -------------------
    b.nib = Nib::Round(0.1);
    b.colour = 0x46525E;
    draw(&mut b, &hand((0..=120).map(|k| Cx::new(-4.0 + k as f64 / 15.0, 1.2)), 0.012));

    // --- a broad nib: an O, thick and thin twice round --------------------
    b.nib = Nib::Broad { width: 0.34, angle: TAU / 8.0 };
    b.taper = 0.0;
    b.colour = 0xE585AC;
    draw(&mut b, &hand((0..=140).map(|k| Cx::new(-2.8, -1.2) + Cx::polar(1.2, k as f64 / 140.0 * TAU)), 0.01));

    // --- a shaky circle, and the same one with the dial turned down -------
    b.nib = Nib::Round(0.09);
    b.colour = 0xE0A44A;
    let shaky = |at: Cx| {
        (0..=200)
            .map(|k| {
                let th = k as f64 / 200.0 * TAU;
                at + Cx::polar(1.2 + 0.11 * (th * 9.0).sin(), th)
            })
            .collect::<Vec<_>>()
    };
    draw(&mut b, &shaky(Cx::new(0.6, -1.2)));

    // The one on the right gets smoothed. Everything already on the page is
    // smoothed too, which is how `F` works — so it is drawn last and the
    // dial is turned once, and the earlier marks are left as a fair test of
    // whether smoothing wrecks an open stroke. It must not: an open mark is
    // refused.
    draw(&mut b, &shaky(Cx::new(4.0, -1.2)));
    let untouched = b.sheet.marks[0].pts.len();
    b.smooth_all(4);
    assert_eq!(b.sheet.marks[0].pts.len(), untouched, "an open stroke must be left alone");

    // --- and a spring comparison: the same shaky line, hard and soft ------
    b.nib = Nib::Round(0.07);
    b.colour = 0x6FCF97;
    let jitter: Vec<Cx> = hand((0..=140).map(|k| Cx::new(-4.0 + k as f64 / 17.5, -3.4)), 0.05);
    b.pull = 1.0;
    draw(&mut b, &jitter);
    b.pull = 0.15;
    b.colour = 0x4FBCD4;
    draw(&mut b, &jitter.iter().map(|z| *z + Cx::new(0.0, -0.7)).collect::<Vec<_>>());

    let mut c = Canvas::new(900, 700);
    c.clear(0x0B1017);
    b.frame().draw(&mut c, &View::centred(900, 700, 82.0));

    let out = std::env::temp_dir().join("page.png");
    c.write_png(out.to_str().expect("a path")).expect("wrote the page");
    println!("look at {}", out.display());
}
