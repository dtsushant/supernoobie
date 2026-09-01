//! # motion — spin, walk, run, as values you can add together
//!
//! A motion is not a thing you *do* to a shape. It is a function from time to
//! a [`Pose`], and a pose is where things are:
//!
//! ```text
//!     z  ->  a z + b
//! ```
//!
//! Two complex numbers. `a` turns and stretches, `b` moves — and that pair is
//! **every similarity of the plane**, which is why one type covers spinning,
//! walking, orbiting, bobbing and pulsing without a special case for any of
//! them.
//!
//! ## Motions compose, and the composition is multiplication
//!
//! Do one pose then another:
//!
//! ```text
//!     (a₂, b₂) ∘ (a₁, b₁)  =  (a₂a₁,  a₂b₁ + b₂)
//! ```
//!
//! That is not an analogy. It *is* the group law of the similarities of the
//! plane, and it is why `Motion::then` can exist at all: walking while spinning
//! is one motion, not two things fighting over the same shape.
//!
//! ```no_run
//! # use shapes::motion::Motion;
//! # use plotkit::Cx;
//! let gait = Motion::walk(Cx::new(1.0, 0.0)).then(Motion::spin(0.25));
//! ```
//!
//! ## Which way is round?
//!
//! **Positive is anticlockwise.** Not a convention picked from a hat:
//! `e^{iθ} = cos θ + i sin θ`, and as `θ` grows that point goes from `1`
//! towards `i` — right, then up. Clockwise is `spin(-rate)`, or
//! [`Motion::reversed`].

use plotkit::{Cx, Shape};
use std::f64::consts::TAU;
use std::sync::Arc;

/// Where something is: `z ↦ a z + b`.
///
/// `a` is a turn and a stretch together, `b` is a move. Everything a rigid or
/// uniformly-scaled thing can do in the plane, in two numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub a: Cx,
    pub b: Cx,
}

impl Pose {
    /// The pose that does nothing: `z ↦ z`.
    pub const STILL: Pose = Pose { a: Cx::ONE, b: Cx::ZERO };

    pub fn new(a: Cx, b: Cx) -> Pose {
        Pose { a, b }
    }

    pub fn apply(self, z: Cx) -> Cx {
        self.a * z + self.b
    }

    /// This pose, then `next`. Reads left to right in the order things happen.
    pub fn then(self, next: Pose) -> Pose {
        Pose { a: next.a * self.a, b: next.a * self.b + next.b }
    }

    /// The pose that undoes this one: `w ↦ (w − b) / a`.
    ///
    /// Needed whenever you have a *screen* position and want to know what it
    /// means to something that has been moved — clicking a member of a
    /// spinning group, for instance. `None` when `a` is zero, because a pose
    /// that collapses the plane to a point cannot be undone.
    pub fn inverse(self) -> Option<Pose> {
        (self.a.abs_sq() > 1e-24).then(|| {
            let inv = self.a.conj().scale(1.0 / self.a.abs_sq());
            Pose { a: inv, b: -(inv * self.b) }
        })
    }

    /// Apply to a whole shape.
    pub fn shape(self, s: Shape) -> Shape {
        s.map(move |z| self.apply(z))
    }
}

/// A motion: time in, [`Pose`] out.
#[derive(Clone)]
pub struct Motion(Arc<dyn Fn(f64) -> Pose + Send + Sync>);

impl Motion {
    pub fn of(f: impl Fn(f64) -> Pose + Send + Sync + 'static) -> Motion {
        Motion(Arc::new(f))
    }

    /// Stand still.
    pub fn still() -> Motion {
        Motion::of(|_| Pose::STILL)
    }

    /// Turn about the origin, in **turns per second**. Positive is
    /// anticlockwise, because that is the way `e^{iθ}` goes.
    ///
    /// Turns rather than radians because "one turn a second" is a thing you can
    /// picture and `6.28 radians a second` is not.
    pub fn spin(turns_per_second: f64) -> Motion {
        Motion::of(move |t| Pose::new(Cx::expi(TAU * turns_per_second * t), Cx::ZERO))
    }

    /// Move at a steady velocity, in units per second.
    pub fn travel(velocity: Cx) -> Motion {
        Motion::of(move |t| Pose::new(Cx::ONE, velocity.scale(t)))
    }

    /// Go round in a circle of radius `r`, `turns_per_second` times a second,
    /// without turning on the spot.
    pub fn orbit(r: f64, turns_per_second: f64) -> Motion {
        Motion::of(move |t| Pose::new(Cx::ONE, Cx::expi(TAU * turns_per_second * t).scale(r)))
    }

    /// Rise and fall by `height`, `per_second` times a second.
    pub fn bob(height: f64, per_second: f64) -> Motion {
        Motion::of(move |t| Pose::new(Cx::ONE, Cx::new(0.0, height * (TAU * per_second * t).sin())))
    }

    /// Breathe: grow and shrink by `amount` either side of full size.
    pub fn pulse(amount: f64, per_second: f64) -> Motion {
        Motion::of(move |t| Pose::new(Cx::new(1.0 + amount * (TAU * per_second * t).sin(), 0.0), Cx::ZERO))
    }

    /// A walk: travelling, with the gentle up-and-down that makes it read as
    /// walking rather than sliding.
    ///
    /// Built out of the pieces above rather than written afresh — the bob is
    /// two steps a second, and it is `travel` doing the actual moving.
    pub fn walk(velocity: Cx) -> Motion {
        let pace = velocity.abs().max(0.2);
        Motion::travel(velocity).then(Motion::bob(0.06 * pace, pace))
    }

    /// A run: the same walk, faster, bouncing higher, and leaning into it.
    ///
    /// The lean is a small constant spin — which is the one place a `Motion`
    /// is used for something that does not change with time, and it works
    /// because a constant is a perfectly good function of `t`.
    pub fn run(velocity: Cx) -> Motion {
        let v = velocity.scale(3.0);
        let pace = v.abs().max(0.6);
        let lean = -0.12 * velocity.re.signum();
        Motion::travel(v)
            .then(Motion::bob(0.10 * pace, pace * 0.9))
            .then(Motion::of(move |_| Pose::new(Cx::expi(lean), Cx::ZERO)))
    }

    /// Do this motion, then `next`. The pose you get is both at once.
    pub fn then(self, next: Motion) -> Motion {
        Motion::of(move |t| self.at(t).then(next.at(t)))
    }

    /// The same motion the other way round.
    pub fn reversed(self) -> Motion {
        Motion::of(move |t| self.at(-t))
    }

    /// Run at a different speed. `2.0` is twice as fast; a negative number
    /// runs it backwards.
    pub fn speed(self, k: f64) -> Motion {
        Motion::of(move |t| self.at(t * k))
    }

    /// Do it about `centre` instead of about the origin: move there, do it,
    /// move back. Which is exactly how you would say it on paper.
    pub fn about(self, centre: Cx) -> Motion {
        Motion::of(move |t| {
            Pose::new(Cx::ONE, -centre).then(self.at(t)).then(Pose::new(Cx::ONE, centre))
        })
    }

    pub fn at(&self, t: f64) -> Pose {
        (self.0)(t)
    }

    /// The shape, moved to where it is at time `t`.
    pub fn shape(&self, s: Shape, t: f64) -> Shape {
        self.at(t).shape(s)
    }
}

impl Default for Motion {
    fn default() -> Motion {
        Motion::still()
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Cx, b: Cx) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn standing_still_changes_nothing() {
        let z = Cx::new(3.0, -1.0);
        assert_eq!(Pose::STILL.apply(z), z);
        assert_eq!(Motion::still().at(99.0), Pose::STILL);
    }

    /// ★ Composing poses is composing the functions they stand for. If this
    /// were wrong, `then` would silently apply motions in the wrong order and
    /// a walking, spinning figure would orbit instead.
    #[test]
    fn composing_poses_is_composing_the_maps() {
        let p = Pose::new(Cx::new(0.0, 2.0), Cx::new(1.0, 0.0)); // 2i z + 1
        let q = Pose::new(Cx::new(3.0, 0.0), Cx::new(0.0, -1.0)); // 3z - i
        for k in 0..20 {
            let z = Cx::polar(0.4 * k as f64, 0.7 * k as f64);
            assert!(close(p.then(q).apply(z), q.apply(p.apply(z))), "p then q must be q after p");
        }
    }

    /// Composition is associative, because function composition is.
    #[test]
    fn composing_is_associative() {
        let (p, q, r) = (
            Pose::new(Cx::new(0.5, 0.5), Cx::new(1.0, 0.0)),
            Pose::new(Cx::new(-1.0, 0.3), Cx::new(0.0, 2.0)),
            Pose::new(Cx::new(2.0, 0.0), Cx::new(-1.0, 1.0)),
        );
        let z = Cx::new(1.3, -0.7);
        assert!(close(p.then(q).then(r).apply(z), p.then(q.then(r)).apply(z)));
    }

    /// ★ The inverse really undoes it. This is what lets you click a member
    /// of a group that is spinning: take the pointer back through the pose and
    /// ask the member where it thinks that is.
    #[test]
    fn the_inverse_undoes_the_pose() {
        let p = Pose::new(Cx::polar(1.7, 0.9), Cx::new(-2.0, 3.0));
        let inv = p.inverse().expect("a is not zero");
        for k in 0..20 {
            let z = Cx::polar(0.3 * k as f64, 1.1 * k as f64);
            assert!(close(inv.apply(p.apply(z)), z), "there and back should land where it started");
            assert!(close(p.apply(inv.apply(z)), z), "and the other way round");
        }
    }

    /// A pose that squashes the plane to a point has no inverse, and says so
    /// rather than dividing by zero.
    #[test]
    fn a_collapsing_pose_has_no_inverse() {
        assert!(Pose::new(Cx::ZERO, Cx::new(1.0, 1.0)).inverse().is_none());
    }

    /// ★ Positive is anticlockwise, because that is the way `e^{iθ}` goes:
    /// from 1 towards i. Getting this backwards would reverse every rotation
    /// in the library at once.
    #[test]
    fn a_positive_spin_goes_anticlockwise() {
        let quarter = Motion::spin(1.0).at(0.25); // a quarter of a turn
        assert!(close(quarter.apply(Cx::ONE), Cx::I), "1 should land on i");
        let back = Motion::spin(-1.0).at(0.25);
        assert!(close(back.apply(Cx::ONE), -Cx::I), "and clockwise on -i");
    }

    #[test]
    fn a_full_turn_comes_home() {
        let z = Cx::new(2.0, -1.0);
        assert!(close(Motion::spin(1.0).at(1.0).apply(z), z));
        assert!(close(Motion::spin(3.0).at(1.0 / 3.0).apply(z), z));
    }

    #[test]
    fn travelling_is_linear_in_time() {
        let m = Motion::travel(Cx::new(2.0, -1.0));
        assert!(close(m.at(0.0).apply(Cx::ZERO), Cx::ZERO));
        assert!(close(m.at(3.0).apply(Cx::ZERO), Cx::new(6.0, -3.0)));
    }

    /// An orbit moves without turning: a shape going round keeps facing the
    /// same way, which is the difference between the moon and a fairground
    /// carousel horse.
    #[test]
    fn an_orbit_moves_without_turning() {
        let m = Motion::orbit(2.0, 1.0);
        for k in 0..12 {
            let p = m.at(k as f64 / 12.0);
            assert!(close(p.a, Cx::ONE), "an orbit should not rotate what it carries");
            assert!((p.b.abs() - 2.0).abs() < 1e-9, "and should stay at radius 2");
        }
    }

    /// ★ `walk` really is `travel` plus a bob, so the thing actually gets
    /// somewhere. A gait that bobbed on the spot would be the classic bug.
    #[test]
    fn a_walk_gets_somewhere() {
        let m = Motion::walk(Cx::new(1.0, 0.0));
        let x = |t: f64| m.at(t).apply(Cx::ZERO).re;
        assert!((x(4.0) - 4.0).abs() < 1e-9, "one unit a second for four seconds");
        // and it goes up and down on the way
        let ys: Vec<f64> = (0..40).map(|k| m.at(k as f64 * 0.05).apply(Cx::ZERO).im).collect();
        let (lo, hi) = (ys.iter().cloned().fold(f64::MAX, f64::min), ys.iter().cloned().fold(f64::MIN, f64::max));
        assert!(hi - lo > 0.02, "a walk should bob, range was {}", hi - lo);
    }

    /// Running is the same walk, faster and bouncier — three times the ground
    /// speed, and a bigger bob.
    #[test]
    fn running_covers_more_ground_than_walking() {
        let v = Cx::new(1.0, 0.0);
        let go = |m: &Motion, t: f64| m.at(t).apply(Cx::ZERO).re;
        assert!(go(&Motion::run(v), 2.0) > go(&Motion::walk(v), 2.0) * 2.5);

        let spread = |m: &Motion| {
            let ys: Vec<f64> = (0..80).map(|k| m.at(k as f64 * 0.02).apply(Cx::ZERO).im).collect();
            ys.iter().cloned().fold(f64::MIN, f64::max) - ys.iter().cloned().fold(f64::MAX, f64::min)
        };
        assert!(spread(&Motion::run(v)) > spread(&Motion::walk(v)), "a run should bounce higher");
    }

    /// ★ `about` turns around a point rather than the origin — move there,
    /// turn, move back, which is what you would write on paper.
    #[test]
    fn spinning_about_a_point_leaves_that_point_alone() {
        let c = Cx::new(3.0, -2.0);
        let m = Motion::spin(1.0).about(c);
        for k in 0..8 {
            assert!(close(m.at(k as f64 / 8.0).apply(c), c), "the centre of the turn must not move");
        }
        // and something a unit away stays a unit away
        let p = c + Cx::ONE;
        assert!(((m.at(0.3).apply(p) - c).abs() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn reversing_and_speed_do_what_they_say() {
        let m = Motion::travel(Cx::new(1.0, 0.0));
        assert!(close(m.clone().reversed().at(2.0).apply(Cx::ZERO), Cx::new(-2.0, 0.0)));
        assert!(close(m.speed(3.0).at(2.0).apply(Cx::ZERO), Cx::new(6.0, 0.0)));
    }

    /// Two motions at once are one motion. Spinning while travelling puts the
    /// shape on the path travel describes, turned by the spin.
    #[test]
    fn walking_while_spinning_is_one_motion() {
        let m = Motion::spin(1.0).then(Motion::travel(Cx::new(4.0, 0.0)));
        // At a quarter second: the spin is a quarter turn, and the travel is
        // 4 units per second FOR A QUARTER SECOND, so one unit.
        let p = m.at(0.25);
        // Spin first: 1 -> i. Then travel: i + 1.
        assert!(close(p.apply(Cx::ONE), Cx::new(1.0, 1.0)), "got {:?}", p.apply(Cx::ONE));

        // Order matters, and the other order puts it somewhere else: travel
        // first moves 1 -> 2, then the quarter turn sends that to 2i.
        let other = Motion::travel(Cx::new(4.0, 0.0)).then(Motion::spin(1.0));
        assert!(close(other.at(0.25).apply(Cx::ONE), Cx::new(0.0, 2.0)));
    }
}
