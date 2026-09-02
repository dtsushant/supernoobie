//! # wave — `a sin(kx + φ)`, and what happens when you add them
//!
//! ## A wave is the shadow of a rotating arrow
//!
//! ```text
//!     a sin(kx + φ)  =  Im( a e^{i(kx+φ)} )
//! ```
//!
//! That is not a re-writing trick, it is the useful fact. Arrows add
//! head-to-tail, and taking the shadow is linear, so shadows add with them:
//! `Im(A) + Im(B) = Im(A + B)`. Adding waves becomes adding arrows.
//!
//! ## Same frequency: the sum is one wave
//!
//! When every frequency agrees, pull the common rotation out:
//!
//! ```text
//!     Σ a_j sin(kx + φ_j)  =  Im( (Σ a_j e^{iφ_j}) · e^{ikx} )
//!                          =  |A| sin(kx + arg A)      where A = Σ a_j e^{iφ_j}
//! ```
//!
//! One complex addition and you have the answer. Every sum-to-product identity
//! in a trigonometry textbook is this line written out in real numbers so that
//! it looks hard.
//!
//! ## Different frequencies: it is not a wave at all
//!
//! There is no common `e^{ikx}` to pull out, so the step above is illegal and
//! the sum is genuinely a new shape. [`combine`] returns `None` rather than
//! inventing an answer — and that refusal is where Fourier series begin.

use plotkit::{Cx, Placeable, Shape};
use std::f64::consts::TAU;

/// A sinusoid `a·sin(kx + φ)`. How tall, how fast, where it starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wave {
    pub a: f64,
    pub k: f64,
    pub phi: f64,
    /// Where its `x` is measured from, and the line it waves about.
    pub origin: Cx,
}

impl Default for Wave {
    fn default() -> Wave {
        Wave::sine()
    }
}

impl Wave {
    pub const fn new(a: f64, k: f64, phi: f64) -> Wave {
        Wave { a, k, phi, origin: Cx::ZERO }
    }

    /// `sin(x)`: amplitude 1, wavelength 2π, through the origin.
    ///
    /// The thing you want when you just want a wave.
    pub const fn sine() -> Wave {
        Wave::new(1.0, 1.0, 0.0)
    }

    /// How tall.
    pub const fn amplitude(mut self, a: f64) -> Wave {
        self.a = a;
        self
    }

    /// How long one whole wave is, end to end.
    ///
    /// The number you can measure off the page. `k` is radians per unit, which
    /// is what the mathematics wants and nobody wants to think in:
    /// `k = 2π/λ`.
    pub fn wavelength(mut self, lambda: f64) -> Wave {
        self.k = TAU / lambda.abs().max(1e-12) * lambda.signum();
        self
    }

    /// Radians per unit, if you would rather say it that way.
    pub const fn frequency(mut self, k: f64) -> Wave {
        self.k = k;
        self
    }

    /// How far along it already is when it starts.
    pub const fn phase(mut self, phi: f64) -> Wave {
        self.phi = phi;
        self
    }

    /// Where it starts: `x` is measured from here, and it waves about this
    /// height.
    pub const fn from(mut self, z: Cx) -> Wave {
        self.origin = z;
        self
    }

    /// What one whole wave measures, end to end.
    pub fn length(self) -> f64 {
        TAU / self.k.abs().max(1e-12)
    }

    /// The wave, drawn across **whatever is on screen** — no endpoints, no
    /// sample count, nothing to choose.
    ///
    /// [`Shape::graph`] is sampled against the visible window, so the wave
    /// runs off both edges however far you pan or zoom. That is what a wave
    /// with no stated extent should do, and it is why this is a `graph` and
    /// not a `param` with two ends somebody had to pick.
    pub fn shape(self) -> Shape {
        Shape::graph(move |x| self.origin.im + self.at(x - self.origin.re))
    }

    /// The height of the wave at `x`.
    pub fn at(self, x: f64) -> f64 {
        self.a * (self.k * x + self.phi).sin()
    }

    /// The rotating arrow the wave is the shadow of. By construction
    /// `arrow(x).im == at(x)`.
    pub fn arrow(self, x: f64) -> Cx {
        Cx::polar(self.a, self.k * x + self.phi)
    }

    /// The arrow at `x = 0` — amplitude and phase in one complex number.
    /// Engineers call it the *phasor*: the wave with the spinning factored out.
    pub fn phasor(self) -> Cx {
        Cx::polar(self.a, self.phi)
    }
}

impl From<Wave> for Shape {
    fn from(w: Wave) -> Shape {
        w.shape()
    }
}

impl Placeable for Wave {
    /// Rebuilt about `at`, not shifted.
    ///
    /// A wave spans the whole window, so it has no endpoints to move; shifting
    /// the samples sideways would leave a bare strip at one edge. Changing
    /// where `x` is measured from moves it properly.
    fn placed(self, at: Cx) -> Shape {
        self.from(at).shape()
    }
}

/// A whole stack of waves, added up and drawn as one curve.
///
/// This is the thing worth having: adding *plots* would only mean drawing
/// both, but adding **functions** makes a third that neither one is — which is
/// the Fourier series, and why a square wave can be made of sines.
pub fn sum(ws: &[Wave]) -> Shape {
    let ws = ws.to_vec();
    Shape::graph(move |x| total(&ws, x))
}

/// The height of a whole stack of waves at `x`.
pub fn total(ws: &[Wave], x: f64) -> f64 {
    ws.iter().map(|w| w.origin.im + w.at(x - w.origin.re)).sum()
}

/// The arrows laid tip to tail at `x`, starting from the origin.
///
/// The last entry is the summed arrow, and its imaginary part is
/// [`total`] — which is why the head-to-tail picture is not a diagram of the
/// addition but the addition itself.
pub fn chain(ws: &[Wave], x: f64) -> Vec<Cx> {
    let mut z = Cx::ZERO;
    let mut out = vec![z];
    for w in ws {
        z = z + w.arrow(x);
        out.push(z);
    }
    out
}

/// The next wave to add, continuing whatever pattern is already there.
///
/// Two rules, and the first is the one that matters:
///
/// * **All the frequencies odd → the next odd one.** A square wave is built
///   from odd harmonics only (1, 3, 5, 7…). Slipping an even one in does not
///   just add detail, it destroys the symmetry that makes the wave square —
///   every even harmonic is symmetric about the half-period where the square
///   wave is antisymmetric, so it cancels the flatness the odd terms built.
/// * Otherwise, the next whole number above the highest frequency present.
///
/// The amplitude is `1/k`, matching the decay every one of these series uses.
/// That is not decoration: coefficients falling off as `1/k` are exactly what a
/// waveform with a **jump** in it needs. Anything faster — `1/2^k`, say — sums
/// to something smooth, which can never have a vertical edge.
///
/// Phase is zero. A stack with mixed phases has no pattern to continue, so
/// guessing one would be inventing rather than following.
pub fn next(ws: &[Wave]) -> Wave {
    let Some(top) = ws.iter().map(|w| w.k).fold(None, |m: Option<f64>, k| Some(m.map_or(k, |a| a.max(k)))) else {
        return Wave::new(1.0, 1.0, 0.0);
    };

    let whole = |k: f64| (k - k.round()).abs() < 1e-6;
    let all_odd = ws.iter().all(|w| whole(w.k) && (w.k.round() as i64).rem_euclid(2) == 1);

    // Always strictly above what is there, whichever rule applied — otherwise
    // "add a wave" could produce a duplicate of one already on screen.
    let k = if all_odd { top.round() + 2.0 } else { (top + 1.0).floor().max(top + 1e-9) };
    Wave::new(1.0 / k.max(1.0), k, 0.0)
}

/// Add a stack of waves into the single wave that results.
///
/// `None` when the frequencies are not all equal, because then no single sine
/// wave is the answer. An empty stack combines to a flat zero at frequency
/// zero, which is the honest answer to "what is the sum of nothing".
pub fn combine(ws: &[Wave]) -> Option<Wave> {
    let k = ws.first().map(|w| w.k).unwrap_or(0.0);
    if ws.iter().any(|w| (w.k - k).abs() > 1e-9) {
        return None;
    }
    let a = ws.iter().fold(Cx::ZERO, |acc, w| acc + w.phasor()); // <- the entire computation
    Some(Wave::new(a.abs(), k, a.arg()))
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{PI, TAU};

    /// ★ The identity every picture of this rests on. If it failed, the
    /// horizontal connector between an arrow and its wave would slope, and the
    /// drawing would be a lie.
    #[test]
    fn a_wave_is_the_shadow_of_its_arrow() {
        let w = Wave::new(0.7, 2.3, 1.1);
        for k in 0..40 {
            let x = -4.0 + 0.31 * k as f64;
            assert!((w.arrow(x).im - w.at(x)).abs() < 1e-12);
        }
    }

    /// Adding shadows and shadowing sums are the same operation, for any
    /// number of waves. This is what licenses drawing them head to tail.
    #[test]
    fn the_chain_ends_on_the_sum() {
        let ws = [Wave::new(0.9, 1.0, 0.3), Wave::new(0.4, 2.7, -1.2), Wave::new(0.6, 0.5, 2.0)];
        for k in 0..50 {
            let x = 0.13 * k as f64;
            assert!((chain(&ws, x).last().expect("chain").im - total(&ws, x)).abs() < 1e-12);
        }
    }

    /// The chain has one more point than there are waves — it starts at the
    /// origin, then one tip per arrow.
    #[test]
    fn the_chain_has_a_link_per_wave() {
        for n in 0..6 {
            let ws: Vec<Wave> = (0..n).map(|k| Wave::new(0.5, 1.0 + k as f64, 0.0)).collect();
            assert_eq!(chain(&ws, 1.3).len(), n + 1);
        }
    }

    /// Same frequency in, same frequency out — and the combined wave agrees
    /// with the pointwise sum everywhere, not only at convenient points.
    #[test]
    fn same_frequency_waves_add_to_one_wave() {
        let ws = [Wave::new(1.0, 1.0, 0.0), Wave::new(0.6, 1.0, 0.9), Wave::new(0.4, 1.0, -2.1)];
        let s = combine(&ws).expect("all the same frequency");
        for k in 0..200 {
            let x = -6.0 + 0.07 * k as f64;
            assert!((s.at(x) - total(&ws, x)).abs() < 1e-12, "disagreed at x = {x}");
        }
    }

    /// sin + cos = √2 sin(x + π/4). The famous one, falling out of `1 + i`.
    #[test]
    fn sin_plus_cos_is_root_two_at_a_slant() {
        let s = combine(&[Wave::new(1.0, 1.0, 0.0), Wave::new(1.0, 1.0, PI / 2.0)]).expect("same frequency");
        assert!((s.a - 2f64.sqrt()).abs() < 1e-12);
        assert!((s.phi - PI / 4.0).abs() < 1e-12);
    }

    /// Half a turn apart is `e^{iπ} = −1`, so the phasors annihilate.
    #[test]
    fn opposite_phase_cancels_exactly() {
        let s = combine(&[Wave::new(1.0, 1.0, 0.0), Wave::new(1.0, 1.0, PI)]).expect("same frequency");
        assert!(s.a < 1e-15, "amplitude was {}", s.a);
    }

    /// Three at 120° apart also cancel — the cube roots of unity sum to zero.
    /// Nothing about the two-wave case was special.
    #[test]
    fn three_waves_at_a_third_of_a_turn_cancel_too() {
        let ws: Vec<Wave> = (0..3).map(|k| Wave::new(1.0, 1.0, TAU * k as f64 / 3.0)).collect();
        assert!(combine(&ws).expect("same frequency").a < 1e-14);
    }

    /// ★ The refusal. Different frequencies have no single-wave answer, and
    /// `combine` says so rather than inventing one.
    #[test]
    fn different_frequencies_have_no_single_wave_answer() {
        assert!(combine(&[Wave::new(1.0, 1.0, 0.0), Wave::new(1.0, 2.0, 0.0)]).is_none());
        // One stray frequency is enough to spoil it, wherever it sits.
        assert!(combine(&[Wave::new(1.0, 1.0, 0.0), Wave::new(1.0, 1.0, 0.4), Wave::new(1.0, 1.7, 0.0)]).is_none());
    }

    /// And the refusal is honest: an octave pair really is a new shape. A pure
    /// sine of frequency 1 is symmetric about its peak; this sum is not.
    #[test]
    fn an_octave_pair_is_genuinely_a_new_shape() {
        let ws = [Wave::new(1.0, 1.0, 0.0), Wave::new(0.6, 2.0, 0.0)];
        let n = 20_000;
        let (mut peak, mut best) = (0.0, f64::NEG_INFINITY);
        for k in 0..n {
            let x = TAU * k as f64 / n as f64;
            if total(&ws, x) > best {
                best = total(&ws, x);
                peak = x;
            }
        }
        let d = 0.6;
        assert!((total(&ws, peak + d) - total(&ws, peak - d)).abs() > 1e-3, "suspiciously sine-like");
    }

    /// ★ Adding to a square wave has to keep it a square wave. An even
    /// harmonic is symmetric about the half-period where a square wave is
    /// antisymmetric, so it does not add detail — it undoes the flatness the
    /// odd terms built.
    #[test]
    fn adding_to_a_square_wave_keeps_it_square() {
        let mut ws = vec![Wave::new(1.0, 1.0, 0.0), Wave::new(1.0 / 3.0, 3.0, 0.0), Wave::new(0.2, 5.0, 0.0)];
        for want in [7.0, 9.0, 11.0, 13.0] {
            let n = next(&ws);
            assert_eq!(n.k, want, "expected the next odd harmonic");
            assert!((n.a - 1.0 / want).abs() < 1e-12, "amplitude should be 1/k");
            ws.push(n);
        }
        assert!(ws.iter().all(|w| (w.k as i64) % 2 == 1), "an even harmonic crept in");
    }

    /// Once anything even is present there is no odd pattern left to continue,
    /// so it just counts up.
    #[test]
    fn a_mixed_stack_just_counts_up() {
        assert_eq!(next(&[Wave::new(1.0, 1.0, 0.0), Wave::new(0.6, 2.0, 0.0)]).k, 3.0);
        assert_eq!(next(&[Wave::new(1.0, 4.0, 0.0)]).k, 5.0);
    }

    /// ★ The bug this replaced: counting the waves rather than reading their
    /// frequencies. From the three-term square wave, `len + 1` is 4 — an even
    /// harmonic, *and* an amplitude of 1/4 which is larger than the 1/5 already
    /// on screen, so the new circle came out bigger than the one above it.
    #[test]
    fn the_new_wave_never_outgrows_the_one_before_it() {
        let mut ws = vec![Wave::new(1.0, 1.0, 0.0), Wave::new(1.0 / 3.0, 3.0, 0.0), Wave::new(0.2, 5.0, 0.0)];
        for _ in 0..6 {
            let n = next(&ws);
            assert!(n.a <= ws.last().expect("non-empty").a + 1e-12, "{} is bigger than {}", n.a, ws.last().unwrap().a);
            ws.push(n);
        }
    }

    /// However the frequencies have been dragged about, the new one is above
    /// all of them — so "add a wave" can never duplicate one already there.
    #[test]
    fn the_new_frequency_is_always_the_highest() {
        for ws in [
            vec![Wave::new(1.0, 2.7, 0.0), Wave::new(0.5, 1.3, 0.0)],
            vec![Wave::new(1.0, 0.0, 0.0)],
            vec![Wave::new(1.0, 8.4, 0.0), Wave::new(1.0, 1.0, 0.0)],
        ] {
            let n = next(&ws);
            assert!(ws.iter().all(|w| n.k > w.k), "{} did not clear {:?}", n.k, ws.iter().map(|w| w.k).collect::<Vec<_>>());
            assert!(n.a > 0.0 && n.a <= 1.0, "amplitude {} is out of range", n.a);
        }
    }

    #[test]
    fn the_first_wave_is_the_fundamental() {
        assert_eq!(next(&[]), Wave::new(1.0, 1.0, 0.0));
    }

    // ---- drawing one ------------------------------------------------------

    fn pts(s: &Shape, lo: f64, hi: f64) -> Vec<Cx> {
        s.polylines(Cx::new(lo, -50.0), Cx::new(hi, 50.0), 400).into_iter().flatten().collect()
    }

    /// ★ A wave has no ends. It is a `graph`, sampled against the visible
    /// window, so it runs off both edges however far you pan or zoom — no
    /// endpoints and no sample count for anybody to pick.
    #[test]
    fn a_wave_reaches_both_edges_of_wherever_it_is_drawn() {
        let s = Wave::sine().shape();
        for (lo, hi) in [(-5.0, 5.0), (-200.0, 200.0), (100.0, 101.0)] {
            let p = pts(&s, lo, hi);
            let (first, last) = (p[0].re, p[p.len() - 1].re);
            assert!((first - lo).abs() < 0.1, "did not start at the left edge of {lo}..{hi}");
            assert!((last - hi).abs() < 0.1, "did not reach the right edge of {lo}..{hi}");
        }
    }

    /// Wavelength is the thing you can measure off the page: one whole wave,
    /// end to end. `k` is radians per unit, and `k = 2π/λ`.
    #[test]
    fn wavelength_is_the_distance_between_repeats() {
        let w = Wave::sine().wavelength(4.0);
        assert!((w.length() - 4.0).abs() < 1e-12);
        for x in [0.0, 1.3, -2.7] {
            assert!((w.at(x) - w.at(x + 4.0)).abs() < 1e-12, "it should repeat after one wavelength");
        }
        assert!((w.at(0.5) - w.at(2.5)).abs() > 0.5, "and not after half of one");
    }

    /// The builder says what it does: taller, longer, later, elsewhere.
    #[test]
    fn the_builder_sets_what_it_says() {
        let w = Wave::sine().amplitude(3.0).wavelength(8.0).phase(0.5).from(Cx::new(1.0, 2.0));
        assert_eq!(w.a, 3.0);
        assert!((w.length() - 8.0).abs() < 1e-12);
        assert_eq!(w.phi, 0.5);
        assert_eq!(w.origin, Cx::new(1.0, 2.0));
    }

    /// ★ Placing a wave rebuilds it about the point rather than shifting the
    /// samples — so it still reaches both edges. Shifting would drag the whole
    /// curve sideways and leave a bare strip at one end.
    #[test]
    fn a_placed_wave_still_reaches_both_edges() {
        let put = Wave::sine().placed(Cx::new(6.0, 2.0));
        let p = pts(&put, -10.0, 10.0);
        assert!((p[0].re + 10.0).abs() < 0.1, "a bare strip appeared on the left");
        assert!((p[p.len() - 1].re - 10.0).abs() < 0.1, "and on the right");

        // And it waves about the height it was placed at, starting there.
        let ys: Vec<f64> = p.iter().map(|q| q.im).collect();
        let mid = (ys.iter().cloned().fold(f64::MIN, f64::max) + ys.iter().cloned().fold(f64::MAX, f64::min)) / 2.0;
        assert!((mid - 2.0).abs() < 0.05, "it should wave about y = 2, not {mid}");
    }

    /// Where it starts is where it starts: at its own origin it is at rest.
    #[test]
    fn a_wave_starts_at_its_origin() {
        let w = Wave::sine().from(Cx::new(3.0, -1.0));
        let p = pts(&w.shape(), 2.9, 3.1);
        let at_origin = p.iter().min_by(|a, b| (a.re - 3.0).abs().partial_cmp(&(b.re - 3.0).abs()).expect("finite"));
        assert!((at_origin.expect("samples").im + 1.0).abs() < 0.02, "it should cross its own origin");
    }

    /// ★ `sum` adds the FUNCTIONS, not the pictures. Adding two plots would
    /// only mean drawing both; adding two functions makes a third that neither
    /// one is — which is the whole of Fourier.
    #[test]
    fn summing_waves_makes_a_new_curve_not_two_old_ones() {
        let ws = [Wave::sine(), Wave::sine().amplitude(1.0 / 3.0).frequency(3.0)];
        let p = pts(&sum(&ws), -3.0, 3.0);
        assert_eq!(p.len(), 401, "one curve, not two");

        for q in &p {
            assert!((q.im - total(&ws, q.re)).abs() < 1e-9, "the drawn curve should be the sum");
        }
        // And it is not either of them.
        assert!(p.iter().any(|q| (q.im - ws[0].at(q.re)).abs() > 0.1));
    }

    #[test]
    fn nothing_sums_to_nothing() {
        let s = combine(&[]).expect("an empty stack is all the same frequency, vacuously");
        assert_eq!(s.a, 0.0);
        assert_eq!(total(&[], 3.7), 0.0);
    }
}
