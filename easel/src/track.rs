//! # track — keyframes, and the one piece of mathematics in them
//!
//! ## Verbs versus keys
//!
//! [`Act`](crate::Act) is a menu of verbs: walk, jump, spin. Quick, and
//! limited to what somebody thought to put on the menu. A **track** is the
//! other way round — you say where the thing *is* at a few moments, and the
//! in-between is worked out:
//!
//! ```text
//!     t=0            t=1.2          t=2.4
//!      |--------------|--------------|
//!    where it is   where it is    where it is
//!            \        /
//!         everything between
//! ```
//!
//! That is what animating means outside a menu, and the verbs become a
//! convenience on top of it rather than the whole vocabulary.
//!
//! ## Why you cannot just average two poses
//!
//! **The one thing in this module worth reading.** A [`Pose`] is `z ↦ az + b`.
//! The `b` is a position and averaging it is right. The `a` carries the turn
//! and the size *together*, as one complex number, and averaging it is wrong
//! twice over:
//!
//! ```text
//!     halfway from  a = 1  to  a = -1   (a half turn)
//!
//!     average:      (1 + (-1)) / 2  =  0
//! ```
//!
//! Zero. The shape shrinks to a point in the middle of the turn and comes back
//! **inside out**, because the straight line between `1` and `−1` passes
//! through the origin. Scaling misbehaves more quietly: halfway between one
//! times and four times comes out at two and a half, when anybody watching
//! expects two — because size is felt in ratios, not in differences.
//!
//! Both are fixed by the same move. Go **round** rather than **across**:
//!
//! ```text
//!     a(s) = a₀ · (a₁/a₀)^s        where w^s = exp(s · ln w)
//! ```
//!
//! The ratio `a₁/a₀` is "what still has to happen". Raising it to `s` does a
//! fraction of it, and because `ln` splits a complex number into `ln|w| + i
//! arg w`, that fraction is taken **geometrically in size and evenly in
//! angle** — one expression doing both, which is the reason for working in the
//! complex plane in the first place.
//!
//! The principal branch of `ln` puts `arg` in `(−π, π]`, which makes it the
//! **shortest way round**. Turning three quarters clockwise rather than one
//! quarter anticlockwise is a thing you have to ask for, with an extra key
//! part way, and that is the right default: nobody expects the long way home.
//!
//! ## Whose idea this is
//!
//! `a₀·(a₁/a₀)^s` is the **exponential map**: it is a straight line drawn not
//! in the plane but in the *logarithm* of it. Sophus Lie's whole programme
//! (1870s) was that a continuous group of motions is understood through the
//! algebra of its infinitesimal generators, and `exp` and `log` are the bridge
//! between them. Rotations under multiplication are a group; `ln` sends them to
//! a line where interpolation is just addition; `exp` sends the answer back.
//! Lie died in 1899 largely unrecognised outside Norway, having quarrelled with
//! Klein over credit; the entire subject is now named after him.
//!
//! The same formula in three dimensions is **slerp** — spherical linear
//! interpolation — which **Ken Shoemake** gave to computer graphics in
//! *Animating Rotation with Quaternion Curves* (SIGGRAPH 1985), the paper that
//! got quaternions into every animation system there is. His argument is the
//! one being made here in the plane: averaging two rotations component by
//! component takes a chord through the sphere instead of an arc along it, so
//! the thing shrinks in the middle and speeds up at the ends. In 2D the same
//! mistake collapses a half-turn through zero, which is the bug this avoids.
//!
//! Interpolating *sizes* geometrically rather than arithmetically is much
//! older: the geometric mean is in Book V of **Euclid**, and it is the right
//! average whenever the quantity is a ratio. Doubling then quadrupling is
//! multiplying by four; half way is two, not two and a half.
//!
//! **To read further:** Shoemake's 1985 paper is four pages and worth the time
//! even in 2D. For the general picture, Stillwell's *Naive Lie Theory*.
//!
//! ## Easing
//!
//! Straight interpolation moves at a constant speed and stops dead, which is
//! the look of something being dragged by a machine. Real things start and
//! stop gradually, so a key says how to *leave* itself:
//!
//! ```text
//!     Smooth   3s² − 2s³   ease out and in. The default, and right for most things.
//!     Linear   s           constant speed. Right for something already moving.
//!     Hold     0           do not move at all until the next key. For a flipbook.
//! ```

use plotkit::Cx;
use shapes::Pose;

/// How a key leaves itself on the way to the next one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ease {
    /// `3s² − 2s³` — starts and stops gently. The default.
    Smooth,
    /// Constant speed.
    Linear,
    /// Stay put, then jump at the next key. A flipbook.
    Hold,
}

impl Ease {
    /// Bend the fraction `s` between two keys.
    pub fn bend(self, s: f64) -> f64 {
        let s = s.clamp(0.0, 1.0);
        match self {
            // Smoothstep: zero slope at both ends, so it eases out of one key
            // and into the next rather than starting and stopping dead.
            Ease::Smooth => s * s * (3.0 - 2.0 * s),
            Ease::Linear => s,
            Ease::Hold => 0.0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Ease::Smooth => "smooth",
            Ease::Linear => "linear",
            Ease::Hold => "hold",
        }
    }

    pub fn spell(word: &str) -> Option<Ease> {
        Some(match word {
            "smooth" => Ease::Smooth,
            "linear" => Ease::Linear,
            "hold" => Ease::Hold,
            _ => return None,
        })
    }
}

/// Where something is at one moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    pub at: f64,
    pub pose: Pose,
    pub ease: Ease,
}

/// Two keys at times closer than this are the same key.
///
/// A key is dropped at whatever the clock happens to read, so asking for two
/// at *exactly* the same instant never happens on purpose — but landing a
/// microsecond away does, and two keys a microsecond apart is a jump rather
/// than a movement.
pub const SAME: f64 = 1e-3;

/// Everything a mark does, as poses at moments.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Track {
    /// Sorted by time. Kept sorted by [`Track::set`] rather than sorted on
    /// use, because reading happens sixty times a second and writing happens
    /// when a hand moves.
    pub keys: Vec<Key>,
    pub looping: bool,
}

impl Track {
    pub fn new() -> Track {
        Track { keys: Vec::new(), looping: true }
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// When the last key is.
    pub fn end(&self) -> f64 {
        self.keys.last().map_or(0.0, |k| k.at)
    }

    /// Put a key here, replacing one already at this moment.
    pub fn set(&mut self, at: f64, pose: Pose, ease: Ease) {
        let at = at.max(0.0);
        match self.keys.iter().position(|k| (k.at - at).abs() < SAME) {
            Some(k) => self.keys[k] = Key { at, pose, ease },
            None => {
                let k = self.keys.partition_point(|k| k.at < at);
                self.keys.insert(k, Key { at, pose, ease });
            }
        }
    }

    /// Take the key at this moment away, if there is one.
    pub fn clear_at(&mut self, at: f64) -> bool {
        match self.keys.iter().position(|k| (k.at - at).abs() < SAME) {
            Some(k) => {
                self.keys.remove(k);
                true
            }
            None => false,
        }
    }

    /// The key at this moment, if there is one.
    pub fn key_at(&self, at: f64) -> Option<&Key> {
        self.keys.iter().find(|k| (k.at - at).abs() < SAME)
    }

    /// Where it is at `t`.
    pub fn at(&self, t: f64) -> Pose {
        if self.keys.is_empty() {
            return Pose::STILL;
        }
        if self.keys.len() == 1 {
            return self.keys[0].pose;
        }
        let end = self.end();
        // Looping wraps back to the first key. Before the first and after the
        // last it **holds**, rather than extrapolating: a track that carried on
        // guessing past its own ends would send things off the page whenever
        // the clock overran.
        let t = if self.looping && end > 0.0 && t > end { t % end } else { t };

        let Some(k) = self.keys.iter().rposition(|k| k.at <= t) else {
            return self.keys[0].pose;
        };
        let Some(next) = self.keys.get(k + 1) else {
            return self.keys[k].pose;
        };
        let here = self.keys[k];
        let span = next.at - here.at;
        let s = if span <= 0.0 { 0.0 } else { here.ease.bend((t - here.at) / span) };
        between(here.pose, next.pose, s)
    }

    /// Every moment a key sits at, for drawing a timeline.
    pub fn moments(&self) -> Vec<f64> {
        self.keys.iter().map(|k| k.at).collect()
    }
}

/// A fraction `s` of the way from one pose to another.
///
/// The turn-and-size part goes **round** rather than across — see the module
/// note. The position part is a straight line, which is what a straight line
/// between two places is.
pub fn between(from: Pose, to: Pose, s: f64) -> Pose {
    Pose::new(turn(from.a, to.a, s), from.b + (to.b - from.b).scale(s))
}

/// `a₀ · (a₁/a₀)^s` — geometric in size, shortest way round in angle.
fn turn(from: Cx, to: Cx, s: f64) -> Cx {
    // A pose with a zero `a` has collapsed the plane to a point and there is
    // nothing to interpolate; fall back to the straight line rather than
    // dividing by nothing.
    if from.abs() < 1e-12 || to.abs() < 1e-12 {
        return from + (to - from).scale(s);
    }
    let ratio = to / from;
    // ln w = ln|w| + i arg w, principal branch -- so the angle taken is the
    // shortest one, and the size is taken geometrically.
    let log = Cx::new(ratio.abs().ln(), ratio.arg());
    from * cexp(log.scale(s))
}

/// `e^z = e^{Re z}(cos Im z + i sin Im z)`.
fn cexp(z: Cx) -> Cx {
    Cx::expi(z.im).scale(z.re.exp())
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{PI, TAU};

    fn pose(scale: f64, turn: f64, at: Cx) -> Pose {
        Pose::new(Cx::polar(scale, turn), at)
    }

    /// ★ **The whole point of the module.** Averaging two poses sends a half
    /// turn through zero: the shape shrinks to a point in the middle and comes
    /// back inside out, because the straight line from `1` to `−1` passes
    /// through the origin.
    #[test]
    fn a_half_turn_does_not_collapse_through_zero() {
        let (from, to) = (pose(1.0, 0.0, Cx::ZERO), pose(1.0, PI, Cx::ZERO));
        for k in 0..=20 {
            let s = k as f64 / 20.0;
            let a = between(from, to, s).a;
            assert!((a.abs() - 1.0).abs() < 1e-9, "at s={s} the size became {}", a.abs());
        }
        // The naive way, for contrast: exactly the failure being avoided.
        let averaged = from.a + (to.a - from.a).scale(0.5);
        assert!(averaged.abs() < 1e-9, "averaging really does give zero");
    }

    /// ★ And size is felt in **ratios**. Halfway between one times and four
    /// times is two, not two and a half — the geometric mean, which is what
    /// `(a₁/a₀)^s` gives and what averaging does not.
    #[test]
    fn halfway_between_one_and_four_times_is_twice() {
        let (from, to) = (pose(1.0, 0.0, Cx::ZERO), pose(4.0, 0.0, Cx::ZERO));
        assert!((between(from, to, 0.5).a.abs() - 2.0).abs() < 1e-9);
        assert!((between(from, to, 0.25).a.abs() - 4.0f64.powf(0.25)).abs() < 1e-9);
    }

    /// ★ The **shortest way round**. Going from 10° to 350° should be 20°
    /// backwards, not 340° forwards — nobody expects the long way home, and
    /// the principal branch of `ln` gives it for nothing.
    #[test]
    fn it_turns_the_short_way() {
        let from = pose(1.0, 10.0_f64.to_radians(), Cx::ZERO);
        let to = pose(1.0, 350.0_f64.to_radians(), Cx::ZERO);
        let middle = between(from, to, 0.5).a.arg().to_degrees();
        // Halfway the short way is 0 degrees; the long way would be 180.
        assert!(middle.abs() < 1e-6, "it went the long way, to {middle} degrees");
    }

    /// The ends are exactly the poses given. An interpolation that only nearly
    /// reaches its keys makes every key you set look slightly wrong.
    #[test]
    fn the_ends_are_exactly_where_they_were_put() {
        let (from, to) = (pose(0.7, 1.1, Cx::new(-2.0, 3.0)), pose(2.3, -0.4, Cx::new(5.0, -1.0)));
        for (s, want) in [(0.0, from), (1.0, to)] {
            let got = between(from, to, s);
            assert!((got.a - want.a).abs() < 1e-9 && (got.b - want.b).abs() < 1e-9, "at s={s}");
        }
    }

    /// ★ Smoothstep starts and stops gently. Constant speed with a dead stop
    /// at each end is the look of something being dragged by a machine.
    #[test]
    fn smoothing_eases_out_of_a_key_and_into_the_next() {
        let step = |s: f64| Ease::Smooth.bend(s);
        assert!((step(0.0)).abs() < 1e-12);
        assert!((step(1.0) - 1.0).abs() < 1e-12);
        assert!((step(0.5) - 0.5).abs() < 1e-12, "and it is symmetric");

        // Slow at the ends, fast in the middle -- which is what "eases" means.
        let speed = |s: f64| (step(s + 0.01) - step(s - 0.01)) / 0.02;
        assert!(speed(0.05) < speed(0.5) * 0.5, "it should leave gently");
        assert!(speed(0.95) < speed(0.5) * 0.5, "and arrive gently");
        assert!((Ease::Linear.bend(0.3) - 0.3).abs() < 1e-12);
        assert_eq!(Ease::Hold.bend(0.9), 0.0, "hold does not move at all");
    }

    /// ★ Keys stay sorted however they are put in. Reading happens sixty times
    /// a second and writing when a hand moves, so the sorting belongs to the
    /// writing.
    #[test]
    fn keys_are_kept_in_order_whatever_order_they_arrive_in() {
        let mut t = Track::new();
        for at in [2.0, 0.5, 3.5, 1.0, 0.0] {
            t.set(at, pose(1.0, 0.0, Cx::new(at, 0.0)), Ease::Smooth);
        }
        assert_eq!(t.moments(), vec![0.0, 0.5, 1.0, 2.0, 3.5]);
        assert!(t.keys.windows(2).all(|w| w[0].at < w[1].at));
    }

    /// Putting a key where one already is replaces it. Two keys a microsecond
    /// apart is a jump rather than a movement, and it happens by accident
    /// whenever a key is dropped at whatever the clock reads.
    #[test]
    fn a_second_key_at_the_same_moment_replaces_the_first() {
        let mut t = Track::new();
        t.set(1.0, pose(1.0, 0.0, Cx::new(1.0, 0.0)), Ease::Smooth);
        t.set(1.0 + SAME / 2.0, pose(1.0, 0.0, Cx::new(9.0, 0.0)), Ease::Linear);
        assert_eq!(t.len(), 1);
        assert!((t.keys[0].pose.b.re - 9.0).abs() < 1e-9, "the newer one should win");
        assert_eq!(t.keys[0].ease, Ease::Linear);
    }

    /// ★ It **holds** before the first key and after the last rather than
    /// extrapolating. A track that carried on guessing past its own ends would
    /// fling things off the page whenever the clock overran.
    #[test]
    fn it_holds_at_the_ends_rather_than_guessing() {
        let mut t = Track::new();
        t.looping = false;
        t.set(1.0, pose(1.0, 0.0, Cx::new(0.0, 0.0)), Ease::Linear);
        t.set(2.0, pose(1.0, 0.0, Cx::new(4.0, 0.0)), Ease::Linear);

        for early in [-5.0, 0.0, 0.999] {
            assert!(t.at(early).b.abs() < 1e-9, "before the first key it should sit still, at t={early}");
        }
        for late in [2.0, 10.0, 1e4] {
            assert!((t.at(late).b.re - 4.0).abs() < 1e-9, "after the last it should stay put, at t={late}");
        }
    }

    /// And looping comes round to the first key again.
    #[test]
    fn a_looping_track_comes_round_again() {
        let mut t = Track::new();
        t.set(0.0, pose(1.0, 0.0, Cx::ZERO), Ease::Linear);
        t.set(2.0, pose(1.0, 0.0, Cx::new(4.0, 0.0)), Ease::Linear);
        assert!((t.at(0.5).b - t.at(2.5).b).abs() < 1e-9, "it should repeat");
        assert!((t.at(4.0).b - t.at(0.0).b).abs() < 1e-9);
    }

    /// A track with nothing in it, or one key in it, is not a special case
    /// anybody should have to think about.
    #[test]
    fn a_track_with_almost_nothing_in_it_still_works() {
        assert_eq!(Track::new().at(3.0), Pose::STILL);

        let mut one = Track::new();
        let only = pose(1.5, 0.3, Cx::new(2.0, 2.0));
        one.set(1.0, only, Ease::Smooth);
        for t in [-1.0, 0.0, 1.0, 99.0] {
            assert_eq!(one.at(t), only, "one key means one pose, at t={t}");
        }
    }

    /// ★ A whole turn needs keys **part way round**, and this is where the
    /// shortest-arc rule shows its edge. Two keys a full turn apart ask for
    /// `a₁/a₀ = 1` — no rotation at all — because "all the way round" and
    /// "stay where you are" end in the same place, and nothing in the two
    /// poses can tell them apart.
    ///
    /// That is not a flaw to be fixed by remembering turn counts; it is what
    /// the poses actually say. The answer is to say more: put a key at a third
    /// and two thirds, and each hop is unambiguous.
    #[test]
    fn a_whole_turn_needs_keys_part_way_round() {
        // How far it actually turns, adding up the small changes -- which is
        // the only honest way to measure, since `arg` wraps.
        let turned = |t: &Track, end: f64| {
            let n = 600;
            let mut total = 0.0;
            let mut last = t.at(0.0).a.arg();
            for k in 1..=n {
                let now = t.at(k as f64 / n as f64 * end).a.arg();
                let mut step = now - last;
                while step > PI {
                    step -= TAU;
                }
                while step < -PI {
                    step += TAU;
                }
                total += step;
                last = now;
            }
            total
        };

        // Two keys, a whole turn apart: they are the same pose, so nothing
        // moves.
        let mut naive = Track::new();
        naive.looping = false;
        naive.set(0.0, pose(1.0, 0.0, Cx::ZERO), Ease::Linear);
        naive.set(3.0, pose(1.0, TAU, Cx::ZERO), Ease::Linear);
        assert!(turned(&naive, 3.0).abs() < 1e-6, "a whole turn is indistinguishable from none");

        // Four keys, a third of a turn each: unambiguous, and it goes round.
        let mut proper = Track::new();
        proper.looping = false;
        for k in 0..4 {
            proper.set(k as f64, pose(1.0, k as f64 * TAU / 3.0, Cx::ZERO), Ease::Linear);
        }
        assert!((turned(&proper, 3.0) - TAU).abs() < 1e-3, "it should go all the way round: {}", turned(&proper, 3.0));
    }

    /// Taking a key away leaves the rest alone.
    #[test]
    fn a_key_can_be_taken_away_again() {
        let mut t = Track::new();
        for at in [0.0, 1.0, 2.0] {
            t.set(at, pose(1.0, 0.0, Cx::new(at, 0.0)), Ease::Smooth);
        }
        assert!(t.clear_at(1.0));
        assert_eq!(t.moments(), vec![0.0, 2.0]);
        assert!(!t.clear_at(1.0), "and taking away what is not there is nothing");
    }

    /// Every ease survives being written down.
    #[test]
    fn every_ease_can_be_written_and_read() {
        for e in [Ease::Smooth, Ease::Linear, Ease::Hold] {
            assert_eq!(Ease::spell(e.name()), Some(e));
        }
        assert_eq!(Ease::spell("springy"), None);
    }
}
