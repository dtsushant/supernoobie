//! # face — a smiley and a ghost
//!
//! Both are drawn about the origin with reach `r`, so `smiley(1.0).at(z)`
//! puts a unit face at `z`.
//!
//! The ghost is the more interesting one: its hem is `|sin 3πu|` along the
//! bottom edge. The absolute value matters — plain `sin` changes sign each
//! lobe, so every other scallop would bulge *upward* into the ghost. That is a
//! sine wave doing decorative work, which is the only excuse for a ghost being
//! in a crate about mathematics.

use crate::fourier::arc;
use crate::recipe::Recipe;
use plotkit::{Cx, Shape};
use std::f64::consts::PI;

/// A face that means well.
pub fn smiley(r: f64) -> Shape {
    smiley_recipe(r).shape()
}

pub fn smiley_recipe(r: f64) -> Recipe {
    Recipe::new("smiley", "the face is |z| = r; the mouth is 0.58r·e^{iθ} for θ from 3.6 to 5.8 rad")
        .step("the face: every point at distance r from the middle", Shape::circle(Cx::ZERO, r))
        .step("two eyes, at ±0.36r + 0.28r·i", eyes(r, 0.36, 0.28, 0.09))
        .step(
            "the mouth: an arc of radius 0.58r from 3.6 to 5.8 radians — the lower part of the circle, so it curves up at the ends",
            Shape::param(move |a| Cx::polar(r * 0.58, a), 3.6, 5.8, 40),
        )
}

/// A ghost. Half an ellipse on top, three scallops underneath.
pub fn ghost(r: f64) -> Shape {
    ghost_recipe(r).shape()
}

pub fn ghost_recipe(r: f64) -> Recipe {
    let foot = -1.05 * r;
    let hem = 0.24 * r;

    let mut body = arc(Cx::ZERO, r, r * 1.05, 0.0, PI, 40); // dome, right round to left
    body.push(Cx::new(-r, foot));
    for k in 0..=48 {
        let u = k as f64 / 48.0;
        body.push(Cx::new(-r + 2.0 * r * u, foot - hem * (PI * 3.0 * u).sin().abs()));
    }
    body.push(Cx::new(r, 0.0));

    Recipe::new("ghost", "the hem is |sin 3πu| — the absolute value is what keeps every scallop pointing down")
        .step("the dome and the hem, as one closed outline", Shape::path(body))
        .step("two eyes, at ±0.40r + 0.30r·i", eyes(r, 0.40, 0.30, 0.15))
        .step("a small round mouth at -0.34r·i", Shape::circle(Cx::new(0.0, -r * 0.34), r * 0.20))
}

/// A symmetric pair — the mirror image is `conj`, so one position does for
/// both.
fn eyes(r: f64, dx: f64, dy: f64, size: f64) -> Shape {
    let at = Cx::new(r * dx, r * dy);
    Shape::group(vec![Shape::circle(-at.conj(), r * size), Shape::circle(at, r * size)])
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn points(s: &Shape) -> Vec<Cx> {
        s.polylines(Cx::new(-99.0, -99.0), Cx::new(99.0, 99.0), 500).into_iter().flatten().collect()
    }

    /// ★ The bug this test exists for: with plain `sin` instead of `|sin|`,
    /// the middle scallop bulges *up* into the body and the ghost looks like a
    /// games controller. Every point of the hem must be at or below the foot.
    #[test]
    fn every_scallop_hangs_downward() {
        let r = 1.0;
        let foot = -1.05 * r;
        let below: Vec<Cx> = points(&ghost(r)).into_iter().filter(|p| p.im < foot + 1e-9).collect();
        assert!(below.len() > 20, "the hem should exist at all");
        assert!(below.iter().all(|p| p.im <= foot + 1e-9), "part of the hem rose above the foot");
        // And it really does dip — a flat hem would pass the test above.
        let lowest = below.iter().fold(0.0f64, |m, p| m.min(p.im));
        assert!(lowest < foot - 0.2, "the hem barely dips: {lowest} against a foot of {foot}");
    }

    /// Both faces look you in the eye: symmetric left to right.
    #[test]
    fn faces_are_symmetric() {
        for s in [smiley(1.0), ghost(1.0)] {
            let p = points(&s);
            let lo = p.iter().fold(f64::MAX, |m, q| m.min(q.re));
            let hi = p.iter().fold(f64::MIN, |m, q| m.max(q.re));
            assert!((lo + hi).abs() < 1e-6, "lopsided: {lo} to {hi}");
        }
    }

    /// Scaling is uniform, so a face at size 2 is the same face.
    #[test]
    fn a_bigger_face_is_the_same_face() {
        let reach = |s: &Shape| points(s).iter().fold(0.0f64, |m, p| m.max(p.abs()));
        let (a, b) = (reach(&smiley(1.0)), reach(&smiley(2.5)));
        assert!((b / a - 2.5).abs() < 1e-9, "ratio was {}", b / a);
    }

    /// The eyes are above the middle and the mouth below, which is the only
    /// arrangement that reads as a face.
    #[test]
    fn eyes_sit_above_the_mouth() {
        let r = smiley_recipe(1.0);
        let mid = |s: &Shape| {
            let p = points(s);
            p.iter().map(|q| q.im).sum::<f64>() / p.len() as f64
        };
        assert!(mid(&r.steps[1].shape) > mid(&r.steps[2].shape));
    }
}
