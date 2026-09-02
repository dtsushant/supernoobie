//! # fall — dropping things, with the air in the way or not
//!
//! ## The formula
//!
//! ```text
//!     s  =  s₀  +  v₀ t  +  ½ g t²
//! ```
//!
//! Where it is, plus how fast it was going times how long, plus the bit
//! gravity adds. The `t²` is the whole discovery: distance grows as the
//! **square** of the time, so a body falls four times as far in twice as long,
//! not twice as far.
//!
//! ## Who worked it out
//!
//! **Galileo Galilei** (1564–1642). Everyone knows the story of dropping two
//! balls off the Leaning Tower of Pisa, and it is probably not true — he
//! almost certainly never did it. What he actually did was better: he could
//! not time a fall accurately with the clocks of 1600, so he **slowed gravity
//! down**, rolling balls along a gently sloping groove and timing them by
//! water flowing into a cup. Diluting the effect until his instruments could
//! see it is as good a piece of experimental thinking as the result.
//!
//! What he found overturned Aristotle, who had taught for nineteen centuries
//! that heavier things fall faster. They do not. **Mass cancels out** — it
//! appears on both sides of `F = ma` and `F = mg` — so everything falls the
//! same, and the only reason a feather does not is the air. Apollo 15 settled
//! it on the Moon in 1971 with a hammer and a falcon feather.
//!
//! ## With the air in the way
//!
//! Air pushes back harder the faster you go. Take the drag proportional to
//! speed and the equation is solvable:
//!
//! ```text
//!     v(t)  =  v∞ ( 1 − e^{−t/τ} )        v∞ = g τ
//! ```
//!
//! It does not keep speeding up. It **approaches a terminal velocity**, and
//! the approach is exponential — the same `e^{−t/τ}` as a branch settling
//! after a gust in [`crate::oscillator`]. Different situation, same shape,
//! because both are "the rate of change is proportional to how far you still
//! have to go". That equation turns up everywhere once you can see it.
//!
//! ## How it is used here
//!
//! A ball you can drop with the gravity turned up or down. Set `g` to the
//! Moon's `1.62` and watch it hang; set it to Jupiter's `24.8` and watch it
//! go. With drag on you can see it stop speeding up, which is the part people
//! do not expect.

use plotkit::{Cx, Shape};

/// Gravity on a few worlds, in metres a second squared — for turning the dial
/// to somewhere real rather than to a number.
pub mod gravity {
    pub const MOON: f64 = 1.62;
    pub const MARS: f64 = 3.72;
    pub const EARTH: f64 = 9.81;
    pub const JUPITER: f64 = 24.79;
}

/// A body falling, with or without air in the way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fall {
    /// Downward acceleration. `9.81` is Earth.
    pub g: f64,
    /// How long the air takes to matter. `None` is a vacuum, where the fall
    /// never stops speeding up.
    pub tau: Option<f64>,
}

impl Fall {
    /// A vacuum: no air, so nothing ever reaches a terminal velocity.
    pub const fn vacuum(g: f64) -> Fall {
        Fall { g, tau: None }
    }

    /// With air. `tau` is the time constant — after one `tau` it is already
    /// 63% of the way to terminal velocity.
    pub const fn in_air(g: f64, tau: f64) -> Fall {
        Fall { g, tau: Some(tau) }
    }

    /// The fastest it will ever go. Infinite in a vacuum, because there is
    /// nothing to stop it.
    pub fn terminal_velocity(self) -> f64 {
        match self.tau {
            None => f64::INFINITY,
            Some(tau) => self.g * tau,
        }
    }

    /// How fast it is going, `t` after it was let go at `v0` downward.
    pub fn speed(self, v0: f64, t: f64) -> f64 {
        if t <= 0.0 {
            return v0;
        }
        match self.tau {
            // Straight line: it just keeps getting faster.
            None => v0 + self.g * t,
            // Exponential approach to terminal, starting from whatever it had.
            Some(tau) => {
                let vt = self.g * tau;
                vt + (v0 - vt) * (-t / tau).exp()
            }
        }
    }

    /// How far it has fallen after `t`.
    pub fn distance(self, v0: f64, t: f64) -> f64 {
        if t <= 0.0 {
            return 0.0;
        }
        match self.tau {
            // The famous one: half g t squared.
            None => v0 * t + 0.5 * self.g * t * t,
            // The integral of the exponential above.
            Some(tau) => {
                let vt = self.g * tau;
                vt * t + (v0 - vt) * tau * (1.0 - (-t / tau).exp())
            }
        }
    }

    /// Where it is at time `t`, dropped from `from` with `v0` downward.
    pub fn at(self, from: Cx, v0: f64, t: f64) -> Cx {
        from - Cx::new(0.0, self.distance(v0, t))
    }

    /// When it reaches the ground at height `floor`, or `None` if it never
    /// does — which cannot happen with gravity, but can with `g = 0`.
    pub fn hits(self, from: Cx, v0: f64, floor: f64) -> Option<f64> {
        let drop = from.im - floor;
        if drop <= 0.0 {
            return Some(0.0);
        }
        if self.g <= 0.0 && v0 <= 0.0 {
            return None; // weightless and not thrown: it stays there
        }
        // The vacuum case has a closed form, but with drag it does not, and
        // one bisection covers both rather than two code paths that can
        // disagree.
        let (mut lo, mut hi) = (0.0, 1.0);
        while self.distance(v0, hi) < drop && hi < 1e6 {
            hi *= 2.0;
        }
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if self.distance(v0, mid) < drop {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Some(0.5 * (lo + hi))
    }

    /// The path it takes, as a curve — a parabola in a vacuum, and something
    /// straighter with the air in the way.
    pub fn path(self, from: Cx, v0: f64, until: f64) -> Shape {
        Shape::param(move |t| self.at(from, v0, t), 0.0, until.max(1e-6), 120)
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use gravity::EARTH;

    /// ★ Galileo's result: distance goes as the **square** of the time. Twice
    /// as long is four times as far, not twice.
    #[test]
    fn distance_goes_as_the_square_of_the_time() {
        let f = Fall::vacuum(EARTH);
        let one = f.distance(0.0, 1.0);
        assert!((f.distance(0.0, 2.0) / one - 4.0).abs() < 1e-9, "twice the time should be four times the fall");
        assert!((f.distance(0.0, 3.0) / one - 9.0).abs() < 1e-9);
        assert!((one - 0.5 * EARTH).abs() < 1e-9, "half g t squared");
    }

    /// ★ Mass cancels, so everything falls the same. There is no mass in this
    /// module at all, and that absence is the physics — Aristotle taught the
    /// opposite for nineteen centuries.
    #[test]
    fn everything_falls_at_the_same_rate() {
        // Nothing to vary: `Fall` has no mass to give it. The test is that the
        // interface makes the wrong answer unsayable.
        let f = Fall::vacuum(EARTH);
        assert!((f.speed(0.0, 2.0) - 2.0 * EARTH).abs() < 1e-9);
    }

    /// Turn gravity up and it falls sooner, in exactly the right proportion:
    /// time to fall goes as `1/√g`.
    #[test]
    fn weaker_gravity_means_a_slower_fall() {
        let drop = |g: f64| Fall::vacuum(g).hits(Cx::new(0.0, 10.0), 0.0, 0.0).expect("it falls");
        let (moon, earth) = (drop(gravity::MOON), drop(EARTH));
        assert!(moon > earth, "the Moon should be the slow one");
        // t = sqrt(2h/g), so the ratio of times is the square root of the
        // inverse ratio of gravities.
        assert!((moon / earth - (EARTH / gravity::MOON).sqrt()).abs() < 1e-6);
    }

    /// ★ With air it stops speeding up. That is the part people do not
    /// expect, and it is why a raindrop does not arrive like a bullet.
    #[test]
    fn with_air_it_reaches_a_terminal_velocity() {
        let f = Fall::in_air(EARTH, 0.8);
        let vt = f.terminal_velocity();
        assert!((vt - EARTH * 0.8).abs() < 1e-12);

        assert!(f.speed(0.0, 100.0) < vt + 1e-6, "it must never pass terminal");
        assert!(f.speed(0.0, 100.0) > vt - 1e-6, "but should get there");

        // After one tau it is 63% of the way — the signature of e^{-t/tau}.
        assert!((f.speed(0.0, 0.8) / vt - (1.0 - (-1.0f64).exp())).abs() < 1e-9);
    }

    /// In a vacuum there is no terminal velocity, and it says so rather than
    /// returning a large number that would quietly become a bug.
    #[test]
    fn a_vacuum_has_no_terminal_velocity() {
        assert_eq!(Fall::vacuum(EARTH).terminal_velocity(), f64::INFINITY);
        assert!(Fall::vacuum(EARTH).speed(0.0, 1e6) > 1e6);
    }

    /// Air always slows it: the same drop takes longer.
    #[test]
    fn air_makes_the_fall_take_longer() {
        let from = Cx::new(0.0, 40.0);
        let bare = Fall::vacuum(EARTH).hits(from, 0.0, 0.0).expect("falls");
        let windy = Fall::in_air(EARTH, 0.5).hits(from, 0.0, 0.0).expect("falls");
        assert!(windy > bare, "air should hold it up, not hurry it");
    }

    /// ★ Speed is the rate of change of distance — the two formulas have to
    /// agree, or one of them is wrong. Checked numerically, both with air and
    /// without.
    #[test]
    fn speed_is_the_slope_of_the_distance() {
        for f in [Fall::vacuum(EARTH), Fall::in_air(EARTH, 0.6)] {
            for t in [0.1, 0.5, 1.0, 2.5] {
                let h = 1e-6;
                let slope = (f.distance(0.0, t + h) - f.distance(0.0, t - h)) / (2.0 * h);
                assert!((slope - f.speed(0.0, t)).abs() < 1e-4, "at t = {t}: {slope} vs {}", f.speed(0.0, t));
            }
        }
    }

    /// It arrives where it was aimed: at the moment it hits, it is at the
    /// floor.
    #[test]
    fn it_lands_on_the_floor() {
        for f in [Fall::vacuum(EARTH), Fall::in_air(EARTH, 0.4), Fall::vacuum(gravity::JUPITER)] {
            let from = Cx::new(2.0, 12.0);
            let t = f.hits(from, 0.0, -3.0).expect("falls");
            assert!((f.at(from, 0.0, t).im + 3.0).abs() < 1e-4, "it did not land on the floor");
            assert!((f.at(from, 0.0, t).re - 2.0).abs() < 1e-12, "and should not have drifted sideways");
        }
    }

    /// Weightless and not thrown, it stays where it is — and says so rather
    /// than searching forever.
    #[test]
    fn without_gravity_it_never_lands() {
        assert_eq!(Fall::vacuum(0.0).hits(Cx::new(0.0, 5.0), 0.0, 0.0), None);
        // Unless you throw it.
        assert!(Fall::vacuum(0.0).hits(Cx::new(0.0, 5.0), 2.0, 0.0).is_some());
    }

    /// Something already at the floor has arrived.
    #[test]
    fn a_thing_on_the_ground_is_already_there() {
        assert_eq!(Fall::vacuum(EARTH).hits(Cx::new(0.0, 0.0), 0.0, 0.0), Some(0.0));
    }
}
