//! House rules a drawing declares for itself.

use easel::Board;

fn game() -> Board {
    let mut b = Board::new();
    b.load("../samples/ludogame.easel").expect("the game opens");
    b
}

/// ★ **A row says it is a house rule, and nothing here knows what Ludo is.**
/// The setup screen is built from the drawing, so any game gets one by writing
/// a comment.
#[test]
fn a_drawing_declares_its_own_house_rules() {
    let b = game();
    let rules = b.sheet.script.house(0.0);
    assert!(rules.len() >= 8, "eight or so, got {}", rules.len());
    let (_, name, label, value) = &rules[0];
    assert_eq!(name, "opens");
    assert_eq!(label, "what brings a token out");
    assert_eq!(*value, 6.0, "and the value it is set to");
    assert!(rules.iter().any(|(_, n, _, _)| n == "mercy"), "including the mercy rule");
}

/// The comment does not disturb the value: `opens = 6  # rule: ...` is still
/// six, and the game still reads it.
#[test]
fn the_comment_does_not_change_the_number() {
    let b = game();
    let vars = b.written().vars;
    for (name, want) in [("opens", 6.0), ("again6", 1.0), ("mercy", 1.0), ("alsoone", 0.0)] {
        let got = vars.iter().find(|(n, _)| n == name).map(|(_, v)| v.re);
        assert_eq!(got, Some(want), "{name}");
    }
}

/// ★ A row without the comment is an ordinary binding and stays off the setup
/// screen — otherwise every working number in the file would be on it.
#[test]
fn only_the_rows_that_say_so_are_house_rules() {
    let b = game();
    let rules = b.sheet.script.house(0.0);
    for plain in ["seed", "rolls", "turn", "at0", "pace", "span"] {
        assert!(!rules.iter().any(|(_, n, _, _)| n == plain), "{plain} is not a house rule");
    }
}

/// And a drawing that declares none has none, rather than guessing.
#[test]
fn a_drawing_with_no_rules_offers_none() {
    let mut b = Board::new();
    b.sheet.script.add("a = 3");
    b.sheet.script.add("b = 4  # just a note");
    assert!(b.sheet.script.house(0.0).is_empty());
}

/// ★ Setting one changes the game, which is the whole point of asking before
/// the game starts.
#[test]
fn setting_a_house_rule_changes_the_game() {
    let mut b = game();
    b.sheet.script.set_dial("opens", 1.0);
    let vars = b.written().vars;
    assert_eq!(vars.iter().find(|(n, _)| n == "opens").map(|(_, v)| v.re), Some(1.0));
    // And it is still declared, with its new value.
    let rules = b.sheet.script.house(0.0);
    assert_eq!(rules.iter().find(|(_, n, _, _)| n == "opens").map(|(_, _, _, v)| *v), Some(1.0));
}

/// ★ It survives a save and a load, so a table's rules are the file.
#[test]
fn house_rules_are_saved_with_the_drawing() {
    let mut b = game();
    b.sheet.script.set_dial("mustcut", 1.0);
    let text = b.sheet.to_text();
    let (back, muddle) = easel::Sheet::from_text(&text);
    assert_eq!(muddle, 0);
    let rules = back.script.house(0.0);
    assert_eq!(rules.iter().find(|(_, n, _, _)| n == "mustcut").map(|(_, _, _, v)| *v), Some(1.0));
    assert_eq!(
        rules.iter().find(|(_, n, _, _)| n == "mustcut").map(|(_, _, l, _)| l.as_str()),
        Some("no way home until you have cut somebody"),
        "and keeps the words a person reads"
    );
}
