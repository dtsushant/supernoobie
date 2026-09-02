//! # speaker — actually making a noise
//!
//! Everything else in this crate is arithmetic: a note is a function, and a
//! function can be checked. This module is the one place that talks to the
//! machine, and it is deliberately small and deliberately separate.
//!
//! ## Why this needs a dependency and nothing else did
//!
//! A picture can be written to a file and looked at later. Sound cannot: it
//! only exists while it is happening. The sound card wants a **callback** —
//! give me the next few hundred samples, now, and be quick — running on its
//! own thread at a rate nothing else in the program cares about. Every
//! operating system spells that differently, and `cpal` is the layer that
//! makes them one thing.
//!
//! That is the whole reason for the dependency, and why it is confined to this
//! file. Delete `speaker.rs` and the rest of the crate is untouched.
//!
//! ## The one idea worth taking away: the callback is a real-time deadline
//!
//! The audio thread asks for samples on a clock and **will not wait**. Miss
//! the deadline and it plays whatever was in the buffer — usually silence,
//! sometimes the last block again — and you hear a click or a crackle. So the
//! callback must not allocate, must not lock, and must not do anything that
//! could take an unpredictable amount of time.
//!
//! Which is why the samples are computed **in advance** here and the callback
//! only copies them out. Synthesising inside the callback would work until it
//! didn't.
//!
//! ## Feature-gated
//!
//! ```text
//!     cargo run -p sound --features play --release -- --play
//! ```
//!
//! Off by default, so the crate still builds on a machine with no audio
//! libraries at all — which is the usual state of a build server, and was the
//! state of this one until a moment ago.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::mpsc;

/// Play samples, and wait until they have finished.
///
/// Blocks on purpose: a note that returns immediately and is cut off when the
/// program exits is worse than no sound at all.
pub fn play(samples: &[f64], rate: u32) -> Result<(), String> {
    if samples.is_empty() {
        return Ok(());
    }

    let host = cpal::default_host();
    let device = host.default_output_device().ok_or("no output device — is anything listening?")?;
    let config = device.default_output_config().map_err(|e| format!("no usable output config: {e}"))?;

    // The card decides the rate and the channel count, not us. Resample to
    // whatever it wants rather than assuming it wants 44100 — assuming is how
    // a note comes out at the wrong pitch.
    let out_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let ready = resample(samples, rate, out_rate);

    let (done, finished) = mpsc::channel();
    let mut at = 0usize;

    // Everything is computed already. The callback only copies — no
    // allocating, no locking, nothing that could miss the deadline.
    let stream = device
        .build_output_stream(
            &config.into(),
            move |out: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for frame in out.chunks_mut(channels) {
                    let s = ready.get(at).copied().unwrap_or(0.0);
                    at += 1;
                    // The same sample to every channel: this is mono.
                    for slot in frame.iter_mut() {
                        *slot = s;
                    }
                }
                if at >= ready.len() {
                    let _ = done.send(());
                }
            },
            move |e| eprintln!("audio stream error: {e}"),
            None,
        )
        .map_err(|e| format!("could not open the stream: {e}"))?;

    stream.play().map_err(|e| format!("could not start it: {e}"))?;
    let _ = finished.recv();
    // A moment for the last buffer to actually leave the card, so the tail of
    // the note is not chopped off.
    std::thread::sleep(std::time::Duration::from_millis(120));
    Ok(())
}

/// Stretch or squash samples to a different rate, straight-line between the
/// ones we have.
///
/// Crude on purpose — good resampling is its own subject, and the audible
/// difference here is nothing. What matters is that it happens **at all**:
/// handing 44100 samples to a card running at 48000 plays the note about a
/// semitone flat, which sounds like a bug in the pitch code rather than in the
/// plumbing.
pub fn resample(samples: &[f64], from: u32, to: u32) -> Vec<f32> {
    if from == to || samples.len() < 2 {
        return samples.iter().map(|s| *s as f32).collect();
    }
    let ratio = f64::from(from) / f64::from(to);
    let n = ((samples.len() as f64) / ratio) as usize;
    (0..n)
        .map(|k| {
            let x = k as f64 * ratio;
            let i = x as usize;
            let frac = x - i as f64;
            let a = samples[i.min(samples.len() - 1)];
            let b = samples[(i + 1).min(samples.len() - 1)];
            (a + (b - a) * frac) as f32
        })
        .collect()
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// ★ Resampling has to change the LENGTH in the right direction, or the
    /// note plays at the wrong pitch — which sounds like a bug in the pitch
    /// code rather than in the plumbing, and costs an afternoon.
    #[test]
    fn resampling_keeps_the_note_the_same_length_in_time() {
        let one_second: Vec<f64> = (0..44_100).map(|k| (k as f64 * 0.01).sin()).collect();

        let up = resample(&one_second, 44_100, 48_000);
        assert!((up.len() as f64 / 48_000.0 - 1.0).abs() < 0.01, "still one second at the higher rate");

        let down = resample(&one_second, 44_100, 22_050);
        assert!((down.len() as f64 / 22_050.0 - 1.0).abs() < 0.01, "and at the lower one");
        assert!(down.len() < one_second.len(), "fewer samples for the same second");
    }

    /// The same rate is not touched at all.
    #[test]
    fn matching_rates_pass_straight_through() {
        let s = vec![0.0, 0.5, -0.5, 1.0];
        let out = resample(&s, 44_100, 44_100);
        assert_eq!(out.len(), 4);
        for (a, b) in s.iter().zip(&out) {
            assert!((*a as f32 - *b).abs() < 1e-6);
        }
    }

    /// It interpolates rather than repeating: a ramp stays a ramp.
    #[test]
    fn resampling_interpolates_rather_than_stepping() {
        let ramp: Vec<f64> = (0..100).map(|k| k as f64 / 99.0).collect();
        let out = resample(&ramp, 100, 200);
        for w in out.windows(2) {
            assert!(w[1] >= w[0] - 1e-6, "a rising ramp should keep rising");
        }
        assert!(out.windows(2).any(|w| (w[1] - w[0]).abs() > 1e-9), "and not be a staircase of repeats");
    }

    /// Nothing to play is not an error, and must not reach the sound card at
    /// all — a zero-length stream is a good way to hang.
    #[test]
    fn silence_is_not_an_error() {
        assert!(play(&[], 44_100).is_ok());
        assert!(resample(&[], 44_100, 48_000).is_empty());
        assert_eq!(resample(&[0.5], 44_100, 48_000).len(), 1, "one sample cannot be interpolated");
    }
}
