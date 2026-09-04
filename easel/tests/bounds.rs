//! A drawing that says how big it is.

use easel::Board;
use plotkit::Cx;

/// ★ **A board has edges.** Saying so is what lets a page fit it to the window
/// and take the wheel away — scrolling a Ludo board is a way to lose it.
#[test]
fn a_bounded_drawing_says_so() {
    let mut b = Board::new();
    b.load("../samples/ludogame.easel").expect("the game opens");
    let (lo, hi) = b.sheet.script.bounds(0.0).expect("ludo is bounded");
    assert_eq!((lo.re, lo.im, hi.re, hi.im), (-9.5, -7.7, 9.5, 7.7));
}

/// ★ And everything it draws is inside the box it claims. A drawing whose
/// bounds were a lie would quietly clip a corner off, which is the sort of
/// thing nobody notices until a token walks into it.
#[test]
fn nothing_is_drawn_outside_the_box() {
    let mut b = Board::new();
    b.load("../samples/ludogame.easel").expect("the game opens");
    b.playing_game = true;
    let (lo, hi) = b.sheet.script.bounds(0.0).expect("bounded");
    // Through a throw, since the die roams and the tokens walk.
    b.clock = 10.0;
    b.play_tap(17);
    for k in 0..24 {
        b.clock = 10.0 + k as f64 * 0.1;
        for (s, _) in b.frame().parts() {
            for run in s.polylines(Cx::new(-40.0, -40.0), Cx::new(40.0, 40.0), 600) {
                for p in run {
                    assert!(p.re >= lo.re && p.re <= hi.re, "x {} outside at {}", p.re, b.clock);
                    assert!(p.im >= lo.im && p.im <= hi.im, "y {} outside at {}", p.im, b.clock);
                }
            }
        }
    }
}

/// A drawing that says nothing is unbounded, and keeps the wheel.
#[test]
fn a_drawing_that_says_nothing_is_unbounded() {
    let mut b = Board::new();
    b.sheet.script.add("param(exp(i*t), 0, tau)");
    assert!(b.sheet.script.bounds(0.0).is_none());
}

/// ★ Nonsense is refused rather than believed. A box with no width would make
/// the page divide by nothing and show an empty screen, which looks like the
/// drawing having failed to load.
#[test]
fn a_box_that_is_not_a_box_is_refused() {
    for bad in [
        "bounds(1, 1, 1, 1)",
        "bounds(4, 0, -4, 2)",
        "bounds(0, 0, 1)",
        "bounds(0, 0, 1, 0)",
        "bounds(0, 0, 1/0, 2)",
    ] {
        let mut b = Board::new();
        b.sheet.script.add(bad);
        assert!(b.sheet.script.bounds(0.0).is_none(), "{bad} should be refused");
    }
}

/// It may be worked out, like anything else in a row.
#[test]
fn a_box_can_be_an_expression() {
    let mut b = Board::new();
    b.sheet.script.add("w = 6");
    b.sheet.script.add("bounds(-w, -w/2, w, w/2)");
    let (lo, hi) = b.sheet.script.bounds(0.0).expect("worked out");
    assert_eq!((lo.re, lo.im, hi.re, hi.im), (-6.0, -3.0, 6.0, 3.0));
}

/// ★ **A command that draws nothing must still be named.** The arm that
/// handles faces draws a ghost for anything it does not recognise, sized by
/// the first argument — so `bounds(-9.5, …)` quietly became a ghost nine and a
/// half units across, sitting over the whole board. It was invisible in the
/// numbers and obvious the moment anything measured the drawing.
#[test]
fn bounds_draws_nothing_at_all() {
    let mut with = Board::new();
    with.sheet.script.add("circle(0, 0, 1)");
    with.sheet.script.add("bounds(-9.5, -7.7, 9.5, 7.7)");
    let mut without = Board::new();
    without.sheet.script.add("circle(0, 0, 1)");

    let ink = |b: &Board| {
        b.written()
            .shapes
            .iter()
            .chain(b.written().solid.iter())
            .flat_map(|(s, _)| s.polylines(Cx::new(-30.0, -30.0), Cx::new(30.0, 30.0), 600))
            .flatten()
            .map(|p| p.re.abs().max(p.im.abs()))
            .fold(0.0f64, f64::max)
    };
    assert_eq!(with.written().shapes.len(), without.written().shapes.len());
    assert!(ink(&with) < 1.01, "nothing bigger than the circle: {}", ink(&with));
}
