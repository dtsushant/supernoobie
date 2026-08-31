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

use plotkit::Cx;

/// A sinusoid `a·sin(kx + φ)`. How tall, how fast, where it starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wave {
    pub a: f64,
    pub k: f64,
    pub phi: f64,
}

impl Wave {
    pub const fn new(a: f64, k: f64, phi: f64) -> Wave {
        Wave { a, k, phi }
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

/// The height of a whole stack of waves at `x`.
pub fn total(ws: &[Wave], x: f64) -> f64 {
    ws.iter().map(|w| w.at(x)).sum()
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

    #[test]
    fn nothing_sums_to_nothing() {
        let s = combine(&[]).expect("an empty stack is all the same frequency, vacuously");
        assert_eq!(s.a, 0.0);
        assert_eq!(total(&[], 3.7), 0.0);
    }
}
