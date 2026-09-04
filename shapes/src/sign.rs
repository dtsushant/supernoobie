//! # sign — a mark that means *not this way*
//!
//! A ring with a bar across it. Wanted for the mouth of a Ludo home column
//! when the table plays *no way home until you have cut somebody*, and general
//! enough to keep: a locked door, a square you may not enter, a move that is
//! refused.
//!
//! ## Why it is two shapes and not one
//!
//! A ring and a bar are drawn separately because they are drawn *differently* —
//! a ring is closed and a bar is not, and a renderer that filled the pair as
//! one path would put the bar's outline inside the ring by the even-odd rule
//! and cut a slot out of it. Keeping them apart also lets a caller stroke the
//! ring and leave the bar heavy, which is what the real sign does.
//!
//! The bar runs from upper-left to lower-right, at 45°, because that is the way
//! round every road sign in the world has it and the other way looks wrong
//! without anybody being able to say why.

use plotkit::{Cx, Shape};
use std::f64::consts::TAU;

/// How far along the radius the bar stops, so it sits inside the ring rather
/// than poking out of it.
pub const REACH: f64 = 0.78;

/// A no-entry sign of radius `r`: the ring, then the bar.
pub fn noway(centre: Cx, r: f64) -> Vec<Shape> {
    let ring: Vec<Cx> = (0..=40).map(|k| centre + Cx::polar(r, k as f64 / 40.0 * TAU)).collect();
    // Upper-left to lower-right.
    let lean = TAU * 3.0 / 8.0;
    let bar = vec![centre + Cx::polar(r * REACH, lean), centre - Cx::polar(r * REACH, lean)];
    vec![Shape::path(ring), Shape::path(bar)]
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn pts(s: &Shape) -> Vec<Cx> {
        s.polylines(Cx::new(-9.0, -9.0), Cx::new(9.0, 9.0), 400).into_iter().flatten().collect()
    }

    /// ★ A ring and a bar, kept apart — one closed, one not.
    #[test]
    fn it_is_a_ring_and_a_bar() {
        let parts = noway(Cx::ZERO, 1.0);
        assert_eq!(parts.len(), 2);
        let ring = pts(&parts[0]);
        assert!(ring.len() > 20, "the ring is drawn round");
        assert!((ring[0] - ring[ring.len() - 1]).abs() < 1e-9, "and closes");
        assert_eq!(pts(&parts[1]).len(), 2, "the bar is one straight line");
    }

    /// ★ The bar stays inside the ring. One poking out of it reads as a plus
    /// sign with a circle round it rather than as a refusal.
    #[test]
    fn the_bar_stays_inside_the_ring() {
        let parts = noway(Cx::new(2.0, -1.0), 0.5);
        for p in pts(&parts[1]) {
            let out = (p - Cx::new(2.0, -1.0)).abs();
            assert!(out < 0.5, "the bar reaches {out} of a radius of 0.5");
        }
    }

    /// ★ Upper-left to lower-right, the way every road sign has it.
    #[test]
    fn the_bar_leans_the_way_signs_do() {
        let parts = noway(Cx::ZERO, 1.0);
        let bar = pts(&parts[1]);
        let (a, b) = (bar[0], bar[1]);
        let (upper, lower) = if a.im > b.im { (a, b) } else { (b, a) };
        assert!(upper.re < 0.0 && upper.im > 0.0, "starts upper-left: {upper:?}");
        assert!(lower.re > 0.0 && lower.im < 0.0, "ends lower-right: {lower:?}");
    }

    /// It sits where it is put and fits the size it was asked for.
    #[test]
    fn it_fits_where_it_is_put() {
        let at = Cx::new(-3.0, 4.0);
        for part in noway(at, 0.4) {
            for p in pts(&part) {
                assert!((p - at).abs() < 0.4 + 1e-9);
            }
        }
    }
}
