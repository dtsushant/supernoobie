//! # sound — the same mathematics, pointed at the ear
//!
//! Nothing new to learn here, which is the point of doing it before three
//! dimensions. A sound is a function of time; the ear is a Fourier analyser;
//! and a note dying away is a damped oscillator. Every idea this crate needs
//! is already in the repository:
//!
//! | in here | is really | from |
//! |---|---|---|
//! | timbre | a sum of harmonics | [`shapes::fourier`](../shapes/fourier/index.html) |
//! | a chord | adding functions, not pictures | [`shapes::wave`](../shapes/wave/index.html) |
//! | a note dying away | `e^{−t/τ}` | [`physics::Oscillator`] |
//! | pitch | a logarithm, because ratios | [`pitch`] |
//!
//! ## The one genuinely new idea: sampling
//!
//! A computer cannot hold a curve, only numbers off it. **Nyquist** and
//! **Shannon** settled how many you need: *more than two per cycle*. Fewer,
//! and the wave does not merely come out rough — it comes back as a
//! **different, lower** frequency, and nothing afterwards can undo it. The
//! same effect as a wagon wheel turning backwards in a film.
//!
//! That single limit is why CDs sample 44 100 times a second: hearing gives
//! out around 20 kHz, twice that is 40 kHz, and the rest is room for a filter.
//!
//! ## Try it
//!
//! ```text
//!     cargo run -p sound --release -- chord.wav              writes a file
//!     cargo run -p sound --features play --release -- --play plays it
//! ```
//!
//! Playing is behind a feature flag because it is the one thing here that
//! needs a sound card and a C library. Everything else is arithmetic, and
//! arithmetic builds anywhere.
//!
//! ## Where this goes
//!
//! Nothing in this crate has a dimension in it, so none of it needs revisiting
//! for three dimensions. Sound gains *position* there — a source somewhere,
//! two ears, and the difference in arrival time between them — but a note is
//! still a note.

pub mod pitch;
/// Making an actual noise. Behind the `play` feature, because it is the one
/// part that needs a sound card and a C library — see [`speaker`].
#[cfg(feature = "play")]
pub mod speaker;
pub mod tone;
pub mod wav;

pub use tone::{Timbre, Tone, RATE};

/// Mix several sounds into one.
///
/// **Adding functions, not pictures** — the same distinction as
/// [`shapes::wave::sum`](../shapes/wave/fn.sum.html). Two notes played
/// together are not two sounds side by side; they are one sound that is
/// neither, and where their harmonics agree they reinforce and where they
/// disagree they beat.
///
/// Scaled by how many there are, because otherwise three notes are three times
/// as loud as one and clip.
pub fn mix(parts: &[Vec<f64>]) -> Vec<f64> {
    let longest = parts.iter().map(Vec::len).max().unwrap_or(0);
    let share = 1.0 / (parts.len().max(1) as f64);
    (0..longest)
        .map(|k| parts.iter().map(|p| p.get(k).copied().unwrap_or(0.0)).sum::<f64>() * share)
        .collect()
}

/// Put one sound after another, with `gap` seconds of silence between.
pub fn after(parts: &[Vec<f64>], gap: f64, rate: u32) -> Vec<f64> {
    let hush = vec![0.0; (gap.max(0.0) * f64::from(rate)) as usize];
    let mut out = Vec::new();
    for (k, p) in parts.iter().enumerate() {
        if k > 0 {
            out.extend_from_slice(&hush);
        }
        out.extend_from_slice(p);
    }
    out
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// ★ Mixing adds the FUNCTIONS. Two notes together are one sound that is
    /// neither of them — the same fact as adding two waves rather than drawing
    /// both.
    #[test]
    fn mixing_makes_one_sound_that_is_neither() {
        let a = Tone::bowed(440.0).samples(0.5, 8_000);
        let b = Tone::bowed(pitch::from_a(7.0)).samples(0.5, 8_000);
        let both = mix(&[a.clone(), b.clone()]);

        assert_eq!(both.len(), a.len());
        assert!(both.iter().zip(&a).any(|(m, x)| (m - x).abs() > 0.05), "it should not just be the first note");
        assert!(both.iter().zip(&b).any(|(m, x)| (m - x).abs() > 0.05), "nor the second");
        for (k, m) in both.iter().enumerate() {
            assert!((m - (a[k] + b[k]) / 2.0).abs() < 1e-12, "it should be their average");
        }
    }

    /// ★ A chord must not be louder than a note. Adding without scaling is the
    /// usual way a mix clips, and clipping is a crunch rather than a volume.
    #[test]
    fn a_chord_is_no_louder_than_one_note() {
        let notes: Vec<Vec<f64>> = ["C4", "E4", "G4", "C5"]
            .iter()
            .map(|n| Tone::pluck(pitch::named(n).expect("a note")).samples(1.0, 8_000))
            .collect();
        for s in mix(&notes) {
            assert!(s.abs() <= 1.0 + 1e-9, "the chord clipped at {s}");
        }
    }

    /// Mixing sounds of different lengths keeps the longest, rather than
    /// truncating to the shortest and cutting a note off.
    #[test]
    fn a_short_note_does_not_cut_a_long_one_short() {
        let short = vec![1.0; 10];
        let long = vec![1.0; 100];
        assert_eq!(mix(&[short, long]).len(), 100);
    }

    #[test]
    fn notes_played_in_turn_take_as_long_as_they_should() {
        let one = vec![0.0; 800];
        let out = after(&[one.clone(), one.clone(), one], 0.1, 8_000);
        // A tenth of a second at 8 kHz is 800 samples of silence, not 80.
        assert_eq!(out.len(), 800 * 3 + 800 * 2, "three notes and two gaps");
    }

    #[test]
    fn mixing_nothing_is_silence_rather_than_a_panic() {
        assert!(mix(&[]).is_empty());
        assert!(after(&[], 0.5, 8_000).is_empty());
    }
}
