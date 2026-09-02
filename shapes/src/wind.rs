//! # wind — what it does to a tree, and what it looks like
//!
//! ## Force goes as the square of the speed
//!
//! Not as the speed. Air arrives at a rate proportional to `v` and each parcel
//! carries momentum proportional to `v`, so the push goes as **`v²`**. That is
//! why a wind twice as strong is four times as bad, and it is the single most
//! important thing about wind.
//!
//! ## How far a tree leans
//!
//! Two moments about the base, and the tree sits where they balance.
//!
//! * **The wind** pushes it over. The force goes as `v²` and as the area the
//!   tree presents, which shrinks by `cos θ` as it lies over. The lever arm
//!   shortens by `cos θ` too. So the wind's moment goes as `v² cos²θ`.
//! * **The tree** pushes back, by `c·θ` — the further it is bent the harder it
//!   resists, which is what stiffness means.
//!
//! ```text
//!     c · θ  =  k v² · cos²θ
//! ```
//!
//! There is no closed form for `θ`, and there does not need to be: the left
//! side rises with `θ` and the right side falls, so they cross exactly once,
//! and bisection walks straight to it.
//!
//! **It saturates on its own.** At `θ = π/2` the wind's moment is zero — a
//! tree lying flat presents nothing to push on — while the tree is still
//! pushing back. So no wind, however strong, quite lays it flat. That is worth
//! having for its own sake: a lean that had to be clamped at flat would be a
//! number pinned by an `if`, and this one is pinned by the geometry.

use plotkit::{Cx, Shape};
use std::f64::consts::{FRAC_PI_2, TAU};

/// Wind, blowing along the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wind {
    /// Signed, in units a second. Positive blows to the right.
    pub speed: f64,
}

impl Default for Wind {
    fn default() -> Wind {
        Wind::still()
    }
}

impl Wind {
    pub const fn new(speed: f64) -> Wind {
        Wind { speed }
    }

    pub const fn still() -> Wind {
        Wind::new(0.0)
    }

    /// How hard it pushes: `v²`, signed by which way it is going.
    ///
    /// The square is the point. Doubling the wind quadruples the push.
    pub fn pressure(self) -> f64 {
        self.speed * self.speed * self.speed.signum()
    }

    /// How far a tree of this stiffness leans, in radians from upright.
    ///
    /// Solved, not chosen: the angle where the wind's moment and the tree's
    /// own stiffness balance. Signed the way the wind is going.
    pub fn lean(self, stiffness: f64) -> f64 {
        let push = self.pressure().abs();
        let c = stiffness.max(1e-9);

        // c·θ − k v² cos²θ. Rises with θ on the left, falls on the right, so
        // it crosses zero exactly once between upright and flat.
        let balance = |th: f64| c * th - push * th.cos() * th.cos();
        if balance(0.0) >= 0.0 {
            return 0.0; // no wind at all
        }

        let (mut lo, mut hi) = (0.0, FRAC_PI_2);
        for _ in 0..48 {
            let mid = 0.5 * (lo + hi);
            if balance(mid) < 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        0.5 * (lo + hi) * self.speed.signum()
    }

    /// Which way a trunk points under this wind: `π/2` upright, `0` flat on
    /// the ground and pointing downwind.
    ///
    /// The number the tree is actually drawn with.
    pub fn trunk_angle(self, stiffness: f64) -> f64 {
        FRAC_PI_2 - self.lean(stiffness)
    }

    /// How much a branch should be shaking, on top of the steady lean.
    ///
    /// Also `v²`, because it is the same push — a gusting wind does not just
    /// lean a tree, it works it back and forth.
    pub fn shake(self, at_rest: f64) -> f64 {
        at_rest * (1.0 + 0.02 * self.pressure().abs())
    }

    /// Gusts: little pieces of wave, drifting downwind and fading.
    ///
    /// Returned with a brightness from 0 to 1 so they can be faded rather than
    /// switched on and off — a gust that blinked into existence would read as
    /// a fault.
    ///
    /// **A gust has ends, and a [`crate::Wave`] does not.** That is the whole
    /// difference: a wave is a `graph` across the whole window, a gust is a
    /// short `param` you can watch cross the sky.
    pub fn gusts(self, n: usize, lo: Cx, hi: Cx, t: f64) -> Vec<(Shape, f64)> {
        // A gust that never moves is not a gust, and a still sky should be
        // still.
        if self.speed.abs() < 1e-6 || n == 0 {
            return Vec::new();
        }
        // Fast wind crosses the sky sooner. The `1 +` keeps a whisper of a
        // breeze from taking an hour.
        let rate = 0.12 * (1.0 + self.speed.abs());
        let (w, h) = (hi.re - lo.re, hi.im - lo.im);

        (0..n)
            .map(|k| {
                let k = k as f64;
                // Staggered by an irrational, so they never all arrive at
                // once and there is no generator to seed. Same trick as the
                // wander and the scatter.
                let age = (t * rate + k * 0.618_033_988_749_895).rem_euclid(1.0);
                let along = if self.speed > 0.0 { age } else { 1.0 - age };

                let y = lo.im + h * frac(k * 0.414_213_562_373_095 + 0.21);
                let start = Cx::new(lo.re + w * along, y);
                let length = 0.6 + 1.4 * frac(k * 0.732_050_807_568_877 + 0.57);
                let amp = 0.04 + 0.10 * frac(k * 0.302_775_637_731_995 + 0.13);

                // Fades in and straight back out. Zero at both ends, so
                // nothing ever appears or vanishes with a snap.
                let bright = (std::f64::consts::PI * age).sin();

                let dir = self.speed.signum();
                (
                    Shape::param(
                        move |s| start + Cx::new(dir * s, amp * (TAU * s / 0.55 + k).sin()),
                        0.0,
                        length,
                        40,
                    ),
                    bright,
                )
            })
            .collect()
    }
}

fn frac(x: f64) -> f64 {
    x - x.floor()
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// ★ Force goes as the square of the speed. Twice the wind is four times
    /// the push, and getting that linear would make every strong wind feel
    /// far too mild.
    #[test]
    fn the_push_goes_as_the_square_of_the_speed() {
        assert!((Wind::new(2.0).pressure() - 4.0).abs() < 1e-12);
        assert!((Wind::new(4.0).pressure() - 16.0).abs() < 1e-12);
        assert!((Wind::new(-3.0).pressure() + 9.0).abs() < 1e-12, "and it keeps its direction");
        assert_eq!(Wind::still().pressure(), 0.0);
    }

    #[test]
    fn still_air_leaves_a_tree_upright() {
        assert_eq!(Wind::still().lean(1.0), 0.0);
        assert!((Wind::still().trunk_angle(1.0) - FRAC_PI_2).abs() < 1e-12);
    }

    /// Harder wind, further over — with no steps or flat spots on the way.
    #[test]
    fn a_harder_wind_leans_it_further() {
        let mut last = 0.0;
        for k in 1..80 {
            let lean = Wind::new(k as f64 * 0.25).lean(1.0);
            assert!(lean > last, "it stopped leaning at speed {}", k as f64 * 0.25);
            last = lean;
        }
    }

    /// ★ It saturates on its own. At flat, the tree presents nothing for the
    /// wind to push on while its own stiffness still pushes back — so no wind
    /// quite lays it down, and nothing had to be clamped to arrange that.
    ///
    /// A lean pinned by an `if` would be a number somebody chose. This one is
    /// pinned by the geometry.
    #[test]
    fn no_wind_however_strong_quite_lays_it_flat() {
        for speed in [10.0, 100.0, 1_000.0, 1e6] {
            let lean = Wind::new(speed).lean(1.0);
            assert!(lean < FRAC_PI_2, "a wind of {speed} laid it past flat: {lean}");
            assert!(lean.is_finite());
        }
        // But it gets very close, which is what "down to nothing" should mean.
        assert!(Wind::new(1e6).lean(1.0) > FRAC_PI_2 - 0.01);
        assert!(Wind::new(1e6).trunk_angle(1.0) < 0.01, "the trunk should be all but flat");
    }

    /// The balance really is a balance: at the answer, the two moments are
    /// equal. This is the test that the bisection solved the right equation
    /// rather than merely converging to something.
    #[test]
    fn the_lean_is_where_the_two_moments_meet() {
        for (speed, c) in [(2.0, 1.0), (5.0, 3.0), (0.7, 0.4), (12.0, 8.0)] {
            let th = Wind::new(speed).lean(c);
            let tree = c * th;
            let wind = Wind::new(speed).pressure() * th.cos() * th.cos();
            assert!((tree - wind).abs() < 1e-6, "at {speed}/{c}: tree {tree}, wind {wind}");
        }
    }

    #[test]
    fn a_stiffer_tree_leans_less() {
        let w = Wind::new(4.0);
        assert!(w.lean(10.0) < w.lean(1.0));
        assert!(w.lean(1.0) < w.lean(0.1));
    }

    /// It leans the way the wind is going, and a wind from the other side
    /// leans it exactly as far the other way.
    #[test]
    fn it_leans_downwind_either_way() {
        let (right, left) = (Wind::new(3.0).lean(1.0), Wind::new(-3.0).lean(1.0));
        assert!(right > 0.0 && left < 0.0);
        assert!((right + left).abs() < 1e-12, "the two sides should mirror");
    }

    /// ★ Gusts drift **downwind**, and turn round when the wind does. A gust
    /// crossing against the wind would be the most obvious possible tell that
    /// the sign was wrong somewhere.
    #[test]
    fn gusts_drift_downwind() {
        let (lo, hi) = (Cx::new(-10.0, 0.0), Cx::new(10.0, 6.0));
        let where_ = |w: Wind, t: f64| {
            w.gusts(1, lo, hi, t)[0].0.polylines(Cx::new(-99.0, -99.0), Cx::new(99.0, 99.0), 200)[0][0].re
        };
        let east = Wind::new(4.0);
        assert!(where_(east, 1.0) > where_(east, 0.0), "a wind to the right should carry them right");

        let west = Wind::new(-4.0);
        assert!(where_(west, 1.0) < where_(west, 0.0), "and a wind to the left, left");
    }

    /// They fade in and out rather than blinking, and a still sky has none.
    #[test]
    fn gusts_arrive_and_leave_gently() {
        let (lo, hi) = (Cx::new(-10.0, 0.0), Cx::new(10.0, 6.0));
        assert!(Wind::still().gusts(8, lo, hi, 3.0).is_empty(), "still air has no gusts");

        let w = Wind::new(3.0);
        let mut lowest: f64 = 1.0;
        let mut highest: f64 = 0.0;
        for k in 0..400 {
            for (_, bright) in w.gusts(6, lo, hi, k as f64 * 0.05) {
                assert!((0.0..=1.0).contains(&bright), "brightness out of range: {bright}");
                lowest = lowest.min(bright);
                highest = highest.max(bright);
            }
        }
        assert!(lowest < 0.05, "they never fade away: dimmest was {lowest}");
        assert!(highest > 0.95, "they never come up to full: brightest was {highest}");
    }

    /// Stronger wind blows them across sooner.
    #[test]
    fn a_stronger_wind_hurries_them_along() {
        let (lo, hi) = (Cx::new(-10.0, 0.0), Cx::new(10.0, 6.0));
        let moved = |w: Wind| {
            let at = |t: f64| w.gusts(1, lo, hi, t)[0].0.polylines(lo, hi, 200)[0][0].re;
            at(0.4) - at(0.0)
        };
        assert!(moved(Wind::new(8.0)) > moved(Wind::new(1.0)));
    }

    /// A pure function of time, so a taped run has the same weather.
    #[test]
    fn the_same_gusts_come_back_every_time() {
        let (lo, hi) = (Cx::new(-5.0, 0.0), Cx::new(5.0, 4.0));
        let brights = |t: f64| Wind::new(2.0).gusts(6, lo, hi, t).iter().map(|(_, b)| *b).collect::<Vec<_>>();
        assert_eq!(brights(2.5), brights(2.5));
    }

    #[test]
    fn wind_works_a_tree_harder_than_still_air() {
        assert!(Wind::new(5.0).shake(0.06) > Wind::still().shake(0.06));
        assert_eq!(Wind::still().shake(0.06), 0.06);
    }
}
