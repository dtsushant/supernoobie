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
//! # use shapes::{Place, Draw, face, digit};
//! # use plotkit::{Cx, Frame};
//! # let mut f = Frame::new();
//! f.place(face::smiley(1.0), Cx::new(-3.0, 2.0));
//! f.place(digit::glyph(7, 40), Cx::new(1.0, 2.0));
//!
//! // or, if you want the shape as a value first:
//! let seven = digit::glyph(7, 40).sized(1.5).at(Cx::new(1.0, 2.0));
//! ```
//!
//! (It is `place`, not `draw`, because `Frame::draw` already means "render
//! this frame onto a canvas". An inherent method silently wins over a trait
//! one, so a second `draw` would not have been a second `draw` — it would have
//! been a puzzle.)
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
//! | [`count`] | tally marks |
//! | [`cyclone`] | a 2D drawing that reads as 3D — one `sin(tilt)` does it |
//! | [`glyph`] | `+`, `=`, `?` |
//! | [`grab`] | shapes you can take hold of — drag to move, drag the rim to resize |
//! | [`motion`] | spin, walk, run, orbit — as values that compose |
//! | [`troupe`] | a group that is itself one of the things it groups |
//! | [`wave`] | `a sin(kx + φ)`, and what happens when you add them |

pub mod count;
pub mod cyclone;
pub mod digit;
pub mod face;
pub mod fourier;
pub mod glyph;
pub mod grab;
pub mod motion;
pub mod recipe;
pub mod troupe;
pub mod wave;

pub use fourier::Series;
pub use cyclone::Cyclone;
pub use grab::Disc;
pub use motion::{Motion, Pose};
pub use troupe::{Actor, Troupe};
pub use recipe::{Recipe, Step, STEP_COLOURS};
pub use wave::Wave;

use plotkit::frame::StyleRef;
use plotkit::{Cx, Frame, Shape};

/// Put a shape somewhere, or change its size.
///
/// Both are ordinary maps — `z ↦ z + at` and `z ↦ kz` — which is why they
/// work identically on a bare [`Shape`] and on a whole [`Recipe`] with its
/// construction lines attached.
pub trait Place: Sized {
    fn at(self, z: Cx) -> Self;
    fn sized(self, k: f64) -> Self;
}

impl Place for Shape {
    fn at(self, z: Cx) -> Shape {
        self.shift(z)
    }
    fn sized(self, k: f64) -> Shape {
        self.scaled(k)
    }
}

impl Place for Recipe {
    fn at(self, z: Cx) -> Recipe {
        self.map_all(move |w| w + z)
    }
    fn sized(self, k: f64) -> Recipe {
        self.map_all(move |w| w.scale(k))
    }
}

/// `frame.place(shape, at)` — put a shape at a coordinate in one call.
///
/// Returns the style, so it reads the way the picture is described:
///
/// ```text
///     f.place(face::ghost(1.0), spot).color(0x9B7BD4).width(3);
/// ```
pub trait Draw {
    fn place(&mut self, s: Shape, at: Cx) -> StyleRef<'_>;

    /// A recipe's finished shape, placed. The construction lines are dropped;
    /// use [`Recipe::steps`] directly to show the working.
    fn place_recipe(&mut self, r: &Recipe, at: Cx) -> StyleRef<'_> {
        self.place(r.shape(), at)
    }
}

impl Draw for Frame {
    fn place(&mut self, s: Shape, at: Cx) -> StyleRef<'_> {
        self.add(s.at(at))
    }
}

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
