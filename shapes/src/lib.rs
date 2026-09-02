//! # shapes — reusable things to draw, each carrying its own working
//!
//! Digits, faces, tally marks, waves. Every one is defined as a [`Recipe`] —
//! the shape *and* the construction lines that produce it — so
//!
//! ```text
//!     cargo run -p shapes -- smiley --steps
//! ```
//!
//! can show it being drawn one line of mathematics at a time, in a terminal,
//! with no window involved.
//!
//! ## The convention: everything is built about its own origin
//!
//! A shape here is always drawn around `0`, at roughly unit size. It does not
//! know where it will end up. Putting it somewhere is then one call:
//!
//! ```no_run
//! # use shapes::{face, digit};
//! # use plotkit::{Cx, Frame};
//! # let mut f = Frame::new();
//! f.place(face::smiley(1.0), Cx::new(-3.0, 2.0));
//! f.place(digit::glyph(7, 40), Cx::new(1.0, 2.0));
//!
//! // or, if you want the shape as a value first:
//! let seven = digit::glyph(7, 40).sized(1.5).at(Cx::new(1.0, 2.0));
//! ```
//!
//! All four of those are **inherent** methods — no trait to remember to
//! import. That was not always true, and the way it failed was nasty: import
//! the types without the trait and `place` was simply not there, with the
//! error blaming `Frame`.
//!
//! (It is `place`, not `draw`, because `Frame::draw` already means "render
//! this frame onto a canvas".)
//!
//! That is the whole reason the origin convention is worth insisting on. A
//! shape that already knows where it lives can only be drawn there. A shape
//! about the origin can be placed, repeated, rotated about anything, mapped
//! through any function — because `at(z)` is just `z ↦ z + at`, and it
//! composes with everything else the same way.
//!
//! ## What is in here
//!
//! | module | |
//! |---|---|
//! | [`fourier`] | any closed curve as a sum of rotating arrows |
//! | [`digit`] | 0–9, as outlines and as truncated Fourier series |
//! | [`face`] | a smiley and a ghost |
//! | [`bough`] | a tree, bent by sums of waves — the way a branch really bends |
//! | [`count`] | tally marks |
//! | [`cyclone`] | a 2D drawing that reads as 3D — one `sin(tilt)` does it |
//! | [`glyph`] | `+`, `=`, `?` |
//! | [`grab`] | shapes you can take hold of — drag to move, drag the rim to resize |
//! | [`motion`] | spin, walk, run, orbit — as values that compose |
//! | [`terrain`] | things standing on the ground, and what knocks them down |
//! | [`troupe`] | a group that is itself one of the things it groups |
//! | [`wave`] | `a sin(kx + φ)`, and what happens when you add them |
//! | [`wind`] | force as `v²`, the lean it causes, and gusts crossing the sky |

pub mod bough;
pub mod count;
pub mod cyclone;
pub mod digit;
pub mod face;
pub mod fourier;
pub mod glyph;
pub mod grab;
pub mod motion;
pub mod recipe;
pub mod stroke;
pub mod terrain;
pub mod troupe;
pub mod wave;
pub mod wind;

pub use fourier::Series;
pub use cyclone::Cyclone;
pub use grab::Disc;
pub use stroke::{Nib, Stroke};
pub use motion::{Motion, Pose};
pub use terrain::{Field, Tree};
pub use troupe::{Actor, Troupe};
pub use recipe::{Recipe, Step, STEP_COLOURS};
pub use wave::Wave;
pub use wind::Wind;

// Placing used to live here, as the traits `Place` and `Draw`. It does not any
// more: `Shape::at`, `Shape::sized`, `Frame::place` and `Recipe::at` are
// ordinary inherent methods now.
//
// The traits existed only because a crate cannot add an inherent method to a
// type it does not own — but `place` is `add(s.at(z))`, which is pure plotkit,
// so plotkit could always have owned it. The cost of getting that wrong was
// paid by anyone importing the types one at a time: the method simply was not
// there, and the error blamed `Frame` rather than the missing import.

/// Look a shape up by name, for the command line. Accepts digits as numerals
/// or words, and `tally7` for seven tally marks.
pub fn find(name: &str) -> Option<Recipe> {
    let n = name.trim().to_ascii_lowercase();
    const WORDS: [&str; 10] = ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine"];

    if let Some(d) = WORDS.iter().position(|w| *w == n) {
        return Some(digit::recipe(d as u32));
    }
    if n.len() == 1 {
        if let Some(d) = n.chars().next().unwrap().to_digit(10) {
            return Some(digit::recipe(d));
        }
    }
    if let Some(rest) = n.strip_prefix("tally") {
        return Some(count::recipe(rest.trim().parse().unwrap_or(5)));
    }
    Some(match n.as_str() {
        "smiley" | "smile" | "happy" => face::smiley_recipe(1.0),
        "ghost" | "boo" | "sad" => face::ghost_recipe(1.0),
        "plus" | "add" => glyph::plus_recipe(),
        "equals" | "eq" => glyph::equals_recipe(),
        "question" | "ask" => glyph::question_recipe(),
        "circle" => glyph::circle_recipe(),
        "square" => glyph::square_recipe(),
        "hexagon" | "hex" => glyph::ngon_recipe(6),
        "pentagon" => glyph::ngon_recipe(5),
        "triangle" => glyph::ngon_recipe(3),
        _ => return None,
    })
}

/// Every name [`find`] answers to, for `--list`.
pub fn catalogue() -> Vec<&'static str> {
    vec![
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "smiley", "ghost", "plus",
        "equals", "question", "circle", "square", "triangle", "pentagon", "hexagon", "tally5",
    ]
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use plotkit::{Cx, Shape};

    fn first_point(s: &Shape) -> Cx {
        s.polylines(Cx::new(-99.0, -99.0), Cx::new(99.0, 99.0), 300)[0][0]
    }

    /// ★ The convention itself: every shape in the catalogue is built about
    /// the origin, so `.at(z)` is enough to place it. If one drifted off
    /// centre, placing it would silently land it somewhere else.
    #[test]
    fn everything_is_centred_on_its_own_origin() {
        for name in catalogue() {
            let s = find(name).unwrap().shape();
            let runs = s.polylines(Cx::new(-99.0, -99.0), Cx::new(99.0, 99.0), 400);
            let all: Vec<Cx> = runs.into_iter().flatten().collect();
            let (lo_x, hi_x) = all.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.re), b.max(p.re)));
            let (lo_y, hi_y) = all.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.im), b.max(p.im)));
            let (cx, cy) = ((lo_x + hi_x) / 2.0, (lo_y + hi_y) / 2.0);
            let span = (hi_x - lo_x).max(hi_y - lo_y);
            assert!(cx.abs() < span * 0.30, "{name} sits off-centre in x: middle at {cx}, span {span}");
            assert!(cy.abs() < span * 0.30, "{name} sits off-centre in y: middle at {cy}, span {span}");
        }
    }

    /// Nothing in the catalogue is wildly out of scale with the rest, so a
    /// scene can place two of them side by side without one dwarfing the
    /// other.
    #[test]
    fn everything_is_about_unit_size() {
        for name in catalogue() {
            let s = find(name).unwrap().shape();
            let all: Vec<Cx> = s.polylines(Cx::new(-99.0, -99.0), Cx::new(99.0, 99.0), 400).into_iter().flatten().collect();
            let r = all.iter().fold(0.0f64, |m, p| m.max(p.abs()));
            assert!((0.3..3.0).contains(&r), "{name} has reach {r}, which is not near unit size");
        }
    }

    #[test]
    fn placing_lands_where_it_says() {
        let spot = Cx::new(4.0, -2.0);
        let s = Shape::point(Cx::ZERO).at(spot);
        assert!((first_point(&s) - spot).abs() < 1e-12);
    }

    /// Size then place is not the same as place then size, and the API keeps
    /// them in the order you would say them: "a smiley, twice as big, over
    /// there".
    #[test]
    fn size_then_place_puts_it_where_asked() {
        let s = Shape::point(Cx::new(1.0, 0.0)).sized(3.0).at(Cx::new(10.0, 0.0));
        assert!((first_point(&s) - Cx::new(13.0, 0.0)).abs() < 1e-12);
    }

    #[test]
    fn names_are_forgiving() {
        assert!(find("7").is_some());
        assert!(find("seven").is_some());
        assert!(find(" SEVEN ").is_some());
        assert!(find("tally3").is_some());
        assert!(find("nonsense").is_none());
    }

    /// Every catalogue entry has working to show, not just a shape.
    #[test]
    fn every_shape_explains_itself() {
        for name in catalogue() {
            let r = find(name).unwrap();
            assert!(!r.is_empty(), "{name} has no steps");
            assert!(!r.maths.is_empty(), "{name} has no line of maths");
            assert!(r.steps.iter().all(|s| !s.says.is_empty()), "{name} has a silent step");
        }
    }
}
