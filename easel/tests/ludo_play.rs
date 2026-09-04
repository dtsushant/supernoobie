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

/// Where the die is lying, worked out the same way the board works it out.
/// A test that taps a fixed spot is a test of where the die used to be.
fn die_at(b: &Board) -> plotkit::Cx {
    let age = (b.clock - v(b, "flung")).max(0.0);
    plotkit::dice::thrown(v(b, "seed"), v(b, "rolls"), age, 6.4).at
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

/// ★ The board asks `plotkit::dice` for the throw rather than working it out
/// in rows. What is left to check here is that it asks correctly — the throw
/// itself has eleven tests of its own, where the sums are.
#[test]
fn the_board_shows_a_real_face_throughout_a_throw() {
    let mut b = game();
    b.play_tap(17);
    for k in 0..80 {
        b.clock = k as f64 * 0.05;
        let face = v(&b, "die");
        assert!((1.0..=6.0).contains(&face), "face {face} at {}", b.clock);
        assert_eq!(face, face.round(), "and a whole one");
    }
}

/// ★ And that it knows when the throw is over — which is what every move in
/// the game waits for.
#[test]
fn the_board_knows_when_the_die_has_stopped() {
    let mut b = game();
    b.clock = 10.0;
    b.play_tap(17);
    assert_eq!(v(&b, "settled"), 0.0, "it has only just left the hand");
    b.clock = 10.0 + plotkit::dice::OVER + 0.1;
    assert_eq!(v(&b, "settled"), 1.0, "and now it is lying still");
}

/// ★ **The die rolls across the whole board, not in a corner.** It is thrown
/// into a square the size of the board and folds off its walls, so over a
/// throw it should visit a good deal of it.
#[test]
fn the_die_uses_the_whole_board() {
    let mut b = game();
    b.clock = 10.0;
    b.play_tap(17);
    let span = 6.4;
    let mut far: f64 = 0.0;
    for k in 0..60 {
        b.clock = 10.0 + k as f64 * 0.05;
        let at = die_at(&b);
        assert!(at.re.abs() <= span + 1e-6, "it left the board at {}", at.re);
        assert!(at.im.abs() <= span + 1e-6, "it left the board at {}", at.im);
        far = far.max(at.re.abs().max(at.im.abs()));
    }
    assert!(far > span * 0.7, "it never got near the edge: {far}");
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
///
/// The twelve that are: four starts at `mod 13 == 0`, and eight stars at
/// `mod 13 == 6` and `== 9` — five and two squares before each seat's home
/// entrance. Square 10 is none of them.
#[test]
fn an_ordinary_square_is_not_safe() {
    let mut b = game();
    b.tally.values.insert("at0".into(), 9.0);
    b.tally.values.insert("at4".into(), 49.0); // 13 + 49 mod 52 = 10
    assert_eq!(v(&b, "sq4"), 10.0);
    assert_eq!(v(&b, "sq0"), 9.0);

    b.tally.values.insert("rolled".into(), 1.0);
    b.tally.values.insert("die".into(), 1.0);
    b.tally.values.insert("turn".into(), 0.0);
    b.clock = 99.0;
    b.play_tap(1);
    assert_eq!(v(&b, "at0"), 10.0);
    assert!(v(&b, "at4") < 0.0, "square 10 is neither a start nor a star, so it is fair game");
}

/// ★ **The twelve safe squares, counted.** Four starts and eight stars — two
/// for each seat, five and two squares before its home entrance. Every one of
/// them lands on `mod 13` of 0, 6 or 9, which is three numbers to check rather
/// than twelve to mistype.
#[test]
fn there_are_twelve_safe_squares() {
    let mut safe: Vec<usize> = Vec::new();
    for seat in 0..4usize {
        for step in [0usize, 45, 48] {
            safe.push((13 * seat + step) % 52);
        }
    }
    safe.sort();
    safe.dedup();
    assert_eq!(safe.len(), 12, "four starts and eight stars, none shared");
    for sq in &safe {
        let m = sq % 13;
        assert!(m == 0 || m == 6 || m == 9, "square {sq} is at mod 13 = {m}");
    }
    // And the stars really are five and two before the home entrance at 50.
    assert_eq!([50 - 45, 50 - 48], [5, 2]);
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
    b.tally.values.insert("at0".into(), 9.0);
    // Seat 1 owns tokens 4..7. Three of them stand on square 10 -- not 9,
    // which is one of the eight safe stars.
    for k in [4, 5, 6] {
        b.tally.values.insert(format!("at{k}"), 49.0);
    }
    b.play_tap(1);
    assert_eq!(v(&b, "at0"), 10.0, "it moved on");
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
    b.tally.values.insert("at0".into(), 9.0);
    b.tally.values.insert("at5".into(), 49.0);
    b.tally.values.insert("at6".into(), 49.0);
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

/// ★ **You have to be able to hit the die.** Every other test in this file
/// calls `play_tap(17)`, which is the rule firing — it says nothing at all
/// about whether a tap where the die is drawn ever *reaches* that rule. It did
/// not: the die was an open stroke, so its middle was a hole and only the
/// 0.08-wide outline was live. Seventeen passing tests and an unrollable die.
#[test]
fn tapping_the_die_rolls_it() {
    let mut b = game();
    assert_eq!(v(&b, "rolled"), 0.0, "nothing thrown yet");
    // Straight at the middle of the box, where anybody would aim.
    let at = die_at(&b);
    b.pointer(at, true);
    b.pointer(at, false);
    assert_eq!(v(&b, "rolled"), 1.0, "the die was thrown");
}

/// And so do the tokens, which is the same question asked of the thing that
/// already worked — so a regression in either shows up here.
#[test]
fn tapping_a_token_moves_it() {
    let mut b = game();
    b.tally.values.insert("turn".into(), 0.0);
    b.tally.values.insert("rolled".into(), 1.0);
    b.tally.values.insert("die".into(), 6.0);
    let at = plotkit::ludo::waiting(0, 0);
    b.pointer(at, true);
    b.pointer(at, false);
    assert_eq!(v(&b, "at0"), 0.0, "out of the yard on a six");
}

/// ★ **The web presses Play, and Play is not Watch.** `playing_game` and
/// `watching` are two flags, and the browser only ever sets the first — so this
/// is the state a person actually clicks in. If a tap does not reach the rules
/// here, the game is unplayable in the browser however well it plays in a test.
#[test]
fn a_tap_rolls_the_die_with_only_play_pressed() {
    let mut b = Board::new();
    b.load("../samples/ludogame.easel").expect("the game opens");
    b.play(true);
    b.playing_game = true; // exactly what the server does for `Play { on: true }`
    let at = die_at(&b);
    b.pointer(at, true);
    b.pointer(at, false);
    assert_eq!(v(&b, "rolled"), 1.0, "the die was thrown");
}

/// ★ **A tap does nothing until Play is pressed**, and that is deliberate —
/// while you are editing, a click chooses a mark. Worth a test because "I
/// clicked the die and nothing happened" has exactly one other explanation,
/// and this rules it out.
#[test]
fn a_tap_does_nothing_while_still_editing() {
    let mut b = Board::new();
    b.load("../samples/ludogame.easel").expect("the game opens");
    let at = die_at(&b);
    b.pointer(at, true);
    b.pointer(at, false);
    assert_eq!(v(&b, "rolled"), 0.0, "no rule fired");
    assert!(b.any_chosen(), "it was chosen instead, which is what editing means");
}

/// ★ **`ludo.easel` is a drawing; `ludogame.easel` is the game.** They are one
/// letter apart in a file list and only one of them has anything to tap — which
/// is a fine way to spend ten minutes clicking a die that is not there.
#[test]
fn the_board_and_the_game_are_different_files() {
    let mut drawing = Board::new();
    drawing.load("../samples/ludo.easel").expect("the board opens");
    assert_eq!(drawing.sheet.len(), 0, "the board has nothing tappable in it");

    let mut game = Board::new();
    game.load("../samples/ludogame.easel").expect("the game opens");
    assert_eq!(game.sheet.len(), 17, "the game has sixteen tokens and a die");
}

/// ★ **A token walks; it does not teleport.** Going from square 6 to square 10
/// it should pass through 7, 8 and 9 — which means being *between* squares at
/// the moments in between, and that is what a fractional step is for.
#[test]
fn a_token_walks_one_square_at_a_time() {
    let mut b = game();
    b.clock = 10.0;
    b.tally.values.insert("at0".into(), 6.0);
    b.tally.values.insert("turn".into(), 0.0);
    b.tally.values.insert("rolled".into(), 1.0);
    b.tally.values.insert("die".into(), 4.0);
    b.tally.values.insert("settled".into(), 1.0);
    b.play_tap(1);
    assert_eq!(v(&b, "at0"), 10.0, "it is going to square ten");

    let where_drawn = |b: &Board| {
        let env = b.sheet.script.env(b.clock, &b.tally);
        b.sheet.marks[0].pose_in(b.clock, &env).apply(plotkit::Cx::ZERO)
    };
    let start = where_drawn(&b);

    // Part way through, it is neither where it was nor where it is going.
    b.clock = 10.0 + 0.2;
    let part = v(&b, "walk0");
    assert!(part > 6.0 && part < 10.0, "part way along, not at either end: {part}");
    assert!((part - part.round()).abs() > 1e-6, "and between two squares, not on one");
    assert!((where_drawn(&b) - start).abs() > 0.3, "so it has visibly set off");

    // It goes forwards, never back.
    let mut last = 6.0;
    for k in 0..20 {
        b.clock = 10.0 + k as f64 * 0.04;
        let now = v(&b, "walk0");
        assert!(now >= last - 1e-9, "it went backwards at {}: {last} then {now}", b.clock);
        last = now;
    }

    // And it arrives, and stays.
    b.clock = 10.0 + 5.0;
    assert_eq!(v(&b, "walk0"), 10.0, "arrived");
    b.clock = 10.0 + 60.0;
    assert_eq!(v(&b, "walk0"), 10.0, "and stopped there");
}

/// ★ A token sent home does **not** walk there. It was carried, not walked,
/// and sliding it across the board would be walking a path that is not on it.
#[test]
fn a_captured_token_does_not_walk_home() {
    let mut b = game();
    b.clock = 10.0;
    b.tally.values.insert("turn".into(), 0.0);
    b.tally.values.insert("rolled".into(), 1.0);
    b.tally.values.insert("die".into(), 1.0);
    b.tally.values.insert("settled".into(), 1.0);
    b.tally.values.insert("at0".into(), 9.0);
    b.tally.values.insert("at4".into(), 49.0);
    b.play_tap(1);
    assert!(v(&b, "at4") < 0.0, "it was cut");
    assert_eq!(v(&b, "walk4"), v(&b, "at4"), "and is in the yard at once");
}

/// ★ Coming out of the yard is a placing, not a walk — there is no path from
/// the yard to the start square to walk along.
#[test]
fn coming_out_of_the_yard_is_not_a_walk() {
    let mut b = game();
    b.clock = 10.0;
    b.tally.values.insert("turn".into(), 0.0);
    b.tally.values.insert("rolled".into(), 1.0);
    b.tally.values.insert("die".into(), 6.0);
    b.tally.values.insert("settled".into(), 1.0);
    b.play_tap(1);
    assert_eq!(v(&b, "at0"), 0.0, "out on its start square");
    assert_eq!(v(&b, "walk0"), 0.0, "and drawn there straight away");
}
