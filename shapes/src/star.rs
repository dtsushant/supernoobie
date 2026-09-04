//! # star — a star, and the ring of points behind it
//!
//! Wanted for the safe squares on a Ludo board, and general enough to be worth
//! keeping: a badge, a spark, an award, a twinkle.
//!
//! ## One ring, taken alternately
//!
//! A star is not two shapes. It is **one** ring of `2n` points whose radius
//! alternates between `outer` and `inner`:
//!
//! ```text
//!     r(k) = if k is even then outer else inner
//!     a(k) = turn + k·π/n
//! ```
//!
//! Written as two loops it is two places to get the angle wrong, and the two
//! disagree by half a step — which looks like a star drawn by somebody in a
//! hurry rather than a star.
//!
//! ## The waist
//!
//! `inner/outer` is the whole character of it. Around 0.38 is the familiar
//! five-pointed star; much above 0.6 it is a cog, and much below 0.25 the
//! points are needles that vanish at small sizes. [`WAIST`] is the default and
//! is the one thing worth fiddling with.

use plotkit::{Cx, Shape};
use std::f64::consts::TAU;

/// How far in the inner points sit, as a fraction of the outer ones.
pub const WAIST: f64 = 0.4;

/// The points of a star, as a closed ring.
///
/// `n` is how many points it has — five unless there is a reason. The ring
/// comes back to where it started, so it can be filled or stroked without a
/// seam.
pub fn ring(centre: Cx, outer: f64, waist: f64, n: usize, turn: f64) -> Vec<Cx> {
    let n = n.max(2);
    let inner = outer * waist.clamp(0.05, 0.95);
    let mut out: Vec<Cx> = (0..2 * n)
        .map(|k| {
            let r = if k % 2 == 0 { outer } else { inner };
            // A point straight up, because a star resting on a flat edge looks
            // like it fell over.
            let a = turn + TAU / 4.0 + k as f64 * TAU / (2 * n) as f64;
            centre + Cx::polar(r, a)
        })
        .collect();
    out.push(out[0]);
    out
}

/// A five-pointed star, which is what anybody means by a star.
pub fn star(centre: Cx, outer: f64) -> Shape {
    Shape::polygon(ring(centre, outer, WAIST, 5, 0.0))
}

/// A star with its points, waist and lean said.
pub fn spiked(centre: Cx, outer: f64, waist: f64, n: usize, turn: f64) -> Shape {
    Shape::polygon(ring(centre, outer, waist, n, turn))
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn radii(pts: &[Cx], centre: Cx) -> Vec<f64> {
        pts[..pts.len() - 1].iter().map(|p| (*p - centre).abs()).collect()
    }

    /// ★ Points and waists alternate, which is the whole of being a star.
    #[test]
    fn the_radius_alternates() {
        let c = Cx::new(2.0, -1.0);
        let r = radii(&ring(c, 1.0, 0.4, 5, 0.0), c);
        assert_eq!(r.len(), 10, "five points and five waists");
        for (k, v) in r.iter().enumerate() {
            let want = if k % 2 == 0 { 1.0 } else { 0.4 };
            assert!((v - want).abs() < 1e-9, "point {k} is {v}, wanted {want}");
        }
    }

    /// ★ **One ring, not two.** The points and the waists are half a step
    /// apart by construction, so they cannot disagree — which is what happens
    /// when the two are written as separate loops.
    #[test]
    fn the_waists_sit_half_way_between_the_points() {
        let c = Cx::ZERO;
        let pts = ring(c, 1.0, 0.4, 5, 0.0);
        for k in 0..5 {
            let (a, waist, b) = (pts[2 * k], pts[2 * k + 1], pts[(2 * k + 2) % 10]);
            let midway = (a.arg() + b.arg()) / 2.0;
            let gap = (waist.arg() - midway).rem_euclid(TAU);
            let gap = gap.min(TAU - gap);
            // Half way round, or half way round the other side of the cut.
            assert!(
                gap < 1e-9 || (gap - std::f64::consts::PI).abs() < 1e-9,
                "waist {k} is {gap} off the middle"
            );
        }
    }

    /// ★ A star stands on a point, not on a flat edge. One that has fallen
    /// over reads as a cog.
    #[test]
    fn a_star_points_upwards() {
        let pts = ring(Cx::ZERO, 1.0, 0.4, 5, 0.0);
        let top = pts[0];
        assert!(top.im > 0.99, "the first point should be straight up: {top:?}");
        assert!(top.re.abs() < 1e-9);
    }

    /// It closes, so it can be filled or stroked without a seam.
    #[test]
    fn the_ring_closes() {
        let pts = ring(Cx::new(1.0, 1.0), 0.5, 0.4, 6, 0.3);
        assert_eq!(pts[0], pts[pts.len() - 1]);
        assert_eq!(pts.len(), 13, "six points, six waists, and back to the start");
    }

    /// Any number of points, and a lean.
    #[test]
    fn it_takes_any_number_of_points_and_a_lean() {
        for n in [3usize, 4, 5, 6, 8, 12] {
            assert_eq!(ring(Cx::ZERO, 1.0, 0.4, n, 0.0).len(), 2 * n + 1, "{n} points");
        }
        let a = ring(Cx::ZERO, 1.0, 0.4, 5, 0.0);
        let b = ring(Cx::ZERO, 1.0, 0.4, 5, TAU / 10.0);
        // A tenth of a turn puts a point where a waist was, in the same
        // direction -- at its own radius, since a point reaches further.
        let gap = (b[0].arg() - a[1].arg()).rem_euclid(TAU);
        assert!(gap.min(TAU - gap) < 1e-9, "off by {gap}");
    }

    /// ★ It fits in the circle it was asked for, so a star of size `r` can be
    /// dropped into a square of side `2r` and stay in it.
    #[test]
    fn nothing_sticks_out_past_the_size_asked_for() {
        for n in [3usize, 5, 7] {
            for p in ring(Cx::new(-3.0, 2.0), 0.7, 0.4, n, 1.1) {
                assert!((p - Cx::new(-3.0, 2.0)).abs() < 0.7 + 1e-9);
            }
        }
    }

    /// The waist is held to something that still looks like a star.
    #[test]
    fn a_silly_waist_is_brought_back() {
        let c = Cx::ZERO;
        let thin = radii(&ring(c, 1.0, 0.0, 5, 0.0), c);
        assert!(thin[1] > 0.0, "a waist of nothing would be a five-armed cross");
        let fat = radii(&ring(c, 1.0, 5.0, 5, 0.0), c);
        assert!(fat[1] < 1.0, "and a waist wider than the points would turn it inside out");
    }
}
