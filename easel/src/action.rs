//! # action — what a mark does, written down
//!
//! ## The same problem as `Mark`, one level up
//!
//! [`shapes::Motion`] is `Arc<dyn Fn(f64) -> Pose>` — a closure. It composes
//! beautifully and it cannot be written to a file, for exactly the reason a
//! [`Shape`](plotkit::Shape) cannot: it is compiled code.
//!
//! So the pattern repeats. An [`Action`] is **numbers and a name**; it builds
//! a `Motion` on demand and only the numbers are ever saved.
//!
//! ```text
//!     Mark    --- numbers ---> file        Shape   is built from it
//!     Action  --- numbers ---> file        Motion  is built from it
//! ```
//!
//! ## An [`Act`] is a sequence, because that is what animating is
//!
//! One motion is a loop: spin forever, bob forever. What people mean by
//! animating something is *"walk for two seconds, then jump, then stand
//! still"* — an ordered list of things with durations.
//!
//! ```text
//!     walk 3.0   |   jump 1.2   |   still 0.5
//!     0 ------- 3.0 --------- 4.2 ------- 4.7  and round again
//! ```
//!
//! ## Why each step starts where the last one finished
//!
//! The hard part, and the only part with any real content in it. If each step
//! were evaluated on its own, a walk that had carried a figure three units to
//! the right would snap back to the start the instant the jump began — the
//! jump's own pose knows nothing about the walk.
//!
//! So the finished steps are **accumulated**: every completed step is
//! evaluated at *its own end* and composed, and the running step is composed
//! on top at its own local time.
//!
//! ```text
//!     pose(t) = step_k(local)  ∘  step_{k−1}(its whole duration)  ∘  ...  ∘  step_1(...)
//! ```
//!
//! That composition is the group law of plane similarities — `(a₂,b₂)∘(a₁,b₁)
//! = (a₂a₁, a₂b₁+b₂)` — which [`Pose::then`](shapes::Pose::then) already is.
//! Nothing new had to be invented for it, which is a good sign that the pose
//! was the right thing to carry around.
//!
//! ## Jumping is not bobbing
//!
//! A bob is a sine: it eases out of the top and spends as long up as down, so
//! it reads as floating. A jump is **ballistic** — thrown up, and gravity
//! takes it from there — so it is fast off the ground, hangs at the apex, and
//! comes down fast. That is `s = v₀t − ½gt²`, and it is done here with
//! [`physics::Fall`] rather than a fresh parabola, so there is one Galileo in
//! the repository rather than two.
//!
//! Given a hop that should reach `h` in `T` seconds, the two conditions
//! `h = v₀²/2g` and `T = 2v₀/g` fix both unknowns: `g = 8h/T²` and `v₀ = 4h/T`.

use physics::Fall;
use plotkit::Cx;
use shapes::{Motion, Pose};

/// One thing a mark can be doing.
///
/// Every variant is plain numbers, so the whole of an animation is a few
/// words in a text file.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    /// Stand there.
    Still,
    /// Along `velocity`, with a bounce in step. Units per second.
    Walk(Cx),
    /// The same, faster and with a lean into it.
    Run(Cx),
    /// Ballistic hops: `height` units up, `rate` of them a second.
    Jump { height: f64, rate: f64 },
    /// Turns per second. Positive is anticlockwise, as angles always are here.
    Spin(f64),
    /// Up and down on a sine — floating rather than jumping.
    Bob { height: f64, rate: f64 },
    /// Bigger and smaller. `amount` is the fraction either way.
    Pulse { amount: f64, rate: f64 },
    /// Round a circle of radius `r`, without turning to face along it.
    Orbit { r: f64, rate: f64 },
    /// Straight along `velocity`, with no gait at all.
    Drift(Cx),
}

impl Action {
    /// The motion this action is.
    pub fn motion(self) -> Motion {
        match self {
            Action::Still => Motion::still(),
            Action::Walk(v) => Motion::walk(v),
            Action::Run(v) => Motion::run(v),
            Action::Drift(v) => Motion::travel(v),
            Action::Spin(turns) => Motion::spin(turns),
            Action::Bob { height, rate } => Motion::bob(height, rate),
            Action::Pulse { amount, rate } => Motion::pulse(amount, rate),
            Action::Orbit { r, rate } => Motion::orbit(r, rate),
            Action::Jump { height, rate } => jump(height, rate),
        }
    }

    /// Where it has got to at `t` seconds.
    pub fn at(self, t: f64) -> Pose {
        self.motion().at(t)
    }

    /// The word used in a file, and the numbers after it.
    pub fn spelt(self) -> (&'static str, [f64; 2]) {
        match self {
            Action::Still => ("still", [0.0, 0.0]),
            Action::Walk(v) => ("walk", [v.re, v.im]),
            Action::Run(v) => ("run", [v.re, v.im]),
            Action::Drift(v) => ("drift", [v.re, v.im]),
            Action::Jump { height, rate } => ("jump", [height, rate]),
            Action::Spin(turns) => ("spin", [turns, 0.0]),
            Action::Bob { height, rate } => ("bob", [height, rate]),
            Action::Pulse { amount, rate } => ("pulse", [amount, rate]),
            Action::Orbit { r, rate } => ("orbit", [r, rate]),
        }
    }

    /// Read one back. `None` for a word this version does not know, so a file
    /// from a later one loses that step rather than the whole drawing.
    pub fn spell(word: &str, n: [f64; 2]) -> Option<Action> {
        Some(match word {
            "still" => Action::Still,
            "walk" => Action::Walk(Cx::new(n[0], n[1])),
            "run" => Action::Run(Cx::new(n[0], n[1])),
            "drift" => Action::Drift(Cx::new(n[0], n[1])),
            "jump" => Action::Jump { height: n[0], rate: n[1] },
            "spin" => Action::Spin(n[0]),
            "bob" => Action::Bob { height: n[0], rate: n[1] },
            "pulse" => Action::Pulse { amount: n[0], rate: n[1] },
            "orbit" => Action::Orbit { r: n[0], rate: n[1] },
            _ => return None,
        })
    }

    /// What to call it on a button.
    pub fn name(self) -> &'static str {
        self.spelt().0
    }
}

/// Ballistic hops — thrown up, and gravity takes it from there.
///
/// Fast off the ground, hanging at the top, fast coming down. A sine would
/// spend as long up as down and read as floating, which is [`Action::Bob`].
fn jump(height: f64, rate: f64) -> Motion {
    let h = height.max(0.0);
    let hop = 1.0 / rate.abs().max(0.05);
    // The two conditions that fix a parabola: it reaches `h`, and it takes
    // `hop` to get up and back down.
    let g = 8.0 * h / (hop * hop);
    let launch = 4.0 * h / hop;
    let air = Fall::vacuum(g);
    Motion::of(move |t| {
        // Where in the current hop. Negative time is treated as standing
        // still rather than as a hop running backwards into the ground.
        let tau = if t <= 0.0 { 0.0 } else { t % hop };
        // `at` measures **downward**, and a jump goes up, so the launch speed
        // is negative. Galileo does the rest.
        Pose::new(Cx::ONE, air.at(Cx::ZERO, -launch, tau))
    })
}

/// One step of an act: something to do, and for how long.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    pub action: Action,
    /// Seconds. A step of no length is skipped rather than dividing by nothing.
    pub seconds: f64,
}

/// Everything a mark does, in order.
#[derive(Clone, Debug, PartialEq)]
pub struct Act {
    pub steps: Vec<Step>,
    /// Start again at the end. Off means it holds its final pose forever.
    pub looping: bool,
}

impl Default for Act {
    fn default() -> Act {
        Act::still()
    }
}

impl Act {
    /// Does nothing, forever.
    pub fn still() -> Act {
        Act { steps: Vec::new(), looping: true }
    }

    /// One thing, forever.
    pub fn just(action: Action) -> Act {
        Act { steps: vec![Step { action, seconds: f64::INFINITY }], looping: false }
    }

    pub fn is_still(&self) -> bool {
        self.steps.iter().all(|s| s.action == Action::Still) || self.total() <= 0.0
    }

    /// Then do this, for this long.
    pub fn then(mut self, action: Action, seconds: f64) -> Act {
        self.steps.push(Step { action, seconds: seconds.max(0.0) });
        self
    }

    pub fn looping(mut self, yes: bool) -> Act {
        self.looping = yes;
        self
    }

    /// How long the whole act takes. Infinite if any step never ends.
    pub fn total(&self) -> f64 {
        self.steps.iter().map(|s| s.seconds).sum()
    }

    /// Where the mark has got to at `t` seconds.
    ///
    /// Each finished step is evaluated at **its own end** and composed, so the
    /// running step starts from wherever the last one left off rather than
    /// from the beginning.
    pub fn at(&self, t: f64) -> Pose {
        let total = self.total();
        if self.steps.is_empty() || !(total > 0.0) {
            return Pose::STILL;
        }
        // Before the start, nothing has happened yet.
        let mut left = if t <= 0.0 {
            0.0
        } else if self.looping && total.is_finite() {
            t % total
        } else {
            t
        };

        let mut so_far = Pose::STILL;
        for step in &self.steps {
            if step.seconds <= 0.0 {
                continue;
            }
            if left < step.seconds {
                return so_far.then(step.action.at(left));
            }
            // Finished. Take it at its end and carry the result forward.
            so_far = so_far.then(step.action.at(step.seconds));
            left -= step.seconds;
        }
        // Ran off the end of a non-looping act: hold the final pose.
        so_far
    }

    /// Which step is running at `t`, for showing on screen.
    pub fn step_at(&self, t: f64) -> Option<usize> {
        let total = self.total();
        if self.steps.is_empty() || !(total > 0.0) {
            return None;
        }
        let mut left = if t <= 0.0 {
            0.0
        } else if self.looping && total.is_finite() {
            t % total
        } else {
            t
        };
        for (k, step) in self.steps.iter().enumerate() {
            if step.seconds <= 0.0 {
                continue;
            }
            if left < step.seconds {
                return Some(k);
            }
            left -= step.seconds;
        }
        Some(self.steps.len() - 1)
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    /// Where the origin ends up under an act at `t`.
    fn where_to(act: &Act, t: f64) -> Cx {
        act.at(t).apply(Cx::ZERO)
    }

    /// ★ An action is numbers and a name, so it survives a file. `Motion` is a
    /// closure and cannot, which is the whole reason this type exists.
    #[test]
    fn every_action_can_be_written_down_and_read_back() {
        let all = [
            Action::Still,
            Action::Walk(Cx::new(1.5, -0.5)),
            Action::Run(Cx::new(-2.0, 0.25)),
            Action::Drift(Cx::new(0.1, 0.2)),
            Action::Jump { height: 1.4, rate: 1.2 },
            Action::Spin(0.75),
            Action::Bob { height: 0.3, rate: 2.0 },
            Action::Pulse { amount: 0.2, rate: 1.5 },
            Action::Orbit { r: 2.0, rate: 0.4 },
        ];
        for a in all {
            let (word, n) = a.spelt();
            assert_eq!(Action::spell(word, n), Some(a), "{word} did not come back");
        }
    }

    /// A word from a later version loses that step, not the file.
    #[test]
    fn an_unknown_action_is_refused_rather_than_guessed() {
        assert_eq!(Action::spell("cartwheel", [1.0, 2.0]), None);
    }

    /// ★ **The one with content in it.** Each step must start where the last
    /// one finished, or a walk that carried a figure three units to the right
    /// snaps back to the start the instant the jump begins.
    #[test]
    fn a_jump_starts_from_where_the_walk_got_to() {
        let act = Act::still().then(Action::Drift(Cx::new(1.0, 0.0)), 3.0).then(Action::Jump { height: 1.0, rate: 1.0 }, 2.0);

        let at_the_end_of_the_walk = where_to(&act, 2.999);
        assert!((at_the_end_of_the_walk.re - 3.0).abs() < 0.01, "it should have walked to 3: {at_the_end_of_the_walk:?}");

        let just_after = where_to(&act, 3.001);
        assert!((just_after.re - 3.0).abs() < 0.01, "and the jump must begin THERE, not at 0: {just_after:?}");
        assert!((just_after - at_the_end_of_the_walk).abs() < 0.05, "and without a jolt at the join");
    }

    /// And three steps compose the same way, so a sequence can be as long as
    /// you like without the joins accumulating error.
    #[test]
    fn a_long_sequence_keeps_adding_up() {
        let one = Cx::new(1.0, 0.0);
        let act = Act::still()
            .then(Action::Drift(one), 1.0)
            .then(Action::Drift(one), 1.0)
            .then(Action::Drift(one), 1.0)
            .then(Action::Still, 1.0);
        assert!((where_to(&act, 3.5).re - 3.0).abs() < 1e-9, "three seconds of drift is three units");
    }

    /// ★ A jump is **ballistic**, not a sine. It is fast off the ground and
    /// hangs at the top; a sine spends as long up as down and reads as
    /// floating, which is what `Bob` is for.
    #[test]
    fn a_jump_hangs_at_the_top_and_a_bob_does_not() {
        // How much of the hop is spent in the top quarter of its height.
        let hang = |a: Action, height: f64| {
            let hop = 1.0;
            let samples = 400;
            (0..samples)
                .filter(|k| {
                    let t = *k as f64 / samples as f64 * hop;
                    a.at(t).apply(Cx::ZERO).im > height * 0.75
                })
                .count()
        };
        let jumping = hang(Action::Jump { height: 1.0, rate: 1.0 }, 1.0);
        let bobbing = hang(Action::Bob { height: 1.0, rate: 1.0 }, 1.0);
        assert!(jumping > bobbing, "a jump should linger at the apex: {jumping} vs {bobbing}");
    }

    /// A jump reaches the height it was asked for, and comes back to the
    /// ground rather than sinking through it.
    #[test]
    fn a_jump_reaches_its_height_and_lands() {
        let a = Action::Jump { height: 1.5, rate: 2.0 };
        let hop = 0.5;
        let ys: Vec<f64> = (0..=200).map(|k| a.at(k as f64 / 200.0 * hop).apply(Cx::ZERO).im).collect();
        let top = ys.iter().fold(f64::MIN, |m, y| m.max(*y));
        assert!((top - 1.5).abs() < 0.02, "it reached {top}");
        assert!(ys[0].abs() < 1e-9, "it starts on the ground");
        assert!(ys[200].abs() < 0.02, "and comes back to it: {}", ys[200]);
        assert!(ys.iter().all(|y| *y > -1e-6), "and never goes below it");
    }

    /// ★ A looping act comes round again, and lands in the same place — an
    /// animation that drifted a little each time round would be visibly wrong
    /// after a minute and invisible in a test that only looked at one cycle.
    #[test]
    fn a_loop_returns_to_where_it_started() {
        let act = Act::still().then(Action::Spin(1.0), 2.0).then(Action::Bob { height: 1.0, rate: 0.5 }, 2.0).looping(true);
        let start = act.at(0.0);
        for turn in 1..6 {
            let round = act.at(4.0 * turn as f64);
            assert!((round.a - start.a).abs() < 1e-9, "cycle {turn} came back turned");
            assert!((round.b - start.b).abs() < 1e-9, "cycle {turn} came back displaced");
        }
    }

    /// Not looping means it **holds** its last pose rather than snapping back
    /// to the beginning or vanishing.
    #[test]
    fn an_act_that_does_not_loop_holds_its_last_pose() {
        let act = Act::still().then(Action::Drift(Cx::new(1.0, 0.0)), 2.0).looping(false);
        let ended = where_to(&act, 2.0);
        for t in [2.5, 10.0, 1000.0] {
            assert!((where_to(&act, t) - ended).abs() < 1e-9, "it moved on after finishing, at t={t}");
        }
    }

    /// ★ Before the clock starts, nothing has happened. A negative time
    /// arrives whenever a shape is placed while the animation is already
    /// running, and a jump evaluated at `t < 0` would otherwise start
    /// underground.
    #[test]
    fn nothing_has_happened_before_the_beginning() {
        let act = Act::still().then(Action::Jump { height: 1.0, rate: 1.0 }, 2.0).then(Action::Run(Cx::new(2.0, 0.0)), 2.0);
        for t in [-0.001, -1.0, -50.0] {
            assert!(where_to(&act, t).abs() < 1e-9, "at t={t} it had already moved");
        }
    }

    /// An empty act, and one made only of zero-length steps, are both simply
    /// still — rather than dividing by nothing.
    #[test]
    fn an_act_with_no_time_in_it_is_simply_still() {
        assert_eq!(Act::still().at(3.0), Pose::STILL);
        let empty = Act::still().then(Action::Run(Cx::new(9.0, 9.0)), 0.0);
        assert_eq!(empty.at(3.0), Pose::STILL);
        assert!(empty.is_still() || empty.total() <= 0.0);
        assert_eq!(empty.step_at(1.0), None);
    }

    /// One thing forever is the common case and must not need a duration
    /// invented for it.
    #[test]
    fn one_thing_forever_needs_no_duration() {
        let act = Act::just(Action::Spin(0.5));
        assert!((act.at(1.0).a - Cx::expi(TAU * 0.5)).abs() < 1e-9);
        assert_eq!(act.step_at(1_000.0), Some(0), "it is still doing it");
    }

    /// Which step is running is reported, so the studio can show it.
    #[test]
    fn it_says_which_step_is_running() {
        let act = Act::still().then(Action::Walk(Cx::ONE), 1.0).then(Action::Jump { height: 1.0, rate: 1.0 }, 1.0).looping(true);
        assert_eq!(act.step_at(0.5), Some(0));
        assert_eq!(act.step_at(1.5), Some(1));
        assert_eq!(act.step_at(2.5), Some(0), "and round again");
    }

    /// Walking and running go the same way, and running goes further —
    /// otherwise the two buttons do the same thing with different labels.
    #[test]
    fn running_covers_more_ground_than_walking() {
        let v = Cx::new(1.0, 0.0);
        let walked = where_to(&Act::just(Action::Walk(v)), 2.0).re;
        let ran = where_to(&Act::just(Action::Run(v)), 2.0).re;
        assert!(walked > 0.5, "a walk should get somewhere: {walked}");
        assert!(ran > walked * 2.0, "and a run should get further: {walked} vs {ran}");
    }
}
