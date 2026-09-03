//! Ludo's rules, written as script rows and played through the board.
//!
//! Not a full game — no turns, no dice yet. What this proves is that the
//! language can now *say* the two things a game needs and could not say
//! before: move the token you tapped, and send home whoever was standing
//! there.

use easel::{Board, Script};
use plotkit::Cx;

/// Four tokens on the board, and a rule per token.
///
/// Each token's rule moves it on by `die` and then asks every other token
/// whether it is now standing where this one landed. Sixteen deeds rather than
/// a loop — verbose, and every one of them is the same line with a different
/// number in it, which is what a repeat would remove.
fn game() -> Board {
    let mut b = Board::new();
    let mut s = Script::new();
    s.add("ludo()");
    s.add("die = 4");
    // Four tokens: seat 0 has two, seat 1 has two.
    for (k, (seat, at)) in [(0, 3), (0, 20), (1, 16), (1, 40)].into_iter().enumerate() {
        s.add(format!("seat{k} = {seat}"));
        s.add(format!("at{k} = {at}"));
        s.add(format!("token(seat{k}, at{k}, 0.34)"));
    }
    // One rule per token. `here` is where it lands; anybody else of another
    // seat standing on the same *square* goes home.
    for k in 0..4 {
        let mut deeds = vec![format!("at[{k}] = at{k} + die"), format!("here = at[{k}]")];
        for j in 0..4 {
            if j == k {
                continue;
            }
            deeds.push(format!(
                "at[{j}] = if(and(seat{j} != seat{k}, at{j} == here), -1, at{j})"
            ));
        }
        s.add(format!("when tap {}: {}", k + 1, deeds.join(", ")));
    }
    b.sheet.script = s;
    b.playing_game = true;
    b
}

fn value(b: &Board, name: &str) -> f64 {
    b.written().vars.iter().find(|(n, _)| n == name).map(|(_, v)| v.re).unwrap_or(f64::NAN)
}

/// ★ **The thing that could not be said before.** Token 0 moves onto the
/// square token 2 is standing on, and token 2 goes home — written as rows, by
/// a rule that works out which name to write to.
#[test]
fn landing_on_an_enemy_sends_it_home() {
    let mut b = game();
    // Token 0 is at 3 and the die is 4, so it lands on 7. Put token 2 there.
    b.tally.values.insert("at2".into(), 7.0);
    assert_eq!(value(&b, "at2"), 7.0);

    b.play_tap(1);
    assert_eq!(value(&b, "at0"), 7.0, "it moved");
    assert_eq!(value(&b, "at2"), -1.0, "and sent the one standing there home");
    assert_eq!(value(&b, "at1"), 20.0, "its own team-mate is untouched");
    assert_eq!(value(&b, "at3"), 40.0, "and so is the one somewhere else");
}

/// Landing on your **own** token is not a capture.
#[test]
fn landing_on_your_own_token_is_not_a_capture() {
    let mut b = game();
    b.tally.values.insert("at1".into(), 7.0);
    b.play_tap(1);
    assert_eq!(value(&b, "at0"), 7.0);
    assert_eq!(value(&b, "at1"), 7.0, "a team-mate stays put");
}

/// Landing on an empty square captures nothing.
#[test]
fn landing_on_nothing_captures_nothing() {
    let mut b = game();
    b.play_tap(1);
    assert_eq!(value(&b, "at0"), 7.0);
    for (n, want) in [("at1", 20.0), ("at2", 16.0), ("at3", 40.0)] {
        assert_eq!(value(&b, n), want, "{n} should not have moved");
    }
}

/// ★ Two seats can stand on the same *step* without being on the same
/// *square* — they start a quarter apart. The rule compares steps, which is
/// wrong for real Ludo and right for this test to catch: it is the next thing
/// the game needs, and it is written down rather than hidden.
#[test]
fn the_same_step_is_not_the_same_square() {
    use shapes::ludo;
    for step in 0..40 {
        assert_ne!(
            ludo::place(0, step),
            ludo::place(1, step),
            "seats 0 and 1 at step {step} are different squares"
        );
    }
    let _ = Cx::ZERO;
}

/// And the whole thing still draws: the board, four tokens, no errors.
#[test]
fn the_rules_do_not_stop_it_drawing() {
    let mut b = game();
    let made = b.written();
    assert!(made.errors.is_empty(), "{:?}", made.errors);
    assert_eq!(made.shapes.len(), shapes::ludo::board().len() + 4);

    b.play_tap(1);
    assert!(b.written().errors.is_empty(), "still fine after a move");
}
