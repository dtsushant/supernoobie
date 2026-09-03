//! What a scene costs.
//!
//! Kept because the answer was **84 milliseconds**, and that is the whole
//! reason the studio felt slow: the clock could not tick faster than twelve
//! times a second, and every tap queued behind a tick.
//!
//! Forty-seven of those milliseconds were one line. `param(…)` evaluates its
//! expression once per sample, and each evaluation copied the entire script's
//! bindings — sixty names for a game of Ludo, three hundred and twenty samples
//! a curve, thirty curves a frame. Handing each curve only the names it
//! mentions took it to six.
//!
//! Run with `--ignored --nocapture` to see the numbers.

use easel::Board;
use plotkit::Cx;

fn ludo() -> Board {
    let mut b = Board::new();
    b.load("../samples/ludogame.easel").expect("the game opens");
    b.playing_game = true;
    b
}

fn look() -> easel::wire::Look {
    easel::wire::Look::new(Cx::new(-12.0, -9.0), Cx::new(12.0, 9.0), 900)
}

#[test]
#[ignore]
fn what_a_scene_costs() {
    let b = ludo();
    let t = std::time::Instant::now();
    let mut n = 0;
    for _ in 0..20 {
        n = easel::wire::scene(&b, look()).len();
    }
    println!("scene: {n} bytes, {:?} each", t.elapsed() / 20);
    let t = std::time::Instant::now();
    for _ in 0..20 {
        let _ = b.frame();
    }
    println!("frame(): {:?} each", t.elapsed() / 20);
}

/// ★ **Coordinates go over the wire as whole numbers of hundredths.** A
/// fortieth of a pixel at the zoom anybody draws at, and whole numbers can be
/// written a digit at a time instead of through the formatting machinery.
#[test]
fn the_wire_carries_hundredths() {
    let b = ludo();
    let out = easel::wire::scene(&b, look());
    let p = out.split("\"p\":[").nth(1).expect("a piece").split(']').next().expect("its points");
    for n in p.split(',').take(20) {
        assert!(!n.contains('.'), "a coordinate should be whole: {n}");
        assert!(n.parse::<i64>().is_ok(), "and a number: {n}");
    }
}

/// ★ A curve is handed only the names it mentions, so sampling it does not
/// copy the whole script. This is the property, not the timing — a timing test
/// fails on a busy machine and tells you nothing.
#[test]
fn a_curve_gets_only_the_names_it_uses() {
    let e = plotkit::expr::parse("0.5*exp(i*t) + ludox(seat0, at0)").expect("it parses");
    let mut base = std::collections::HashMap::new();
    for k in 0..60 {
        base.insert(format!("junk{k}"), Cx::ZERO);
    }
    for n in ["i", "t", "seat0", "at0"] {
        base.insert(n.into(), Cx::ONE);
    }
    let small = plotkit::expr::env_for(&e, &base);
    assert!(small.len() <= 4, "it took {} of {} bindings", small.len(), base.len());
    assert!(small.contains_key("seat0") && small.contains_key("at0"));
}

/// And anything with a subscript asks for everything, because `at[k]` cannot
/// be resolved without knowing `k`. The safe answer, and rare.
#[test]
fn a_subscript_asks_for_everything() {
    let e = plotkit::expr::parse("at[k] + 1").expect("it parses");
    let mut base = std::collections::HashMap::new();
    for k in 0..10 {
        base.insert(format!("at{k}"), Cx::ZERO);
    }
    base.insert("k".into(), Cx::ONE);
    assert_eq!(plotkit::expr::env_for(&e, &base).len(), base.len());
}
