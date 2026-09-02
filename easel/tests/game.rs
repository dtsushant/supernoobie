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

/// ★ A right answer shows a smile and a wrong one shows a ghost — and each
/// puts the other into the past, or a fading smile hangs about inside the
/// ghost that follows it.
#[test]
fn a_right_answer_smiles_and_a_wrong_one_says_boo() {
    let mut b = game();
    let faces = |b: &Board| {
        // Both faces are always in the script; only their size changes, and a
        // face of no size is not drawn at all.
        b.written().shapes.len()
    };
    let quiet = faces(&b);

    tap(&mut b, box_at(1));
    assert!(faces(&b) > quiet, "a right answer should show something");
    let cheered = faces(&b);

    b.clock += 3.0;
    assert_eq!(faces(&b), quiet, "and it should fade away again");

    tap(&mut b, box_at(0));
    assert_eq!(faces(&b), cheered, "a wrong answer should show something too");
    // One face at a time: the smile just shown must not still be there.
    b.clock += 0.05;
    assert_eq!(faces(&b), cheered, "and only one of them");
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
