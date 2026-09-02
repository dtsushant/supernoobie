//! # trigger — turning motion into events
//!
//! ## The problem
//!
//! A ball has no "bounce". It has a height, which goes down and then goes up,
//! and *the turn* is the bounce. A tree has no "creak" — it has a branch
//! moving, and a creak is that movement passing some strain. Nothing in the
//! physics announces itself; there are only numbers changing.
//!
//! So something has to watch a number and say **when something happened to
//! it**, and how hard. That is all this is, and it is what lets a sound, a
//! flash or a score be bound to a motion rather than to a manual call.
//!
//! ```text
//!     a number, every frame   ->   [ Trigger ]   ->   sometimes: "now, this hard"
//! ```
//!
//! ## The two ways something happens
//!
//! ```text
//!     Rising(level)   it crossed a line going up      strain, a limit reached
//!     Falling(level)  crossed it going down
//!     Turning         it stopped and went back        a bounce, the top of a swing
//! ```
//!
//! `Turning` is the interesting one, because it needs no threshold at all: a
//! bounce *is* the velocity changing sign, whatever height it happens at.
//!
//! ## Why it is not just `if x > level`
//!
//! Because that is true on every frame afterwards. A ball resting on the floor
//! would bounce sixty times a second, which is a buzz rather than a bounce.
//!
//! Two things stop that, and they are the whole content of this module:
//!
//! * **Edges, not levels.** It fires on the frame the crossing happens, once.
//! * **Hysteresis.** Having fired, it will not fire again until the value has
//!   gone properly back the other way. Without it, a number sitting exactly on
//!   the line and jittering by a hair fires every other frame — and the jitter
//!   is always there, because floating point.
//!
//! The strength comes back too, because *how hard* is usually the point: a
//! ball dropped from a table should not sound like one nudged off a step.

/// What counts as something happening.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Edge {
    /// Crossed `level` going up.
    Rising(f64),
    /// Crossed `level` going down.
    Falling(f64),
    /// Stopped and went back the other way. A bounce.
    Turning,
}

/// Watches a number and reports when something happens to it.
#[derive(Clone, Copy, Debug)]
pub struct Trigger {
    pub edge: Edge,
    /// How far back the value must come before it can fire again.
    ///
    /// The guard against a number sitting on the line and jittering. For
    /// `Turning`, the smallest speed that counts as a real turn rather than
    /// noise.
    pub slack: f64,
    last: Option<f64>,
    speed: f64,
    armed: bool,
}

impl Trigger {
    pub const fn new(edge: Edge, slack: f64) -> Trigger {
        Trigger { edge, slack, last: None, speed: 0.0, armed: true }
    }

    /// Fires when the value crosses `level` on the way up.
    pub const fn rising(level: f64, slack: f64) -> Trigger {
        Trigger::new(Edge::Rising(level), slack)
    }

    /// Fires when the value crosses `level` on the way down.
    pub const fn falling(level: f64, slack: f64) -> Trigger {
        Trigger::new(Edge::Falling(level), slack)
    }

    /// Fires when the value turns round — a bounce, whatever height it happens
    /// at. `slack` is the slowest turn that still counts.
    pub const fn turning(slack: f64) -> Trigger {
        Trigger::new(Edge::Turning, slack)
    }

    /// Show it this frame's value.
    ///
    /// `Some(strength)` on the frame something happened, and `None` otherwise.
    /// Strength is how fast the value was moving — which is what makes a hard
    /// landing sound different from a gentle one.
    pub fn saw(&mut self, x: f64) -> Option<f64> {
        let Some(last) = self.last else {
            // Nothing to compare against yet. A trigger cannot know that the
            // very first number it is shown is a crossing.
            self.last = Some(x);
            return None;
        };
        let v = x - last;
        let was = self.speed;
        self.last = Some(x);
        self.speed = v;

        match self.edge {
            Edge::Rising(level) => {
                if self.armed && last < level && x >= level {
                    self.armed = false;
                    return Some(v.abs());
                }
                // Only ready again once it has gone properly back below.
                if x < level - self.slack {
                    self.armed = true;
                }
                None
            }
            Edge::Falling(level) => {
                if self.armed && last > level && x <= level {
                    self.armed = false;
                    return Some(v.abs());
                }
                if x > level + self.slack {
                    self.armed = true;
                }
                None
            }
            Edge::Turning => {
                // It was going one way and now goes the other. The strength is
                // the speed it had BEFORE the turn — the speed it arrived
                // with, which is what a bounce is loud in proportion to.
                let turned = was * v < 0.0 && was.abs() >= self.slack;
                turned.then_some(was.abs())
            }
        }
    }

    /// Forget everything seen so far, without changing what it watches for.
    pub fn reset(&mut self) {
        self.last = None;
        self.speed = 0.0;
        self.armed = true;
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// Feed it a series and collect the frames it fired on.
    fn fires(t: &mut Trigger, xs: &[f64]) -> Vec<usize> {
        xs.iter().enumerate().filter_map(|(k, x)| t.saw(*x).map(|_| k)).collect()
    }

    /// ★ It fires on the crossing, once — not on every frame afterwards. The
    /// naive `if x > level` is true forever after, which turns a landing into
    /// a sixty-a-second buzz.
    #[test]
    fn it_fires_on_the_crossing_and_not_after() {
        let mut t = Trigger::rising(1.0, 0.1);
        let climb: Vec<f64> = (0..40).map(|k| k as f64 * 0.1).collect();
        assert_eq!(fires(&mut t, &climb), vec![10], "once, on the frame it crossed");
    }

    /// ★ And having fired it stays quiet until the value has come properly
    /// back. Without that slack, a number resting exactly on the line and
    /// jittering by a hair fires every other frame — and the jitter is always
    /// there, because floating point.
    #[test]
    fn a_value_sitting_on_the_line_does_not_chatter() {
        let mut t = Trigger::rising(1.0, 0.2);
        let mut jitter = vec![0.0, 1.0]; // one real crossing
        for k in 0..200 {
            // Hovering within a hair of the line, above and below.
            jitter.push(1.0 + if k % 2 == 0 { 1e-9 } else { -1e-9 });
        }
        assert_eq!(fires(&mut t, &jitter).len(), 1, "one crossing, one firing");
    }

    /// It rearms once the value really has gone back, so a genuine second
    /// crossing is heard.
    #[test]
    fn a_real_second_crossing_fires_again() {
        let mut t = Trigger::rising(1.0, 0.2);
        let there_and_back = [0.0, 1.5, 0.5, 1.5, 0.5, 1.5];
        assert_eq!(fires(&mut t, &there_and_back).len(), 3);
    }

    /// ★ A bounce is the velocity changing sign, at whatever height it
    /// happens. No threshold to pick, and none to get wrong.
    #[test]
    fn turning_catches_a_bounce_wherever_it_happens() {
        let mut t = Trigger::turning(0.01);
        // Down, then up: the turn is between frames 3 and 4.
        let bounce = [3.0, 2.0, 1.0, 0.0, 1.0, 2.0, 3.0];
        assert_eq!(fires(&mut t, &bounce), vec![4]);

        // The same bounce a long way off the ground still counts.
        let mut high = Trigger::turning(0.01);
        let up_high = [103.0, 102.0, 101.0, 100.0, 101.0, 102.0];
        assert_eq!(fires(&mut high, &up_high), vec![4]);
    }

    /// ★ And the strength is the speed it arrived with, which is what makes a
    /// ball dropped from a table sound unlike one nudged off a step.
    #[test]
    fn how_hard_it_hit_comes_back_with_it() {
        let hardness = |drop: f64| {
            let mut t = Trigger::turning(0.001);
            let path = [drop * 3.0, drop * 2.0, drop, 0.0, drop];
            path.iter().find_map(|x| t.saw(*x)).expect("it bounced")
        };
        assert!(hardness(2.0) > hardness(0.5), "a faster arrival should report harder");
        assert!((hardness(1.0) - 1.0).abs() < 1e-9, "and report the actual speed");
    }

    /// Slow wobble is not a bounce. Without a floor on the speed, a value
    /// drifting about fires constantly and every quiet moment is full of
    /// clicks.
    #[test]
    fn a_slow_wobble_is_not_a_bounce() {
        let mut t = Trigger::turning(0.5);
        let drift: Vec<f64> = (0..200).map(|k| (k as f64 * 0.3).sin() * 0.2).collect();
        assert!(fires(&mut t, &drift).is_empty(), "it should ignore a gentle drift");

        // But a real one still gets through.
        let mut same = Trigger::turning(0.5);
        assert!(!fires(&mut same, &[5.0, 3.0, 1.0, 3.0, 5.0]).is_empty());
    }

    /// ★ The very first value cannot be a crossing — there is nothing to have
    /// crossed from. A trigger that fired on its first sample would make every
    /// object shout the moment it appeared.
    #[test]
    fn nothing_happens_on_the_first_frame() {
        for mut t in [Trigger::rising(1.0, 0.1), Trigger::falling(1.0, 0.1), Trigger::turning(0.01)] {
            assert_eq!(t.saw(99.0), None, "the first sample must be silent");
        }
    }

    /// Falling is the mirror of rising, and neither fires on the other's
    /// crossing.
    #[test]
    fn falling_is_the_mirror_of_rising() {
        let mut down = Trigger::falling(1.0, 0.2);
        assert_eq!(fires(&mut down, &[2.0, 0.5, 2.0, 0.5]).len(), 2);

        let mut up = Trigger::rising(1.0, 0.2);
        assert!(fires(&mut up, &[2.0, 1.5, 1.2]).is_empty(), "already above: nothing was crossed");
    }

    /// Reset forgets the past but keeps the job — for when a thing is put back
    /// where it started.
    #[test]
    fn reset_forgets_the_past_but_not_the_purpose() {
        let mut t = Trigger::rising(1.0, 0.2);
        let _ = fires(&mut t, &[0.0, 2.0]);
        t.reset();
        assert_eq!(t.edge, Edge::Rising(1.0));
        assert_eq!(t.saw(0.0), None, "and the first sample after a reset is silent again");
        assert!(t.saw(2.0).is_some(), "then it works as before");
    }
}
