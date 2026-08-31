//! # glyph — punctuation and plain shapes
//!
//! `+`, `=`, `?`, and the shapes that are their own definition: a circle, a
//! square, a regular `n`-gon. All about the origin at roughly unit size.
//!
//! The `n`-gon is worth a look. Its corners are the **`n`-th roots of unity** —
//! the `n` solutions of `z^n = 1` — which are `e^{i·2πk/n}` for `k = 0..n`.
//! There is no polygon code here, only that.

use crate::recipe::Recipe;
use plotkit::{Cx, Shape};
use std::f64::consts::TAU;

pub fn plus() -> Shape {
    plus_recipe().shape()
}

pub fn plus_recipe() -> Recipe {
    let s = 0.55;
    Recipe::new("plus", "two strokes through the origin, one real and one imaginary: ±s and ±is")
        .step("the across stroke, from -s to +s", Shape::path(vec![Cx::new(-s, 0.0), Cx::new(s, 0.0)]))
        .step("the up stroke — the same segment turned by i", Shape::path(vec![Cx::new(0.0, -s), Cx::new(0.0, s)]))
}

pub fn equals() -> Shape {
    equals_recipe().shape()
}

pub fn equals_recipe() -> Recipe {
    let (s, d) = (0.55, 0.34);
    let bar = |y: f64| Shape::path(vec![Cx::new(-s, y), Cx::new(s, y)]);
    Recipe::new("equals", "one bar, and the same bar shifted by ±di")
        .step("a bar above the line", bar(d))
        .step("the same bar, shifted down by 2di", bar(-d))
}

pub fn question() -> Shape {
    question_recipe().shape()
}

pub fn question_recipe() -> Recipe {
    Recipe::new("question", "an arc that stops short, a stem, and a dot")
        .step("the hook: an arc of radius 0.36 from 3.4 down to -0.9 radians", Shape::param(|a| Cx::new(0.0, 0.5) + Cx::polar(0.36, a), 3.4, -0.9, 40))
        .step("the stem, falling from the end of the hook", Shape::path(vec![Cx::new(0.02, 0.28), Cx::new(0.02, -0.3)]))
        .step("the dot", Shape::circle(Cx::new(0.02, -0.62), 0.07))
}

pub fn circle_recipe() -> Recipe {
    Recipe::new("circle", "|z| = 1, or equally z = e^{iθ} — the two are the same statement")
        .step("every point at distance 1 from the origin", Shape::circle(Cx::ZERO, 1.0))
        .step("the same curve as a journey: z = e^{iθ}, θ from 0 to 2π", Shape::param(Cx::expi, 0.0, TAU, 200))
}

pub fn square_recipe() -> Recipe {
    let c = Cx::new(0.7, 0.7);
    Recipe::new("square", "one corner, turned by i three times: c, ic, i²c, i³c")
        .step("a corner at 0.7 + 0.7i", Shape::point(c))
        .step("multiply by i — a quarter turn — three times over", Shape::points((1..4).map(|k| c * Cx::I.powi(k)).collect::<Vec<_>>()))
        .step("join them up", Shape::polygon((0..4).map(|k| c * Cx::I.powi(k)).collect::<Vec<_>>()))
}

/// A regular `n`-gon: the `n`-th roots of unity, joined up.
pub fn ngon_recipe(n: usize) -> Recipe {
    let pts: Vec<Cx> = (0..n).map(|k| Cx::expi(TAU * k as f64 / n as f64)).collect();
    Recipe::new(
        format!("{}-gon", n),
        format!("the {n} solutions of z^{n} = 1, which are e^{{i·2πk/{n}}} — the roots of unity ARE the corners"),
    )
    .step("the unit circle they all sit on", Shape::circle(Cx::ZERO, 1.0))
    .step(format!("the {n} roots of unity, evenly spaced round it"), Shape::points(pts.clone()))
    .step("joined up", Shape::polygon(pts))
}

// Small helper so `i^k` reads the way it does on paper.
trait Powi {
    fn powi(self, k: i32) -> Cx;
}
impl Powi for Cx {
    fn powi(self, k: i32) -> Cx {
        (0..k).fold(Cx::ONE, |a, _| a * self)
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn points(s: &Shape) -> Vec<Cx> {
        s.polylines(Cx::new(-99.0, -99.0), Cx::new(99.0, 99.0), 400).into_iter().flatten().collect()
    }

    /// ★ A square built by multiplying one corner by `i` really does close
    /// up — because `i⁴ = 1`. That is the whole construction, and if it failed
    /// the shape would be a spiral.
    #[test]
    fn multiplying_a_corner_by_i_four_times_comes_home() {
        let c = Cx::new(0.7, 0.7);
        assert!((c * Cx::I.powi(4) - c).abs() < 1e-15);
    }

    /// The corners of an n-gon are the n-th roots of unity: raise any of them
    /// to the n and you get 1 back.
    #[test]
    fn ngon_corners_are_roots_of_unity() {
        for n in [3usize, 5, 6, 8] {
            for k in 0..n {
                let z = Cx::expi(TAU * k as f64 / n as f64);
                let p = (0..n).fold(Cx::ONE, |a, _| a * z);
                assert!((p - Cx::ONE).abs() < 1e-12, "{n}-gon corner {k} is not an {n}-th root of 1");
            }
        }
    }

    /// The two ways of saying "circle" — `|z| = 1` and `z = e^{iθ}` — must
    /// produce the same curve, or the recipe would be teaching a falsehood.
    #[test]
    fn the_two_definitions_of_a_circle_agree() {
        let r = circle_recipe();
        for s in [&r.steps[0].shape, &r.steps[1].shape] {
            for p in points(s) {
                assert!((p.abs() - 1.0).abs() < 0.02, "a point at radius {}", p.abs());
            }
        }
    }

    #[test]
    fn plus_and_equals_are_centred_and_symmetric() {
        for s in [plus(), equals()] {
            let p = points(&s);
            let sx: f64 = p.iter().map(|q| q.re).sum();
            let sy: f64 = p.iter().map(|q| q.im).sum();
            assert!(sx.abs() < 1e-9 && sy.abs() < 1e-9, "off-centre: {sx}, {sy}");
        }
    }

    /// A plus has two strokes that actually cross; two parallel bars would be
    /// an equals sign.
    #[test]
    fn a_plus_crosses_and_an_equals_does_not() {
        let runs = |s: Shape| s.polylines(Cx::new(-99.0, -99.0), Cx::new(99.0, 99.0), 400);
        let p = runs(plus());
        assert_eq!(p.len(), 2);
        let horiz = |r: &Vec<Cx>| (r[r.len() - 1].im - r[0].im).abs() < 1e-9;
        assert!(horiz(&p[0]) != horiz(&p[1]), "both strokes of the plus run the same way");

        let e = runs(equals());
        assert_eq!(e.len(), 2);
        assert!(horiz(&e[0]) && horiz(&e[1]), "the equals bars should both be level");
    }
}
