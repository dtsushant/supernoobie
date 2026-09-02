//! # oscillator — the thing that wobbles and settles
//!
//! ## The formula
//!
//! ```text
//!     m ẍ  +  c ẋ  +  k x  =  F(t)
//!     ^^^^     ^^^^     ^^^
//!     inertia  drag     springiness
//! ```
//!
//! A mass on a spring with something slowing it down. Written the way people
//! actually use it, by dividing through by `m`:
//!
//! ```text
//!     ẍ  +  2ζω ẋ  +  ω² x  =  f(t)
//! ```
//!
//! Two numbers instead of three, and both mean something you can picture:
//!
//! * **`ω`** — the natural frequency. How fast it wants to wobble.
//! * **`ζ`** (zeta) — the damping ratio. How quickly it gives up.
//!
//! ## What it does
//!
//! Kick it and it does one of three things, and `ζ = 1` is the border:
//!
//! ```text
//!     ζ < 1   underdamped     overshoots, wobbles, settles     a branch
//!     ζ = 1   critically      goes there and stops, fastest    a door closer
//!     ζ > 1   overdamped      creeps there, never overshoots   a thick oil
//! ```
//!
//! Almost everything that springs back is one of these. A branch after a gust.
//! A car over a bump. A needle on a dial. The reason it is worth learning once
//! is that it is the *same equation* every time.
//!
//! ## Where it comes from: Laplace
//!
//! **Pierre-Simon Laplace** (1749–1827) — the astronomer who showed the solar
//! system is stable, and who, asked by Napoleon why his book on the heavens
//! never mentioned God, is supposed to have said *"I had no need of that
//! hypothesis."* The transform that carries his name grew out of his work on
//! probability; **Oliver Heaviside**, a self-taught telegraph engineer, turned
//! it into a working tool a century later and was mocked for it until it was
//! proved sound.
//!
//! The idea is worth the whole story. Write `s` for "differentiate", and a
//! differential equation becomes **algebra**:
//!
//! ```text
//!     ẍ + 2ζω ẋ + ω² x = f       becomes       (s² + 2ζω s + ω²) X = F
//!
//!                                so             X = F / (s² + 2ζω s + ω²)
//! ```
//!
//! Calculus in, arithmetic out. And then the *roots of the bottom* — the
//! **poles** — tell you everything the thing will ever do:
//!
//! ```text
//!     s = −ζω ± ω √(ζ² − 1)
//! ```
//!
//! The **real part is the decay** and the **imaginary part is the wobble**.
//! Poles in the left half of the plane settle; on the axis they ring forever;
//! in the right half they run away. That one picture is most of control
//! engineering.
//!
//! ## Fourier and Laplace are the same tool
//!
//! [`shapes::fourier`](../../shapes/fourier/index.html) breaks a thing into
//! `e^{iωt}` — pure oscillation, no growth. Laplace uses `e^{st}` with
//! `s = σ + iω` — oscillation **times** growth or decay. Fourier is Laplace
//! on the imaginary axis.
//!
//! So: *Fourier answers "what is this made of". Laplace answers "how does this
//! respond to being pushed".*
//!
//! ## How it is used here
//!
//! A tree in wind. The wind pushes; the tree resists and has weight; so it is
//! this equation exactly. Take the wind away and the branches do not stop
//! dead — they swing a few times and settle, and [`Oscillator::settling_time`]
//! says how long that takes. Without this the tree would either stop instantly
//! or ring forever, and both look wrong the moment you see them.

use plotkit::Cx;

/// A damped spring: something that gets pushed, overshoots, and settles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oscillator {
    /// Natural frequency `ω`, radians a second. How fast it wants to wobble.
    pub omega: f64,
    /// Damping ratio `ζ`. Below 1 it wobbles; at 1 it just eases over; above
    /// 1 it creeps.
    pub zeta: f64,
    /// Where it is.
    pub x: f64,
    /// How fast it is going.
    pub v: f64,
}

impl Oscillator {
    pub const fn new(omega: f64, zeta: f64) -> Oscillator {
        Oscillator { omega, zeta, x: 0.0, v: 0.0 }
    }

    /// The **poles**: the roots of `s² + 2ζω s + ω²`.
    ///
    /// Everything it will ever do is in these two numbers. The real part is
    /// how fast it dies away, the imaginary part is how fast it wobbles.
    /// Complex pair when `ζ < 1`; both real when `ζ > 1`; and at exactly
    /// `ζ = 1` they collide on the real axis, which is what makes that case
    /// the fastest way to arrive without overshooting.
    pub fn poles(self) -> (Cx, Cx) {
        let real = -self.zeta * self.omega;
        let disc = self.zeta * self.zeta - 1.0;
        if disc >= 0.0 {
            let d = self.omega * disc.sqrt();
            (Cx::new(real + d, 0.0), Cx::new(real - d, 0.0))
        } else {
            let d = self.omega * (-disc).sqrt();
            (Cx::new(real, d), Cx::new(real, -d))
        }
    }

    /// The frequency it actually wobbles at, which is **not** `ω` unless it is
    /// undamped: `ω_d = ω√(1 − ζ²)`. Damping slows the wobble as well as
    /// shrinking it. Zero once `ζ ≥ 1`, because then it does not wobble at all.
    pub fn damped_frequency(self) -> f64 {
        if self.zeta >= 1.0 {
            0.0
        } else {
            self.omega * (1.0 - self.zeta * self.zeta).sqrt()
        }
    }

    /// Roughly how long until it has finished moving — the usual engineer's
    /// rule of "within 2% and staying there", which is about four time
    /// constants.
    ///
    /// **The slowest pole decides.** Below critical damping both poles are the
    /// same distance from the axis and this is the familiar `4/ζω`. Above it
    /// they split into a fast one and a slow one, and it is the slow one you
    /// wait for — which is why piling on damping past critical makes things
    /// *worse*, and why `4/ζω` quietly stops being true exactly there.
    pub fn settling_time(self) -> f64 {
        let (p, q) = self.poles();
        let slowest = p.re.abs().min(q.re.abs()); // the one nearest the axis
        if slowest < 1e-9 {
            f64::INFINITY // nothing pulling it back: it never settles
        } else {
            4.0 / slowest
        }
    }

    /// What it does after being let go from `1` with no push — the shape you
    /// see when you pull a branch back and release it.
    ///
    /// This is the inverse Laplace transform of the poles above, written out.
    /// Nothing is being simulated: the answer is analytic, which is the point
    /// of having done the transform at all.
    pub fn released(self, t: f64) -> f64 {
        if t <= 0.0 {
            return 1.0;
        }
        let (z, w) = (self.zeta, self.omega);
        let envelope = (-z * w * t).exp();
        if z < 1.0 {
            // Wobbling, inside a shrinking envelope.
            let wd = self.damped_frequency();
            envelope * ((wd * t).cos() + z / (1.0 - z * z).sqrt() * (wd * t).sin())
        } else if (z - 1.0).abs() < 1e-9 {
            // The two poles have collided. No wobble, and the fastest arrival
            // there is without overshooting.
            envelope * (1.0 + w * t)
        } else {
            // Two real poles: a slow one and a fast one, and the slow one
            // decides how long you wait.
            let d = w * (z * z - 1.0).sqrt();
            let (a, b) = (-z * w + d, -z * w - d);
            (b * (a * t).exp() - a * (b * t).exp()) / (b - a)
        }
    }

    /// Push it for one step of `dt`, then let it move.
    ///
    /// Semi-implicit Euler: **update the speed first, then move using the new
    /// speed.** Doing it the other way round quietly adds energy every step,
    /// and an oscillator that gains energy is one that eventually explodes —
    /// which is the classic way a simulation like this goes wrong.
    pub fn step(&mut self, dt: f64, push: f64) {
        let a = push - 2.0 * self.zeta * self.omega * self.v - self.omega * self.omega * self.x;
        self.v += a * dt;
        self.x += self.v * dt;
    }

    /// Where it comes to rest under a steady push. At `s = 0` the transfer
    /// function is `1/ω²`, so a push of `f` settles at `f/ω²`.
    pub fn rest_under(self, push: f64) -> f64 {
        push / (self.omega * self.omega).max(1e-12)
    }

    /// How much it magnifies a shake at frequency `w` — the size of the
    /// transfer function on the imaginary axis, `|H(iw)|·ω²`.
    ///
    /// **Resonance.** Shake it near its own `ω` and a small push makes a big
    /// movement — the peak is about `1/2ζ` tall, so a lightly damped thing can
    /// be shaken to pieces by a push that would do nothing at any other speed.
    /// It is why soldiers break step on bridges.
    pub fn gain_at(self, w: f64) -> f64 {
        let (wn, z) = (self.omega, self.zeta);
        let a = wn * wn - w * w;
        let b = 2.0 * z * wn * w;
        wn * wn / (a * a + b * b).sqrt().max(1e-12)
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// ★ The poles are the whole story, so they must be the actual roots of
    /// `s² + 2ζω s + ω²`. Substituting them back has to give zero.
    #[test]
    fn the_poles_are_the_roots_of_the_denominator() {
        for zeta in [0.05, 0.4, 0.999, 1.0, 1.6, 4.0] {
            let o = Oscillator::new(3.0, zeta);
            let (p, q) = o.poles();
            for s in [p, q] {
                let value = s * s + s.scale(2.0 * zeta * o.omega) + Cx::new(o.omega * o.omega, 0.0);
                assert!(value.abs() < 1e-9, "zeta {zeta}: pole {s:?} is not a root ({value:?})");
            }
        }
    }

    /// ★ Left half plane settles, imaginary axis rings forever. That one fact
    /// is most of control engineering, and it is visible in the sign of the
    /// real part.
    #[test]
    fn damping_puts_the_poles_in_the_left_half_plane() {
        for zeta in [0.1, 0.7, 1.0, 3.0] {
            let (p, q) = Oscillator::new(2.0, zeta).poles();
            assert!(p.re < 0.0 && q.re < 0.0, "zeta {zeta} should settle");
        }
        let (p, q) = Oscillator::new(2.0, 0.0).poles();
        assert!(p.re.abs() < 1e-12 && q.re.abs() < 1e-12, "no damping should sit on the axis");
        assert!(p.im.abs() > 0.0, "and still wobble");
    }

    /// ★ At exactly `ζ = 1` the two poles collide on the real axis. That
    /// collision is the border between wobbling and creeping, and it is why
    /// critical damping is the fastest arrival without overshoot.
    #[test]
    fn at_critical_damping_the_poles_collide() {
        let (p, q) = Oscillator::new(5.0, 1.0).poles();
        assert!((p - q).abs() < 1e-9, "they should be the same pole twice");
        assert!(p.im.abs() < 1e-12, "and sitting on the real axis");
        assert!((p.re + 5.0).abs() < 1e-9, "at -omega");
    }

    /// Under a light damping it overshoots; critically damped or heavier it
    /// never does. That is the whole practical meaning of `ζ`.
    #[test]
    fn only_a_light_damping_overshoots() {
        let past_zero = |zeta: f64| {
            let o = Oscillator::new(6.0, zeta);
            (1..400).map(|k| o.released(k as f64 * 0.005)).fold(0.0f64, |m, x| m.min(x))
        };
        assert!(past_zero(0.2) < -0.1, "a light damping should swing past and come back");
        assert!(past_zero(1.0) > -1e-6, "critical should not overshoot at all");
        assert!(past_zero(3.0) > -1e-6, "and heavy certainly should not");
    }

    /// It starts where it was let go, and ends up back at rest.
    #[test]
    fn it_starts_where_released_and_ends_at_rest() {
        for zeta in [0.15, 0.6, 1.0, 2.5] {
            let o = Oscillator::new(4.0, zeta);
            assert!((o.released(0.0) - 1.0).abs() < 1e-12, "zeta {zeta} should start at 1");
            assert!(o.released(o.settling_time() * 3.0).abs() < 0.02, "zeta {zeta} should have settled");
        }
    }

    /// Undamped, it never settles — and says so rather than returning a
    /// plausible number.
    #[test]
    fn without_damping_it_never_settles() {
        assert_eq!(Oscillator::new(3.0, 0.0).settling_time(), f64::INFINITY);
        // Still swinging as wide as ever a hundred seconds later. Sampled
        // across a whole period, because `released` at one arbitrary instant
        // is wherever in the swing it happens to be, not the amplitude.
        let o = Oscillator::new(3.0, 0.0);
        let widest = (0..300).map(|k| o.released(100.0 + k as f64 * 0.01).abs()).fold(0.0f64, f64::max);
        assert!(widest > 0.99, "it should still be at full swing, reached only {widest}");
    }

    /// Heavier damping settles sooner — up to a point. Past critical it gets
    /// *slower* again, which is the surprise, and the reason critical damping
    /// is the answer rather than "as much damping as possible".
    /// ★ Past critical damping, `4/ζω` stops being true: the poles split and
    /// the slow one is the one you wait for. Trusting the familiar formula
    /// there says a heavily damped thing settles almost instantly, when in
    /// fact it is the slowest of the lot.
    #[test]
    fn the_slowest_pole_decides_how_long_it_takes() {
        // Underdamped: both poles the same distance out, so the old rule holds.
        let light = Oscillator::new(4.0, 0.5);
        assert!((light.settling_time() - 4.0 / (0.5 * 4.0)).abs() < 1e-9);

        // Overdamped: the slow pole is much nearer the axis than -zeta*omega.
        let heavy = Oscillator::new(4.0, 2.5);
        assert!(heavy.settling_time() > 4.0 / (2.5 * 4.0) * 4.0, "the naive rule is far too hopeful here");

        // And the estimate has to match what it actually does.
        for zeta in [0.2, 0.7, 1.0, 2.5, 6.0] {
            let o = Oscillator::new(4.0, zeta);
            assert!(o.released(o.settling_time() * 2.0).abs() < 0.02, "zeta {zeta} was still moving");
        }
    }

    #[test]
    fn damping_helps_until_it_does_not() {
        // The last moment it was still moving — which is the settling time,
        // rather than the first moment it happened to pass through small.
        let quiet_by = |zeta: f64| {
            let o = Oscillator::new(6.0, zeta);
            (0..4000)
                .map(|k| k as f64 * 0.005)
                .filter(|t| o.released(*t).abs() > 0.02)
                .fold(0.0f64, f64::max)
        };
        assert!(quiet_by(1.0) < quiet_by(0.15), "critical should beat a light damping");
        assert!(quiet_by(1.0) < quiet_by(6.0), "and beat a heavy one too");
    }

    /// ★ The damped wobble is slower than the natural one. Damping does not
    /// only shrink the swing, it stretches it — and past critical there is no
    /// wobble left to slow down.
    #[test]
    fn damping_slows_the_wobble_as_well_as_shrinking_it() {
        let o = |z: f64| Oscillator::new(10.0, z);
        assert!((o(0.0).damped_frequency() - 10.0).abs() < 1e-12);
        assert!(o(0.6).damped_frequency() < 10.0);
        assert!(o(0.6).damped_frequency() > 0.0);
        assert_eq!(o(1.0).damped_frequency(), 0.0);
        assert_eq!(o(2.0).damped_frequency(), 0.0);
    }

    /// ★ Resonance. Shake it at its own frequency and a small push makes a big
    /// movement — about `1/2ζ` times bigger. This is why soldiers break step
    /// on bridges, and it falls straight out of the transfer function.
    #[test]
    fn it_answers_loudest_when_shaken_at_its_own_frequency() {
        let o = Oscillator::new(8.0, 0.05);
        let peak = o.gain_at(8.0);
        assert!(peak > 8.0, "a lightly damped thing should magnify a lot, got {peak}");
        assert!((peak - 1.0 / (2.0 * 0.05)).abs() < 0.6, "the peak should be about 1/2zeta");

        assert!(o.gain_at(0.5) < peak, "and much less well far below");
        assert!(o.gain_at(60.0) < peak * 0.05, "and hardly at all far above");
        assert!((o.gain_at(0.0) - 1.0).abs() < 1e-9, "a steady push just gets through");
    }

    /// A heavily damped thing barely resonates at all — the peak flattens out.
    #[test]
    fn heavy_damping_flattens_the_resonance() {
        assert!(Oscillator::new(8.0, 0.7).gain_at(8.0) < Oscillator::new(8.0, 0.1).gain_at(8.0));
    }

    /// ★ Semi-implicit Euler: speed first, then move. The other order quietly
    /// adds energy every step, and an oscillator that gains energy explodes —
    /// slowly enough that you blame something else.
    #[test]
    fn stepping_settles_rather_than_growing() {
        let mut o = Oscillator::new(6.0, 0.2);
        o.x = 1.0;
        let mut biggest = 0.0f64;
        for _ in 0..20_000 {
            o.step(1.0 / 240.0, 0.0);
            biggest = biggest.max(o.x.abs());
        }
        assert!(biggest <= 1.01, "it gained energy: reached {biggest}");
        assert!(o.x.abs() < 0.01, "and it should have settled, not still be going");
    }

    /// Pushed steadily, it comes to rest somewhere definite — and that place
    /// is what the transfer function says at `s = 0`.
    #[test]
    fn a_steady_push_settles_at_a_definite_place() {
        let mut o = Oscillator::new(4.0, 0.8);
        for _ in 0..20_000 {
            o.step(1.0 / 240.0, 3.0);
        }
        assert!((o.x - o.rest_under(3.0)).abs() < 0.01, "settled at {} not {}", o.x, o.rest_under(3.0));
    }
}
