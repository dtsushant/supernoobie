//! The house rules: the same game, played differently because a number at the
//! top of the file is different. Two tables differ by a file.

use easel::Board;

fn game() -> Board {
    let mut b = Board::new();
    b.load("../samples/ludogame.easel").expect("the game opens");
    b.playing_game = true;
    b.watching = true;
    b.clock = 99.0;
    b
}

fn v(b: &Board, n: &str) -> f64 {
    b.written().vars.iter().find(|(k, _)| k == n).map(|(_, x)| x.re).unwrap_or(f64::NAN)
}

/// Set the board up for seat 0 with a given die, and no accidental turn pass.
fn ready(b: &mut Board, die: f64) {
    b.tally.values.insert("turn".into(), 0.0);
    b.tally.values.insert("rolled".into(), 1.0);
    b.tally.values.insert("die".into(), die);
}

/// ★ **What opens the gate is a dial.** The default is a six; a table that
/// plays "one or six" changes one number and the whole game follows.
#[test]
fn what_brings_a_token_out_is_a_house_rule() {
    let mut b = game();
    ready(&mut b, 1.0);
    b.play_tap(1);
    assert!(v(&b, "at0") < 0.0, "by default a one does not open the gate");

    let mut b = game();
    b.tally.values.insert("alsoone".into(), 1.0);
    ready(&mut b, 1.0);
    b.play_tap(1);
    assert_eq!(v(&b, "at0"), 0.0, "but this table plays one-or-six");
}

/// And a table that opens on anything can say so with the same dial.
#[test]
fn a_table_can_open_on_any_number() {
    let mut b = game();
    b.tally.values.insert("opens".into(), 3.0);
    ready(&mut b, 3.0);
    b.play_tap(1);
    assert_eq!(v(&b, "at0"), 0.0, "threes open the gate here");
}

/// ★ **"You must cut before you may come home."** With it on, a seat that has
/// captured nobody cannot enter its home column — and the same seat may,
/// once it has.
#[test]
fn a_table_can_ask_for_a_cut_before_home() {
    let mut b = game();
    b.tally.values.insert("mustcut".into(), 1.0);
    // Mercy off, or the farthest token is let home anyway -- which is the
    // point of the mercy rule and would hide what this test is asking.
    b.tally.values.insert("mercy".into(), 0.0);
    b.tally.values.insert("at0".into(), 49.0);
    ready(&mut b, 4.0);
    b.play_tap(1);
    assert_eq!(v(&b, "at0"), 49.0, "no cut yet, so the column is shut");

    b.tally.values.insert("cuts0".into(), 1.0);
    ready(&mut b, 4.0);
    b.play_tap(1);
    assert_eq!(v(&b, "at0"), 53.0, "and open once it has cut somebody");
}

/// With the rule off, the column is always open.
#[test]
fn without_that_rule_the_column_is_always_open() {
    let mut b = game();
    b.tally.values.insert("at0".into(), 49.0);
    ready(&mut b, 4.0);
    b.play_tap(1);
    assert_eq!(v(&b, "at0"), 53.0);
}

/// ★ A capture is counted, so the rule above has something to read.
#[test]
fn a_capture_is_counted() {
    let mut b = game();
    assert_eq!(v(&b, "cuts0"), 0.0);
    // Square 10, which is not safe: the eight stars are at mod 13 of 6 and 9,
    // and the four starts at 0.
    b.tally.values.insert("at0".into(), 9.0);
    b.tally.values.insert("at4".into(), 49.0); // seat 1, square 10
    ready(&mut b, 1.0);
    b.play_tap(1);
    assert!(v(&b, "at4") < 0.0, "it cut");
    assert_eq!(v(&b, "cuts0"), 1.0, "and the seat's count went up");
    assert_eq!(v(&b, "cuts1"), 0.0, "and only that seat's");
}

/// ★ **What earns another turn is a dial too.** A capture keeping the turn is
/// a common house rule and is off by default.
#[test]
fn a_table_can_give_another_turn_for_a_capture() {
    let set = |again: f64| {
        let mut b = game();
        b.tally.values.insert("againcut".into(), again);
        b.tally.values.insert("at0".into(), 9.0);
        b.tally.values.insert("at4".into(), 49.0);
        ready(&mut b, 1.0);
        b.play_tap(1);
        v(&b, "turn")
    };
    assert_eq!(set(0.0), 1.0, "by default a capture passes the turn");
    assert_eq!(set(1.0), 0.0, "and here it keeps it");
}

/// And a six need not, if the table says so.
#[test]
fn a_table_can_stop_a_six_earning_another_turn() {
    let mut b = game();
    b.tally.values.insert("again6".into(), 0.0);
    b.tally.values.insert("at0".into(), 10.0);
    ready(&mut b, 6.0);
    b.play_tap(1);
    assert_eq!(v(&b, "turn"), 1.0, "here a six is just a six");
}

/// ★ **Barriers.** Two of one seat on a square, and nobody else may land
/// there — off or on with one number.
#[test]
fn two_together_can_be_made_to_block() {
    let both_there = |blockade: f64| {
        let mut b = game();
        b.tally.values.insert("blockade".into(), blockade);
        // Seat 1 owns tokens 4..7. Two of them stand together on square 9.
        b.tally.values.insert("at4".into(), 48.0);
        b.tally.values.insert("at5".into(), 48.0);
        b.tally.values.insert("at0".into(), 8.0);
        ready(&mut b, 1.0);
        b.play_tap(1);
        v(&b, "at0")
    };
    assert_eq!(both_there(1.0), 8.0, "a barrier turns it back");
    assert_eq!(both_there(0.0), 9.0, "and without the rule it walks straight in");
}

/// One token alone is not a barrier.
#[test]
fn one_token_is_not_a_barrier() {
    let mut b = game();
    b.tally.values.insert("at4".into(), 48.0);
    b.tally.values.insert("at0".into(), 8.0);
    ready(&mut b, 1.0);
    b.play_tap(1);
    assert_eq!(v(&b, "at0"), 9.0, "one is a target, not a wall");
}

/// The stars can be put away, for a table that does not use them. They are
/// drawn at no size rather than removed — the same way anything is hidden
/// here — so what shrinks is the ink, not the count, and not the board's
/// bounding box either: the stars sit well inside it.
#[test]
fn the_stars_can_be_put_away() {
    let ink = |b: &Board| {
        let (a, z) = (plotkit::Cx::new(-12.0, -12.0), plotkit::Cx::new(12.0, 12.0));
        b.written()
            .shapes
            .iter()
            .flat_map(|(s, _)| s.polylines(a, z, 600))
            .flat_map(|run| run.windows(2).map(|w| (w[1] - w[0]).abs()).collect::<Vec<_>>())
            .sum::<f64>()
    };
    let mut b = game();
    let drawn = ink(&b);
    b.tally.values.insert("stars".into(), 0.0);
    let bare = ink(&b);
    // Eight rings of radius 0.3 is about 8·2π·0.3 ≈ 15 units of ink.
    assert!(drawn - bare > 10.0, "the stars were worth drawing: {drawn} -> {bare}");
}

/// And the whole thing still draws, with every house rule readable as a dial.
#[test]
fn every_house_rule_is_a_dial() {
    let b = game();
    let made = b.written();
    assert!(made.errors.is_empty(), "{:?}", made.errors);
    let dials = b.sheet.script.dials(0.0);
    for name in ["opens", "alsoone", "mustcut", "again6", "againcut", "againhome", "blockade", "stars"] {
        assert!(dials.iter().any(|(n, _)| n == name), "{name} should have a slider");
    }
}
