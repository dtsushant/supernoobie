//! Plays the hot-seat Ludo game the way four people at a table would: tap the
//! die, tap a token, pass the turn. Nothing here reaches past the board.

use easel::Board;

fn game() -> Board {
    let mut b = Board::new();
    b.load("../samples/ludogame.easel").expect("the game opens");
    b.playing_game = true;
    b.watching = true;
    b
}

fn v(b: &Board, name: &str) -> f64 {
    b.written().vars.iter().find(|(n, _)| n == name).map(|(_, x)| x.re).unwrap_or(f64::NAN)
}

/// The die is figure 9; tokens are figures 1..8.
fn roll(b: &mut Board) -> f64 {
    b.play_tap(9);
    // The die is thrown and takes a moment to settle, so the number is not the
    // number until it has stopped -- which is the whole point of throwing it.
    b.clock += 4.0;
    v(b, "die")
}

/// ★ **A thrown die.** It whirls, eases and stops -- the rate of tumbling
/// decays, so the number is not the number until it has settled. One
/// `exp(-age/relax)` does all of it, and nothing is stepped frame by frame:
/// `flung` is *when*, so the whole throw is a function of the clock.
#[test]
fn the_die_tumbles_and_then_settles() {
    let mut b = game();
    b.play_tap(9);

    // While it is going, the face changes.
    let mut seen = std::collections::HashSet::new();
    for k in 0..12 {
        b.clock = k as f64 * 0.06;
        seen.insert(v(&b, "die") as i64);
    }
    assert!(seen.len() > 2, "it should be turning over: {seen:?}");
    assert_eq!(v(&b, "settled"), 0.0, "and not settled yet");

    // And then it stops, and stays stopped.
    b.clock = 4.0;
    assert_eq!(v(&b, "settled"), 1.0);
    let face = v(&b, "die");
    for k in 0..20 {
        b.clock = 4.0 + k as f64;
        assert_eq!(v(&b, "die"), face, "a settled die does not change its mind");
    }
}

/// ★ And it slows down rather than stopping dead: it turns through far more
/// faces in its first moment than in its last.
#[test]
fn the_die_slows_rather_than_halting() {
    let mut b = game();
    b.play_tap(9);
    let turned = |b: &mut Board, from: f64, to: f64| {
        b.clock = from;
        let a = v(b, "tumbles");
        b.clock = to;
        v(b, "tumbles") - a
    };
    let early = turned(&mut b, 0.0, 0.2);
    let late = turned(&mut b, 1.0, 1.2);
    assert!(early > late * 5.0, "the first moment should turn far more: {early} then {late}");
}

/// ★ It bounces off the walls of its box rather than sliding out of it.
#[test]
fn the_die_stays_in_its_box() {
    let mut b = game();
    b.play_tap(9);
    for k in 0..80 {
        b.clock = k as f64 * 0.05;
        let dx = v(&b, "dx");
        let box_w = v(&b, "box");
        assert!((0.0..=box_w + 1e-9).contains(&dx), "it left the box at {}: {dx}", b.clock);
    }
}

/// ★ A move is refused until the die has stopped. Moving on a number that is
/// still turning over is moving on a number nobody has seen.
#[test]
fn you_cannot_move_while_the_die_is_still_going() {
    let mut b = game();
    b.tally.values.insert("at0".into(), 10.0);
    b.play_tap(9);
    b.clock = 0.1;
    b.play_tap(1);
    assert_eq!(v(&b, "at0"), 10.0, "not while it is turning");

    b.clock = 4.0;
    b.play_tap(1);
    assert_ne!(v(&b, "at0"), 10.0, "and now it may");
}

/// ★ A real die: one to six, and not the same number every time.
#[test]
fn the_die_rolls_one_to_six_and_varies() {
    let mut b = game();
    let mut seen = std::collections::HashSet::new();
    for _ in 0..60 {
        let d = roll(&mut b);
        assert!((1.0..=6.0).contains(&d), "a die gave {d}");
        seen.insert(d as i64);
        // Clear the roll so the next tap rolls again.
        b.tally.values.insert("rolled".into(), 0.0);
    }
    assert!(seen.len() >= 5, "sixty rolls should show most faces, saw {seen:?}");
}

/// ★ And rolling twice in one turn changes nothing — you get one roll.
#[test]
fn a_second_tap_on_the_die_does_not_roll_again() {
    let mut b = game();
    let first = roll(&mut b);
    for _ in 0..5 {
        assert_eq!(roll(&mut b), first, "the die is already rolled");
    }
}

/// ★ No random numbers anywhere, so the same game replays exactly — which is
/// what makes a match repeatable and "he cheated" answerable.
#[test]
fn the_same_game_rolls_the_same_way_twice() {
    let run = || {
        let mut b = game();
        let mut out = Vec::new();
        for _ in 0..12 {
            out.push(roll(&mut b) as i64);
            b.tally.values.insert("rolled".into(), 0.0);
        }
        out
    };
    assert_eq!(run(), run());
}

/// ★ A token leaves the yard only on a six.
#[test]
fn a_token_comes_out_only_on_a_six() {
    let mut b = game();
    b.tally.values.insert("rolled".into(), 1.0);
    b.clock = 99.0;
    b.tally.values.insert("turn".into(), 0.0);

    b.tally.values.insert("die".into(), 3.0);
    b.play_tap(1);
    assert!(v(&b, "at0") < 0.0, "three does not open the gate");

    b.tally.values.insert("rolled".into(), 1.0);
    b.clock = 99.0;
    b.tally.values.insert("die".into(), 6.0);
    b.play_tap(1);
    assert_eq!(v(&b, "at0"), 0.0, "six does");
}

/// ★ The turn passes — except on a six, which rolls again.
#[test]
fn the_turn_passes_unless_it_was_a_six() {
    let mut b = game();
    // Seat 0 has a token out, and rolls a three.
    b.tally.values.insert("at0".into(), 10.0);
    b.tally.values.insert("rolled".into(), 1.0);
    b.clock = 99.0;
    b.tally.values.insert("die".into(), 3.0);
    b.play_tap(1);
    assert_eq!(v(&b, "at0"), 13.0, "it moved");
    assert_eq!(v(&b, "turn"), 1.0, "and the turn passed");
    assert_eq!(v(&b, "rolled"), 0.0, "and the die must be rolled again");

    // Seat 1 rolls a six.
    b.tally.values.insert("at2".into(), 10.0);
    b.tally.values.insert("rolled".into(), 1.0);
    b.clock = 99.0;
    b.tally.values.insert("die".into(), 6.0);
    b.play_tap(3);
    assert_eq!(v(&b, "turn"), 1.0, "a six keeps the turn");
}

/// ★ It is not your turn, so nothing happens. The thing that makes four
/// players at one screen a game rather than a free-for-all.
#[test]
fn you_cannot_move_on_somebody_elses_turn() {
    let mut b = game();
    b.tally.values.insert("at2".into(), 10.0);
    b.tally.values.insert("rolled".into(), 1.0);
    b.clock = 99.0;
    b.tally.values.insert("die".into(), 3.0);
    b.tally.values.insert("turn".into(), 0.0);
    b.play_tap(3); // seat 1's token, on seat 0's turn
    assert_eq!(v(&b, "at2"), 10.0, "it must not move");
    assert_eq!(v(&b, "turn"), 0.0, "and the turn must not pass");
}

/// ★ Landing where an enemy stands sends it home — and compares **squares**,
/// not steps, since two seats at the same step are a quarter of the loop
/// apart.
#[test]
fn landing_on_an_enemy_sends_it_home() {
    let mut b = game();
    // Seat 0's token at step 10 is on square 10. Seat 1 starts at 13, so its
    // token needs step 10 - 13 + 52 = 49 to stand on the same square.
    b.tally.values.insert("at0".into(), 7.0);
    b.tally.values.insert("at2".into(), 49.0);
    assert_eq!(v(&b, "sq2"), 10.0, "the two are on the same square");

    b.tally.values.insert("rolled".into(), 1.0);
    b.clock = 99.0;
    b.tally.values.insert("die".into(), 3.0);
    b.tally.values.insert("turn".into(), 0.0);
    b.play_tap(1);
    assert_eq!(v(&b, "at0"), 10.0, "it moved");
    assert!(v(&b, "at2") < 0.0, "and sent the enemy home");
    assert_eq!(v(&b, "at1"), -2.0, "its team-mate is untouched");
}

/// A team-mate is not captured.
#[test]
fn landing_on_your_own_token_is_safe() {
    let mut b = game();
    b.tally.values.insert("at0".into(), 7.0);
    b.tally.values.insert("at1".into(), 10.0);
    b.tally.values.insert("rolled".into(), 1.0);
    b.clock = 99.0;
    b.tally.values.insert("die".into(), 3.0);
    b.play_tap(1);
    assert_eq!(v(&b, "at1"), 10.0, "a team-mate stays put");
}

/// ★ Home needs the exact count: 57 and no further.
#[test]
fn a_token_needs_the_exact_count_to_finish() {
    let mut b = game();
    b.tally.values.insert("at0".into(), 55.0);
    b.tally.values.insert("rolled".into(), 1.0);
    b.clock = 99.0;
    b.tally.values.insert("die".into(), 4.0);
    b.play_tap(1);
    assert_eq!(v(&b, "at0"), 55.0, "four would overshoot, so it may not move");

    b.tally.values.insert("rolled".into(), 1.0);
    b.clock = 99.0;
    b.tally.values.insert("die".into(), 2.0);
    b.play_tap(1);
    assert_eq!(v(&b, "at0"), 57.0, "two is exact, so it goes home");
}

/// ★ Both tokens home wins, and the game says who.
#[test]
fn getting_both_tokens_home_wins() {
    let mut b = game();
    assert_eq!(v(&b, "won"), 0.0);
    b.tally.values.insert("at0".into(), 57.0);
    b.tally.values.insert("at1".into(), 57.0);
    b.settle();
    assert_eq!(v(&b, "won"), 1.0, "seat 1 won");
}

/// And the whole thing draws, with no complaint from any row.
#[test]
fn the_game_draws_without_complaint() {
    let mut b = game();
    let made = b.written();
    assert!(made.errors.is_empty(), "{:?}", made.errors);
    assert!(made.shapes.len() > 90, "the board, the die and the numbers");
    assert_eq!(b.sheet.len(), 9, "eight tokens and a die, all tappable");

    // And the tokens are where their numbers say.
    b.tally.values.insert("at0".into(), 20.0);
    let env = b.sheet.script.env(0.0, &b.tally);
    let at = b.sheet.marks[0].pose_in(0.0, &env).apply(b.sheet.marks[0].anchor());
    assert!((at - plotkit::ludo::place(0, 20)).abs() < 0.05, "token 0 stands on square 20");
}
