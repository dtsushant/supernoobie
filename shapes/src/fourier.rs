//! # Fourier — any closed curve as a sum of rotating arrows
//!
//! Walk around a closed curve at a steady pace and write down where you are.
//! You get `z(θ)`, a complex number in and a complex number out, that comes
//! back where it started. Every such function can be written
//!
//! ```text
//!     z(θ)  =  Σ  c_n · e^{i n θ}
//!             n
//! ```
//!
//! — a stack of arrows, the `n`-th of length `|c_n|` and turning `n` times per
//! lap, laid tip to tail. `e^{inθ} = cos nθ + i sin nθ`, so this really is a
//! sum of sines and cosines, two per term.
//!
//! Getting the coefficients back out is one line. Multiply by `e^{-inθ}` and
//! average: that un-spins the term you asked for so it stands still and
//! survives the average, while every other term keeps turning and cancels
//! itself out over a full lap.
//!
//! ```text
//!     c_n  =  (1/N) Σ  z_k · e^{-i n θ_k}
//!                   k
//! ```
//!
//! Keep the biggest handful of `c_n` and you get a recognisable version of the
//! curve out of very few waves. That is the whole of [`digit`](crate::digit).

use plotkit::{Cx, Shape};
use std::f64::consts::TAU;
use std::sync::Arc;

/// The coefficients of a closed curve, biggest first.
///
/// Sorted by `|c_n|` rather than by `n`, so "the first `m` terms" means "the
/// `m` waves that matter most" and truncation is always the best answer
/// available at that budget.
#[derive(Clone, Debug)]
pub struct Series {
    pub terms: Vec<(i32, Cx)>,
}

impl Series {
    /// Transform a closed curve, re-spacing it evenly along its length first.
    /// `samples` decides how much detail survives: the highest frequency
    /// representable is `samples/2`.
    ///
    /// The path is closed before resampling. Without that step the final
    /// segment — the one joining the end back to the start — would be missing
    /// from the total length, and every sample would land slightly early.
    pub fn of(path: &[Cx], samples: usize) -> Series {
        let mut p = path.to_vec();
        if (p[0] - p[p.len() - 1]).abs() > 1e-12 {
            p.push(p[0]);
        }
        Series::of_samples(&resample(&p, samples))
    }

    /// Transform samples exactly as given, without re-spacing them.
    ///
    /// Use this when the parametrisation is already the one you mean. An
    /// ellipse stepped at even *angles* is exactly two terms; re-spaced by
    /// arclength it is not, because arclength is not proportional to angle.
    pub fn of_samples(z: &[Cx]) -> Series {
        let n = z.len();
        let half = (n / 2) as i32;
        let mut terms: Vec<(i32, Cx)> = (-half..half)
            .map(|f| {
                let mut acc = Cx::ZERO;
                for (k, zk) in z.iter().enumerate() {
                    acc = acc + *zk * Cx::expi(-TAU * f as f64 * k as f64 / n as f64);
                }
                (f, acc.scale(1.0 / n as f64))
            })
            .collect();
        terms.sort_by(|a, b| b.1.abs().partial_cmp(&a.1.abs()).expect("coefficients are finite"));
        Series { terms }
    }

    pub fn len(&self) -> usize {
        self.terms.len()
    }
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// Add the first `n` waves back up. One term is a point, two are a circle,
    /// six are a wobbly digit, forty are the digit.
    pub fn at(&self, n: usize, theta: f64) -> Cx {
        self.terms.iter().take(n.min(self.len())).fold(Cx::ZERO, |acc, (f, c)| acc + *c * Cx::expi(*f as f64 * theta))
    }

    /// Every partial sum, so the arrows can be drawn tip to tail. The last
    /// entry is [`Series::at`].
    pub fn arrows(&self, n: usize, theta: f64) -> Vec<Cx> {
        let mut z = Cx::ZERO;
        let mut out = vec![z];
        for (f, c) in self.terms.iter().take(n.min(self.len())) {
            z = z + *c * Cx::expi(*f as f64 * theta);
            out.push(z);
        }
        out
    }

    /// The curve the first `n` waves trace, as a shape about the origin.
    pub fn curve(&self, n: usize) -> Shape {
        let me = Arc::new(self.clone());
        Shape::param(move |th| me.at(n, th), 0.0, TAU, 420)
    }

    /// The arrows at one instant, as a shape — the chain, and a circle showing
    /// the reach of each one.
    pub fn machine(&self, n: usize, theta: f64) -> Shape {
        let chain = self.arrows(n, theta);
        let mut parts: Vec<Shape> = chain
            .windows(2)
            .filter(|w| (w[1] - w[0]).abs() > 0.02)
            .map(|w| Shape::circle(w[0], (w[1] - w[0]).abs()))
            .collect();
        parts.push(Shape::path(chain));
        Shape::group(parts)
    }
}

/// Re-space a path so consecutive points are equally far apart *along the
/// curve*.
///
/// Without this the pen dawdles where the source points are dense and sprints
/// where they are sparse. The full series would still be exact, but a
/// truncated one would spend its few terms describing the dawdling instead of
/// the shape.
pub fn resample(path: &[Cx], n: usize) -> Vec<Cx> {
    assert!(path.len() >= 2, "a path needs at least two points");
    let mut cum = vec![0.0];
    for w in path.windows(2) {
        cum.push(cum.last().expect("seeded") + (w[1] - w[0]).abs());
    }
    let total = *cum.last().expect("seeded");
    if total <= 0.0 {
        return vec![path[0]; n];
    }
    let mut out = Vec::with_capacity(n);
    let mut j = 0;
    for k in 0..n {
        let want = total * k as f64 / n as f64;
        while j + 2 < path.len() && cum[j + 1] < want {
            j += 1;
        }
        let span = (cum[j + 1] - cum[j]).max(1e-12);
        let u = ((want - cum[j]) / span).clamp(0.0, 1.0);
        out.push(path[j] + (path[j + 1] - path[j]).scale(u));
    }
    out
}

/// A slice of an ellipse, as points — the building block most outlines in this
/// crate are made of. `c + (rx cos θ, ry sin θ)` for `θ` from `a0` to `a1`.
pub fn arc(c: Cx, rx: f64, ry: f64, a0: f64, a1: f64, n: usize) -> Vec<Cx> {
    (0..=n)
        .map(|k| {
            let a = a0 + (a1 - a0) * k as f64 / n as f64;
            c + Cx::new(rx * a.cos(), ry * a.sin())
        })
        .collect()
}

/// Walk a stroke out and back, so an open path becomes a closed loop.
///
/// The Fourier series only represents periodic things, so the curve has to
/// return to where it started. The return trip lands exactly on the outward
/// one, so it is invisible; it costs a factor of two in samples and nothing
/// else.
pub fn there_and_back(stroke: Vec<Cx>) -> Vec<Cx> {
    let mut out = stroke.clone();
    out.extend(stroke.into_iter().rev().skip(1));
    out
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// A circle sampled at 128 points. The count matches the transform size
    /// on purpose: resampling a regular 128-gon by chord length returns its
    /// own vertices exactly, so the tests below can demand 1e-9 rather than
    /// the 1e-5 you get from landing part-way along a chord.
    fn circle(c: Cx, r: f64) -> Vec<Cx> {
        arc(c, r, r, 0.0, TAU, 128)
    }

    /// With every term kept this is not an approximation, it is an identity —
    /// the claim everything else rests on.
    ///
    /// `of_samples`, so the points compared against are exactly the points
    /// transformed. Reconstruction is exact *at the sample points*; asking it
    /// to also be exact at points it never saw would be a different claim.
    #[test]
    fn all_the_waves_add_back_up_to_the_curve() {
        let pts = resample(&circle(Cx::new(0.3, -0.2), 0.8), 128);
        let s = Series::of_samples(&pts);
        for (k, want) in pts.iter().enumerate() {
            assert!((s.at(s.len(), TAU * k as f64 / 128.0) - *want).abs() < 1e-9, "sample {k}");
        }
    }

    /// `of` closes the path before measuring its length. Skipping that would
    /// leave out the segment joining the end back to the start, so every
    /// sample would land slightly early and the curve would come out rotated.
    #[test]
    fn a_path_is_closed_before_it_is_resampled() {
        let open: Vec<Cx> = arc(Cx::ZERO, 1.0, 1.0, 0.0, TAU, 128)[..128].to_vec();
        assert!((open[0] - open[127]).abs() > 0.04, "this fixture is meant to be left open");
        let s = Series::of(&open, 128);
        // A circle is still two terms, and the second still has radius 1.
        assert!((s.terms[0].1.abs() - 1.0).abs() < 1e-6, "radius came out {}", s.terms[0].1.abs());
        assert!(s.terms[1].1.abs() < 1e-6, "a closed circle should need only one turning term");
    }

    /// ★ One wave is a point; two are a circle. A circle is the one shape
    /// whose entire series is `c_0 + c_1 e^{iθ}` and nothing more.
    #[test]
    fn one_wave_is_a_point_and_two_are_a_circle() {
        let s = Series::of(&circle(Cx::new(2.0, 0.0), 1.0), 128);
        for k in 0..40 {
            let th = TAU * k as f64 / 40.0;
            assert!((s.at(1, th) - Cx::new(2.0, 0.0)).abs() < 1e-9, "one term should stand still");
            assert!(((s.at(2, th) - Cx::new(2.0, 0.0)).abs() - 1.0).abs() < 1e-9, "two terms should trace radius 1");
        }
        assert!(s.terms[2].1.abs() < 1e-12, "and there is genuinely nothing else in there");
    }

    /// An ellipse needs **two counter-rotating** waves, `n = +1` and `n = −1`.
    /// One circle turning forwards plus one turning backwards is an ellipse,
    /// which is why an ellipse is not a two-term *circle*.
    #[test]
    fn an_ellipse_is_two_circles_spinning_opposite_ways() {
        let (rx, ry) = (0.40, 0.88);
        // `of_samples`, not `of`: this fact is about the even-ANGLE
        // parametrisation, and resampling by arclength would destroy it.
        let s = Series::of_samples(&arc(Cx::ZERO, rx, ry, 0.0, TAU, 256)[..256]);
        let pick = |n: i32| s.terms.iter().find(|(f, _)| *f == n).expect("term").1;
        assert!((pick(1) - Cx::new((rx + ry) / 2.0, 0.0)).abs() < 1e-9);
        assert!((pick(-1) - Cx::new((rx - ry) / 2.0, 0.0)).abs() < 1e-9);
        assert!(s.terms[2].1.abs() < 1e-9, "and nothing else");
    }

    /// Error must fall as terms are added. Sorting by size is what makes that
    /// true — the next term is always the biggest one left.
    #[test]
    fn adding_waves_only_ever_helps() {
        let pts = resample(&there_and_back(vec![Cx::new(-1.0, 0.9), Cx::new(1.0, 0.9), Cx::new(0.0, -0.9)]), 128);
        let s = Series::of_samples(&pts);
        let err = |m: usize| {
            pts.iter().enumerate().map(|(k, w)| (s.at(m, TAU * k as f64 / 128.0) - *w).abs()).fold(0.0f64, f64::max)
        };
        let mut prev = err(1);
        for m in [2, 4, 8, 16, 32, 64, 128] {
            let e = err(m);
            assert!(e <= prev + 1e-12, "error grew from {prev} to {e} at {m} terms");
            prev = e;
        }
        assert!(prev < 1e-9, "the full series should be exact, got {prev}");
    }

    #[test]
    fn the_loudest_waves_come_first() {
        let s = Series::of(&circle(Cx::new(1.0, 1.0), 0.7), 64);

        for w in s.terms.windows(2) {
            assert!(w[0].1.abs() >= w[1].1.abs() - 1e-15);
        }
    }

    #[test]
    fn the_arrow_chain_ends_on_the_curve() {
        let s = Series::of(&circle(Cx::new(0.2, 0.4), 0.9), 128);
        for k in 0..20 {
            let th = TAU * k as f64 / 20.0;
            assert!((*s.arrows(12, th).last().expect("chain") - s.at(12, th)).abs() < 1e-12);
        }
    }

    /// Stepping an ellipse at even *angles* covers 0.88 of a unit near the
    /// poles and 0.40 near the equator. Resampling fixes that.
    #[test]
    fn resampling_evens_out_the_pace() {
        let spread = |p: &[Cx]| {
            let g: Vec<f64> = p.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
            g.iter().cloned().fold(0.0, f64::max) / g.iter().cloned().fold(f64::MAX, f64::min)
        };
        let raw = arc(Cx::ZERO, 0.40, 0.88, 0.0, TAU, 240);
        assert!(spread(&raw) > 2.0, "even angles really are uneven pace");
        assert!(spread(&resample(&raw, 240)) < 1.01);
    }

    /// A walked-out-and-back stroke turns around at the far end, and two
    /// samples straddling that reversal sit almost on top of each other
    /// however evenly they are spaced *along* the curve. Chord distance is the
    /// wrong ruler at a reversal — not evidence of bad resampling.
    #[test]
    fn a_reversal_squashes_chords_without_squashing_arclength() {
        let p = there_and_back(vec![Cx::ZERO, Cx::new(1.0, 0.0)]);
        // 201, not 200: the turn is at arclength 1.0, so an even count would
        // land a sample exactly on it and nothing would straddle.
        let r = resample(&p, 201);
        let tiny = r.windows(2).filter(|w| (w[1] - w[0]).abs() < 0.005).count();
        assert_eq!(tiny, 1, "exactly one chord — the one over the turn — should collapse");
    }

    #[test]
    fn there_and_back_closes_the_loop() {
        let p = there_and_back(vec![Cx::ZERO, Cx::new(1.0, 0.0), Cx::new(1.0, 1.0)]);
        assert_eq!(p.len(), 5);
        assert!((p[0] - p[p.len() - 1]).abs() < 1e-12);
    }
}
