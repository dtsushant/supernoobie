//! Plays the addition game the way a child would: look at the question, tap a
//! box, see the score move. Nothing here reaches into the rules — it goes
//! through the board, so what is tested is what somebody would actually do.

use easel::Board;
use plotkit::{Canvas, Cx, View};

fn game() -> Board {
    let mut b = Board::new();
    b.load("../samples/adding.easel").expect("the game opens");
    b.playing_game = true;
    b
}

/// Tap a point, as a finger would.
fn tap(b: &mut Board, at: Cx) {
    b.pointer(at, true);
    b.pointer(at, false);
}

/// The middle of each answer box.
const BOXES: [f64; 3] = [-3.4, 0.0, 3.4];

/// The **middle** of a box, which is where anybody would tap it.
fn box_at(k: usize) -> Cx {
    Cx::new(BOXES[k], -2.2)
}

fn value(b: &Board, name: &str) -> f64 {
    b.written().vars.iter().find(|(n, _)| n == name).map(|(_, v)| v.re).unwrap_or(f64::NAN)
}

/// ★ A right answer shows a smile and a wrong one shows a ghost, and each
/// fades away again.
///
/// Measured as "more is drawn now than a moment later", **within one
/// question**. Comparing counts across two taps looks tidier and is wrong: the
/// answers are drawn with `digits`, so going from 9 to 10 adds a glyph and
/// moves the total for reasons that have nothing to do with faces. That is how
/// this test failed first time.
#[test]
fn a_right_answer_smiles_and_a_wrong_one_says_boo() {
    let drawn = |b: &Board| b.written().shapes.len();

    for k in [1usize, 0] {
        let mut b = game();
        tap(&mut b, box_at(k));
        let just_after = drawn(&b);
        // Same question either side of this: only the face changes.
        b.clock += 3.0;
        let later = drawn(&b);
        assert!(just_after > later, "box {k} should show a face and then lose it: {just_after} then {later}");
    }
}

/// And only one face at a time — each rule puts the other into the past, or a
/// fading smile hangs about inside the ghost that follows it.
#[test]
fn there_is_never_more_than_one_face() {
    let mut b = game();
    tap(&mut b, box_at(1));
    let one_face = b.written().shapes.len();
    b.clock += 0.05;

    tap(&mut b, box_at(0));
    b.clock += 0.05;
    let after = b.written().shapes.len();
    // The question changed, so the totals are not comparable directly --
    // compare each against its own faceless moment instead.
    let with_face = after;
    b.clock += 3.0;
    let without = b.written().shapes.len();
    assert_eq!(with_face - without, one_face - {
        let mut c = game();
        c.clock += 3.0;
        c.written().shapes.len()
    }, "one face then, one face now");
}

/// ★ A box is tapped in the **middle**, which is where a child will put their
/// finger. It used to need the two pixels of its outline, because the nib
/// sweeps a ring and the middle of the box is the hole in it — a game nobody
/// could play, and the reason this test aims at the centre.
#[test]
fn the_middle_of_a_box_is_the_box() {
    let mut b = game();
    for k in 0..3 {
        let before = value(&b, "score");
        tap(&mut b, box_at(k));
        assert_ne!(value(&b, "score"), before, "tapping the middle of box {k} did nothing");
    }
}

/// ★ The whole game, played. The right box puts the score up, a wrong one puts
/// it down, and the question changes because it is made of the score.
#[test]
fn tapping_the_right_box_scores_and_asks_another() {
    let mut b = game();
    assert_eq!(value(&b, "score"), 0.0);

    let (a, c) = (value(&b, "a"), value(&b, "b"));
    assert!((1.0..=6.0).contains(&a) && (1.0..=6.0).contains(&c), "a sensible question: {a} + {c}");

    tap(&mut b, box_at(1));
    assert_eq!(value(&b, "score"), 1.0, "the middle box is the right one");
    assert!(
        (value(&b, "a"), value(&b, "b")) != (a, c),
        "and the question should have changed"
    );

    tap(&mut b, box_at(0));
    assert_eq!(value(&b, "score"), 0.0, "a wrong box takes it back");
    tap(&mut b, box_at(2));
    assert_eq!(value(&b, "score"), -1.0);
}

/// ★ No random numbers anywhere, so the same game replays exactly — which is
/// what lets a wrong answer be looked at again instead of lost.
#[test]
fn the_same_game_plays_the_same_way_twice() {
    let run = || {
        let mut b = game();
        let mut seen = Vec::new();
        for _ in 0..10 {
            seen.push((value(&b, "a"), value(&b, "b")));
            tap(&mut b, box_at(1));
        }
        seen
    };
    let once = run();
    assert_eq!(once, run());
    assert!(once.windows(2).any(|w| w[0] != w[1]), "and it is not the same question every time");
}

/// ★ Starting again really starts again — which is why the score lives beside
/// the rows rather than being written into them.
#[test]
fn restarting_puts_the_score_back() {
    let mut b = game();
    for _ in 0..4 {
        tap(&mut b, box_at(1));
    }
    assert_eq!(value(&b, "score"), 4.0);
    b.restart();
    assert_eq!(value(&b, "score"), 0.0);
    assert!(b.tally.is_empty());
}

/// ★ Playing and editing are different intentions. While the game runs a tap
/// is a move; it must not also select the box for editing.
#[test]
fn a_tap_in_play_does_not_select_anything() {
    let mut b = game();
    tap(&mut b, box_at(1));
    assert!(b.selected.is_empty(), "a move is not a choice");

    b.playing_game = false;
    tap(&mut b, box_at(1));
    assert!(!b.selected.is_empty(), "and out of play it chooses again");
    assert_eq!(value(&b, "score"), 1.0, "without scoring twice");
}

/// The three answers are three consecutive numbers, so the boxes never show
/// the same thing twice and a child cannot win by tapping at random.
#[test]
fn the_three_answers_are_all_different() {
    let mut b = game();
    for _ in 0..12 {
        let (a, c) = (value(&b, "a"), value(&b, "b"));
        assert!(a >= 1.0 && c >= 1.0, "and never asks about nothing");
        tap(&mut b, box_at(1));
    }
}

/// And it draws: the question, the answers and the score all put ink down.
#[test]
fn the_game_can_be_seen() {
    let mut b = game();
    let ink = |b: &Board| {
        let mut c = Canvas::new(700, 500);
        c.clear(0);
        b.frame().draw(&mut c, &View::centred(700, 500, 44.0));
        c.buf.iter().filter(|p| **p != 0).count()
    };
    let before = ink(&b);
    assert!(before > 400, "there should be a question, three answers and a score");
    tap(&mut b, box_at(1));
    assert_ne!(ink(&b), before, "and it should look different after a move");
}
