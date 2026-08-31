//! # count — tally marks
//!
//! Four uprights and a stroke through them, the way a number is written when
//! it is being counted rather than computed. Centred on the origin, so
//! `tally(7).at(z)` puts seven marks at `z`.

use crate::recipe::Recipe;
use plotkit::{Cx, Shape};

const GAP: f64 = 0.34;
const TALL: f64 = 0.9;
/// How far the diagonal overhangs the four uprights it crosses.
const OVER: f64 = 0.16;

/// How far the marks reach either side of the first upright.
///
/// The left edge is not zero as soon as there is one completed group, because
/// that group's diagonal overhangs the upright it starts from. Forgetting that
/// is what makes six marks sit off-centre while five look fine.
fn edges(n: u32) -> (f64, f64) {
    if n == 0 {
        return (0.0, 0.0);
    }
    let (grp, i) = (((n - 1) / 5) as f64, ((n - 1) % 5) as f64);
    let right = grp * (GAP * 6.0) + if i < 4.0 { i * GAP } else { 3.0 * GAP + OVER };
    let left = if n >= 5 { -OVER } else { 0.0 };
    (left, right)
}

/// How wide `n` marks come out, so they can be centred under something.
pub fn width(n: u32) -> f64 {
    let (l, r) = edges(n);
    r - l
}

/// Where the first upright goes so that the whole group straddles the origin.
fn origin(n: u32) -> f64 {
    -width(n) / 2.0 - edges(n).0
}

/// `n` tally marks, centred on the origin.
pub fn tally(n: u32) -> Shape {
    Shape::group((0..n).map(|k| mark(k, origin(n))).collect::<Vec<_>>())
}

/// The `k`-th mark. Every fifth one is the diagonal that closes a group.
fn mark(k: u32, x0: f64) -> Shape {
    let (grp, i) = ((k / 5) as f64, (k % 5) as f64);
    let x = x0 + grp * (GAP * 6.0);
    if i < 4.0 {
        let x = x + i * GAP;
        Shape::path(vec![Cx::new(x, -TALL / 2.0), Cx::new(x, TALL / 2.0)])
    } else {
        Shape::path(vec![Cx::new(x - OVER, -TALL / 2.0 - 0.06), Cx::new(x + 3.0 * GAP + OVER, TALL / 2.0 + 0.06)])
    }
}

pub fn recipe(n: u32) -> Recipe {
    let x0 = origin(n);
    let mut r = Recipe::new(format!("tally{n}"), format!("{n} marks, in groups of five: four uprights and a stroke across them"));
    for k in 0..n {
        let says = if k % 5 == 4 {
            format!("mark {} — the fifth, so it strikes through the previous four", k + 1)
        } else {
            format!("mark {}", k + 1)
        };
        r = r.step(says, mark(k, x0));
    }
    r
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn strokes(n: u32) -> usize {
        tally(n).polylines(Cx::new(-99.0, -99.0), Cx::new(99.0, 99.0), 400).len()
    }

    /// `n` marks means `n` strokes. The fifth is a diagonal rather than an
    /// upright, but it is still one stroke and still counts one.
    #[test]
    fn n_marks_means_n_strokes() {
        for n in 0..13 {
            assert_eq!(strokes(n), n as usize, "{n} marks");
        }
    }

    /// ★ The fifth mark crosses the four before it. If it did not, a group of
    /// five would read as five uprights and the whole point of tallying — that
    /// you can count the groups at a glance — would be lost.
    #[test]
    fn the_fifth_mark_crosses_the_other_four() {
        let runs = tally(5).polylines(Cx::new(-99.0, -99.0), Cx::new(99.0, 99.0), 400);
        let fifth = runs.last().expect("five marks");
        let (a, b) = (fifth[0], fifth[fifth.len() - 1]);
        assert!((b.re - a.re).abs() > GAP * 3.0, "the fifth mark is not wide enough to cross the others");
        assert!((b.im - a.im).abs() > 0.5, "the fifth mark is not slanted");
        // and it reaches past both ends of the group
        let uprights: Vec<f64> = runs[..4].iter().map(|r| r[0].re).collect();
        let (lo, hi) = (uprights[0], uprights[3]);
        assert!(a.re < lo && b.re > hi, "the stroke stops short of the marks it should cross");
    }

    #[test]
    fn marks_are_centred_on_the_origin() {
        for n in 1..12 {
            let p: Vec<Cx> = tally(n).polylines(Cx::new(-99.0, -99.0), Cx::new(99.0, 99.0), 400).into_iter().flatten().collect();
            let lo = p.iter().fold(f64::MAX, |m, q| m.min(q.re));
            let hi = p.iter().fold(f64::MIN, |m, q| m.max(q.re));
            assert!((lo + hi).abs() < 1e-9, "{n} marks sit off-centre: {lo} to {hi}");
        }
    }

    /// The reported width is the width actually drawn — otherwise centring a
    /// tally under a digit would be off by however much the two disagreed.
    #[test]
    fn the_stated_width_is_the_drawn_width() {
        for n in 1..12 {
            let p: Vec<Cx> = tally(n).polylines(Cx::new(-99.0, -99.0), Cx::new(99.0, 99.0), 400).into_iter().flatten().collect();
            let lo = p.iter().fold(f64::MAX, |m, q| m.min(q.re));
            let hi = p.iter().fold(f64::MIN, |m, q| m.max(q.re));
            assert!(((hi - lo) - width(n)).abs() < 1e-9, "{n}: drawn {} but width() says {}", hi - lo, width(n));
        }
    }

    #[test]
    fn six_marks_start_a_second_group() {
        // The sixth is an upright well to the right of the first group.
        let runs = tally(6).polylines(Cx::new(-99.0, -99.0), Cx::new(99.0, 99.0), 400);
        assert_eq!(runs.len(), 6);
        assert!(runs[5][0].re > runs[3][0].re + GAP);
    }
}
