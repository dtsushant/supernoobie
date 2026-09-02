//! # bough — a tree made of sums of waves
//!
//! Every branch here is bent by [`wave::sum`]. Not as decoration: **that is
//! how a branch actually bends.**
//!
//! ## A branch is a sum of waves
//!
//! Hold one end of a stick and shake it. It does not bend into an arbitrary
//! shape — it bends into a sum of a few particular ones, its *modes*. For
//! something fixed at one end and free at the other, those are
//!
//! ```text
//!     sin( (2n-1) · π s / 2L )        n = 1, 2, 3, ...
//! ```
//!
//! — a quarter wave, three quarters, five quarters. Each is zero at the base,
//! because that end is held, and steepest at the tip, because that end is not.
//! The branch's shape at any instant is those added together:
//!
//! ```text
//!     bend(s, t)  =  Σ  A_n · sin(k_n s) · cos(ω_n t + φ_n)
//!                    n
//! ```
//!
//! The space part and the time part separate, which is what makes this a *sum
//! of waves* rather than a mess. Stiffer modes are faster, so `ω_n` grows with
//! `k_n`, and the amplitudes fall off as `1/n²` because most of the energy is
//! in the slow ones — which is why a branch sways rather than buzzing.
//!
//! ## Turning that into a picture
//!
//! In the branch's own frame `s` runs along it and the bend goes across. One
//! complex multiply puts that frame wherever the branch is:
//!
//! ```text
//!     z(s)  =  base  +  e^{i·angle} · ( s + i · bend(s) )
//!                       ^^^^^^^^^^^     ^^^^^^^^^^^^^^^^
//!                       point it        along, and across
//! ```
//!
//! Which is the whole reason this library keeps everything as one `Cx`.
//!
//! ## And then it does it again
//!
//! A bough ends in two more boughs, shorter and turned aside, each with its
//! own phases so they do not all sway in step. That is the only "tree" part of
//! this; everything else is the sum of waves.

use crate::wave::{self, Wave};
use plotkit::{Cx, Shape};
use std::f64::consts::PI;

/// How many modes each branch bends in. Three is plenty — the fourth is a
/// sixteenth the size of the first and does nothing you can see.
const MODES: usize = 3;

/// The waves a branch of this length is bending in, at this instant.
///
/// The spatial shapes are fixed; what changes with `t` is **how much of each**
/// — so the amplitude carries the time and the frequency carries the shape.
/// That separation is what makes a swaying branch a sum of waves rather than a
/// new curve every frame.
pub fn modes(length: f64, sway: f64, seed: f64, t: f64) -> Vec<Wave> {
    let l = length.max(1e-6);
    (1..=MODES)
        .map(|n| {
            // Fixed at one end, free at the other: a quarter wave, then three
            // quarters, then five.
            let k = (2 * n - 1) as f64 * PI / (2.0 * l);
            // Stiffer modes are faster. Proportional is the honest first guess.
            let omega = 1.9 * k;
            let amount = sway / (n * n) as f64;
            Wave::sine().amplitude(amount * (omega * t + seed * n as f64).cos()).frequency(k)
        })
        .collect()
}

/// One branch, as a curve.
///
/// `s` runs from the base to the tip; the bend across it is the sum of the
/// modes; and `e^{i·angle}` puts that frame where the branch belongs.
pub fn bough(base: Cx, angle: f64, length: f64, sway: f64, seed: f64, t: f64) -> Shape {
    let ws = modes(length, sway, seed, t);
    let dir = Cx::expi(angle);
    Shape::param(move |s| base + dir * Cx::new(s, wave::total(&ws, s)), 0.0, length, 48)
}

/// Where a branch ends up, and which way it is pointing there.
///
/// The direction matters: a child growing from the tip should follow the bend
/// rather than ignoring it, or the tree comes apart at every joint when it
/// sways.
pub fn tip(base: Cx, angle: f64, length: f64, sway: f64, seed: f64, t: f64) -> (Cx, f64) {
    let ws = modes(length, sway, seed, t);
    let dir = Cx::expi(angle);
    let at = |s: f64| base + dir * Cx::new(s, wave::total(&ws, s));

    let end = at(length);
    let just_before = at(length - (length * 0.02).max(1e-6));
    (end, (end - just_before).arg())
}

/// A whole tree, **one shape per level** — trunk first, twigs last.
///
/// Separate levels so an application can draw the trunk thick and the twigs
/// thin, which is most of what makes a drawing of a tree look like a tree.
///
/// `depth` counts levels, so `1` is a bare trunk and `n` gives `2^n − 1`
/// branches. `spread` is how far each child turns aside, in radians.
pub fn tree(base: Cx, angle: f64, length: f64, depth: usize, spread: f64, sway: f64, t: f64) -> Vec<Shape> {
    let mut levels: Vec<Vec<Shape>> = vec![Vec::new(); depth];
    grow(&mut levels, 0, base, angle, length, spread, sway, t, 0.0);
    levels.into_iter().map(Shape::group).collect()
}

#[allow(clippy::too_many_arguments)]
fn grow(
    levels: &mut Vec<Vec<Shape>>,
    level: usize,
    base: Cx,
    angle: f64,
    length: f64,
    spread: f64,
    sway: f64,
    t: f64,
    seed: f64,
) {
    if level >= levels.len() || length < 1e-4 {
        return;
    }
    levels[level].push(bough(base, angle, length, sway, seed, t));

    // Where this branch actually ends, bend included — so the children stay
    // attached however hard it is swaying.
    let (end, heading) = tip(base, angle, length, sway, seed, t);

    // Thinner branches are floppier, and each child gets its own phase so the
    // tree does not sway as one rigid fan.
    let (next_len, next_sway) = (length * 0.72, sway * 1.35);
    for (k, side) in [-1.0, 1.0].into_iter().enumerate() {
        grow(
            levels,
            level + 1,
            end,
            heading + side * spread,
            next_len,
            spread,
            next_sway,
            t,
            seed + 1.7 + k as f64 * 2.3,
        );
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn runs(s: &Shape) -> Vec<Vec<Cx>> {
        s.polylines(Cx::new(-99.0, -99.0), Cx::new(99.0, 99.0), 400)
    }

    /// A tree of `n` levels has `2^n − 1` branches, and they are handed back
    /// level by level so the trunk can be drawn thick and the twigs thin.
    #[test]
    fn a_tree_branches_in_two_every_level() {
        for depth in 1..7 {
            let levels = tree(Cx::ZERO, PI / 2.0, 3.0, depth, 0.5, 0.1, 0.0);
            assert_eq!(levels.len(), depth);
            let total: usize = levels.iter().map(|l| runs(l).len()).sum();
            assert_eq!(total, (1 << depth) - 1, "{depth} levels");
            assert_eq!(runs(&levels[0]).len(), 1, "one trunk");
        }
    }

    /// ★ Every branch starts exactly where its parent ended — including the
    /// bend. Use the straight-line tip instead and the tree comes apart at
    /// every joint the moment it sways, which looks like a rendering fault
    /// rather than the arithmetic mistake it is.
    #[test]
    fn every_branch_stays_attached_however_hard_it_sways() {
        for t in [0.0, 0.4, 1.9, 5.5] {
            let levels = tree(Cx::ZERO, PI / 2.0, 3.0, 5, 0.55, 0.35, t);

            for k in 1..levels.len() {
                let parents: Vec<Cx> = runs(&levels[k - 1]).iter().map(|r| r[r.len() - 1]).collect();
                for child in runs(&levels[k]) {
                    let start = child[0];
                    let nearest = parents.iter().fold(f64::MAX, |m, p| m.min((*p - start).abs()));
                    assert!(nearest < 1e-9, "at t = {t}, level {k} has a branch floating {nearest} from any tip");
                }
            }
        }
    }

    /// ★ The base is held. A branch fixed at one end has zero deflection
    /// there — that is what "fixed" means, and it is why the modes are sines
    /// rather than cosines.
    #[test]
    fn the_base_is_held_still() {
        for t in [0.0, 0.7, 3.3] {
            for sway in [0.0, 0.2, 0.9] {
                let ws = modes(3.0, sway, 1.1, t);
                assert!(wave::total(&ws, 0.0).abs() < 1e-12, "the root moved at t = {t}, sway {sway}");
            }
            let trunk = &runs(&tree(Cx::new(2.0, -1.0), PI / 2.0, 3.0, 3, 0.5, 0.4, t)[0])[0];
            assert!((trunk[0] - Cx::new(2.0, -1.0)).abs() < 1e-9, "the trunk left its planting spot");
        }
    }

    /// ★ And the tip is free, so it is the part that moves. A branch whose tip
    /// did not move would not be swaying at all.
    #[test]
    fn the_tip_is_free_and_actually_moves() {
        let where_ = |t: f64| tip(Cx::ZERO, PI / 2.0, 3.0, 0.4, 0.0, t).0;
        let travel = (0..40).map(|k| (where_(k as f64 * 0.1) - where_(0.0)).abs()).fold(0.0f64, f64::max);
        assert!(travel > 0.1, "the tip barely moved: {travel}");
    }

    /// With no sway it is straight — so the curve and the direction agree, and
    /// any bend you see later is the waves rather than an error.
    #[test]
    fn no_sway_means_a_straight_branch() {
        let (end, heading) = tip(Cx::ZERO, 0.3, 4.0, 0.0, 2.2, 1.4);
        assert!((end - Cx::expi(0.3).scale(4.0)).abs() < 1e-9, "it should end straight ahead");
        assert!((heading - 0.3).abs() < 1e-9, "and still be pointing that way");

        let p: Vec<Cx> = runs(&bough(Cx::ZERO, 0.3, 4.0, 0.0, 2.2, 1.4)).into_iter().flatten().collect();
        let line = Cx::expi(0.3);
        for q in p {
            assert!(q.cross(line).abs() < 1e-9, "a branch with no sway wandered off the line");
        }
    }

    /// The modes are the ones for a stick held at one end: a quarter wave,
    /// three quarters, five quarters. Get the family wrong — full waves, say —
    /// and the tip becomes a node and stops moving, which is the opposite of
    /// what a branch does.
    #[test]
    fn the_modes_are_quarter_waves() {
        let l = 2.0;
        let ws = modes(l, 1.0, 0.0, 0.0);
        assert_eq!(ws.len(), MODES);
        for (n, w) in ws.iter().enumerate() {
            let quarters = (2 * (n + 1) - 1) as f64;
            assert!((w.length() - 4.0 * l / quarters).abs() < 1e-9, "mode {n} has the wrong wavelength");
            // Each is at full stretch at the tip, which is what "free" means.
            let shape = Wave::sine().frequency(w.k);
            assert!((shape.at(l).abs() - 1.0).abs() < 1e-9, "mode {n} is not at its maximum at the tip");
        }
    }

    /// Most of the movement is in the slow modes, which is why a branch sways
    /// rather than buzzing.
    #[test]
    fn the_slow_modes_carry_the_movement() {
        let ws = modes(3.0, 1.0, 0.0, 0.0);
        assert!(ws[0].a.abs() > ws[1].a.abs() * 3.0, "the first mode should dominate");
        assert!(ws[1].a.abs() > ws[2].a.abs());
    }

    /// A pure function of `t`: same time, same tree. So a taped run grows the
    /// same tree, and there is no generator to record.
    #[test]
    fn the_same_tree_comes_back_every_time() {
        let count = |t: f64| tree(Cx::ZERO, 1.5, 3.0, 4, 0.5, 0.3, t).iter().map(|l| runs(l).len()).sum::<usize>();
        assert_eq!(count(2.5), count(2.5));

        let ends = |t: f64| tip(Cx::ZERO, 1.5, 3.0, 0.3, 0.9, t).0;
        assert_eq!(ends(2.5), ends(2.5));
    }

    #[test]
    fn a_tree_of_no_depth_is_nothing_and_does_not_panic() {
        assert!(tree(Cx::ZERO, 1.5, 3.0, 0, 0.5, 0.3, 1.0).is_empty());
    }
}
