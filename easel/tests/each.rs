//! `each` — a repeat in the deed language.
//!
//! The thing that made four tokens a seat possible. Ludo's capture is one
//! sentence, and written a token at a time it was 240 lines.

use easel::rule::{self, Tally};
use std::collections::HashMap;

fn env() -> HashMap<String, plotkit::Cx> {
    HashMap::new()
}

fn run(text: &str, start: &[(&str, f64)]) -> Tally {
    let r = rule::read(text).expect("a rule");
    let mut t = Tally::new();
    for (k, v) in start {
        t.values.insert((*k).into(), *v);
    }
    rule::carry_out(&r, &mut t, &env());
    t
}

/// ★ **One line does what sixteen did.** The index is bound, the subscript is
/// worked out from it, and each pass writes to a different slot.
#[test]
fn one_deed_writes_to_many_slots() {
    let t = run("when tap 1: each j in 0..4 (at[j] = 10*j)", &[]);
    for j in 0..4 {
        assert_eq!(t.get(&format!("at{j}")), Some(10.0 * j as f64));
    }
    assert_eq!(t.get("at4"), None, "and stops where it says it stops");
}

/// The short form starts at nought, because that is where it nearly always
/// starts.
#[test]
fn a_bare_count_starts_at_nought() {
    let t = run("when tap 1: each j in 3 (at[j] = 1)", &[]);
    assert_eq!(t.get("at0"), Some(1.0));
    assert_eq!(t.get("at2"), Some(1.0));
    assert_eq!(t.get("at3"), None);
}

/// And it need not.
#[test]
fn a_range_need_not_start_at_nought() {
    let t = run("when tap 1: each j in 2..5 (at[j] = 1)", &[]);
    assert_eq!(t.get("at1"), None);
    assert_eq!(t.get("at2"), Some(1.0));
    assert_eq!(t.get("at4"), Some(1.0));
}

/// ★ Deeds inside the loop still see each other, and still see the passes
/// before them — so a loop can **add things up**, which is how "did this move
/// cut anybody" is asked without a fifteen-term expression.
#[test]
fn a_loop_can_accumulate() {
    let t = run(
        "when tap 1: total = 0, each j in 0..4 (total = total + at[j])",
        &[("at0", 1.0), ("at1", 2.0), ("at2", 4.0), ("at3", 8.0)],
    );
    assert_eq!(t.get("total"), Some(15.0));
}

/// ★ A loop reads indexed names as well as writing them — `at[j]` on the right
/// is `at0`, `at1`, … So "everybody standing where I landed" is one line.
#[test]
fn the_capture_is_one_line() {
    let t = run(
        "when tap 1: each j in 0..4 (at[j] = if(at[j] == here, -1, at[j]))",
        &[("here", 7.0), ("at0", 7.0), ("at1", 3.0), ("at2", 7.0), ("at3", 9.0)],
    );
    assert_eq!(t.get("at0"), Some(-1.0), "sent home");
    assert_eq!(t.get("at1"), Some(3.0), "left alone");
    assert_eq!(t.get("at2"), Some(-1.0), "sent home");
    assert_eq!(t.get("at3"), Some(9.0), "left alone");
}

/// ★ The index wins over anything the game happens to have called the same
/// thing. A loop whose counter could be captured by the score would be a
/// horrible thing to find.
#[test]
fn the_index_is_not_captured_by_the_game() {
    let t = run("when tap 1: each j in 0..3 (at[j] = j)", &[("j", 99.0)]);
    assert_eq!(t.get("at2"), Some(2.0));
    assert_eq!(t.get("j"), Some(99.0), "and the game's own value is untouched");
}

/// Deeds after the loop still run — the parentheses say where it ends, which
/// is why the body is bracketed rather than running to the end of the line.
#[test]
fn the_loop_ends_at_its_bracket() {
    let t = run("when tap 1: each j in 0..3 (at[j] = 1), turn = 5", &[]);
    assert_eq!(t.get("at2"), Some(1.0));
    assert_eq!(t.get("turn"), Some(5.0));
}

/// And two loops in one rule are two loops.
#[test]
fn two_loops_are_two_loops() {
    let t = run("when tap 1: each j in 0..2 (a[j] = 1), each j in 0..2 (b[j] = 2)", &[]);
    assert_eq!(t.get("a1"), Some(1.0));
    assert_eq!(t.get("b1"), Some(2.0));
}

/// They nest, so a pairwise question is two lines rather than n².
#[test]
fn loops_nest() {
    let t = run("when tap 1: n = 0, each a in 0..3 (each b in 0..3 (n = n + 1))", &[]);
    assert_eq!(t.get("n"), Some(9.0));
}

/// ★ **The range is written down, not worked out.** A range the game could
/// change is a loop whose length the game could change, and a rule that fires
/// once a frame with a length nothing bounds is a hung frame rather than a
/// wrong answer.
#[test]
fn a_range_must_be_written_down() {
    assert!(rule::read("when tap 1: each j in 0..n (at[j] = 1)").is_none());
    assert!(rule::read("when tap 1: each j in 0..1000 (at[j] = 1)").is_none(), "and it is capped");
    assert!(rule::read("when tap 1: each j in 5..2 (at[j] = 1)").is_none(), "and it goes forwards");
}

/// A malformed loop is refused, and — like any bad deed — takes only itself
/// with it.
#[test]
fn a_malformed_loop_takes_only_itself() {
    let t = run("when tap 1: each j 0..3 (at[j] = 1), turn = 5", &[]);
    assert_eq!(t.get("at1"), None, "the loop was not read");
    assert_eq!(t.get("turn"), Some(5.0), "and the rest of the rule still works");
}

/// Nesting cannot be used to hang the frame either: the writes are counted
/// across the whole rule, however deep.
#[test]
fn nesting_is_bounded_too() {
    let t = run(
        "when tap 1: n = 0, each a in 0..512 (each b in 0..512 (n = n + 1))",
        &[],
    );
    let n = t.get("n").expect("it did some of it");
    assert!(n <= rule::STEPS as f64, "and stopped: {n}");
}

/// ★ And the whole point: the same rule, at the size that made it necessary.
#[test]
fn sixteen_tokens_is_still_one_line() {
    let start: Vec<(String, f64)> = (0..16).map(|j| (format!("at{j}"), (j % 4) as f64)).collect();
    let start: Vec<(&str, f64)> = start.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    let t = run(
        "when tap 1: each j in 0..16 (at[j] = if(at[j] == here, 0 - 1 - mod(j, 4), at[j]))",
        &[&[("here", 2.0)][..], &start[..]].concat().as_slice(),
    );
    // The four tokens whose step was 2 went back to their own yard places.
    for j in 0..16 {
        let want = if j % 4 == 2 { -1.0 - (j % 4) as f64 } else { (j % 4) as f64 };
        assert_eq!(t.get(&format!("at{j}")), Some(want), "token {j}");
    }
}
