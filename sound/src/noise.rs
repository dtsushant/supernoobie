//! # noise — a sound written down as numbers
//!
//! The same rule as everything else here: **numbers save, closures don't.** A
//! noise is a list of [`Grain`]s — when, what pitch, how long it takes to die
//! away — and nothing else. Which means one description can be
//!
//! - played by [`render`], so a test can *measure* it;
//! - written to a file by [`crate::wav`], so it can be listened to;
//! - sent to a browser, which builds the same thing out of oscillators.
//!
//! A noise written as code in one of those places is a noise the other two
//! cannot have. The game's sounds lived in the page as hand-written JavaScript
//! for exactly one commit, and in that commit there was no way to hear them
//! without opening a browser and no way to check them at all.
//!
//! ## Testing a sound
//!
//! Not by comparing samples — that pins the arithmetic, not the sound, and
//! breaks on a rounding change. What is worth asserting is what a listener
//! would notice:
//!
//! | | |
//! |---|---|
//! | [`peak`] | it is audible, and it does not clip |
//! | [`fades`] | it dies away rather than stopping dead |
//! | [`length`] | it is over when it should be |
//! | [`brightness`] | dull like card, or bright like metal |
//! | [`tonality`] | does it ring, or is it just a knock |
//! | [`knocks`] | how many separate hits are in it |
//!
//! [`tonality`] is the one that mattered here. "It sounds like a spoon hitting
//! metal, not quite rolling" is a complaint about *pitch*: a tone with a decay
//! is a pitched thing being struck, and pitched things ring. A die on card has
//! no pitch to speak of. Brightness could not see it — filtered noise changes
//! faster between samples than a pure tone does, so by that measure the wrong
//! sound scored better, and the test written around it passed.

use crate::mixer::{Mixer, Voice};
use crate::tone::{Timbre, Tone};

/// One sound within a noise.
///
/// `freq` of nought means a **knock** — filtered noise dying away, which is one
/// thing striking another. Anything else is a note.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grain {
    /// Seconds from the start of the noise.
    pub at: f64,
    /// Hertz, or nought for a knock.
    pub freq: f64,
    /// Seconds to fall to a thirty-seventh — `τ`.
    pub tau: f64,
    /// 0 to 1.
    pub gain: f64,
    /// Low-pass corner in hertz. **The whole of the difference between card
    /// and metal**, for a knock.
    pub cut: f64,
}

impl Grain {
    /// A knock: one thing striking another.
    pub fn knock(at: f64, cut: f64, tau: f64, gain: f64) -> Grain {
        Grain { at, freq: 0.0, tau, gain, cut }
    }

    /// A note.
    pub fn note(at: f64, freq: f64, tau: f64, gain: f64) -> Grain {
        Grain { at, freq, tau, gain, cut: 0.0 }
    }

    /// How this grain is played.
    pub fn voice(&self) -> Voice {
        if self.freq <= 0.0 {
            Voice::knock(self.cut.max(20.0), self.tau.max(1e-4)).at(self.gain)
        } else {
            Voice::note(
                Tone::pluck(self.freq)
                    .with_timbre(Timbre::triangle(5))
                    .with_decay(self.tau.max(1e-4))
                    .with_attack(0.002),
            )
            .at(self.gain)
        }
    }
}

/// How long a noise lasts, in seconds — the last grain's start plus four of
/// its time constants, which is the usual answer to *when has an exponential
/// finished*.
pub fn length(noise: &[Grain]) -> f64 {
    noise.iter().map(|g| g.at + 4.0 * g.tau).fold(0.0, f64::max)
}

/// Play a noise into samples, so something can look at it.
pub fn render(noise: &[Grain], rate: u32) -> Vec<f32> {
    let seconds = length(noise) + 0.05;
    let total = (seconds * rate as f64) as usize;
    let mut mix = Mixer::new();
    let mut out = vec![0.0f32; total];
    let mut waiting: Vec<&Grain> = noise.iter().collect();
    waiting.sort_by(|a, b| a.at.partial_cmp(&b.at).unwrap_or(std::cmp::Ordering::Equal));
    let mut next = 0usize;

    // A block at a time, because that is how a sound card asks for it -- and
    // because a grain must be struck between blocks rather than inside one.
    const BLOCK: usize = 64;
    let mut done = 0usize;
    while done < total {
        let now = done as f64 / rate as f64;
        while next < waiting.len() && waiting[next].at <= now {
            mix.strike(waiting[next].voice());
            next += 1;
        }
        let take = BLOCK.min(total - done);
        mix.fill(&mut out[done..done + take], 1, rate);
        done += take;
    }
    out
}

/// The loudest sample. Under about 0.02 nobody hears it; over 1.0 it clips.
pub fn peak(samples: &[f32]) -> f64 {
    samples.iter().fold(0.0f64, |a, s| a.max(s.abs() as f64))
}

/// How much quieter the last quarter is than the first — **the shape of a
/// thing dying away**. One is no decay at all.
pub fn fades(samples: &[f32]) -> f64 {
    if samples.len() < 8 {
        return 1.0;
    }
    let q = samples.len() / 4;
    let power = |s: &[f32]| s.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / s.len() as f64;
    let first = power(&samples[..q]).max(1e-12);
    (power(&samples[samples.len() - q..]) / first).sqrt()
}

/// How bright it is: 0 is a rumble, 1 is a hiss.
///
/// The energy in the difference between one sample and the next, against the
/// energy in the signal. A high, ringing thing changes fast between samples; a
/// dull thud does not.
///
/// **Not a zero-crossing count.** That was tried, and it read a shorter note as
/// duller and could not see harmonics at all.
/// ## Why the difference between samples measures brightness
///
/// Differencing a signal is a **high-pass filter**: it is the discrete
/// derivative, and differentiating multiplies each frequency component by its
/// own frequency (`d/dt e^{iωt} = iω e^{iωt}`). So the energy in the
/// differences, against the energy in the signal, is a weighted average of
/// frequency — a cheap cousin of the **spectral centroid**, which is the
/// standard measure of brightness in music information retrieval and dates to
/// Grey's 1977 work on timbre perception at Stanford's CCRMA.
///
/// John Grey's contribution was to show, by multidimensional scaling of how
/// listeners judged instrument sounds, that timbre is *low-dimensional*: two or
/// three numbers account for most of what people hear as the difference between
/// a clarinet and a trumpet, and the first of them is brightness. That is why
/// one number here is worth having at all.
///
/// **To read further:** Grey, *Multidimensional perceptual scaling of musical
/// timbres* (JASA, 1977).
pub fn brightness(samples: &[f32]) -> f64 {
    if samples.len() < 4 {
        return 0.0;
    }
    let signal: f64 = samples.iter().map(|v| (*v as f64) * (*v as f64)).sum();
    let change: f64 = samples.windows(2).map(|w| {
        let d = (w[1] - w[0]) as f64;
        d * d
    }).sum();
    if signal < 1e-12 {
        return 0.0;
    }
    (change / signal / 4.0).sqrt().min(1.0)
}

/// How much of a **pitch** there is: 0 is noise, 1 is a pure tone.
///
/// The best match the signal makes with a delayed copy of itself. A pitched
/// thing repeats — that is what having a pitch *is* — so a copy slid along by
/// one period lines up almost exactly. Noise never lines up with itself.
///
/// **This is the measurement that names the bug.** "It sounds like a spoon
/// hitting metal, not quite rolling" is a complaint about tonality: a tone with
/// a decay is a pitched thing being struck, and pitched things ring. A die on
/// card does not. Brightness cannot see it — filtered noise changes faster
/// between samples than a pure tone does, so by that measure the wrong sound
/// scored *better*.
/// ## Whose idea this is
///
/// Matching a signal against a delayed copy of itself is **autocorrelation**,
/// and the reason it works is the Wiener–Khinchin theorem: the
/// autocorrelation of a signal and its power spectrum are Fourier transforms of
/// one another. Norbert Wiener proved it in 1930 (*Generalized harmonic
/// analysis*) and Aleksandr Khinchin independently in 1934. So asking "how well
/// does this line up with itself" and asking "what frequencies is this made of"
/// are the same question asked two ways — which is why a periodicity measure
/// can stand in for a spectrum without ever computing one.
///
/// Wiener built the theory during the war for gun-laying predictors: given a
/// noisy track of an aeroplane, where will it be in four seconds. The
/// Wiener filter that came out of it is the direct ancestor of everything in
/// this file.
///
/// Autocorrelation as a **pitch detector** is older than computers in spirit
/// and was made practical by Noll in 1967 (cepstrum) and Rabiner in 1977, whose
/// paper *On the use of autocorrelation analysis for pitch detection* is still
/// the standard reference. The modern refinement most people reach for is YIN
/// (de Cheveigné & Kawahara, 2002), which is this with the normalisation done
/// more carefully.
///
/// **To read further:** Rabiner's 1977 paper is short and readable; for the
/// theorem, any book on stochastic processes — Papoulis is the usual one.
pub fn tonality(samples: &[f32]) -> f64 {
    // From the loud part, since the tail of anything is mostly nothing.
    let take = samples.len().min(4096);
    let s: Vec<f64> = samples[..take].iter().map(|v| *v as f64).collect();
    let power: f64 = s.iter().map(|v| v * v).sum();
    if power < 1e-12 || s.len() < 64 {
        return 0.0;
    }
    // Lags for 40 Hz up to about 2 kHz, which is every pitch worth hearing as
    // a pitch.
    let lo = 22usize;
    let hi = (s.len() / 2).min(1100);
    let mut best = 0.0f64;
    for lag in lo..hi {
        let n = s.len() - lag;
        let mut top = 0.0;
        let mut bottom = 0.0;
        for k in 0..n {
            top += s[k] * s[k + lag];
            bottom += s[k] * s[k];
        }
        if bottom > 1e-12 {
            best = best.max(top / bottom);
        }
    }
    best.clamp(0.0, 1.0)
}

/// How many separate hits are in a noise — the number of times it gets
/// suddenly louder after being quiet.
///
/// For asking *does this sound like one thing landing, or like a die
/// tumbling*, which is a question about rhythm and not about pitch.
pub fn knocks(samples: &[f32], rate: u32) -> usize {
    // Loudness over a two-millisecond window, which is about as fine as an ear
    // resolves separate hits.
    let win = (rate as f64 * 0.002).max(1.0) as usize;
    let env: Vec<f64> = samples
        .chunks(win)
        .map(|c| c.iter().fold(0.0f64, |a, s| a.max(s.abs() as f64)))
        .collect();
    let top = env.iter().cloned().fold(0.0f64, f64::max);
    if top < 1e-6 {
        return 0;
    }
    // A hit is a rise past a fifth of the loudest, having first fallen below a
    // tenth. Two thresholds rather than one, so a wobble at the edge of a
    // single hit is not counted twice.
    let (mut n, mut armed) = (0usize, true);
    for v in env {
        if armed && v > top * 0.2 {
            n += 1;
            armed = false;
        } else if !armed && v < top * 0.1 {
            armed = true;
        }
    }
    n
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 44_100;

    /// ★ **A knock has no pitch and a note does.** The whole difference
    /// between a die landing on card and a spoon hitting a saucepan, and the
    /// measurement that names it.
    ///
    /// Brightness cannot: filtered noise changes faster between samples than a
    /// pure tone does, so by *that* measure the wrong sound scored better. It
    /// was measured and it said so, which is the only reason this test is
    /// about tonality instead.
    #[test]
    fn a_knock_has_no_pitch_and_a_note_does() {
        let knock = tonality(&render(&[Grain::knock(0.0, 900.0, 0.02, 0.5)], RATE));
        let note = tonality(&render(&[Grain::note(0.0, 900.0, 0.02, 0.5)], RATE));
        assert!(note > 0.75, "a note should ring: {note:.3}");
        assert!(knock < 0.5, "a knock should not: {knock:.3}");
    }

    /// And the low-pass is what does it: the same knock cut lower is duller
    /// still.
    #[test]
    fn a_lower_cut_is_duller() {
        let dull = render(&[Grain::knock(0.0, 500.0, 0.02, 0.5)], RATE);
        let bright = render(&[Grain::knock(0.0, 6000.0, 0.02, 0.5)], RATE);
        assert!(brightness(&dull) < brightness(&bright) * 0.8, "{} vs {}", brightness(&dull), brightness(&bright));
    }

    /// ★ Everything here dies away. A sound that stopped dead would click.
    #[test]
    fn a_grain_fades() {
        for g in [Grain::knock(0.0, 1200.0, 0.03, 0.5), Grain::note(0.0, 440.0, 0.05, 0.5)] {
            let s = render(&[g], RATE);
            assert!(fades(&s) < 0.05, "it should be nearly gone by the end: {}", fades(&s));
        }
    }

    /// ★ It is audible, and it does not clip. Both matter and they pull
    /// against each other.
    #[test]
    fn a_grain_is_audible_and_does_not_clip() {
        let s = render(&[Grain::knock(0.0, 1200.0, 0.03, 0.5)], RATE);
        assert!(peak(&s) > 0.02, "too quiet to hear: {}", peak(&s));
        assert!(peak(&s) <= 1.0, "it clips: {}", peak(&s));
    }

    /// ★ Separate hits are counted as separate hits — which is what tells a
    /// tumble from a single landing.
    #[test]
    fn the_hits_in_a_noise_are_counted() {
        let one = render(&[Grain::knock(0.0, 1000.0, 0.01, 0.5)], RATE);
        assert_eq!(knocks(&one, RATE), 1);

        let three: Vec<Grain> =
            [0.0, 0.12, 0.24].iter().map(|t| Grain::knock(*t, 1000.0, 0.01, 0.5)).collect();
        assert_eq!(knocks(&render(&three, RATE), RATE), 3);
    }

    /// A noise is over when its last grain is, and not before.
    #[test]
    fn a_noise_ends_when_its_last_grain_does() {
        let n = [Grain::knock(0.0, 1000.0, 0.01, 0.5), Grain::knock(0.4, 1000.0, 0.02, 0.5)];
        assert!((length(&n) - (0.4 + 0.08)).abs() < 1e-9);
    }

    /// ★ The same noise renders the same samples every time. No random number
    /// generator anywhere, so a recorded game replays exactly.
    #[test]
    fn a_noise_is_the_same_every_time() {
        let n = [Grain::knock(0.0, 1400.0, 0.03, 0.5), Grain::note(0.05, 660.0, 0.04, 0.3)];
        assert_eq!(render(&n, RATE), render(&n, RATE));
    }

    /// Silence is silence rather than a panic.
    #[test]
    fn nothing_makes_no_sound() {
        assert_eq!(peak(&render(&[], RATE)), 0.0);
        assert_eq!(knocks(&render(&[], RATE), RATE), 0);
    }
}
