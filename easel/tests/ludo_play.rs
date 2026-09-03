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
    b.play_tap(17);
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
    b.play_tap(17);

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
    b.play_tap(17);
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
    b.play_tap(17);
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
    b.play_tap(17);
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
///
/// A token has to be out first: with nothing legal the turn passes by itself,
/// the die is cleared, and a second tap rightly *does* roll again.
#[test]
fn a_second_tap_on_the_die_does_not_roll_again() {
    let mut b = game();
    b.tally.values.insert("at0".into(), 10.0);
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

    // Nothing was legal, so the turn passed by itself. Set it back: this test
    // is about the gate, not about whose go it is.
    b.tally.values.insert("turn".into(), 0.0);
    b.tally.values.insert("rolled".into(), 1.0);
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
    b.tally.values.insert("at4".into(), 10.0);
    b.tally.values.insert("rolled".into(), 1.0);
    b.clock = 99.0;
    b.tally.values.insert("die".into(), 6.0);
    b.play_tap(5);
    assert_eq!(v(&b, "turn"), 1.0, "a six keeps the turn");
}

/// ★ It is not your turn, so nothing happens. The thing that makes four
/// players at one screen a game rather than a free-for-all.
#[test]
fn you_cannot_move_on_somebody_elses_turn() {
    let mut b = game();
    b.tally.values.insert("at4".into(), 10.0);
    b.tally.values.insert("rolled".into(), 1.0);
    b.clock = 99.0;
    b.tally.values.insert("die".into(), 3.0);
    // Seat 0 has a legal move, so the turn will not pass by itself and this
    // test is about the tap alone.
    b.tally.values.insert("at0".into(), 10.0);
    b.tally.values.insert("turn".into(), 0.0);
    b.play_tap(5); // seat 1's token, on seat 0's turn
    assert_eq!(v(&b, "at4"), 10.0, "it must not move");
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
    b.tally.values.insert("at4".into(), 49.0);
    assert_eq!(v(&b, "sq4"), 10.0, "the two are on the same square");

    b.tally.values.insert("rolled".into(), 1.0);
    b.clock = 99.0;
    b.tally.values.insert("die".into(), 3.0);
    b.tally.values.insert("turn".into(), 0.0);
    b.play_tap(1);
    assert_eq!(v(&b, "at0"), 10.0, "it moved");
    assert!(v(&b, "at4") < 0.0, "and sent the enemy home");
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

    // And with nothing legal the turn passed on its own, which is the point of
    // that rule. Put it back to test the other half.
    b.tally.values.insert("turn".into(), 0.0);
    b.tally.values.insert("rolled".into(), 1.0);
    b.tally.values.insert("die".into(), 2.0);
    b.play_tap(1);
    assert_eq!(v(&b, "at0"), 57.0, "two is exact, so it goes home");
}

/// ★ **A turn nobody can play passes by itself.** Roll a three with both
/// tokens in the yard and nothing at all is legal — before this the game
/// simply stopped, which is the kind of bug that ends an evening rather than
/// looking like a bug.
#[test]
fn a_turn_nobody_can_play_passes_by_itself() {
    let mut b = game();
    // Seat 0, both tokens in the yard, and a three.
    b.tally.values.insert("turn".into(), 0.0);
    b.tally.values.insert("rolled".into(), 1.0);
    b.tally.values.insert("flung".into(), 0.0);
    b.tally.values.insert("rolls".into(), 3.0);
    b.clock = 99.0;
    assert!(v(&b, "settled") > 0.5);
    if v(&b, "die") == 6.0 {
        // That seed happened to give a six, which IS playable. Nudge to a roll
        // that is not, rather than asserting about the wrong thing.
        b.tally.values.insert("rolls".into(), 4.0);
    }
    assert_ne!(v(&b, "die"), 6.0, "for this test the die must not open the gate");
    assert_eq!(v(&b, "anycan"), 0.0, "nothing is legal");

    b.settle();
    assert_eq!(v(&b, "turn"), 1.0, "the turn passed on its own");
    assert_eq!(v(&b, "rolled"), 0.0, "and the die must be thrown again");
}

/// ★ Whether a token may move is named **once**, and the ring you can see is
/// the same test the move uses. Three copies of a rule is three chances for
/// them to disagree.
#[test]
fn the_ring_you_can_see_is_the_rule_that_is_applied() {
    let mut b = game();
    b.tally.values.insert("turn".into(), 0.0);
    b.tally.values.insert("rolled".into(), 1.0);
    b.tally.values.insert("die".into(), 3.0);
    b.tally.values.insert("at0".into(), 10.0);
    b.clock = 99.0;

    assert_eq!(v(&b, "can0"), 1.0, "out on the board with a three: it may move");
    assert_eq!(v(&b, "can1"), 0.0, "in the yard without a six: it may not");
    assert_eq!(v(&b, "can2"), 0.0, "and it is not seat 1's turn at all");

    // And the move agrees with the ring.
    b.play_tap(2);
    assert!(v(&b, "at1") < 0.0, "the one with no ring did not move");
    b.play_tap(1);
    assert_eq!(v(&b, "at0"), 13.0, "the one with a ring did");
}

/// ★ A start square is safe: landing on one does not send anybody home. The
/// four starts and the four starred squares are one `mod`, not a list of eight
/// numbers to mistype.
#[test]
fn nobody_is_captured_on_a_safe_square() {
    let mut b = game();
    // Seat 1's token sits on square 13, which is seat 1's own start.
    b.tally.values.insert("at4".into(), 0.0);
    assert_eq!(v(&b, "sq4"), 13.0);

    // Seat 0 lands there: step 13, from 10, with a three.
    b.tally.values.insert("at0".into(), 10.0);
    b.tally.values.insert("rolled".into(), 1.0);
    b.tally.values.insert("die".into(), 3.0);
    b.tally.values.insert("turn".into(), 0.0);
    b.clock = 99.0;
    b.play_tap(1);

    assert_eq!(v(&b, "at0"), 13.0, "it moved onto the square");
    assert_eq!(v(&b, "at4"), 0.0, "and the one standing there is safe");
}

/// But an ordinary square is not safe.
#[test]
fn an_ordinary_square_is_not_safe() {
    let mut b = game();
    b.tally.values.insert("at0".into(), 8.0);
    b.tally.values.insert("at4".into(), 48.0); // 13 + 48 mod 52 = 9
    assert_eq!(v(&b, "sq4"), 9.0);
    assert_eq!(v(&b, "sq0"), 8.0);

    b.tally.values.insert("rolled".into(), 1.0);
    b.tally.values.insert("die".into(), 1.0);
    b.tally.values.insert("turn".into(), 0.0);
    b.clock = 99.0;
    b.play_tap(1);
    assert_eq!(v(&b, "at0"), 9.0);
    assert!(v(&b, "at4") < 0.0, "square 9 is nobody's start, so it is fair game");
}

/// ★ Three sixes forfeit the turn. The face is not known until the die stops,
/// so the counting waits for that rather than for the throw.
#[test]
fn three_sixes_give_the_turn_away() {
    let mut b = game();
    b.tally.values.insert("turn".into(), 0.0);
    for n in 1..=3 {
        // The count follows the EDGE of "rolled and settled", so the die has
        // to be picked up between throws -- which a real move does.
        b.tally.values.insert("rolled".into(), 0.0);
        b.settle();
        b.tally.values.insert("rolled".into(), 1.0);
        b.tally.values.insert("die".into(), 6.0);
        b.tally.values.insert("turn".into(), 0.0);
        b.clock += 10.0;
        b.settle();
        if n < 3 {
            assert_eq!(v(&b, "turn"), 0.0, "after {n} sixes the turn is still yours");
        }
    }
    assert_eq!(v(&b, "turn"), 1.0, "three sixes and you lose it");
    assert_eq!(v(&b, "sixes"), 0.0, "and the count starts again");
}

/// And a roll that is not a six clears the count.
#[test]
fn anything_but_a_six_clears_the_count() {
    let mut b = game();
    b.tally.values.insert("rolled".into(), 1.0);
    b.tally.values.insert("die".into(), 6.0);
    b.clock += 10.0;
    b.settle();
    assert_eq!(v(&b, "sixes"), 1.0);

    b.tally.values.insert("rolled".into(), 0.0);
    b.settle();
    b.tally.values.insert("rolled".into(), 1.0);
    b.tally.values.insert("die".into(), 2.0);
    b.clock += 10.0;
    b.settle();
    assert_eq!(v(&b, "sixes"), 0.0);
}

/// How many of each seat's tokens are in, shown down the side.
#[test]
fn it_says_how_many_are_in() {
    let mut b = game();
    assert_eq!(v(&b, "home0"), 0.0);
    b.tally.values.insert("at0".into(), 57.0);
    assert_eq!(v(&b, "home0"), 1.0);
    b.tally.values.insert("at1".into(), 57.0);
    assert_eq!(v(&b, "home0"), 2.0);
    assert_eq!(v(&b, "home1"), 0.0, "and only that seat's");
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
    assert_eq!(b.sheet.len(), 17, "sixteen tokens and a die, all tappable");

    // And the tokens are where their numbers say.
    b.tally.values.insert("at0".into(), 20.0);
    let env = b.sheet.script.env(0.0, &b.tally);
    let at = b.sheet.marks[0].pose_in(0.0, &env).apply(b.sheet.marks[0].anchor());
    assert!((at - plotkit::ludo::place(0, 20)).abs() < 0.05, "token 0 stands on square 20");
}

/// ★ **Four a seat, and the capture is still one line.** Three enemy tokens
/// stacked on one square all go home together — which is the thing the loop
/// bought, and which no amount of written-out pairs would have made pleasant.
#[test]
fn a_whole_stack_is_captured_at_once() {
    let mut b = game();
    b.tally.values.insert("turn".into(), 0.0);
    b.tally.values.insert("rolled".into(), 1.0);
    b.tally.values.insert("die".into(), 1.0);
    b.tally.values.insert("blockade".into(), 0.0); // else the stack is a wall
    b.tally.values.insert("at0".into(), 8.0);
    // Seat 1 owns tokens 4..7. Three of them stand on square 9.
    for k in [4, 5, 6] {
        b.tally.values.insert(format!("at{k}"), 48.0);
    }
    b.play_tap(1);
    assert_eq!(v(&b, "at0"), 9.0, "it moved on");
    for k in [4, 5, 6] {
        assert!(v(&b, &format!("at{k}")) < 0.0, "token {k} went home");
    }
    assert!(v(&b, "at7") < 0.0, "and the fourth was in the yard all along");
    assert_eq!(v(&b, "cuts0"), 1.0, "one move, counted once");
}

/// Each captured token goes back to **its own** yard place, worked out from the
/// index rather than written down sixteen times.
#[test]
fn a_captured_token_returns_to_its_own_place() {
    let mut b = game();
    b.tally.values.insert("turn".into(), 0.0);
    b.tally.values.insert("rolled".into(), 1.0);
    b.tally.values.insert("die".into(), 1.0);
    b.tally.values.insert("blockade".into(), 0.0);
    b.tally.values.insert("at0".into(), 8.0);
    b.tally.values.insert("at5".into(), 48.0);
    b.tally.values.insert("at6".into(), 48.0);
    b.play_tap(1);
    assert_eq!(v(&b, "at5"), -2.0, "token 5 is its seat's second");
    assert_eq!(v(&b, "at6"), -3.0, "and token 6 its third");
}

/// Sixteen tokens, four seats, four each — and every one of them tappable.
#[test]
fn there_are_four_tokens_a_seat() {
    let b = game();
    for seat in 0..4 {
        let mine = (0..16).filter(|k| v(&b, &format!("seat{k}")) == seat as f64).count();
        assert_eq!(mine, 4, "seat {seat}");
    }
}
