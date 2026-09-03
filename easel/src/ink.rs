//! # ink — the stroke while it is still being made
//!
//! A finished [`Mark`] is a list of points. Getting there from a pen is not
//! quite nothing, and this is the three things in between.
//!
//! ```text
//!     pen position, every frame
//!            |
//!            v
//!     [ the spring ]   the ink follows the pen on a lead
//!            |
//!            v
//!     [ the sieve  ]   points closer than a hair are dropped
//!            |
//!            v
//!     [ the loop   ]   ending near the start means you meant a closed shape
//!            |
//!            v
//!          Mark
//! ```
//!
//! ## The spring, and why it is here rather than in `shapes`
//!
//! A hand shakes, a touchscreen quantises, and a pen sampled once a frame
//! gives a run of points with a visible tremble in it. The fix every serious
//! drawing program uses is the same one: **the pen leads and the ink follows
//! on a lead**, catching up rather than teleporting.
//!
//! ```text
//!     ink += (pen − ink) · pull        pull in 0..1, per frame
//! ```
//!
//! That single line is the same `e^{−t/τ}` as everything else in this
//! repository — a branch settling, a note dying away, a raindrop reaching
//! terminal velocity. *"The rate of change is proportional to how far there is
//! left to go."* Here the thing it is approaching happens to be your hand.
//!
//! It belongs to the **input**, not to the geometry, which is why it is in
//! this crate and not in [`shapes::Stroke`]. By the time a stroke is a curve
//! it no longer knows whether a hand or a function made it, and it should not
//! have to.
//!
//! ## What the spring costs, and why the ends are the price
//!
//! Following on a lead means **lagging**, so the ink arrives at the end of a
//! stroke a little after the pen does. Left alone, every stroke would stop
//! short of where you lifted off. So a stroke is **run out** when it finishes:
//! the spring is stepped a few more times with the pen held at its last
//! position, and the ink catches up. Without that, a hard `pull` looks fine
//! while drawing and every mark is visibly short.
//!
//! ## The sieve
//!
//! A pen resting still still reports a position every frame, which is sixty
//! identical points a second. They add nothing, they make files large, and
//! they give the [`Quill`](shapes::Nib::Quill) nib a stream of zero-length
//! steps to read as "stopped". Anything closer than a hair to the last point
//! kept is dropped.

use plotkit::Cx;
use shapes::Nib;

use crate::mark::Mark;

/// How hard the ink is pulled towards the pen, per frame.
///
/// `1.0` is no smoothing at all — the ink is the pen. Around `0.35` takes a
/// tremble out while still feeling attached to your hand; below `0.15` it
/// starts to feel like drawing through treacle, which some people like for
/// long sweeping curves.
pub const PULL: f64 = 0.35;

/// Points closer together than this are the same point.
pub const HAIR: f64 = 0.004;

/// How near the start you must finish for the mark to be treated as closed,
/// as a fraction of how far the stroke travelled.
///
/// Relative, not absolute: ending within a centimetre means something quite
/// different on a stroke a centimetre long and one a metre long.
pub const LOOP_BACK: f64 = 0.08;

/// A stroke in progress.
#[derive(Clone, Debug)]
pub struct Ink {
    /// Where the ink has got to — behind the pen, catching up.
    at: Option<Cx>,
    /// The points kept so far.
    pts: Vec<Cx>,
    pub pull: f64,
    pub nib: Nib,
    pub taper: f64,
    pub colour: u32,
}

impl Ink {
    pub fn new(nib: Nib, colour: u32) -> Ink {
        Ink { at: None, pts: Vec::new(), pull: PULL, nib, taper: 0.0, colour }
    }

    pub fn with_pull(mut self, pull: f64) -> Ink {
        self.pull = pull.clamp(0.02, 1.0);
        self
    }

    pub fn with_taper(mut self, taper: f64) -> Ink {
        self.taper = taper.clamp(0.0, 0.5);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.pts.is_empty()
    }

    pub fn len(&self) -> usize {
        self.pts.len()
    }

    pub fn points(&self) -> &[Cx] {
        &self.pts
    }

    /// One frame of the pen being somewhere.
    ///
    /// The first sample starts the ink **at the pen** rather than at the
    /// origin — otherwise every stroke begins with a streak in from wherever
    /// the last one ended, or from nowhere.
    pub fn sample(&mut self, pen: Cx) {
        let at = match self.at {
            None => pen,
            Some(ink) => ink + (pen - ink).scale(self.pull),
        };
        self.at = Some(at);
        self.keep(at);
    }

    /// Add a point unless it is on top of the last one.
    fn keep(&mut self, z: Cx) {
        match self.pts.last() {
            Some(last) if (z - *last).abs() < HAIR => {}
            _ => self.pts.push(z),
        }
    }

    /// Let the ink catch up with where the pen stopped.
    ///
    /// Without this a stroke ends short of where you lifted off, by roughly
    /// the lag the spring introduces — which is small, constant, and maddening
    /// once you have noticed it.
    fn run_out(&mut self, pen: Cx) {
        let Some(mut at) = self.at else { return };
        // `(1 − pull)^k` has to get the remaining gap under a hair. At the
        // default pull that takes about a dozen steps; at the heaviest
        // allowed it takes a few hundred, which is why the bound is generous.
        // It is a bound rather than a `while` because this runs at the exact
        // moment somebody lifts the pen, and a loop that could fail to
        // terminate there would hang the window.
        for _ in 0..1_000 {
            if (pen - at).abs() < HAIR {
                break;
            }
            at = at + (pen - at).scale(self.pull);
            self.keep(at);
        }
        self.at = Some(at);
    }

    /// The pen lifted. Give back the mark, if there is one.
    ///
    /// `None` for a tap: fewer than two points is not a stroke, and returning
    /// a one-point mark would leave invisible specks all over the drawing that
    /// can still be clicked on.
    pub fn lift(mut self, pen: Cx) -> Option<Mark> {
        self.run_out(pen);
        if self.pts.len() < 2 {
            return None;
        }
        let closed = self.looped();
        let mut pts = std::mem::take(&mut self.pts);
        if closed {
            // The last point is a duplicate of the first, near enough, and a
            // closed mark sweeps round to its start by itself.
            pts.pop();
        }
        Some(Mark { pts, nib: self.nib, taper: self.taper, colour: self.colour, filled: true, closed, act: crate::Act::still(), track: crate::Track::new(), place: None, spin: None, group: 0 })
    }

    /// Did the stroke come back to where it started?
    ///
    /// Two conditions, and the second one is the interesting one.
    ///
    /// **It finished near its start** — judged against how far it travelled,
    /// not against a fixed distance, because ending within a centimetre means
    /// something quite different on a short stroke and a long one.
    ///
    /// **It went round something.** Ending where you began is not enough: a
    /// line scribbled back and forth ends exactly where it started and is not
    /// a loop at all. What tells them apart is **enclosed area**, by the
    /// shoelace sum — every step contributes the triangle it sweeps out about
    /// the origin, and on a there-and-back the two passes sweep opposite
    /// triangles and cancel to nothing.
    ///
    /// Compared against `travelled²`, which makes it a question of *shape*
    /// rather than of size: a circle scores `1/4π ≈ 0.0796` whatever its
    /// radius, and a scribble scores about zero whatever its length. So a
    /// deliberate loop the size of a full stop still closes, and a long
    /// scribble still does not.
    fn looped(&self) -> bool {
        let (Some(first), Some(last)) = (self.pts.first(), self.pts.last()) else {
            return false;
        };
        let travelled: f64 = self.pts.windows(2).map(|w| (w[1] - w[0]).abs()).sum();
        if travelled <= HAIR * 40.0 || (*last - *first).abs() >= travelled * LOOP_BACK {
            return false;
        }
        let swept: f64 = self.pts.windows(2).map(|w| w[0].cross(w[1])).sum::<f64>() + last.cross(*first);
        // An eighth of a circle's roundness: generous to a lopsided hand-drawn
        // loop, and nowhere near enough for a there-and-back.
        (swept / 2.0).abs() > travelled * travelled * 0.01
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    /// Draw a path with a given hand, one sample per frame.
    fn drawn(path: &[Cx], pull: f64) -> Option<Mark> {
        let mut ink = Ink::new(Nib::Round(0.2), 0xFFFFFF).with_pull(pull);
        for z in path {
            ink.sample(*z);
        }
        ink.lift(*path.last().expect("a path"))
    }

    /// A straight run with a tremble on it, like a real hand.
    fn shaky_line(n: usize, shake: f64) -> Vec<Cx> {
        (0..n)
            .map(|k| {
                let x = k as f64 * 0.1;
                // Fast and small: a tremble, not a wobble in the drawing.
                Cx::new(x, shake * (k as f64 * 2.1).sin())
            })
            .collect()
    }

    /// How far the points stray from the line the hand was trying to draw.
    ///
    /// Measured against `y = 0`, which is where `shaky_line` actually goes,
    /// rather than against the chord between the first and last points — the
    /// last point lands wherever the shake happened to be at that instant, so
    /// the chord is tilted and a tilt would be read as tremble.
    ///
    /// And measured over the middle **half** of the stroke. The ends are
    /// deliberately not smooth: a stroke starts exactly where the pen went
    /// down and is run out to exactly where it lifted, shake and all, because
    /// ending in the wrong place is worse than ending roughly. The run-out
    /// alone can be a dozen points long, all of them converging on whatever
    /// the shake was doing at the instant the pen came up.
    fn tremble(m: &Mark) -> f64 {
        let n = m.pts.len();
        m.pts[n / 4..n * 3 / 4].iter().map(|z| z.im.abs()).fold(0.0, f64::max)
    }

    /// ★ The spring takes the shake out. This is the single biggest
    /// improvement to how a pen feels, and it is one line of arithmetic — the
    /// same `e^{−t/τ}` as a branch settling or a note dying away.
    #[test]
    fn the_ink_follows_on_a_lead_and_the_tremble_goes() {
        let path = shaky_line(60, 0.05);
        let raw = drawn(&path, 1.0).expect("a mark");
        let smoothed = drawn(&path, 0.25).expect("a mark");

        assert!(tremble(&raw) > 0.04, "with no spring the shake is all there: {}", tremble(&raw));
        assert!(
            tremble(&smoothed) < tremble(&raw) / 4.0,
            "the spring should take most of it: {} -> {}",
            tremble(&raw),
            tremble(&smoothed)
        );
    }

    /// ★ And the price of a lead is lag, so the stroke must be **run out** at
    /// the lift or every mark stops short of where you lifted off. Small,
    /// constant, and maddening once noticed.
    #[test]
    fn a_stroke_ends_where_the_pen_lifted() {
        let path: Vec<Cx> = (0..40).map(|k| Cx::new(k as f64 * 0.25, 0.0)).collect();
        let end = *path.last().expect("a path");
        for pull in [1.0, 0.5, 0.25, 0.08] {
            let m = drawn(&path, pull).expect("a mark");
            let stopped = *m.pts.last().expect("points");
            assert!((stopped - end).abs() < HAIR * 2.0, "at pull {pull} it stopped {stopped:?}, short of {end:?}");
        }
    }

    /// ★ And it **starts** where the pen went down. If the ink began at the
    /// origin and sprang towards the pen, every stroke would open with a
    /// streak in from the middle of the page.
    #[test]
    fn a_stroke_starts_where_the_pen_went_down() {
        let start = Cx::new(7.0, -3.0);
        let path: Vec<Cx> = (0..20).map(|k| start + Cx::new(k as f64 * 0.1, 0.0)).collect();
        let m = drawn(&path, 0.2).expect("a mark");
        assert!((m.pts[0] - start).abs() < 1e-9, "it began at {:?}", m.pts[0]);
    }

    /// ★ A pen resting still reports sixty identical points a second. They add
    /// nothing, they make files large, and they give the quill nib a stream of
    /// zero-length steps to read as "stopped".
    #[test]
    fn a_resting_pen_does_not_fill_the_stroke_with_nothing() {
        let mut ink = Ink::new(Nib::Round(0.2), 0xFFFFFF);
        for _ in 0..600 {
            ink.sample(Cx::new(1.0, 1.0));
        }
        assert!(ink.len() <= 2, "ten seconds of holding still kept {} points", ink.len());
    }

    /// ★ Ending near where you started means you meant a closed shape, and
    /// that is judged against **how far you travelled** — finishing within a
    /// centimetre means something quite different on a short stroke and a long
    /// one.
    #[test]
    fn a_stroke_that_comes_back_to_its_start_is_closed() {
        let circle: Vec<Cx> = (0..80).map(|k| Cx::polar(2.0, k as f64 / 80.0 * TAU)).collect();
        assert!(drawn(&circle, 1.0).expect("a mark").closed, "a ring should close");

        let line: Vec<Cx> = (0..40).map(|k| Cx::new(k as f64 * 0.2, 0.0)).collect();
        assert!(!drawn(&line, 1.0).expect("a mark").closed, "a line should not");

        // A long stroke that ends a good way from its start is open, even
        // though the gap is bigger than a whole short stroke.
        let hook: Vec<Cx> = (0..120).map(|k| Cx::polar(6.0, k as f64 / 120.0 * TAU * 0.75)).collect();
        assert!(!drawn(&hook, 1.0).expect("a mark").closed, "three quarters of a circle is not a circle");
    }

    /// ★ Ending where you began is not enough. A line scribbled back and forth
    /// finishes exactly at its start and is not a loop at all — what tells it
    /// from a circle is that it **went round nothing**, and the shoelace sum
    /// says so because the two passes sweep opposite triangles and cancel.
    #[test]
    fn a_line_scribbled_back_and_forth_is_not_a_loop() {
        let mut scribble = Vec::new();
        for _ in 0..8 {
            scribble.extend((0..12).map(|k| Cx::new(k as f64 * 0.1, 0.0)));
            scribble.extend((0..12).rev().map(|k| Cx::new(k as f64 * 0.1, 0.0)));
        }
        let m = drawn(&scribble, 1.0).expect("a mark");
        assert!(!m.closed, "there and back many times encloses nothing");
    }

    /// And the test is about **shape**, not size, so a deliberate loop the
    /// size of a full stop still closes.
    #[test]
    fn a_tiny_deliberate_loop_still_closes() {
        let tiny: Vec<Cx> = (0..40).map(|k| Cx::polar(0.05, k as f64 / 40.0 * TAU)).collect();
        assert!(drawn(&tiny, 1.0).expect("a mark").closed, "a small circle is still a circle");
    }

    /// ★ A tap is not a stroke. A one-point mark is invisible and can still be
    /// clicked on, so a page full of accidental taps becomes a page full of
    /// things that grab the pointer for no reason.
    #[test]
    fn a_tap_makes_no_mark() {
        let mut ink = Ink::new(Nib::Round(0.2), 0xFFFFFF);
        ink.sample(Cx::new(1.0, 1.0));
        assert!(ink.lift(Cx::new(1.0, 1.0)).is_none());

        assert!(Ink::new(Nib::Round(0.2), 0xFFFFFF).lift(Cx::ZERO).is_none(), "and neither does nothing at all");
    }

    /// The mark carries the nib and colour it was drawn with, so changing the
    /// tool afterwards does not reach back and change what is already down.
    #[test]
    fn a_finished_mark_keeps_the_tool_it_was_drawn_with() {
        let mut ink = Ink::new(Nib::Broad { width: 0.5, angle: 0.4 }, 0x00FF00).with_taper(0.2);
        for k in 0..20 {
            ink.sample(Cx::new(k as f64 * 0.2, 0.0));
        }
        let m = ink.lift(Cx::new(3.8, 0.0)).expect("a mark");
        assert_eq!(m.nib, Nib::Broad { width: 0.5, angle: 0.4 });
        assert_eq!(m.colour, 0x00FF00);
        assert!((m.taper - 0.2).abs() < 1e-9);
        assert!(m.filled, "a drawn stroke is a filled region, not a hairline");
    }

    /// ★ A very heavy spring must still finish. `run_out` steps until the ink
    /// arrives, and a loop with no bound would hang the window at the exact
    /// moment somebody lifts the pen.
    #[test]
    fn even_the_heaviest_spring_finishes_lifting() {
        let path: Vec<Cx> = (0..30).map(|k| Cx::new(k as f64, 0.0)).collect();
        let m = drawn(&path, 0.02).expect("a mark");
        assert!(m.pts.len() > 2);
        assert!(m.pts.iter().all(|z| z.re.is_finite() && z.im.is_finite()));
    }
}
