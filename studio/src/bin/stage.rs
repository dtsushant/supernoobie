//! # stage — a group that moves as one, whose members still do their own thing
//!
//! ```text
//!     cargo run -p studio --release --bin stage
//! ```
//!
//! Five discs in a [`Troupe`]. The whole group walks, runs, spins or orbits —
//! and every disc is still individually draggable while it moves. Drag a rim to
//! resize one, drag a middle to move one, and the group carries on regardless.
//!
//! ```text
//!   arrows   which way: x, x', y, y', or any diagonal
//!   1        stand still            Q   spin anticlockwise
//!   2        walk                   E   spin clockwise
//!   3        run                    R   reset
//!   4        orbit
//!   5        walk and spin at once
//!
//!   drag a rim to resize one   drag a middle to move one   G graph paper
//! ```
//!
//! ## Direction is one complex number
//!
//! There is no `walk_left`, `walk_up`, `walk_down`. The arrow keys hand back a
//! direction as a `Cx` — right is `1`, up is `i` — and it goes straight into
//! `Motion::walk(dir)`. Diagonals work without anyone having thought about
//! them, because `1 + i` is a perfectly ordinary direction.
//!
//! ## What this file is for
//!
//! Nothing here knows any geometry, any hit-testing, or how a drag differs from
//! a tap. All of that is in [`shapes`], which has no window and no `main`, so
//! it can be used from this file, from `sketch`, from a test, or from a file
//! you write tomorrow. This is the demonstration that it composes.
//!
//! **A `Troupe` is an `Actor`, and so are its members** — the same trait — so a
//! group nests inside a group and everything that takes one takes the other.
//! That is what makes "a group of shapes is itself a shape" true rather than
//! merely convenient.

use studio::prelude::*;

const PALETTE: [u32; 5] = [0x4FBCD4, 0xE0A44A, 0xE585AC, 0x6FCF97, 0x9B7BD4];

/// What the group is doing. The direction is kept separately, because walking
/// left and walking right are the same *mode* pointed a different way.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Still,
    Walk,
    Run,
    Orbit,
    WalkAndSpin,
    SpinCcw,
    SpinCw,
}

impl Mode {
    /// The motion this mode makes, pointed in `dir`.
    fn motion(self, dir: Cx) -> Motion {
        match self {
            Mode::Still => Motion::still(),
            Mode::Walk => Motion::walk(dir),
            Mode::Run => Motion::run(dir),
            // An orbit starts off in `dir`, so the arrows still mean something.
            Mode::Orbit => Motion::orbit(3.0, 0.2).then(Motion::of(move |_| Pose::new(dir.unit(), Cx::ZERO))),
            // Two motions, one value. This is the whole point of `then`.
            Mode::WalkAndSpin => Motion::walk(dir).then(Motion::spin(0.2)),
            Mode::SpinCcw => Motion::spin(0.15),
            Mode::SpinCw => Motion::spin(-0.15),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Mode::Still => "still",
            Mode::Walk => "walk(dir) - travel, plus the bob that makes it read as walking",
            Mode::Run => "run(dir) - the same walk, 3x the speed and bouncier",
            Mode::Orbit => "orbit(3, 0.2) - going round without turning",
            Mode::WalkAndSpin => "walk(dir).then(spin(0.2)) - one motion, not two",
            Mode::SpinCcw => "spin(0.15) - anticlockwise, the way e^(it) turns",
            Mode::SpinCw => "spin(-0.15) - clockwise",
        }
    }

    /// Does the direction mean anything here? A spin does not care which way
    /// the arrows point, and saying so keeps the readout honest.
    fn uses_direction(self) -> bool {
        matches!(self, Mode::Walk | Mode::Run | Mode::Orbit | Mode::WalkAndSpin)
    }
}

struct Stage {
    troupe: Troupe,
    mode: Mode,
    /// Which way, as a unit complex number. Right is `1`, up is `i`.
    dir: Cx,
}

impl Stage {
    /// A fresh stage, with its clock already set to `now`.
    ///
    /// The `now` matters. A new troupe starts at zero, and if the graph then
    /// hands it the wall clock a `travel` motion would leap to
    /// `velocity x elapsed` — which is exactly what made reset look broken.
    fn fresh(now: f64) -> Stage {
        let mut troupe = ring();
        troupe.tick(now);
        // Spinning rather than walking, so it stays on screen until asked to
        // go somewhere.
        let mut s = Stage { troupe, mode: Mode::SpinCcw, dir: Cx::new(1.0, 0.0) };
        s.retune();
        s
    }

    /// Rebuild the motion from the mode and the direction.
    ///
    /// `set_motion` starts the new motion's clock now and keeps whatever the
    /// old one had already travelled, so changing direction mid-walk carries on
    /// from where the group got to instead of flinging it back to the start.
    fn retune(&mut self) {
        self.troupe.set_motion(self.mode.motion(self.dir));
    }

    /// Only retune when something actually changed. Retuning every frame would
    /// restart the clock every frame, and the group would never get anywhere.
    fn point(&mut self, d: Cx) {
        if d != Cx::ZERO && (d.unit() - self.dir).abs() > 1e-9 {
            self.dir = d.unit();
            self.retune();
        }
    }

    fn set(&mut self, m: Mode) {
        if m != self.mode {
            self.mode = m;
            self.retune();
        }
    }
}

/// Five discs in a ring — the ring is the fifth roots of unity, scaled.
fn ring() -> Troupe {
    (0..5).fold(Troupe::new(), |t, k| {
        let at = Cx::expi(TAU * k as f64 / 5.0).scale(3.0);
        t.and(Disc::new(at, 0.8).grip_width(0.28))
    })
}

fn main() {
    Graph::new("STAGE  -  a group that moves as one")
        .scale(46.0)
        .with(Stage::fresh(0.0))
        // The troupe needs the clock before anything asks where it is.
        .each_frame(|s, t| s.troupe.tick(t))
        .on_pointer(|s, at, down| s.troupe.drag(at, down))
        // The arrows hand back a direction as one complex number, so there is
        // no left/right/up/down anywhere — and diagonals work for free.
        .on_arrows(|s, dir| s.point(dir))
        .on_digit(|s, d| {
            if let Some(m) =
                [Mode::Still, Mode::Walk, Mode::Run, Mode::Orbit, Mode::WalkAndSpin].get(d.wrapping_sub(1) as usize)
            {
                s.set(*m);
            }
        })
        .on('q', |s| s.set(Mode::SpinCcw))
        .on('e', |s| s.set(Mode::SpinCw))
        .on('r', |s| *s = Stage::fresh(s.troupe.now()))
        .run(scene);
}

fn scene(s: &Stage) -> Frame {
    let mut f = Frame::new();

    // `parts()` gives each member on its own, already carried by the group's
    // motion, so they can have their own colours. `troupe.shape()` would hand
    // back the whole group as one shape — the same geometry, the other view.
    for (k, part) in s.troupe.parts().into_iter().enumerate() {
        let held = s.troupe.holding() == Some(k);
        f.add(part).color(PALETTE[k % PALETTE.len()]).width(if held { 4 } else { 2 }).dot(if held { 5.0 } else { 3.0 });
    }

    // The direction, drawn as the arrow it is.
    if s.mode.uses_direction() {
        let from = s.troupe.pose().b;
        let tip = from + s.dir.scale(5.2);
        f.add(Shape::path(vec![from, tip])).color(0x6B7987).width(2);
        f.add(Shape::point(tip)).color(0x6B7987).dot(6.0);
    }

    f.label(Cx::new(0.0, 6.4), s.mode.name(), 0x9AA7B4, 2);
    if s.mode.uses_direction() {
        f.label(Cx::new(0.0, 5.6), format!("dir = {:.2} + {:.2}i", s.dir.re, s.dir.im), 0x5A6774, 2);
    }
    f.label(Cx::new(0.0, -6.1), "arrows aim it   drag a rim to resize, a middle to move", 0x5A6774, 2);
    f.label(Cx::new(0.0, -6.9), "1 still  2 walk  3 run  4 orbit  5 both  Q/E spin  R reset", 0x46525E, 2);
    f
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn centre_of(s: &Shape) -> Cx {
        let pts: Vec<Cx> =
            s.polylines(Cx::new(-90.0, -90.0), Cx::new(90.0, 90.0), 400).into_iter().flatten().collect();
        pts.iter().fold(Cx::ZERO, |a, p| a + *p).scale(1.0 / pts.len() as f64)
    }

    /// ★ The bug this file had: switching to a walk on an absolute clock put
    /// the group `velocity x elapsed` away instantly, which for any real
    /// session is far off screen. Nothing was misbehaving about walking — it
    /// had already walked, before you asked it to.
    #[test]
    fn choosing_a_walk_late_does_not_fling_it_off_screen() {
        let mut s = Stage::fresh(0.0);
        s.troupe.tick(45.0); // three quarters of a minute of fiddling about
        s.set(Mode::Walk); // and only now ask it to walk

        let where_ = centre_of(&s.troupe.parts()[0]);
        assert!(where_.abs() < 6.0, "it jumped to {where_:?} the moment walking was chosen");
    }

    /// And reset really resets, whatever the clock says.
    #[test]
    fn reset_brings_it_home() {
        let mut s = Stage::fresh(0.0);
        s.set(Mode::Run);
        s.troupe.tick(20.0);
        let far = centre_of(&s.troupe.parts()[0]);
        assert!(far.abs() > 8.0, "it should have run a long way by now: {far:?}");

        // Reset at the same wall clock the app is really at — the case that
        // was broken, because a fresh troupe starts its own clock at zero.
        let now = s.troupe.now();
        s = Stage::fresh(now);
        s.troupe.tick(now + 2.0);
        assert!(centre_of(&s.troupe.parts()[0]).abs() < 6.0, "reset should put it back near the middle");
    }

    /// ★ Walking works in every direction, because a direction is one complex
    /// number and `walk` takes it whole. No left/right/up/down anywhere.
    #[test]
    fn it_walks_whichever_way_it_is_pointed() {
        for (name, dir) in [
            ("x", Cx::new(1.0, 0.0)),
            ("x'", Cx::new(-1.0, 0.0)),
            ("y", Cx::new(0.0, 1.0)),
            ("y'", Cx::new(0.0, -1.0)),
            ("diagonal", Cx::new(1.0, 1.0)),
        ] {
            let mut s = Stage::fresh(0.0);
            s.point(dir);
            s.set(Mode::Walk);
            let start = centre_of(&s.troupe.parts()[0]);
            s.troupe.tick(3.0);
            let went = centre_of(&s.troupe.parts()[0]) - start;

            assert!(went.abs() > 2.0, "walking {name} went nowhere");
            // It went the way it was pointed: the two agree in direction.
            assert!(went.unit().dot(dir.unit()) > 0.97, "walking {name} went {went:?} instead");
        }
    }

    /// Running covers more ground than walking, in whatever direction.
    #[test]
    fn running_outpaces_walking() {
        let go = |m: Mode| {
            let mut s = Stage::fresh(0.0);
            s.point(Cx::new(0.0, 1.0));
            s.set(m);
            let start = centre_of(&s.troupe.parts()[0]);
            s.troupe.tick(2.0);
            (centre_of(&s.troupe.parts()[0]) - start).abs()
        };
        assert!(go(Mode::Run) > go(Mode::Walk) * 2.0);
    }

    /// ★ Retuning restarts the motion's clock, so it must only happen when
    /// something changed. Retuning every frame would leave the group forever at
    /// `t = 0`, looking frozen while apparently walking.
    #[test]
    fn pointing_the_same_way_again_does_not_restart_the_walk() {
        let mut s = Stage::fresh(0.0);
        s.set(Mode::Walk);
        let start = centre_of(&s.troupe.parts()[0]);

        for k in 1..=30 {
            s.troupe.tick(k as f64 * 0.1);
            s.point(Cx::new(1.0, 0.0)); // the same direction, every frame
            s.set(Mode::Walk); // and the same mode
        }
        let went = (centre_of(&s.troupe.parts()[0]) - start).abs();
        assert!(went > 2.0, "it only got {went} — the clock is being restarted every frame");
    }

    /// Changing direction mid-walk carries on from where it got to rather than
    /// flinging everything back to the start.
    #[test]
    fn turning_a_corner_does_not_teleport() {
        let mut s = Stage::fresh(0.0);
        s.set(Mode::Walk);
        s.troupe.tick(3.0);
        let corner = centre_of(&s.troupe.parts()[0]);

        s.point(Cx::new(0.0, 1.0));
        assert!((centre_of(&s.troupe.parts()[0]) - corner).abs() < 1e-9, "it jumped when it turned");

        s.troupe.tick(6.0);
        let end = centre_of(&s.troupe.parts()[0]);
        assert!(end.im > corner.im + 1.0, "it should have gone up from the corner");
        assert!((end.re - corner.re).abs() < 0.5, "and not sideways any more");
    }

    #[test]
    fn spinning_both_ways_goes_both_ways() {
        let turn = |m: Mode| {
            let mut s = Stage::fresh(0.0);
            s.set(m);
            s.troupe.tick(1.0);
            s.troupe.pose().a.arg()
        };
        assert!(turn(Mode::SpinCcw) > 0.0, "Q should turn anticlockwise");
        assert!(turn(Mode::SpinCw) < 0.0, "E should turn clockwise");
    }

    /// A spin ignores the arrows, and the readout says so rather than showing a
    /// direction that does nothing.
    #[test]
    fn only_the_modes_that_use_a_direction_claim_to() {
        assert!(Mode::Walk.uses_direction() && Mode::Run.uses_direction());
        assert!(!Mode::SpinCw.uses_direction() && !Mode::Still.uses_direction());
    }

    #[test]
    fn the_ring_is_five_discs() {
        assert_eq!(ring().len(), 5);
    }

    /// A member can still be resized while the group is moving — which needs
    /// the pointer taken back through the group's pose, the right member
    /// picked, and the drag delivered, all three.
    #[test]
    fn a_member_can_be_resized_while_the_group_moves() {
        let mut t = Troupe::new().and(Disc::new(Cx::new(3.0, 0.0), 1.0)).moving(Motion::travel(Cx::new(10.0, 0.0)));
        t.tick(1.0); // the group is 10 to the right, so the disc is at 13

        assert!(t.hit(Cx::new(13.0, 0.0)), "reachable where it now is");
        t.drag(Cx::new(14.0, 0.0), true); // grab its rim
        assert_eq!(t.holding(), Some(0));
        t.drag(Cx::new(15.0, 0.0), true); // pull it out by one

        let pts: Vec<Cx> =
            t.parts()[0].polylines(Cx::new(-40.0, -40.0), Cx::new(40.0, 40.0), 400).into_iter().flatten().collect();
        let far = pts.iter().fold(0.0f64, |m, p| m.max((*p - Cx::new(13.0, 0.0)).abs()));
        assert!((far - 2.0).abs() < 0.05, "expected radius 2, drew {far}");
    }
}

