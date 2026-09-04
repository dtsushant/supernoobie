//! What a drawing says makes a noise.
//!
//! The synthesis is in the browser and is not covered — nothing here runs one.
//! What the page is *told* is covered, and that is where the game logic lives:
//! a sound is a number going up.

use easel::Board;

fn game() -> Board {
    let mut b = Board::new();
    b.load("../samples/ludogame.easel").expect("the game opens");
    b.playing_game = true;
    b.clock = 10.0;
    b
}

fn heard(b: &Board, name: &str) -> f64 {
    b.sheet
        .script
        .sounds(b.clock, &b.tally)
        .into_iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v)
        .unwrap_or(f64::NAN)
}

/// ★ A drawing declares its own sounds, and nothing in the studio knows what
/// any of them mean.
#[test]
fn a_drawing_says_what_makes_a_noise() {
    let b = game();
    let names: Vec<String> = b.sheet.script.sounds(0.0, &b.tally).into_iter().map(|(n, _)| n).collect();
    assert_eq!(names, vec!["roll", "step", "cut", "home"]);
}

/// ★ **Throwing the die puts `roll` up by one.** A sound is an event, and the
/// event here is a number changing — not a condition being true, which would
/// hold for a second and play forty times.
#[test]
fn throwing_the_die_counts_a_roll() {
    let mut b = game();
    let before = heard(&b, "roll");
    b.play_tap(17);
    assert_eq!(heard(&b, "roll"), before + 1.0);
    // And tapping it again mid-throw does not, because the throw does not
    // restart -- so neither does the noise.
    b.play_tap(17);
    assert_eq!(heard(&b, "roll"), before + 1.0);
}

/// ★ One tick a square. `steps` is the whole-number part of every token's
/// walk added up, so it goes up by one each time any token crosses onto a new
/// square — which is exactly when a foot should land.
#[test]
fn a_walking_token_ticks_once_a_square() {
    let mut b = game();
    b.tally.values.insert("at0".into(), 6.0);
    b.tally.values.insert("turn".into(), 0.0);
    b.tally.values.insert("rolled".into(), 1.0);
    b.tally.values.insert("die".into(), 4.0);
    b.tally.values.insert("settled".into(), 1.0);
    let before = heard(&b, "step");
    b.play_tap(1);

    let mut ticks = 0.0;
    let mut last = heard(&b, "step");
    for k in 0..40 {
        b.clock = 10.0 + k as f64 * 0.02;
        let now = heard(&b, "step");
        if now > last {
            ticks += now - last;
        }
        last = now;
    }
    assert_eq!(ticks, 4.0, "four squares, four ticks");
    assert_eq!(heard(&b, "step"), before + 4.0);
}

/// ★ And a captured token does **not** tick on its way back. It goes down, and
/// a sound only ever plays on the way up — which is why the count works and a
/// flag would not.
#[test]
fn a_capture_does_not_tick_as_it_goes_home() {
    let mut b = game();
    b.tally.values.insert("turn".into(), 0.0);
    b.tally.values.insert("rolled".into(), 1.0);
    b.tally.values.insert("die".into(), 1.0);
    b.tally.values.insert("settled".into(), 1.0);
    b.tally.values.insert("at0".into(), 3.0);
    b.tally.values.insert("at4".into(), 43.0);
    let steps = heard(&b, "step");
    let cuts = heard(&b, "cut");
    b.play_tap(1);
    b.clock = 40.0;
    assert_eq!(heard(&b, "cut"), cuts + 1.0, "the cut is counted");
    assert!(heard(&b, "step") < steps, "and the walk count went down, so nothing ticks");
}

/// A token reaching home counts once.
#[test]
fn getting_home_counts_once() {
    let mut b = game();
    b.tally.values.insert("turn".into(), 0.0);
    b.tally.values.insert("rolled".into(), 1.0);
    b.tally.values.insert("die".into(), 4.0);
    b.tally.values.insert("settled".into(), 1.0);
    b.tally.values.insert("at0".into(), 53.0);
    let before = heard(&b, "home");
    b.play_tap(1);
    assert_eq!(heard(&b, "home"), before + 1.0);
}

/// ★ **The way home is shut, and says so.** A legal-looking move that is
/// refused with no explanation is the worst thing a rule can do.
#[test]
fn a_shut_home_column_shows_a_sign() {
    let ink = |b: &Board| {
        b.written()
            .shapes
            .iter()
            .flat_map(|(s, _)| s.polylines(plotkit::Cx::new(-12.0, -12.0), plotkit::Cx::new(12.0, 12.0), 600))
            .flat_map(|r| r.windows(2).map(|w| (w[1] - w[0]).abs()).collect::<Vec<_>>())
            .sum::<f64>()
    };
    let mut b = game();
    let open = ink(&b);
    b.tally.values.insert("mustcut".into(), 1.0);
    let shut = ink(&b);
    assert!(shut - open > 5.0, "four signs' worth of ink appeared: {open} -> {shut}");

    // And it goes again once that seat has cut somebody.
    for seat in 0..4 {
        b.tally.values.insert(format!("cuts{seat}"), 1.0);
    }
    assert!((ink(&b) - open).abs() < 0.01, "and the signs go when the way opens");
}

/// The sign is only for the seats that are actually shut out.
#[test]
fn only_the_seats_that_are_shut_are_marked() {
    let mut b = game();
    b.tally.values.insert("mustcut".into(), 1.0);
    b.tally.values.insert("cuts1".into(), 2.0);
    let v = |n: &str| b.written().vars.iter().find(|(k, _)| k == n).map(|(_, x)| x.re);
    assert_eq!(v("shut0"), Some(1.0));
    assert_eq!(v("shut1"), Some(0.0), "seat 1 has cut somebody");
    assert_eq!(v("shut2"), Some(1.0));
}
