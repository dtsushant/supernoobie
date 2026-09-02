//! # tone — a note, and why two instruments playing it sound different
//!
//! ## A note is three things
//!
//! ```text
//!     s(t)  =  envelope(t)  ·  Σ  aₙ sin(2π n f t + φₙ)
//!              ^^^^^^^^^^^      ^^^^^^^^^^^^^^^^^^^^^^^
//!              how loud,        what it is made of:
//!              over time        the fundamental and its harmonics
//! ```
//!
//! * **Pitch** — `f`, which note it is. See [`crate::pitch`].
//! * **Timbre** — the `aₙ`, why a violin and a flute playing the same note are
//!   not the same sound.
//! * **Envelope** — how it starts and stops. A plucked string and a bowed one
//!   can have identical harmonics and still be unmistakable.
//!
//! ## Timbre is Fourier, exactly
//!
//! A note from a real instrument is not one sine. It is the **fundamental**
//! `f` plus **harmonics** at `2f, 3f, 4f…`, and the recipe of how loud each
//! one is *is* the instrument's voice. That is the same decomposition as
//! [`shapes::fourier`](../../shapes/fourier/index.html) — a periodic thing as a
//! sum of sines — pointed at the ear instead of the eye.
//!
//! Some recipes worth knowing:
//!
//! ```text
//!     only the fundamental      a flute, or a tuning fork. pure, hollow
//!     odd harmonics, 1/n        a clarinet, or a square wave. woody
//!     all harmonics, 1/n        a sawtooth. bright, buzzy — strings, brass
//!     all harmonics, 1/n²       a triangle. soft
//! ```
//!
//! **`1/n` is the sound of a corner.** A waveform with a jump or a kink needs
//! its coefficients to fall off that slowly — which is the same fact as the
//! square wave in [`shapes::wave`](../../shapes/wave/index.html) needing `1/n`
//! to have a vertical edge. Bright sounds and sharp corners are the same thing.
//!
//! ## The envelope is Laplace, exactly
//!
//! Pluck a string and it dies away as `e^{−t/τ}`. That is a damped oscillator
//! — [`physics::Oscillator`] — and its damping decides everything about the
//! character:
//!
//! ```text
//!     lightly damped     rings a long time      a bell, a plucked string
//!     heavily damped     stops almost at once   a thud, a woodblock
//! ```
//!
//! So the two halves of this crate are the two halves of the repository:
//! **Fourier makes the timbre, Laplace makes the envelope.**
//!
//! ## Sampling, and the limit nobody can get round
//!
//! A computer cannot store a curve, only numbers off it, `1/rate` apart. How
//! often is enough?
//!
//! **Harry Nyquist** (1928) and **Claude Shannon** (1948) settled it: you need
//! **more than two samples per cycle**. Fewer and the wave is not merely rough,
//! it is *wrong* — it comes back as a different, lower frequency, and once
//! that has happened nothing can undo it. That is **aliasing**, the same effect
//! as a wagon wheel appearing to turn backwards in a film.
//!
//! Which is why CDs sample at 44100 a second. Human hearing stops around
//! 20 000 Hz, twice that is 40 000, and the rest is room to put a filter in.

use physics::Oscillator;
use std::f64::consts::TAU;

/// Samples a second. 44100 is the CD rate, and the reason is Nyquist: a little
/// over twice the top of human hearing.
pub const RATE: u32 = 44_100;

/// The recipe of harmonics that gives an instrument its voice.
///
/// Entry `n` is how loud the `(n+1)`th harmonic is, relative to the
/// fundamental at entry 0.
#[derive(Clone, Debug, PartialEq)]
pub struct Timbre(pub Vec<f64>);

impl Timbre {
    /// One sine and nothing else. Pure and a bit hollow — a tuning fork, or a
    /// flute played gently.
    pub fn pure() -> Timbre {
        Timbre(vec![1.0])
    }

    /// Odd harmonics falling as `1/n` — a square wave, which is roughly a
    /// clarinet. The missing even harmonics are what make it sound woody
    /// rather than bright.
    pub fn clarinet(n: usize) -> Timbre {
        Timbre((1..=n).map(|k| if k % 2 == 1 { 1.0 / k as f64 } else { 0.0 }).collect())
    }

    /// Every harmonic falling as `1/n` — a sawtooth. The brightest of the
    /// simple shapes, because it has the sharpest corner: bowed strings and
    /// brass live near here.
    pub fn saw(n: usize) -> Timbre {
        Timbre((1..=n).map(|k| 1.0 / k as f64).collect())
    }

    /// Odd harmonics falling as `1/n²` — a triangle. Soft, close to pure,
    /// because the faster fall-off means less of the sharp corner.
    pub fn triangle(n: usize) -> Timbre {
        Timbre((1..=n).map(|k| if k % 2 == 1 { 1.0 / (k * k) as f64 } else { 0.0 }).collect())
    }

    /// The sum of the parts, so a note can be normalised and never clip.
    pub fn total(&self) -> f64 {
        self.0.iter().map(|a| a.abs()).sum::<f64>().max(1e-12)
    }
}

/// A note: what pitch, what it is made of, and how it dies away.
#[derive(Clone, Debug)]
pub struct Tone {
    /// The fundamental, in hertz.
    pub freq: f64,
    pub timbre: Timbre,
    /// How the loudness decays. `τ` seconds to fall to `1/e`.
    pub decay: f64,
    /// How long the note takes to get going. Zero is a pluck; longer is a bow.
    ///
    /// Without it every note begins with a step, and a step is a corner — so
    /// it clicks.
    pub attack: f64,
}

impl Tone {
    /// A plucked note: no attack to speak of, dying away.
    pub fn pluck(freq: f64) -> Tone {
        Tone { freq, timbre: Timbre::saw(12), decay: 1.2, attack: 0.004 }
    }

    /// A bowed or blown note: eased in, and held.
    pub fn bowed(freq: f64) -> Tone {
        Tone { freq, timbre: Timbre::clarinet(11), decay: 6.0, attack: 0.08 }
    }

    pub fn with_timbre(mut self, t: Timbre) -> Tone {
        self.timbre = t;
        self
    }

    pub fn with_decay(mut self, tau: f64) -> Tone {
        self.decay = tau;
        self
    }

    /// How loud it is at time `t`: eased in, then dying away.
    ///
    /// The decay is `e^{−t/τ}`, which is a damped oscillator seen from the
    /// side — the same exponential as a branch settling or a raindrop reaching
    /// terminal velocity.
    pub fn envelope(&self, t: f64) -> f64 {
        if t < 0.0 {
            return 0.0;
        }
        let rise = if self.attack <= 0.0 { 1.0 } else { (t / self.attack).min(1.0) };
        rise * (-t / self.decay.max(1e-6)).exp()
    }

    /// The sound itself at time `t`, between −1 and 1.
    pub fn at(&self, t: f64) -> f64 {
        let mut sum = 0.0;
        for (k, a) in self.timbre.0.iter().enumerate() {
            let n = (k + 1) as f64;
            sum += a * (TAU * n * self.freq * t).sin();
        }
        self.envelope(t) * sum / self.timbre.total()
    }

    /// The note, sampled — the numbers that actually get played.
    pub fn samples(&self, seconds: f64, rate: u32) -> Vec<f64> {
        let n = (seconds.max(0.0) * f64::from(rate)) as usize;
        (0..n).map(|k| self.at(k as f64 / f64::from(rate))).collect()
    }

    /// The highest harmonic this note actually contains.
    ///
    /// Worth asking before playing it: anything above half the sample rate
    /// will not be recorded, it will come back as a **different, lower**
    /// frequency. See [`Tone::aliases_at`].
    pub fn top_frequency(&self) -> f64 {
        let top = self.timbre.0.iter().rposition(|a| a.abs() > 1e-9).map_or(0, |k| k + 1);
        self.freq * top as f64
    }

    /// Does this note contain anything too high to be sampled at this rate?
    ///
    /// **Nyquist**: you need more than two samples per cycle. A high note with
    /// a bright timbre is the usual way to trip over it — the fundamental is
    /// fine and the twelfth harmonic is not.
    pub fn aliases_at(&self, rate: u32) -> bool {
        self.top_frequency() >= f64::from(rate) / 2.0
    }

    /// The envelope as the damped oscillator it is, if you would rather think
    /// of it that way — `τ` becomes `1/ζω`.
    pub fn as_oscillator(&self) -> Oscillator {
        let omega = TAU * self.freq;
        Oscillator::new(omega, 1.0 / (self.decay.max(1e-6) * omega))
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// ★ A note stays inside the range that can be played. Anything past ±1
    /// clips, which is a nasty crunch and the most common way a synthesiser
    /// sounds broken.
    #[test]
    fn a_note_never_clips() {
        for t in [Tone::pluck(440.0), Tone::bowed(220.0), Tone::pluck(110.0).with_timbre(Timbre::saw(30))] {
            for s in t.samples(2.0, 8_000) {
                assert!(s.abs() <= 1.0 + 1e-9, "sample out of range: {s}");
            }
        }
    }

    /// ★ Timbre is Fourier: the same pitch, different recipes, genuinely
    /// different sounds. If two timbres produced the same samples, there would
    /// be no such thing as an instrument.
    #[test]
    fn the_same_note_on_two_timbres_is_two_sounds() {
        let flute = Tone::pluck(440.0).with_timbre(Timbre::pure());
        let reed = Tone::pluck(440.0).with_timbre(Timbre::clarinet(9));
        let differ = (1..500).filter(|k| {
            let t = *k as f64 / 8_000.0;
            (flute.at(t) - reed.at(t)).abs() > 0.05
        });
        assert!(differ.count() > 100, "the two timbres are barely distinguishable");
    }

    /// A clarinet has no even harmonics. That absence is what makes it woody
    /// rather than bright, and it is the whole difference from a sawtooth.
    #[test]
    fn a_clarinet_is_missing_its_even_harmonics() {
        let Timbre(parts) = Timbre::clarinet(10);
        for (k, a) in parts.iter().enumerate() {
            let harmonic = k + 1;
            if harmonic % 2 == 0 {
                assert_eq!(*a, 0.0, "harmonic {harmonic} should be absent");
            } else {
                assert!(*a > 0.0, "harmonic {harmonic} should be there");
            }
        }
        assert!(Timbre::saw(10).0.iter().all(|a| *a > 0.0), "a saw keeps all of them");
    }

    /// ★ `1/n` is the sound of a corner. A sawtooth falls off as `1/n` and a
    /// triangle as `1/n²`, and that is exactly why one is bright and the other
    /// soft — the same fact as a square wave needing `1/n` to have a vertical
    /// edge.
    #[test]
    fn brightness_is_how_slowly_the_harmonics_fall_off() {
        let energy_up_high = |t: Timbre| {
            let all: f64 = t.0.iter().map(|a| a.abs()).sum();
            let high: f64 = t.0.iter().skip(4).map(|a| a.abs()).sum();
            high / all
        };
        assert!(energy_up_high(Timbre::saw(16)) > energy_up_high(Timbre::triangle(16)));
        assert!(energy_up_high(Timbre::triangle(16)) > energy_up_high(Timbre::pure()));
        assert_eq!(energy_up_high(Timbre::pure()), 0.0, "one sine has nothing up high at all");
    }

    /// ★ The envelope is `e^{−t/τ}`: after one time constant it is down to
    /// `1/e`. The same exponential as a branch settling — that is not an
    /// analogy, it is the same equation.
    #[test]
    fn the_envelope_dies_away_exponentially() {
        let t = Tone::pluck(440.0).with_decay(0.5);
        let peak = t.envelope(t.attack);
        assert!((t.envelope(0.5) / peak - (-1.0f64).exp() / (-t.attack / 0.5f64).exp()).abs() < 0.02);
        assert!(t.envelope(5.0) < 0.001, "it should be all but gone");
        assert_eq!(t.envelope(-1.0), 0.0, "and silent before it starts");
    }

    /// A note eases in rather than starting with a step. A step is a corner,
    /// and a corner is a click.
    #[test]
    fn a_note_starts_from_silence() {
        let t = Tone::bowed(440.0);
        assert_eq!(t.envelope(0.0), 0.0);
        assert!(t.envelope(t.attack / 2.0) < t.envelope(t.attack));
        assert!(t.at(0.0).abs() < 1e-12, "the first sample must be silence");
    }

    /// ★ Nyquist. A bright note played high runs out of room: the fundamental
    /// fits and the twelfth harmonic does not, and what does not fit does not
    /// simply vanish — it comes back as a lower frequency that was never
    /// played.
    #[test]
    fn a_bright_high_note_asks_for_more_than_the_rate_can_give() {
        let low = Tone::pluck(220.0).with_timbre(Timbre::saw(12));
        assert!(!low.aliases_at(RATE), "220 Hz with 12 harmonics is only 2.6 kHz");

        let high = Tone::pluck(4_000.0).with_timbre(Timbre::saw(12));
        assert!(high.top_frequency() >= 48_000.0, "4 kHz times twelve harmonics is exactly 48 kHz");
        assert!(high.aliases_at(RATE), "4 kHz with 12 harmonics needs 96 kHz to record");

        // And the rule really is HALF the rate, not the rate.
        let edge = Tone::pluck(11_000.0).with_timbre(Timbre::pure());
        assert!(!edge.aliases_at(RATE), "11 kHz fits in 44.1 kHz");
        assert!(edge.aliases_at(20_000), "but not in 20 kHz");
    }

    /// And here is aliasing actually happening: sampled too slowly, a high
    /// note is indistinguishable from a low one it never contained.
    #[test]
    fn too_few_samples_turns_a_high_note_into_a_low_one() {
        let rate = 1_000;
        // 1100 Hz sampled at 1000 comes back as 100 Hz. Nothing can undo it.
        let high = Tone::pluck(1_100.0).with_timbre(Timbre::pure()).with_decay(1e6);
        let ghost = Tone::pluck(100.0).with_timbre(Timbre::pure()).with_decay(1e6);
        let (a, b) = (high.samples(0.05, rate), ghost.samples(0.05, rate));
        let worst = a.iter().zip(&b).skip(10).map(|(x, y)| (x - y).abs()).fold(0.0f64, f64::max);
        assert!(worst < 0.02, "they should be indistinguishable, but differ by {worst}");
    }

    #[test]
    fn sampling_gives_the_number_of_samples_it_says() {
        assert_eq!(Tone::pluck(440.0).samples(1.0, 8_000).len(), 8_000);
        assert_eq!(Tone::pluck(440.0).samples(0.0, 8_000).len(), 0);
    }

    /// The envelope really is a damped oscillator, so the two views agree.
    #[test]
    fn the_envelope_and_the_oscillator_are_the_same_thing() {
        let t = Tone::pluck(440.0).with_decay(0.75);
        let o = t.as_oscillator();
        // zeta*omega is 1/tau: the rate the envelope decays at.
        assert!((o.zeta * o.omega - 1.0 / 0.75).abs() < 1e-9);
    }
}
