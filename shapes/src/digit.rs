//! # digit — 0 to 9, made of sine waves
//!
//! No font anywhere. Each digit is a closed outline, and each outline goes
//! through [`Series`] to become a sum of rotating arrows. Ask for six waves
//! and you get a wobbly ribbon; ask for forty and you get the digit.
//!
//! Everything is drawn about the origin in a box of roughly
//! `x ∈ [-0.4, 0.4]`, `y ∈ [-0.9, 0.9]`, so `glyph(7, 40).at(z)` puts a seven
//! at `z`.

use crate::fourier::{arc, there_and_back, Series};
use crate::recipe::Recipe;
use plotkit::{Cx, Shape};
use std::f64::consts::{PI, TAU};

/// The outline of a digit, as one continuous closed loop.
///
/// Each is a single stroke of the pen wherever possible, because a stroke that
/// jumps would need a travel line and the travel line would be drawn.
pub fn outline(d: u32) -> Vec<Cx> {
    let p = |x: f64, y: f64| Cx::new(x, y);
    let stroke: Vec<Cx> = match d {
        // Already closed: a plain ellipse.
        0 => return arc(Cx::ZERO, 0.40, 0.88, 0.0, TAU, 120),

        1 => vec![p(-0.30, 0.50), p(0.02, 0.92), p(0.02, -0.90)],

        2 => {
            let mut v = vec![p(-0.36, 0.50)];
            v.extend(arc(p(0.00, 0.48), 0.36, 0.40, 2.6, -0.2, 34));
            v.extend([p(-0.34, -0.88), p(0.38, -0.88)]);
            v
        }

        3 => {
            let mut v = arc(p(0.00, 0.48), 0.34, 0.40, 2.3, -1.4, 32);
            v.extend(arc(p(0.00, -0.42), 0.38, 0.46, 1.5, -2.5, 36));
            v
        }

        // Up the stem, down the diagonal, across the bar, back to the stem.
        4 => vec![p(0.20, 0.92), p(-0.40, -0.14), p(0.40, -0.14), p(0.20, -0.14), p(0.20, -0.90)],

        5 => {
            let mut v = vec![p(0.36, 0.90), p(-0.28, 0.90), p(-0.32, 0.10)];
            v.extend(arc(p(0.02, -0.40), 0.38, 0.48, 1.7, -2.4, 36));
            v
        }

        // A tail curling into a closed bowl.
        6 => {
            let mut v = arc(p(0.02, 0.30), 0.36, 0.58, 0.9, PI, 30);
            v.extend(arc(p(0.00, -0.40), 0.38, 0.46, PI, PI - TAU, 46));
            v
        }

        7 => vec![p(-0.38, 0.90), p(0.38, 0.90), p(-0.06, -0.90)],

        // A figure of eight, traced through the junction at (0, 0.04).
        8 => {
            let mut v = arc(p(0.00, 0.44), 0.32, 0.40, -PI / 2.0, -PI / 2.0 + TAU, 52);
            v.extend(arc(p(0.00, -0.42), 0.38, 0.46, PI / 2.0, PI / 2.0 - TAU, 56));
            return v;
        }

        // The mirror of 6: a closed bowl on top, with a tail falling away.
        _ => {
            let mut v = arc(p(0.00, 0.42), 0.36, 0.44, 0.0, TAU, 46);
            v.extend([p(0.34, -0.28), p(0.28, -0.62), p(0.10, -0.84), p(-0.20, -0.90)]);
            v
        }
    };
    there_and_back(stroke)
}

/// A digit's waves. Costs a 256-point transform, so hold on to the result if
/// it is wanted every frame.
pub fn series(d: u32) -> Series {
    Series::of(&outline(d), 256)
}

/// A digit as a shape, built from its `terms` loudest waves, about the origin.
pub fn glyph(d: u32, terms: usize) -> Shape {
    series(d).curve(terms)
}

/// A digit, with its working shown: the outline it came from, then the same
/// curve rebuilt from a handful of waves, then from all of them.
pub fn recipe(d: u32) -> Recipe {
    let s = series(d);
    Recipe::new(
        format!("{d}"),
        "z(θ) = Σ c_n e^{inθ},  c_n = (1/N) Σ z_k e^{-inθ_k}   —  a digit is a sum of rotating arrows",
    )
    .step(format!("the outline of a {d}, walked out and back so it closes"), Shape::path(outline(d)))
    .step("1 wave: one arrow, standing still — the middle of the digit", s.curve(1))
    .step("2 waves: a second arrow turns once per lap, so a circle", s.curve(2))
    .step("6 waves: enough to guess the digit, not to read it", s.curve(6))
    .step("16 waves", s.curve(16))
    .step("40 waves — the digit", s.curve(40))
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// An open outline would make the series ring at the seam and the digit
    /// would grow a whisker.
    #[test]
    fn every_digit_is_a_closed_loop() {
        for d in 0..10 {
            let o = outline(d);
            assert!((o[0] - *o.last().expect("points")).abs() < 0.06, "digit {d} does not close");
        }
    }

    /// Every digit sits in the same box, so digits placed side by side line up
    /// and none of them dwarfs its neighbours.
    #[test]
    fn every_digit_fits_the_same_box() {
        for d in 0..10 {
            let o = outline(d);
            let x = o.iter().fold(0.0f64, |m, p| m.max(p.re.abs()));
            let y = o.iter().fold(0.0f64, |m, p| m.max(p.im.abs()));
            assert!(x <= 0.45, "digit {d} is {x} wide");
            assert!((0.80..=0.95).contains(&y), "digit {d} is {y} tall");
        }
    }

    /// ★ Forty waves is a digit; six is a wobble. The truncation has to
    /// actually converge, or the `-`/`=` keys in the game would be showing
    /// noise rather than a lesson.
    #[test]
    fn more_waves_means_a_closer_digit() {
        for d in 0..10 {
            let o = crate::fourier::resample(&outline(d), 256);
            let s = series(d);
            let err = |m: usize| {
                o.iter().enumerate().map(|(k, w)| (s.at(m, TAU * k as f64 / 256.0) - *w).abs()).fold(0.0f64, f64::max)
            };
            assert!(err(40) < err(6), "digit {d}: 40 waves is no better than 6");
            assert!(err(40) < 0.09, "digit {d}: 40 waves still off by {}", err(40));
        }
    }

    #[test]
    fn a_recipe_ends_on_the_finished_digit() {
        let r = recipe(7);
        assert!(r.len() >= 3);
        assert!(!r.maths.is_empty());
    }
}
