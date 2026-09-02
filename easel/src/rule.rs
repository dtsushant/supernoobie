//! # rule — what happens when you tap something
//!
//! The one thing a drawing needs before it is a game: *when this is tapped, do
//! that*.
//!
//! ```text
//!     score = 0
//!     a = 1 + floor(9*abs(sin(37*(score+1))))
//!     b = 1 + floor(9*abs(sin(53*(score+1))))
//!
//!     when tap 1: score = score + 1        the right answer
//!     when tap 2: score = score - 1
//! ```
//!
//! A rule is a **row like any other**, so it is written, saved, switched off
//! and typed into with everything else. It has to be: a game whose rules lived
//! somewhere the shapes did not would be two files to keep in step.
//!
//! ## The one place a second copy of a value is right
//!
//! Everywhere else in this crate the text is the only truth —
//! [`Script::set_dial`](crate::Script::set_dial) *rewrites the row*, because a
//! value kept beside the text is a second place for it to live and then
//! saving, undoing and hand-editing all have to agree about which one wins.
//!
//! Playing is different, and it is worth being exact about why. A dial being
//! moved is **authoring**: you are changing the drawing, and you want that
//! saved and undoable. A score going up is **playing**: it is not a change to
//! the drawing at all, and a game that rewrote its own source as you played
//! would fill the undo history with your score, mark the file as modified
//! every time you tapped, and lose the starting position it needs to begin
//! again.
//!
//! So a rule writes to a [`Tally`] — a live copy that shadows the rows while
//! the game runs and is thrown away when you rewind. The rows say where the
//! game *starts*; the tally says where it has *got to*.
//!
//! ## Why the deed is an expression and not a number
//!
//! `score = score + 1` needs to read the value it is changing, so the
//! right-hand side is a full expression evaluated against everything else —
//! rows, tally and all. That also means a rule can say
//! `a = 1 + floor(9*abs(sin(37*score)))` and get a fresh question out of the
//! score it just changed, with no random number generator anywhere: the same
//! game replays identically, which is what the tapes in this repository have
//! always needed.

use std::collections::HashMap;

use plotkit::expr::{self, Expr};
use plotkit::Cx;

/// What sets a rule off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum On {
    /// A figure was tapped, by its group number.
    Tap(u32),
}

/// One rule: when this happens, do these.
#[derive(Clone, Debug)]
pub struct Rule {
    pub on: On,
    /// `name = expression`, in order.
    pub deeds: Vec<(String, Expr)>,
}

/// What the game has got to: values that shadow the rows while it runs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tally {
    pub values: HashMap<String, f64>,
}

impl Tally {
    pub fn new() -> Tally {
        Tally::default()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Forget everything and start again.
    pub fn clear(&mut self) {
        self.values.clear();
    }

    pub fn get(&self, name: &str) -> Option<f64> {
        self.values.get(name).copied()
    }

    /// The rows that put these values into a script, ahead of the real ones.
    ///
    /// Written as source rather than pushed into an environment, so a live
    /// value is an ordinary binding that behaves exactly like a typed one —
    /// the same reason `time` is bound by writing a line.
    pub fn as_rows(&self) -> String {
        let mut names: Vec<&String> = self.values.keys().collect();
        // Sorted, so the same tally always makes the same source. Two
        // identical games that differed by hash order would be a horrible
        // thing to debug.
        names.sort();
        names.iter().map(|n| format!("{n} = {}\n", self.values[*n])).collect()
    }
}

/// Read a rule from a row, or `None` if the row is not one.
///
/// ```text
///     when tap 1: score = score + 1, a = a + 2
/// ```
pub fn read(text: &str) -> Option<Rule> {
    let rest = text.trim().strip_prefix("when ")?;
    let (head, body) = rest.split_once(':')?;
    let mut word = head.split_whitespace();
    let on = match (word.next()?, word.next()) {
        ("tap", Some(g)) => On::Tap(g.trim().parse().ok()?),
        _ => return None,
    };

    let mut deeds = Vec::new();
    for one in body.split(',') {
        let Some((name, value)) = one.split_once('=') else {
            continue;
        };
        let name = name.trim().to_string();
        if name.is_empty() {
            continue;
        }
        // A deed that will not parse is dropped and the rest of the rule
        // still works, for the same reason a bad row does not blank the
        // drawing: half-typed text is the normal state of a row being edited.
        if let Ok(e) = expr::parse(value.trim()) {
            deeds.push((name, e));
        }
    }
    (!deeds.is_empty()).then_some(Rule { on, deeds })
}

/// Is this row a rule? Cheap enough to ask on every row, every frame.
pub fn is_rule(text: &str) -> bool {
    text.trim().starts_with("when ")
}

/// Carry out a rule's deeds, reading and writing the tally.
///
/// Deeds happen **in order and each sees the last**, so
/// `score = score + 1, a = floor(9*abs(sin(score)))` gives a question from the
/// score it has just changed rather than from the old one. Writing them all at
/// once from a frozen snapshot would be defensible and is not what anybody
/// reading the line left to right expects.
pub fn carry_out(rule: &Rule, tally: &mut Tally, env: &HashMap<String, Cx>) {
    for (name, value) in &rule.deeds {
        let mut here = env.clone();
        for (k, v) in &tally.values {
            here.insert(k.clone(), Cx::new(*v, 0.0));
        }
        match value.eval(&here) {
            Ok(v) if v.re.is_finite() => {
                tally.values.insert(name.clone(), v.re);
            }
            // A deed that will not evaluate leaves the value alone. The
            // alternative is a score that becomes NaN on one bad tap and stays
            // NaN for ever, which looks like the game having broken rather
            // than one rule having a typo in it.
            _ => {}
        }
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> HashMap<String, Cx> {
        HashMap::new()
    }

    /// ★ The shape of a rule: when this is tapped, do that.
    #[test]
    fn a_rule_says_what_a_tap_does() {
        let r = read("when tap 1: score = score + 1").expect("a rule");
        assert_eq!(r.on, On::Tap(1));
        assert_eq!(r.deeds.len(), 1);
        assert_eq!(r.deeds[0].0, "score");
    }

    /// Several deeds to one tap, separated by commas.
    #[test]
    fn one_tap_can_do_several_things() {
        let r = read("when tap 2: score = score + 1, a = 3, b = 4").expect("a rule");
        assert_eq!(r.deeds.len(), 3);
    }

    /// ★ A deed reads the value it is changing, so `score = score + 1` works —
    /// which is the whole point of the right-hand side being an expression
    /// rather than a number.
    #[test]
    fn a_deed_can_read_what_it_is_changing() {
        let r = read("when tap 1: score = score + 1").expect("a rule");
        let mut t = Tally::new();
        t.values.insert("score".into(), 4.0);
        carry_out(&r, &mut t, &env());
        assert_eq!(t.get("score"), Some(5.0));
    }

    /// ★ Deeds happen **in order and each sees the last**, so a new question
    /// comes from the score just changed rather than from the old one. Writing
    /// them all from a frozen snapshot is defensible and is not what anybody
    /// reading the line left to right expects.
    #[test]
    fn deeds_happen_in_order_and_each_sees_the_last() {
        let r = read("when tap 1: score = score + 1, a = score * 10").expect("a rule");
        let mut t = Tally::new();
        t.values.insert("score".into(), 2.0);
        carry_out(&r, &mut t, &env());
        assert_eq!(t.get("score"), Some(3.0));
        assert_eq!(t.get("a"), Some(30.0), "it should have used the NEW score");
    }

    /// ★ A deed that will not evaluate leaves the value alone. A score that
    /// became NaN on one bad tap and stayed NaN for ever would look like the
    /// game having broken rather than one rule having a typo in it.
    #[test]
    fn a_broken_deed_leaves_the_score_where_it_was() {
        let r = read("when tap 1: score = score + 1, score = nonsense * 2").expect("a rule");
        let mut t = Tally::new();
        t.values.insert("score".into(), 7.0);
        carry_out(&r, &mut t, &env());
        assert_eq!(t.get("score"), Some(8.0), "the good deed should have happened and stuck");
    }

    /// And a deed that will not even parse is dropped, while the rest of the
    /// rule still works — the same rule as a bad row not blanking the drawing.
    #[test]
    fn a_half_typed_deed_does_not_lose_the_rule() {
        let r = read("when tap 1: score = score + 1, a = ((((").expect("a rule");
        assert_eq!(r.deeds.len(), 1);
    }

    /// Things that are not rules are not rules.
    #[test]
    fn it_knows_a_rule_from_an_ordinary_row() {
        assert!(read("circle(0, 2)").is_none());
        assert!(read("a = 3").is_none());
        assert!(read("when tap 1").is_none(), "no deeds");
        assert!(read("when tap: score = 1").is_none(), "no figure named");
        assert!(read("when tap x: score = 1").is_none(), "and it has to be a number");
        assert!(read("when tap 1:").is_none(), "an empty rule is not a rule");

        assert!(is_rule("when tap 1: a = 2"));
        assert!(!is_rule("a = 2"));
    }

    /// ★ The live values shadow the rows, and are written as source rather
    /// than pushed into an environment — so a value the game has changed is an
    /// ordinary binding behaving exactly like a typed one.
    #[test]
    fn the_tally_becomes_ordinary_rows() {
        let mut t = Tally::new();
        t.values.insert("score".into(), 3.0);
        t.values.insert("a".into(), 7.0);
        let rows = t.as_rows();
        assert!(rows.contains("score = 3"));
        assert!(rows.contains("a = 7"));
        // Sorted, so the same tally always makes the same source -- two
        // identical games differing by hash order would be horrible to debug.
        assert!(rows.find("a =") < rows.find("score ="));
    }

    /// Rewinding forgets where the game got to, so it can start again — which
    /// is why the live values are not written into the rows in the first place.
    #[test]
    fn rewinding_forgets_where_the_game_got_to() {
        let mut t = Tally::new();
        t.values.insert("score".into(), 9.0);
        t.clear();
        assert!(t.is_empty());
        assert_eq!(t.get("score"), None);
    }

    /// ★ No random number generator anywhere: a question comes from the score
    /// by arithmetic, so the same game replays identically — which is what the
    /// tapes in this repository have always needed.
    #[test]
    fn the_questions_are_made_of_arithmetic_and_so_replay_the_same() {
        let r = read("when tap 1: score = score + 1, a = 1 + floor(9*abs(sin(37*score)))").expect("a rule");
        let play = || {
            let mut t = Tally::new();
            t.values.insert("score".into(), 0.0);
            let mut seen = Vec::new();
            for _ in 0..8 {
                carry_out(&r, &mut t, &env());
                seen.push(t.get("a").expect("a"));
            }
            seen
        };
        let once = play();
        assert_eq!(once, play(), "the same game twice must be the same game");
        assert!(once.iter().all(|a| (1.0..=10.0).contains(a)), "and in range: {once:?}");
        assert!(once.windows(2).any(|w| w[0] != w[1]), "and not the same question every time");
    }
}
