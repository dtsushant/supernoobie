//! # mark — one thing on the page, and the only thing that is ever saved
//!
//! ## Why a `Shape` cannot be the saved unit
//!
//! [`plotkit::Shape`] holds **closures** — `Param { f }`, `Graph { f }`,
//! `Mapped(_, f)`. A closure is compiled code. It has no representation you
//! can write to a file and nothing you can read back, so a drawing made of
//! `Shape`s could be looked at and never reopened.
//!
//! A [`Mark`] is the opposite: **numbers only**. Where the pen went, what nib
//! it was, what colour. From those a real `Shape` is built on demand, and it
//! can then be placed, scaled, mapped, grouped and given a motion exactly like
//! one written in code — but the file only ever contains numbers.
//!
//! ```text
//!     Mark  --- numbers, saved ---->  file
//!       |
//!       +--- shape() --->  Shape  --- closures, never saved --->  pixels
//! ```
//!
//! ## Points are the truth, and the dial makes them fewer
//!
//! A mark keeps **the points the hand actually made**. It would be tempting to
//! store Fourier coefficients instead, since a stroke needs perhaps twenty of
//! those against five hundred points — but then every mark carries two
//! representations that can disagree, and the one the hand made would be the
//! one thrown away.
//!
//! Instead the two dials *replace* the points with a shorter run rebuilt from
//! the transform, and the file gets smaller as a side effect rather than as a
//! scheme. One representation, one truth.
//!
//! The two are easy to confuse and are not the same operation:
//!
//! ```text
//!     smooth(n)     keep every wave up to pitch n   -> the shake gone, however big
//!     simplify(n)   keep the n LOUDEST waves        -> the smallest file for the look
//! ```
//!
//! [`Series`] sorts its terms by size, so a budget keeps a large wobble
//! precisely because it is large. It is the filter that takes a shake out,
//! because a shake is *fast* rather than *big*.

use plotkit::{Cx, Shape};
use shapes::fourier::Series;
use shapes::{Nib, Pose, Stroke};

use crate::action::Act;

/// One mark on the page.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    /// Where the pen went, in world coordinates.
    pub pts: Vec<Cx>,
    pub nib: Nib,
    pub taper: f64,
    pub colour: u32,
    /// Paint the swept region, or trace the centreline.
    ///
    /// A drawn stroke is filled. A construction line — an axis, a guide — is
    /// not, and wants to stay one pixel wide however far you zoom.
    pub filled: bool,
    /// Whether the ends join up. A closed mark can be simplified by
    /// [`Series`], and an open one cannot be, honestly.
    pub closed: bool,
    /// What it does when the clock is running. Numbers, like everything else
    /// here, so an animation is a few extra words in the file.
    pub act: Act,
    /// Which group it belongs to. `0` is none.
    ///
    /// A number rather than a tree of parents, because a figure is a handful
    /// of strokes that move together and nothing more. A tree would need
    /// re-parenting, cycle checks and a way to write nesting into the file,
    /// all to express something nobody has asked for yet. When nesting is
    /// genuinely needed this becomes a path, and the file gains one word.
    pub group: u32,
}

impl Mark {
    /// A stroke as the pen made it.
    pub fn new(pts: impl Into<Vec<Cx>>, nib: Nib, colour: u32) -> Mark {
        Mark { pts: pts.into(), nib, taper: 0.0, colour, filled: true, closed: false, act: Act::still(), group: 0 }
    }

    pub fn taper(mut self, f: f64) -> Mark {
        self.taper = f.clamp(0.0, 0.5);
        self
    }

    pub fn closed(mut self, yes: bool) -> Mark {
        self.closed = yes;
        self
    }

    pub fn outlined(mut self) -> Mark {
        self.filled = false;
        self
    }

    /// The mark as a stroke, ready to be turned into an outline.
    pub fn stroke(&self) -> Stroke {
        let mut pts = self.pts.clone();
        // A closed mark is swept right back to its start, or it has a notch
        // where the two ends nearly meet.
        if self.closed && pts.len() > 2 {
            pts.push(pts[0]);
        }
        Stroke { pts, nib: self.nib, taper: self.taper }
    }

    /// What to draw.
    ///
    /// Filled marks come back as their **outline** — the region the nib swept
    /// — and unfilled ones as the centreline, because that is what an
    /// unfilled mark means.
    pub fn shape(&self) -> Shape {
        if self.filled {
            self.stroke().shape()
        } else if self.closed {
            Shape::polygon(self.pts.clone())
        } else {
            Shape::path(self.pts.clone())
        }
    }

    /// Give it something to do.
    pub fn doing(mut self, act: Act) -> Mark {
        self.act = act;
        self
    }

    /// The middle of the mark, which is what it turns and grows about.
    ///
    /// The centre of its bounding box, not the average of its points. A
    /// hand-drawn stroke has its points bunched wherever the hand was slow, so
    /// the average sits nearer the dawdling end — and a shape that spun about
    /// a point slightly off its middle would wobble like a buckled wheel.
    pub fn anchor(&self) -> Cx {
        match self.bounds() {
            Some((lo, hi)) => (lo + hi).scale(0.5),
            None => Cx::ZERO,
        }
    }

    /// The mark as it looks under a pose — **turned and grown about its own
    /// middle**, and moved wherever the pose says.
    ///
    /// The `about` part is the whole of it. A pose from
    /// [`Motion::spin`](shapes::Motion::spin) turns about the **origin**, so a
    /// figure drawn off to one side would not spin at all — it would be flung
    /// round the middle of the page on the end of a rope. Turning about its
    /// own middle is what "spin" means to anybody watching.
    pub fn posed(&self, pose: Pose) -> Shape {
        let here = self.anchor();
        self.shape().map(move |z| pose.apply(z - here) + here)
    }

    /// What it looks like `t` seconds in.
    pub fn at(&self, t: f64) -> Shape {
        self.posed(self.act.at(t))
    }

    /// Everything the mark covers, for hit testing and for framing the page.
    pub fn bounds(&self) -> Option<(Cx, Cx)> {
        let pts = if self.filled { self.stroke().outline() } else { self.pts.clone() };
        let mut it = pts.into_iter();
        let first = it.next()?;
        Some(it.fold((first, first), |(lo, hi), z| {
            (Cx::new(lo.re.min(z.re), lo.im.min(z.im)), Cx::new(hi.re.max(z.re), hi.im.max(z.im)))
        }))
    }

    /// Move it.
    pub fn shifted(&self, by: Cx) -> Mark {
        Mark { pts: self.pts.iter().map(|z| *z + by).collect(), ..self.clone() }
    }

    /// Scale and turn it about a point.
    ///
    /// `by` is a complex number, so one multiplication does both: its modulus
    /// is the scale and its argument is the turn. That is the whole reason
    /// this repository works in the complex plane.
    pub fn mapped(&self, about: Cx, by: Cx) -> Mark {
        let pts = self.pts.iter().map(|z| about + (*z - about) * by).collect();
        // The nib scales with the drawing, or a shrunk mark becomes a blob.
        let k = by.abs();
        let nib = match self.nib {
            Nib::Round(w) => Nib::Round(w * k),
            Nib::Quill { slow, fast, pace } => Nib::Quill { slow: slow * k, fast: fast * k, pace: pace * k },
            // The nib turns with the drawing too, which is what makes a
            // rotated piece of calligraphy still look written rather than
            // sheared.
            Nib::Broad { width, angle } => Nib::Broad { width: width * k, angle: angle + by.arg() },
        };
        Mark { pts, nib, ..self.clone() }
    }

    /// How long the centreline is, walked end to end.
    pub fn length(&self) -> f64 {
        self.pts.windows(2).map(|w| (w[1] - w[0]).abs()).sum()
    }

    /// **Keep only the `n` biggest waves.** A budget, not a filter.
    ///
    /// [`Series`] sorts its terms by size, so this is the best picture
    /// obtainable from `n` terms — which makes it the right thing for making a
    /// drawing cheap to store, and the *wrong* thing for taking a shake out.
    /// A large wobble is a large term, and a budget keeps large terms.
    ///
    /// For the shake, use [`smooth`](Mark::smooth). The two are easy to
    /// confuse and do genuinely different things:
    ///
    /// ```text
    ///     simplify(n)   the n LOUDEST waves       -> smallest file for the look
    ///     smooth(n)     every wave up to pitch n  -> the shake gone, whatever its size
    /// ```
    ///
    /// Only honest on a **closed** mark; see [`smooth`](Mark::smooth).
    pub fn simplify(&self, n: usize) -> Mark {
        self.rebuilt(n * 8, |series, theta| series.at(n, theta))
    }

    /// **The dial between your hand and the ideal.** Drop every wave above
    /// pitch `cut`.
    ///
    /// A low-pass filter, and the thing people mean by smoothing. A shake is
    /// *fast* — it is a high harmonic — and this removes it however large it
    /// is, where a budget would keep it precisely because it is large. Turn
    /// the dial down far enough and a hand-drawn circle becomes a true one,
    /// because a circle is the single harmonic `n = 1`.
    ///
    /// Only honest on a **closed** mark. [`Series`] treats its input as one
    /// period of a periodic function, so an open stroke acquires a jump from
    /// its last point back to its first — and a jump's coefficients fall off
    /// as only `1/n`, so the reconstruction rings along the whole length
    /// instead of smoothing it. An open mark is left alone rather than wrecked.
    pub fn smooth(&self, cut: usize) -> Mark {
        let keep = cut as i32;
        self.rebuilt(cut * 10, |series, theta| {
            series
                .terms
                .iter()
                .filter(|(f, _)| f.abs() <= keep)
                .fold(Cx::ZERO, |acc, (f, c)| acc + *c * Cx::expi(*f as f64 * theta))
        })
    }

    /// Shared by both dials: transform, resynthesise, and refuse politely.
    fn rebuilt(&self, want: usize, sum: impl Fn(&Series, f64) -> Cx) -> Mark {
        if !self.closed || self.pts.len() < 8 || want == 0 {
            return self.clone();
        }
        let series = Series::of(&self.pts, self.pts.len().min(512));
        let samples = want.clamp(32, self.pts.len());
        let pts = (0..samples)
            .map(|k| sum(&series, k as f64 / samples as f64 * std::f64::consts::TAU))
            .collect::<Vec<_>>();
        Mark { pts, ..self.clone() }
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    fn ring(n: usize, r: f64, wobble: f64) -> Vec<Cx> {
        (0..n)
            .map(|k| {
                let th = k as f64 / n as f64 * TAU;
                // A wobble in a HIGH harmonic, which is what a shaky hand puts
                // in and what the dial should take out.
                Cx::polar(r + wobble * (th * 11.0).sin(), th)
            })
            .collect()
    }

    /// ★ The saved unit is numbers only. If a `Shape` — which holds closures —
    /// ever became the thing stored, a drawing could be looked at and never
    /// reopened.
    #[test]
    fn a_mark_is_numbers_and_a_shape_is_built_from_it() {
        let m = Mark::new(ring(40, 2.0, 0.0), Nib::Round(0.2), 0xFFFFFF).closed(true);
        let same = m.clone();
        assert_eq!(m, same, "a mark can be compared, copied and therefore written down");
        assert!(!m.shape().polylines(Cx::new(-9.0, -9.0), Cx::new(9.0, 9.0), 400).is_empty());
    }

    /// ★ The dial: fewer harmonics, less wobble. The shake lives in the high
    /// harmonics and the circle lives in the low ones, which is the same fact
    /// as coefficient decay measuring smoothness.
    #[test]
    fn turning_the_dial_down_takes_the_shake_out() {
        let shaky = Mark::new(ring(200, 2.0, 0.12), Nib::Round(0.1), 0xFFFFFF).closed(true);

        // How far the radius wanders from its average: the wobble, measured.
        let roughness = |m: &Mark| {
            let rs: Vec<f64> = m.pts.iter().map(|z| z.abs()).collect();
            let mean = rs.iter().sum::<f64>() / rs.len() as f64;
            rs.iter().map(|r| (r - mean).abs()).fold(0.0, f64::max)
        };

        let rough = roughness(&shaky);
        let smoothed = roughness(&shaky.smooth(3));
        assert!(rough > 0.1, "the shaky one should be shaky: {rough}");
        assert!(smoothed < rough / 4.0, "and cutting above pitch 3 should take it out: {rough} -> {smoothed}");
    }

    /// ★ And the distinction I had glossed over: a **budget** is not a
    /// filter. `Series` sorts its terms by size, so keeping the three biggest
    /// waves keeps a big wobble -- it is big. Keeping everything up to pitch
    /// three throws the wobble away however big it is, because it is fast.
    ///
    /// Both are useful and they are not the same operation.
    #[test]
    fn a_budget_keeps_a_loud_shake_and_a_filter_does_not() {
        let shaky = Mark::new(ring(200, 2.0, 0.12), Nib::Round(0.1), 0xFFFFFF).closed(true);
        let roughness = |m: &Mark| {
            let rs: Vec<f64> = m.pts.iter().map(|z| z.abs()).collect();
            let mean = rs.iter().sum::<f64>() / rs.len() as f64;
            rs.iter().map(|r| (r - mean).abs()).fold(0.0, f64::max)
        };
        // The circle is one term and the wobble is two more, so a budget of
        // three spends it all on keeping the wobble.
        assert!(roughness(&shaky.simplify(3)) > 0.08, "a budget keeps what is loud");
        assert!(roughness(&shaky.smooth(3)) < 0.03, "a filter drops what is fast");
    }

    /// And it is still the same circle — the dial smooths, it does not shrink.
    /// A simplification that quietly changed the size would be useless.
    #[test]
    fn the_dial_keeps_the_shape_it_was_given() {
        let shaky = Mark::new(ring(200, 2.0, 0.12), Nib::Round(0.1), 0xFFFFFF).closed(true);
        let radius = |m: &Mark| m.pts.iter().map(|z| z.abs()).sum::<f64>() / m.pts.len() as f64;
        assert!((radius(&shaky.smooth(4)) - 2.0).abs() < 0.05, "it should still be radius 2");
    }

    /// ★ It gets **smaller**, which is the point: the smoothing and the
    /// compression are the same act rather than two schemes that can disagree.
    #[test]
    fn simplifying_makes_the_mark_smaller_to_store() {
        let long = Mark::new(ring(400, 2.0, 0.05), Nib::Round(0.1), 0xFFFFFF).closed(true);
        assert!(long.simplify(6).pts.len() < long.pts.len() / 4, "it should cost far less to keep");
    }

    /// ★ An OPEN mark is left alone. `Series` treats its input as one period
    /// of a periodic function, so an open stroke acquires a jump from its last
    /// point back to its first — and a jump's coefficients fall off as only
    /// 1/n, so the reconstruction rings along the whole length instead of
    /// smoothing it. Better to refuse than to wreck the stroke.
    #[test]
    fn an_open_stroke_is_not_simplified() {
        let open = Mark::new(ring(200, 2.0, 0.1)[..120].to_vec(), Nib::Round(0.1), 0xFFFFFF);
        assert_eq!(open.simplify(4), open, "a budget should refuse it");
        assert_eq!(open.smooth(4), open, "and so should a filter");
    }

    /// ★ Scaling scales the nib too. Without that, shrinking a drawing turns
    /// every mark in it into a blob, because the strokes keep their old width
    /// while the picture gets small.
    #[test]
    fn shrinking_a_mark_shrinks_its_nib() {
        let m = Mark::new(ring(40, 2.0, 0.0), Nib::Round(0.4), 0xFFFFFF).closed(true);
        let half = m.mapped(Cx::ZERO, Cx::new(0.5, 0.0));
        assert_eq!(half.nib, Nib::Round(0.2));

        let (lo, hi) = half.bounds().expect("it has bounds");
        assert!((hi.re - lo.re - 2.2).abs() < 0.05, "radius 1 plus a 0.2 nib: {}", hi.re - lo.re);
    }

    /// ★ And a broad nib **turns** with the drawing. If the angle stayed put,
    /// rotating a piece of calligraphy would shear it — the thick parts would
    /// stay pointing the same way while the letters turned under them.
    #[test]
    fn rotating_calligraphy_turns_the_nib_with_it() {
        let m = Mark::new(ring(40, 2.0, 0.0), Nib::Broad { width: 0.4, angle: 0.0 }, 0xFFFFFF).closed(true);
        let quarter = m.mapped(Cx::ZERO, Cx::polar(1.0, TAU / 4.0));
        match quarter.nib {
            Nib::Broad { width, angle } => {
                assert!((width - 0.4).abs() < 1e-9, "the width should not change");
                assert!((angle - TAU / 4.0).abs() < 1e-9, "but the angle should follow: {angle}");
            }
            other => panic!("the nib changed kind: {other:?}"),
        }
    }

    /// Moving does not change anything but position — not the width, not the
    /// colour, not the nib angle.
    #[test]
    fn moving_a_mark_changes_only_where_it_is() {
        let m = Mark::new(ring(30, 1.0, 0.0), Nib::Broad { width: 0.3, angle: 0.7 }, 0x123456).closed(true);
        let there = m.shifted(Cx::new(5.0, -2.0));
        assert_eq!(there.nib, m.nib);
        assert_eq!(there.colour, m.colour);
        let (lo, _) = there.bounds().expect("bounds");
        let (was, _) = m.bounds().expect("bounds");
        assert!((lo - was - Cx::new(5.0, -2.0)).abs() < 1e-9);
    }

    /// A closed mark is swept right back to its start, or there is a notch
    /// where the two ends nearly meet — which is very visible on a thick nib
    /// and looks like a bug in the fill rather than in the path.
    #[test]
    fn a_closed_mark_has_no_notch_where_it_joins() {
        let m = Mark::new(ring(40, 2.0, 0.0), Nib::Round(0.3), 0xFFFFFF).closed(true);
        assert_eq!(m.stroke().pts.len(), 41, "the first point should come round again");
        assert!((m.stroke().pts[40] - m.stroke().pts[0]).abs() < 1e-12);
    }

    /// Bounds include the nib, not just the centreline — a mark drawn with a
    /// wide nib really does cover more page than the path it followed.
    #[test]
    fn bounds_include_the_width_of_the_mark() {
        let line = vec![Cx::new(-1.0, 0.0), Cx::new(1.0, 0.0)];
        let thin = Mark::new(line.clone(), Nib::Round(0.1), 0xFFFFFF);
        let fat = Mark::new(line, Nib::Round(1.0), 0xFFFFFF);
        let tall = |m: &Mark| {
            let (lo, hi) = m.bounds().expect("bounds");
            hi.im - lo.im
        };
        assert!((tall(&thin) - 0.1).abs() < 1e-9);
        assert!((tall(&fat) - 1.0).abs() < 1e-9);
    }

    /// An empty mark has no bounds rather than a panic or a point at the
    /// origin — a stroke can be abandoned before it has any points at all.
    #[test]
    fn an_empty_mark_has_no_bounds() {
        assert!(Mark::new(Vec::new(), Nib::Round(0.2), 0).bounds().is_none());
    }
}
