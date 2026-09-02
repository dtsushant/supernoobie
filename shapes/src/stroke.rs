//! # stroke — a mark is a swept region, not a line
//!
//! ## The problem this solves
//!
//! Everything drawn so far is a polyline with one thickness: `Shape::path`
//! plus `.width(2)`, a single integer for the whole mark. Nothing about how it
//! was drawn can show, because there is nowhere for it to live. Every stroke
//! comes out the same weight from end to end, which is the difference between
//! a diagram and a drawing.
//!
//! A real mark is not a line. It is **the region a nib sweeps** as it travels
//! — the set of every place the nib covered, which is the Minkowski sum of the
//! path and the nib shape. Change the nib and the same path becomes a
//! different mark:
//!
//! ```text
//!     round nib, one size      a fat line, ends blunt
//!     round nib, varying       taper: anything you can measure becomes weight
//!     a line-segment nib       calligraphy
//! ```
//!
//! ## The pen has no pressure, and it does not need any
//!
//! No pressure and no tilt reach this program, and none ever will through the
//! window it uses. But a stroke knows four things about itself — **how fast**,
//! **which way**, **how sharply it is turning**, and **where it is along its
//! own length** — and every one of those is a legitimate source of weight.
//!
//! **How fast** is the surprising one, and it is free. The points arrive one
//! per frame, at a steady rate, so *the gap between consecutive points is the
//! speed*. No timing code and no clock: the frame rate does the measuring.
//! And thin-when-fast is not a stylistic choice, it is what a real brush does,
//! because a brush moving quickly has less time to give up its ink.
//!
//! ## Calligraphy comes out of the geometry, not out of a formula
//!
//! A broad nib is a short line segment held at a **fixed** angle. Sweeping one
//! along a path means offsetting the path by `±(w/2)·e^{iφ}` — the same
//! direction every time, regardless of where the stroke is going.
//!
//! And that is the whole of it. A stroke travelling **along** the nib's angle
//! has its two edges land on top of each other and disappears to a hairline; a
//! stroke crossing it at a right angle is full width. The classic
//! `w·|sin(θ_stroke − θ_nib)|` is never computed here — it *falls out*, because
//! that is what the projection of a fixed segment onto the perpendicular is.
//!
//! ## How the outline is built
//!
//! Down one side and back along the other, into a single closed loop:
//!
//! ```text
//!         +---- offset by +n·w/2 ---->
//!     start                           end
//!         <---- offset by -n·w/2 ----+
//! ```
//!
//! which is then filled with the even-odd rule by
//! [`Canvas::fill_poly`](plotkit::Canvas::fill_poly). One list of corners, one
//! polygon, no special cases.
//!
//! ## What is deliberately not here
//!
//! **Smoothing.** A shaky hand wants the ink to follow the pen on a spring,
//! and that is worth having — but it belongs to the *input*, not to the
//! geometry. By the time a stroke reaches this module it is a curve, and a
//! curve does not know whether a hand or a function made it.
//!
//! **Joins and caps.** Offsetting a sharp corner leaves the outer edge with a
//! notch. At the widths a pen actually draws with, the fill closes it and it
//! cannot be seen; at very wide settings it can. The proper fix is a round
//! join, which is a fan of points around the corner, and it can be added here
//! without anything else changing.
//!
//! **A stroke that doubles back inside its own width.** If the path reverses —
//! a hand jittering backwards, or a curve with a cusp — the two sides of the
//! outline cross, and the even-odd rule then reads the overlap as *outside*
//! and punches a hole in the mark.
//!
//! ```text
//!     ---->---->----+
//!                   |   the loop crosses itself, and even-odd
//!     <----<----<---+   cancels where it overlaps
//! ```
//!
//! That is even-odd behaving correctly rather than a mistake, and the fix is a
//! different rule: **nonzero winding**, which counts which *way* each crossing
//! goes instead of merely counting them, and so fills an overlap solidly. It
//! is a small change to
//! [`fill_poly`](plotkit::Canvas::fill_poly) — but the same rule would have to
//! go into [`Shape::contains`](plotkit::Shape::contains) at the same time, or
//! the paint and the clicking would stop agreeing about what is inside.

use plotkit::{Cx, Shape};

/// What is being dragged along the path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Nib {
    /// A round nib of one size. A fat line.
    Round(f64),
    /// A round nib that thins as the hand moves faster.
    ///
    /// `slow` is the width when barely moving, `fast` the width at full pelt,
    /// and `pace` the distance-per-frame at which "full pelt" is reached.
    /// Because the points arrive one per frame, distance per point *is* speed.
    Quill { slow: f64, fast: f64, pace: f64 },
    /// A line segment held at a fixed angle. Calligraphy.
    ///
    /// `angle` is in radians, measured the usual way. The apparent width is
    /// not computed — it falls out of sweeping a fixed segment.
    Broad { width: f64, angle: f64 },
}

/// A mark: where the nib went, and what the nib was.
#[derive(Clone, Debug)]
pub struct Stroke {
    pub pts: Vec<Cx>,
    pub nib: Nib,
    /// The fraction of the stroke over which it grows from nothing at each
    /// end. `0.0` leaves the ends blunt; `0.15` is a pen lifting off.
    ///
    /// Blunt ends read as machine-drawn. It is a small thing that does more
    /// for the look of a mark than almost anything else here.
    pub taper: f64,
}

impl Stroke {
    /// A stroke down a path, with a round nib of one size.
    pub fn new(pts: impl Into<Vec<Cx>>) -> Stroke {
        Stroke { pts: pts.into(), nib: Nib::Round(0.08), taper: 0.0 }
    }

    pub fn round(mut self, width: f64) -> Stroke {
        self.nib = Nib::Round(width.max(0.0));
        self
    }

    /// Thin when quick, heavy when slow — the closest thing to pressure that
    /// costs nothing.
    pub fn quill(mut self, slow: f64, fast: f64, pace: f64) -> Stroke {
        self.nib = Nib::Quill { slow: slow.max(0.0), fast: fast.max(0.0), pace: pace.max(1e-6) };
        self
    }

    /// A broad nib at a fixed angle.
    pub fn broad(mut self, width: f64, angle: f64) -> Stroke {
        self.nib = Nib::Broad { width: width.max(0.0), angle };
        self
    }

    /// Lift off at both ends, over this fraction of the length.
    pub fn taper(mut self, fraction: f64) -> Stroke {
        self.taper = fraction.clamp(0.0, 0.5);
        self
    }

    /// Does the path come back to exactly where it started?
    ///
    /// If it does there is no "end" anywhere on it, and the two neighbours of
    /// every point — including the first and the last — are found by going
    /// round.
    pub fn is_ring(&self) -> bool {
        self.pts.len() > 3 && (self.pts[0] - self.pts[self.pts.len() - 1]).abs() < 1e-9
    }

    /// The points either side of `k`, and how many steps apart they are.
    ///
    /// Central differences in the middle — `(next − previous)/2`, which is
    /// symmetric and so does not lean the estimate one way.
    ///
    /// At the two ends of an **open** stroke there is nothing on the far side
    /// to average with, so the difference is one-sided.
    ///
    /// On a **ring** there are no ends: the neighbours wrap round. Without
    /// that, the first and last points of a closed stroke get one-sided
    /// directions that do not quite agree, the outline is laid off at two
    /// slightly different angles where it joins, and the mark has a small tick
    /// sticking out of it at the seam. Very visible on a wide nib, and it
    /// looks like a bug in the fill rather than in the path — which is where
    /// I went looking first.
    fn neighbours(&self, k: usize) -> (Cx, Cx, f64) {
        let n = self.pts.len();
        if self.is_ring() {
            // `pts[n − 1]` is a copy of `pts[0]`, so the real ring is
            // `pts[0 .. n − 1]` and stepping wraps within that.
            let last = n - 1;
            let back = self.pts[(k + last - 1) % last];
            let forward = self.pts[(k + 1) % last];
            (back, forward, 2.0)
        } else {
            let back = self.pts[k.saturating_sub(1)];
            let forward = self.pts[(k + 1).min(n - 1)];
            let span = if k == 0 || k + 1 == n { 1.0 } else { 2.0 };
            (back, forward, span)
        }
    }

    /// The direction of travel at each point, as a unit complex number.
    pub fn headings(&self) -> Vec<Cx> {
        (0..self.pts.len())
            .map(|k| {
                let (back, forward, _) = self.neighbours(k);
                let d = forward - back;
                // A pen held still gives two identical points and no direction
                // at all. Carry on along the last known heading rather than
                // dividing by nothing.
                if d.abs() < 1e-12 {
                    Cx::new(1.0, 0.0)
                } else {
                    d.unit()
                }
            })
            .collect()
    }

    /// How fast the hand was moving at each point, in world units per frame.
    ///
    /// Which is simply how far apart the points are, because they arrive at a
    /// steady rate. The rate never appears in the arithmetic.
    pub fn pace(&self) -> Vec<f64> {
        (0..self.pts.len())
            .map(|k| {
                let (back, forward, span) = self.neighbours(k);
                (forward - back).abs() / span
            })
            .collect()
    }

    /// The half-width at each point, and which way it is laid off.
    ///
    /// Returned together because for a broad nib they are not independent: the
    /// offset is along the **nib**, not along the path's normal, and that is
    /// the entire difference between calligraphy and a fat line.
    fn offsets(&self) -> Vec<Cx> {
        let n = self.pts.len();
        if n == 0 {
            return Vec::new();
        }
        let headings = self.headings();
        let pace = self.pace();

        (0..n)
            .map(|k| {
                // How far along, 0 at the start and 1 at the end.
                let s = if n == 1 { 0.0 } else { k as f64 / (n - 1) as f64 };
                let lift = self.lift(s);

                match self.nib {
                    // The normal is the heading turned a quarter turn, which
                    // for a complex number is multiplying by i. No trigonometry
                    // and no special cases at the vertical.
                    Nib::Round(w) => (headings[k] * Cx::I).scale(w * lift / 2.0),
                    Nib::Quill { slow, fast, pace: full } => {
                        let hurry = (pace[k] / full).clamp(0.0, 1.0);
                        let w = slow + (fast - slow) * hurry;
                        (headings[k] * Cx::I).scale(w * lift / 2.0)
                    }
                    // The same direction at every point, whatever the stroke is
                    // doing. Everything calligraphic follows from that.
                    Nib::Broad { width, angle } => Cx::polar(width * lift / 2.0, angle),
                }
            })
            .collect()
    }

    /// How much of full width is in effect at `s` along the stroke.
    fn lift(&self, s: f64) -> f64 {
        if self.taper <= 0.0 {
            return 1.0;
        }
        let in_from_end = s.min(1.0 - s);
        (in_from_end / self.taper).clamp(0.0, 1.0)
    }

    /// The closed outline of the mark: down one side and back along the other.
    pub fn outline(&self) -> Vec<Cx> {
        let offsets = self.offsets();
        if offsets.len() < 2 {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(offsets.len() * 2);
        for (k, o) in offsets.iter().enumerate() {
            out.push(self.pts[k] + *o);
        }
        for (k, o) in offsets.iter().enumerate().rev() {
            out.push(self.pts[k] - *o);
        }
        out
    }

    /// The mark, ready to be added to a frame and **filled**.
    ///
    /// ```text
    ///     f.add(stroke.shape()).color(INK).fill();
    /// ```
    ///
    /// Without `.fill()` you get the outline traced, which is a useful thing to
    /// look at while working out why a stroke is the wrong shape.
    pub fn shape(&self) -> Shape {
        Shape::polygon(self.outline())
    }

    /// The widest the mark gets — for laying things out without rasterising.
    pub fn widest(&self) -> f64 {
        self.offsets().iter().map(|o| o.abs() * 2.0).fold(0.0, f64::max)
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{PI, TAU};

    /// A straight run of `n` points along the real axis, `step` apart.
    fn straight(n: usize, step: f64) -> Vec<Cx> {
        (0..n).map(|k| Cx::new(k as f64 * step, 0.0)).collect()
    }

    /// The height of the outline at its middle — the mark's actual width there.
    fn thickness_at_middle(s: &Stroke) -> f64 {
        let o = s.offsets();
        o[o.len() / 2].abs() * 2.0
    }

    /// ★ A round nib lays the mark off along the **normal**, so a horizontal
    /// stroke comes out with vertical thickness. Getting this backwards gives
    /// a mark that is zero wide, which looks exactly like nothing being drawn
    /// at all.
    #[test]
    fn a_round_nib_lays_the_width_across_the_stroke() {
        let s = Stroke::new(straight(9, 1.0)).round(0.4);
        let out = s.outline();
        assert_eq!(out.len(), 18, "down one side and back along the other");

        let tall = out.iter().map(|z| z.im).fold(0.0f64, |a, b| a.max(b.abs()));
        let long = out.iter().map(|z| z.re).fold(0.0f64, |a, b| a.max(b));
        assert!((tall - 0.2).abs() < 1e-9, "half of 0.4 either side, not {tall}");
        assert!((long - 8.0).abs() < 1e-9, "and it should still be as long as the path");
    }

    /// ★ **Thin when quick.** The points arrive one per frame, so the gap
    /// between them IS the speed — no clock is consulted anywhere.
    #[test]
    fn moving_faster_makes_a_thinner_mark() {
        let nib = |s: Stroke| s.quill(0.5, 0.05, 1.0);
        let dawdle = nib(Stroke::new(straight(9, 0.1)));
        let dash = nib(Stroke::new(straight(9, 1.0)));

        assert!(thickness_at_middle(&dawdle) > thickness_at_middle(&dash) * 3.0, "a flick should be much thinner");
        assert!((thickness_at_middle(&dash) - 0.05).abs() < 1e-9, "at full pelt it is the fast width");
    }

    /// And it varies **within** one stroke, which is the whole point — a mark
    /// that slows into a corner should get heavier there.
    #[test]
    fn one_stroke_can_be_heavy_in_one_place_and_light_in_another() {
        // Fast for the first half, then almost stopping.
        let mut pts = straight(6, 1.0);
        let tail = pts.last().copied().unwrap_or(Cx::ZERO);
        for k in 1..6 {
            pts.push(tail + Cx::new(k as f64 * 0.05, 0.0));
        }
        let s = Stroke::new(pts).quill(0.5, 0.05, 1.0);
        let widths: Vec<f64> = s.offsets().iter().map(|o| o.abs() * 2.0).collect();
        assert!(widths[2] < 0.1, "the quick part should be thin: {}", widths[2]);
        assert!(widths[9] > 0.3, "and the slow part heavy: {}", widths[9]);
    }

    /// ★ Calligraphy, and nothing computes a sine. A broad nib offsets by the
    /// SAME vector everywhere, so a stroke running along the nib's angle has
    /// its two edges land on each other and vanishes to a hairline, while one
    /// crossing it is full width. That is the geometry doing the work.
    #[test]
    fn a_broad_nib_is_thin_along_its_own_angle_and_fat_across_it() {
        let width = |heading: f64| {
            let pts: Vec<Cx> = (0..9).map(|k| Cx::polar(k as f64, heading)).collect();
            let s = Stroke::new(pts).broad(0.6, 0.0); // nib lies along the real axis
            let out = s.outline();
            // Thickness measured across the direction of travel.
            let across = Cx::polar(1.0, heading) * Cx::I;
            let projected: Vec<f64> = out.iter().map(|z| z.re * across.re + z.im * across.im).collect();
            let (lo, hi) = projected.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(*p), b.max(*p)));
            hi - lo
        };

        let along = width(0.0);
        let across = width(PI / 2.0);
        assert!(along < 1e-9, "along the nib it should vanish, not be {along}");
        assert!((across - 0.6).abs() < 1e-9, "across the nib it should be full width, not {across}");

        let diagonal = width(PI / 4.0);
        // |sin(45 deg)| = 0.7071, and nothing in the code ever wrote that down.
        assert!((diagonal - 0.6 * (PI / 4.0).sin()).abs() < 1e-9, "the |sin| should fall out: {diagonal}");
    }

    /// ★ Taper: the mark should come to nothing at each end. Blunt ends are
    /// the single clearest tell that something was drawn by a computer.
    #[test]
    fn a_tapered_stroke_comes_to_a_point_at_both_ends() {
        let s = Stroke::new(straight(21, 0.5)).round(0.4).taper(0.2);
        let w: Vec<f64> = s.offsets().iter().map(|o| o.abs() * 2.0).collect();

        assert!(w[0] < 1e-9, "it should start at nothing");
        assert!(w[w.len() - 1] < 1e-9, "and end at nothing");
        assert!((w[w.len() / 2] - 0.4).abs() < 1e-9, "and be full width in the middle");
        assert!(w[2] > w[1] && w[1] > w[0], "growing on the way in");
    }

    /// Without a taper the ends stay blunt, so it is a choice rather than
    /// something that happens to every mark.
    #[test]
    fn without_a_taper_the_ends_are_blunt() {
        let s = Stroke::new(straight(9, 0.5)).round(0.4);
        let w: Vec<f64> = s.offsets().iter().map(|o| o.abs() * 2.0).collect();
        assert!((w[0] - 0.4).abs() < 1e-9);
        assert!((w[w.len() - 1] - 0.4).abs() < 1e-9);
    }

    /// ★ A pen held perfectly still gives two identical points, and a
    /// direction of nothing. That must not become a `NaN` — one `NaN` corner
    /// poisons the whole polygon and the mark disappears, which is a horrible
    /// thing to debug because it only happens when you pause.
    #[test]
    fn a_pen_held_still_does_not_poison_the_mark() {
        let stuck = vec![Cx::new(1.0, 1.0); 6];
        let s = Stroke::new(stuck).round(0.3);
        for z in s.outline() {
            assert!(z.re.is_finite() && z.im.is_finite(), "got {z:?}");
        }

        // And a stroke that stops halfway and starts again.
        let mut pts = straight(4, 0.5);
        pts.extend(vec![Cx::new(1.5, 0.0); 4]);
        pts.extend((1..4).map(|k| Cx::new(1.5 + k as f64 * 0.5, 0.0)));
        for z in Stroke::new(pts).quill(0.4, 0.05, 1.0).outline() {
            assert!(z.re.is_finite() && z.im.is_finite());
        }
    }

    /// One click is not a stroke. It must come back as nothing rather than as
    /// a degenerate polygon or a panic — a tap is an ordinary thing to do.
    #[test]
    fn a_single_point_is_not_a_mark() {
        assert!(Stroke::new(vec![Cx::new(1.0, 1.0)]).outline().is_empty());
        assert!(Stroke::new(Vec::new()).outline().is_empty());
    }

    /// ★ The outline is a closed loop that goes down one side and back along
    /// the other, so the two halves must mirror: the k-th point out and the
    /// k-th point back straddle the centreline evenly. If they did not, the
    /// mark would be laid to one side of where the pen actually went.
    #[test]
    fn the_mark_is_centred_on_where_the_pen_went() {
        let curve: Vec<Cx> = (0..25).map(|k| Cx::polar(2.0, k as f64 * 0.2)).collect();
        let s = Stroke::new(curve.clone()).round(0.3);
        let out = s.outline();
        let n = curve.len();
        for k in 0..n {
            let middle = (out[k] + out[out.len() - 1 - k]).scale(0.5);
            assert!((middle - curve[k]).abs() < 1e-9, "point {k} drifted to {middle:?}");
        }
    }

    /// ★ A closed stroke has **no seam**. Its neighbours wrap round, so the
    /// first and last points get the same direction and the outline joins up
    /// exactly. Without that they get one-sided estimates that do not quite
    /// agree, and the mark has a small tick sticking out of it where it
    /// closes -- very visible on a wide nib, and it looks like a bug in the
    /// fill rather than in the path.
    #[test]
    fn a_closed_stroke_joins_up_without_a_tick() {
        let mut pts: Vec<Cx> = (0..60).map(|k| Cx::polar(2.0, k as f64 / 60.0 * TAU)).collect();
        pts.push(pts[0]);
        let s = Stroke::new(pts).round(0.4);
        assert!(s.is_ring());

        let h = s.headings();
        let first = h[0];
        let last = h[h.len() - 1];
        assert!((first - last).abs() < 1e-9, "the seam is laid off at two angles: {first:?} vs {last:?}");

        // And the direction at the seam agrees with its neighbours, rather
        // than being a spike between them.
        assert!((h[0] - h[1]).abs() < 0.2, "the heading should turn smoothly through the join");

        let out = s.outline();
        let n = s.pts.len();
        assert!((out[0] - out[n - 1]).abs() < 1e-9, "the two sides should meet exactly");
    }

    /// And an open stroke keeps its one-sided ends, because it genuinely has
    /// ends -- wrapping there would bend the start of a stroke towards its
    /// finish, which may be right across the page.
    #[test]
    fn an_open_stroke_still_has_ends() {
        let s = Stroke::new(straight(9, 1.0)).round(0.4);
        assert!(!s.is_ring());
        let h = s.headings();
        assert!((h[0] - Cx::new(1.0, 0.0)).abs() < 1e-9, "straight along the path");

        // A path whose ends merely happen to be near each other is not a ring
        // unless they are the SAME point.
        let mut nearly: Vec<Cx> = (0..30).map(|k| Cx::polar(2.0, k as f64 / 30.0 * TAU * 0.97)).collect();
        nearly.push(nearly[0] + Cx::new(0.01, 0.0));
        assert!(!Stroke::new(nearly).round(0.2).is_ring());
    }

    /// A mark turning a corner keeps its width round the bend, rather than
    /// pinching or ballooning where the direction changes fastest.
    #[test]
    fn the_width_survives_a_corner() {
        let mut pts: Vec<Cx> = (0..8).map(|k| Cx::new(k as f64 * 0.4, 0.0)).collect();
        pts.extend((1..8).map(|k| Cx::new(2.8, k as f64 * 0.4)));
        let s = Stroke::new(pts).round(0.5);
        for (k, o) in s.offsets().iter().enumerate() {
            assert!((o.abs() * 2.0 - 0.5).abs() < 1e-9, "point {k} came out {} wide", o.abs() * 2.0);
        }
    }
}
