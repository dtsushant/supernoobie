//! # script — shapes written rather than drawn, the Desmos way
//!
//! A drawing has two halves and they belong side by side. Some shapes are made
//! by a hand — those are [`Mark`](crate::Mark)s. Some are made by saying what
//! they *are*:
//!
//! ```text
//!     r = 2
//!     n = 7
//!     param(r * exp(i*t) + 0.4*exp(7i*t), 0, tau)
//!     ngon(0, r, n)
//! ```
//!
//! Change `n` and the shape changes. Change `r` and **everything that mentions
//! it** changes at once, which is the whole point of a variable and the reason
//! Desmos feels the way it does.
//!
//! ## Rows, not a file
//!
//! The script is a **list of rows**, not one lump of text, and that is the
//! design decision everything else follows from. A row can be switched off
//! without deleting it, moved, or given a slider — and each one is small
//! enough that a mistake in it is obviously local. A single text box makes all
//! of those either impossible or fiddly.
//!
//! Rows are joined into one source before running, so a variable defined in
//! row 2 is available in row 9. They are one program written on several lines,
//! not several programs.
//!
//! ## The language is already here
//!
//! None of the parsing is new: [`plotkit::expr`] does it, and it works in
//! complex numbers, so a point and an offset and a plain number are the same
//! kind of thing. `a*z + b` is any rotation, scaling and shift written exactly
//! as it reads.
//!
//! ```text
//!     built in     i  pi  tau  e  time
//!     functions    exp ln sin cos tan sqrt abs arg conj re im polar pow
//!     commands     point line polygon circle ngon plot param implicit color
//! ```
//!
//! ## `time` is the one thing added
//!
//! Desmos has no clock. This does, so the studio's clock is bound as `time`
//! before every run — which means a written shape can animate without any
//! keyframes at all:
//!
//! ```text
//!     param(exp(i*(t + time)), 0, tau)
//! ```
//!
//! It is bound by **writing a line into the source**, not by a special case in
//! the evaluator, so `time` is an ordinary variable that behaves like any
//! other and shows up in the same list.
//!
//! ## Why a bad row is not fatal
//!
//! Half-typed text is the normal state of a row being edited. If a mistake
//! blanked the drawing you would lose sight of your work on the way to every
//! change. So errors are collected and reported per line, and every row that
//! *does* parse still draws.

use std::collections::HashMap;

use plotkit::expr::{env_of, eval_with, Cmd, Expr};
use plotkit::{plot, Cx, Shape};

use crate::rule::{self, Rule, Tally};

/// Colours a script cycles through when it does not say.
pub const PALETTE: [u32; 6] = [0x4FBCD4, 0xE0A44A, 0xE585AC, 0x6FCF97, 0x9B7BD4, 0xE0704A];

/// How many samples a written curve gets.
const SAMPLES: usize = 320;

/// One line of the script.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub text: String,
    /// Off means it is skipped — kept, but not run. Deleting a row to try
    /// something is how you lose the row.
    pub on: bool,
}

impl Row {
    pub fn new(text: impl Into<String>) -> Row {
        Row { text: text.into(), on: true }
    }

    pub fn off(mut self) -> Row {
        self.on = false;
        self
    }

    /// The name this row binds, if it binds one. `a = 3` binds `a`.
    pub fn binds(&self) -> Option<&str> {
        let (left, _) = self.text.split_once('=')?;
        let name = left.trim();
        // A name, not an expression: `a = 3` binds, `f(x) = 3` and `a + b = 3`
        // do not, and neither does `==` if anybody ever writes one.
        let ok = !name.is_empty()
            && name.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        ok.then_some(name)
    }
}

/// What a run produced.
#[derive(Clone, Default)]
pub struct Made {
    pub shapes: Vec<(Shape, u32)>,
    /// Every binding, for the sliders.
    pub vars: Vec<(String, Cx)>,
    /// `(row, message)`. Reported, never fatal.
    pub errors: Vec<(usize, String)>,
}

/// The written half of a drawing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Script {
    pub rows: Vec<Row>,
}

impl Script {
    pub fn new() -> Script {
        Script { rows: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.rows.iter().all(|r| r.text.trim().is_empty())
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn add(&mut self, text: impl Into<String>) {
        self.rows.push(Row::new(text));
    }

    /// The rows as one program.
    ///
    /// Rows that are switched off become blank lines rather than
    /// disappearing, so the line numbers in an error still point at the row
    /// you can see. So do rows this crate handles itself — rules, and
    /// [`digits`](crate::script) — since `plotkit::expr` has never heard of
    /// them and would report every one as a mistake.
    fn source(&self, time: f64, tally: &Tally) -> String {
        let mut out = format!("time = {time}\n");
        // What the game has got to goes first, because `expr` evaluates
        // commands where it finds them: a binding written *after* a `circle`
        // never reaches that circle. So the live values have to be in place
        // before anything is drawn.
        //
        // Which means shadowing cannot be "last one wins" — it has to be the
        // row **standing down**. A row the tally has a value for is skipped,
        // so `score = 0` says where the game starts and stops saying anything
        // the moment play has moved on. Left in, it would reset the score on
        // every single frame.
        out.push_str(&tally.as_rows());
        for r in &self.rows {
            let shadowed = r.binds().is_some_and(|n| tally.values.contains_key(n));
            if r.on && !mine(&r.text) && !shadowed {
                out.push_str(&r.text);
            }
            out.push('\n');
        }
        out
    }

    /// Every rule in the script.
    pub fn rules(&self) -> Vec<Rule> {
        self.rows.iter().filter(|r| r.on).filter_map(|r| rule::read(&r.text)).collect()
    }

    /// Run it, with the clock bound as `time`.
    pub fn run(&self, time: f64) -> Made {
        self.play(time, &Tally::new())
    }

    /// Run it as a game that has got somewhere.
    pub fn play(&self, time: f64, tally: &Tally) -> Made {
        let program = plotkit::expr::run(&self.source(time, tally));
        let env = env_of(&program);

        let mut made = Made {
            // The `time =` line and the tally's rows come before the rows, so
            // every reported line number is that much further down than the
            // row it came from.
            errors: program
                .errors
                .iter()
                .map(|(line, msg)| (line.saturating_sub(1 + tally.values.len()), msg.clone()))
                .collect(),
            vars: program.vars.clone(),
            shapes: Vec::new(),
        };

        let mut colour = PALETTE[0];
        let mut pinned = false;
        let mut next = 0usize;

        for cmd in &program.cmds {
            if let Cmd::Color(c) = cmd {
                colour = *c;
                pinned = true;
                continue;
            }
            let col = if pinned {
                colour
            } else {
                let c = PALETTE[next % PALETTE.len()];
                next += 1;
                c
            };
            if let Some(shape) = shape_of(cmd, &env) {
                made.shapes.push((shape, col));
            }
        }
        made.shapes.extend(self.shown(&env));
        made
    }

    /// The numbers this script asks to be shown, as shapes.
    ///
    /// `digits(value, at, size)` — a number written out where you say, at the
    /// size you say. Not part of [`plotkit::expr`], because the digits are
    /// drawn by [`shapes::digit`] and `plotkit` has never heard of `shapes`;
    /// the dependency only runs one way and it should stay that way.
    ///
    /// So this crate reads those rows itself, evaluates their arguments
    /// against everything else, and leaves the rest of the line to `expr`.
    fn shown(&self, env: &std::collections::HashMap<String, Cx>) -> Vec<(Shape, u32)> {
        let mut out = Vec::new();
        for r in &self.rows {
            if !r.on {
                continue;
            }
            let Some(args) = r.text.trim().strip_prefix("digits(").and_then(|a| a.strip_suffix(')')) else {
                continue;
            };
            let mut arg = args.split(',');
            let mut number = |fallback: f64| {
                arg.next()
                    .and_then(|a| plotkit::expr::parse(a.trim()).ok())
                    .and_then(|e| e.eval(env).ok())
                    .map_or(fallback, |z| z.re)
            };
            let value = number(0.0);
            let (x, y, size) = (number(0.0), number(0.0), number(1.0));
            out.push((number_shape(value, Cx::new(x, y), size), 0xE3E9EF));
        }
        out
    }

    /// The bindings that are plain real numbers, which are the ones a slider
    /// can sensibly move.
    ///
    /// `time` is left out: it has a clock of its own, and a slider that fought
    /// the clock would be maddening.
    pub fn dials(&self, at: f64) -> Vec<(String, f64)> {
        self.run(at)
            .vars
            .into_iter()
            .filter(|(name, v)| name != "time" && v.im.abs() < 1e-12)
            .map(|(name, v)| (name, v.re))
            .collect()
    }

    /// Read a plain script file — one row per line.
    ///
    /// The `.rec` files this repository already has are exactly this: a
    /// program, one statement to a line. Comments and blank lines become rows
    /// too, kept as they are, because a comment explaining a row belongs next
    /// to it and losing it on the way in would be rude.
    pub fn from_rec(text: &str) -> Script {
        Script { rows: text.lines().map(|l| Row::new(l.trim_end())).collect() }
    }

    /// The rows back out as a plain script file.
    pub fn to_rec(&self) -> String {
        let mut out = String::new();
        for r in &self.rows {
            // A switched-off row is written commented out, which is what off
            // means in a file that has no notion of off.
            if !r.on && !r.text.trim().is_empty() {
                out.push_str("# ");
            }
            out.push_str(&r.text);
            out.push('\n');
        }
        out
    }

    /// Move a dial, by rewriting the row that binds it.
    ///
    /// The **text stays the only truth**. An overriding table beside it would
    /// be a second place for the value to live, and then saving, undoing and
    /// editing the row by hand all have to agree about which one wins.
    pub fn set_dial(&mut self, name: &str, value: f64) -> bool {
        for r in self.rows.iter_mut() {
            if r.binds() == Some(name) {
                r.text = format!("{name} = {value:.4}");
                return true;
            }
        }
        false
    }
}

/// Is this row one this crate handles rather than `plotkit::expr`?
fn mine(text: &str) -> bool {
    let t = text.trim();
    rule::is_rule(t) || t.starts_with("digits(")
}

/// A whole number written out, about a point.
///
/// Negative numbers get a bar in front, and a number too long to be a number
/// anybody meant is cut off rather than drawn across the whole page.
fn number_shape(value: f64, at: Cx, size: f64) -> Shape {
    let size = size.abs().max(0.05);
    let n = value.round();
    let text = format!("{}", n.abs().min(1e9) as u64);
    let mut parts = Vec::new();
    let step = size * 1.35;
    let mut x = 0.0;
    if n < 0.0 {
        parts.push(Shape::path(vec![Cx::new(-size * 0.4, 0.0), Cx::new(size * 0.4, 0.0)]).at(at + Cx::new(x, 0.0)));
        x += step;
    }
    for c in text.chars() {
        let d = c.to_digit(10).unwrap_or(0);
        parts.push(shapes::digit::glyph(d, 40).sized(size).at(at + Cx::new(x, 0.0)));
        x += step;
    }
    // Centred on the point given, which is what anybody means by "put the
    // number here" -- a number that grew to the right as it got bigger would
    // walk out of its own box.
    Shape::group(parts).at(Cx::new(-x * 0.5 + step * 0.5, 0.0))
}

/// A command as a drawable shape.
///
/// The deferred ones — `plot`, `param`, `implicit` — carry an expression with
/// a free variable, and this is where it finally gets bound: once per sample,
/// inside the closure, so the curve stays smooth at any zoom rather than being
/// a fixed list of points.
fn shape_of(cmd: &Cmd, env: &HashMap<String, Cx>) -> Option<Shape> {
    Some(match cmd {
        Cmd::Color(_) => return None,
        Cmd::Point(pts) => Shape::points(pts.clone()),
        Cmd::Line(pts) => Shape::path(pts.clone()),
        Cmd::Polygon(pts) => Shape::polygon(pts.clone()),
        Cmd::Circle(centre, r) => Shape::circle(*centre, *r),
        Cmd::Ngon(centre, r, n) => Shape::polygon(plot::ngon(*centre, *r, *n, 0.0)),
        Cmd::Plot(e) => {
            let (e, env) = (e.clone(), env.clone());
            // A sample that will not evaluate becomes a gap rather than a
            // panic: `1/x` at zero is a real thing to write.
            Shape::graph(move |x| bind(&e, "x", Cx::new(x, 0.0), &env).map_or(f64::NAN, |z| z.re))
        }
        Cmd::Param(e, t0, t1) => {
            let (e, env) = (e.clone(), env.clone());
            Shape::param(move |t| bind(&e, "t", Cx::new(t, 0.0), &env).unwrap_or(Cx::ZERO), *t0, *t1, SAMPLES)
        }
        Cmd::Implicit(e, level) => {
            let (e, env) = (e.clone(), env.clone());
            Shape::implicit(
                move |x, y| {
                    let mut here = env.clone();
                    here.insert("x".into(), Cx::new(x, 0.0));
                    here.insert("y".into(), Cx::new(y, 0.0));
                    e.eval(&here).map_or(f64::NAN, |z| z.re)
                },
                *level,
            )
        }
    })
}

fn bind(e: &Expr, name: &str, v: Cx, env: &HashMap<String, Cx>) -> Option<Cx> {
    eval_with(e, name, v, env).ok()
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule::Tally;

    fn ink(shapes: &[(Shape, u32)]) -> usize {
        let (lo, hi) = (Cx::new(-10.0, -10.0), Cx::new(10.0, 10.0));
        shapes.iter().map(|(s, _)| s.polylines(lo, hi, 600).iter().map(Vec::len).sum::<usize>()).sum()
    }

    /// Where the shapes reach, so a change of size can be measured.
    fn extent(shapes: &[(Shape, u32)]) -> f64 {
        let (lo, hi) = (Cx::new(-50.0, -50.0), Cx::new(50.0, 50.0));
        shapes
            .iter()
            .flat_map(|(s, _)| s.polylines(lo, hi, 600))
            .flatten()
            .map(|z| z.abs())
            .fold(0.0, f64::max)
    }

    /// ★ **The whole point of a variable.** One number, changed in one place,
    /// moves everything that mentions it — which is what makes this worth
    /// having over a list of fixed shapes.
    #[test]
    fn changing_one_number_changes_everything_that_mentions_it() {
        let mut s = Script::new();
        s.add("r = 2");
        s.add("circle(0, r)");
        s.add("ngon(0, r, 6)");
        s.add("param(r * exp(i*t), 0, tau)");

        let small = extent(&s.run(0.0).shapes);
        assert!((small - 2.0).abs() < 0.1, "everything should be radius 2, not {small}");

        assert!(s.set_dial("r", 5.0));
        let big = extent(&s.run(0.0).shapes);
        assert!((big - 5.0).abs() < 0.1, "all three should have grown, to {big}");
    }

    /// ★ And the text stays the only truth. A value kept beside the text would
    /// be a second place for it to live, and then saving, undoing and editing
    /// the row by hand all have to agree about which one wins.
    #[test]
    fn moving_a_dial_rewrites_the_row_itself() {
        let mut s = Script::new();
        s.add("a = 1");
        s.set_dial("a", 3.5);
        assert!(s.rows[0].text.contains("3.5"), "the row should say so: {}", s.rows[0].text);
        assert_eq!(s.dials(0.0), vec![("a".to_string(), 3.5)]);
    }

    /// Rows are one program written on several lines, so a variable defined
    /// near the top is available near the bottom.
    #[test]
    fn a_variable_defined_in_one_row_is_available_in_another() {
        let mut s = Script::new();
        s.add("a = 3");
        s.add("b = a * 2");
        s.add("circle(0, b)");
        assert!((extent(&s.run(0.0).shapes) - 6.0).abs() < 0.1);
    }

    /// ★ A bad row loses that row and nothing else. Half-typed text is the
    /// normal state of a row being edited, and a drawing that blanked on every
    /// keystroke would hide your work exactly while you were changing it.
    #[test]
    fn a_half_typed_row_does_not_blank_the_drawing() {
        let mut s = Script::new();
        s.add("circle(0, 2)");
        s.add("param(exp(i*t");
        s.add("ngon(0, 1, 5)");

        let made = s.run(0.0);
        assert!(!made.errors.is_empty(), "it should say something is wrong");
        assert_eq!(made.shapes.len(), 2, "and still draw the two good rows");
    }

    /// And the error points at the row you can see, not one line further down
    /// because of the `time =` line quietly added at the top.
    #[test]
    fn an_error_points_at_the_row_it_came_from() {
        let mut s = Script::new();
        s.add("circle(0, 1)");
        s.add("this is not a thing");
        let made = s.run(0.0);
        let (row, _) = made.errors.first().expect("an error");
        assert_eq!(*row, 2, "the second row, counting from one");
    }

    /// ★ A row switched off is kept and skipped. Deleting a row to try
    /// something is how you lose the row.
    #[test]
    fn a_row_can_be_switched_off_without_losing_it() {
        let mut s = Script::new();
        s.add("circle(0, 2)");
        s.add("ngon(0, 3, 5)");
        assert_eq!(s.run(0.0).shapes.len(), 2);

        s.rows[1].on = false;
        assert_eq!(s.run(0.0).shapes.len(), 1);
        assert_eq!(s.rows[1].text, "ngon(0, 3, 5)", "the text is still there");

        s.rows[1].on = true;
        assert_eq!(s.run(0.0).shapes.len(), 2, "and it comes back");
    }

    /// Switching a row off must not shift the line numbers of the rows below
    /// it, or every error afterwards points at the wrong row.
    #[test]
    fn switching_a_row_off_does_not_move_the_others() {
        let mut s = Script::new();
        s.add("circle(0, 1)");
        s.add("ngon(0, 1, 5)");
        s.add("rubbish here");
        s.rows[1].on = false;
        assert_eq!(s.run(0.0).errors.first().map(|(r, _)| *r), Some(3));
    }

    /// ★ `time` is an ordinary variable, bound by writing a line rather than
    /// by a special case in the evaluator — so a written shape can animate
    /// with no keyframes at all.
    #[test]
    fn a_written_shape_can_move_with_the_clock() {
        let mut s = Script::new();
        s.add("circle(time, 1)");

        let centre = |t: f64| {
            let made = s.run(t);
            let (lo, hi) = (Cx::new(-50.0, -50.0), Cx::new(50.0, 50.0));
            let pts: Vec<Cx> = made.shapes[0].0.polylines(lo, hi, 400).into_iter().flatten().collect();
            let sum = pts.iter().fold(Cx::ZERO, |a, z| a + *z);
            sum.scale(1.0 / pts.len() as f64)
        };
        assert!(centre(0.0).re.abs() < 0.1);
        assert!((centre(3.0).re - 3.0).abs() < 0.1, "it should have moved with the clock");
    }

    /// But `time` is not offered as a dial — it has a clock of its own, and a
    /// slider fighting the clock would be maddening.
    #[test]
    fn the_clock_is_not_offered_as_a_slider() {
        let mut s = Script::new();
        s.add("a = 2");
        s.add("circle(time, a)");
        let dials = s.dials(1.0);
        assert!(dials.iter().any(|(n, _)| n == "a"));
        assert!(!dials.iter().any(|(n, _)| n == "time"));
    }

    /// Only real numbers get a dial. A slider that could only ever move half
    /// of a complex number is a worse lie than no slider.
    #[test]
    fn only_plain_numbers_get_a_dial() {
        let mut s = Script::new();
        s.add("a = 2");
        s.add("c = 1 + 2i");
        let dials = s.dials(0.0);
        assert!(dials.iter().any(|(n, _)| n == "a"));
        assert!(!dials.iter().any(|(n, _)| n == "c"), "a complex binding is not a slider");
    }

    /// What counts as binding a name, and what does not.
    #[test]
    fn it_knows_a_binding_from_an_expression() {
        assert_eq!(Row::new("a = 3").binds(), Some("a"));
        assert_eq!(Row::new("  wobble  =  1 + 2i ").binds(), Some("wobble"));
        assert_eq!(Row::new("a_2 = 3").binds(), Some("a_2"));
        assert_eq!(Row::new("circle(0, 1)").binds(), None);
        assert_eq!(Row::new("2a = 3").binds(), None, "a name does not start with a digit");
        assert_eq!(Row::new("a + b = 3").binds(), None);
        assert_eq!(Row::new("= 3").binds(), None);
    }

    /// ★ A sample that will not evaluate is a **gap**, not a panic. `1/x` at
    /// zero is a perfectly ordinary thing to write.
    #[test]
    fn a_curve_with_a_hole_in_it_is_survivable() {
        let mut s = Script::new();
        s.add("plot(1/x)");
        let made = s.run(0.0);
        assert_eq!(made.shapes.len(), 1);
        // Drawing it must not panic, and must produce something either side.
        assert!(ink(&made.shapes) > 10);
    }

    /// An empty script is an empty drawing, not an error.
    #[test]
    fn an_empty_script_draws_nothing_and_complains_about_nothing() {
        let s = Script::new();
        let made = s.run(0.0);
        assert!(made.shapes.is_empty());
        assert!(made.errors.is_empty());
        assert!(s.is_empty());

        let mut blanks = Script::new();
        blanks.add("");
        blanks.add("   ");
        assert!(blanks.is_empty());
        assert!(blanks.run(0.0).errors.is_empty(), "blank rows are not mistakes");
    }

    /// ★ A plain script file comes in as rows, comments and all -- a comment
    /// explaining a row belongs next to it, and losing it on the way in would
    /// be rude.
    #[test]
    fn a_plain_script_file_comes_in_as_rows() {
        let text = "# the radius
r = 2

circle(0, r)
";
        let s = Script::from_rec(text);
        assert_eq!(s.len(), 4, "including the comment and the blank line");
        assert_eq!(s.rows[0].text, "# the radius");
        assert_eq!(s.rows[3].text, "circle(0, r)");
        assert_eq!(s.run(0.0).shapes.len(), 1);
    }

    /// And goes back out again, with a switched-off row commented -- which is
    /// what "off" means in a file that has no notion of off.
    #[test]
    fn rows_go_back_out_as_a_script_file() {
        let mut s = Script::new();
        s.add("r = 2");
        s.rows.push(Row::new("ngon(0, r, 5)").off());
        let text = s.to_rec();
        assert!(text.contains("r = 2"));
        assert!(text.contains("# ngon(0, r, 5)"), "off means commented: {text}");

        // And a round trip keeps the shapes that were on.
        let back = Script::from_rec(&text);
        assert_eq!(back.run(0.0).shapes.len(), s.run(0.0).shapes.len());
    }

    /// ★ A rule is a row like any other, and `expr` has never heard of it --
    /// so it must be kept out of the program rather than reported as a
    /// mistake on every frame.
    #[test]
    fn a_rule_row_is_not_a_mistake() {
        let mut s = Script::new();
        s.add("score = 0");
        s.add("when tap 1: score = score + 1");
        s.add("circle(0, 2)");

        let made = s.run(0.0);
        assert!(made.errors.is_empty(), "a rule should not be an error: {:?}", made.errors);
        assert_eq!(made.shapes.len(), 1);
        assert_eq!(s.rules().len(), 1);
    }

    /// ★ What the game has got to shadows what the rows say, so the rows are
    /// the STARTING position and the tally is where play has reached.
    #[test]
    fn the_tally_shadows_the_rows() {
        let mut s = Script::new();
        s.add("score = 0");
        s.add("circle(0, 1 + score)");

        let radius = |made: &Made| {
            let (lo, hi) = (Cx::new(-50.0, -50.0), Cx::new(50.0, 50.0));
            made.shapes[0].0.polylines(lo, hi, 400).into_iter().flatten().map(|z| z.abs()).fold(0.0, f64::max)
        };
        assert!((radius(&s.run(0.0)) - 1.0).abs() < 0.05, "the row says nought");

        let mut t = Tally::new();
        t.values.insert("score".into(), 4.0);
        assert!((radius(&s.play(0.0, &t)) - 5.0).abs() < 0.05, "and the game says four");

        t.clear();
        assert!((radius(&s.play(0.0, &t)) - 1.0).abs() < 0.05, "rewinding gets the starting position back");
    }

    /// ★ `digits(...)` writes a number out. Not part of `expr`, because the
    /// digits are drawn by `shapes` and `plotkit` has never heard of `shapes`.
    #[test]
    fn a_number_can_be_written_out() {
        let mut s = Script::new();
        s.add("a = 3");
        s.add("b = 4");
        s.add("digits(a + b, 0, 0, 1)");
        let made = s.run(0.0);
        assert!(made.errors.is_empty(), "{:?}", made.errors);
        assert_eq!(made.shapes.len(), 1, "one number");

        // Seven is one digit; seventy-seven is two, and wider.
        let width = |src: &str| {
            let mut s = Script::new();
            s.add(src);
            let (lo, hi) = (Cx::new(-50.0, -50.0), Cx::new(50.0, 50.0));
            let pts: Vec<Cx> = s.run(0.0).shapes[0].0.polylines(lo, hi, 500).into_iter().flatten().collect();
            let (a, b) = pts.iter().fold((f64::MAX, f64::MIN), |(a, b), z| (a.min(z.re), b.max(z.re)));
            b - a
        };
        assert!(width("digits(77, 0, 0, 1)") > width("digits(7, 0, 0, 1)") * 1.5, "two digits should be wider");
    }

    /// And the numbers actually differ, so it is writing the number and not
    /// the same glyph every time.
    #[test]
    fn different_numbers_look_different() {
        let ink = |n: i32| {
            let mut s = Script::new();
            s.add(&format!("digits({n}, 0, 0, 1)"));
            let (lo, hi) = (Cx::new(-9.0, -9.0), Cx::new(9.0, 9.0));
            let pts: Vec<Cx> = s.run(0.0).shapes[0].0.polylines(lo, hi, 400).into_iter().flatten().collect();
            pts.iter().fold(0.0, |a: f64, z| a + z.abs())
        };
        assert!((ink(1) - ink(8)).abs() > 1e-6, "a one and an eight should not be the same drawing");
    }

    /// Colours cycle so a script of bare shapes is still readable, and
    /// `color(...)` pins from there on.
    #[test]
    fn shapes_take_different_colours_until_told_otherwise() {
        let mut s = Script::new();
        s.add("circle(0, 1)");
        s.add("circle(0, 2)");
        let made = s.run(0.0);
        assert_ne!(made.shapes[0].1, made.shapes[1].1);

        let mut pinned = Script::new();
        pinned.add("color(0xFF0000)");
        pinned.add("circle(0, 1)");
        pinned.add("circle(0, 2)");
        let made = pinned.run(0.0);
        assert_eq!(made.shapes[0].1, made.shapes[1].1, "pinned means pinned");
    }
}
