//! # kit — the sounds themselves
//!
//! [`mixer`](super::mixer) is the machinery; this is the vocabulary. One
//! function per thing that can happen in the animation, each taking **how
//! hard** and giving back a [`Voice`] to strike.
//!
//! ```text
//!     trigger fires with a strength   ->   kit::bounce(strength)   ->   mixer.strike(..)
//! ```
//!
//! ## Why every one of these takes a strength
//!
//! Because a sound that is always the same sound stops being information. A
//! ball dropped from a table and one nudged off a step make the same *event*
//! and must not make the same *noise*, or the ear learns to ignore both. So
//! hardness changes three things at once, which is what changes in life:
//!
//! ```text
//!     louder      the obvious one, and the least of the three
//!     brighter    a hard knock has high harmonics a soft one does not
//!     shorter     energy that went in fast comes out fast
//! ```
//!
//! The second is the one people miss. Turning a quiet knock up does not make
//! it a hard knock; it makes a loud quiet knock, and the ear is not fooled for
//! a moment. Brightness is the tell.
//!
//! ## And why they are all made of things already in the repository
//!
//! Nothing here is a recording or a wavetable. A bounce is
//! [`Tone`](crate::Tone) — harmonics from
//! [`fourier`](../../shapes/fourier/index.html), an envelope from
//! [`Oscillator`](physics::Oscillator) — and wind is noise through the same
//! `e^{−t/τ}` filter as everything else that approaches a target. If you can
//! read the maths you can predict the sound, which is the entire point.

use crate::mixer::Voice;
use crate::tone::{Timbre, Tone};

/// The name the wind is held under. Wind is one continuous sound whose level
/// is pushed at it every frame, not a series of events.
pub const WIND: u32 = 1;

/// A ball landing.
///
/// `hardness` is the speed it arrived with, which is exactly what
/// [`Trigger::turning`](physics::Trigger::turning) hands back — so a bounce is
/// one line: `mixer.strike(kit::bounce(speed))`.
///
/// Low and short. **The pitch does not move with hardness** — it is the same
/// ball, and a ball does not change size when you drop it from higher. What
/// changes is how much of it rings: hit harder, the high harmonics come alive.
/// (Pitch is set by what the thing *is*, so a beach ball thuds and a marble
/// ticks — that belongs to the object, not to the landing.)
pub fn bounce(hardness: f64) -> Voice {
    let h = hardness.clamp(0.0, 1.0);
    Voice::note(
        Tone::pluck(110.0)
            // Brighter when it is hit harder. The part that makes it *sound*
            // harder rather than merely louder.
            .with_timbre(Timbre::triangle(3 + (h * 9.0) as usize))
            // And shorter: energy in fast is energy out fast.
            .with_decay(0.18 - 0.10 * h)
            .with_attack(0.001),
    )
    .at(0.25 + 0.6 * h)
}

/// Wood under strain — a branch bending past where it wants to be.
///
/// Not a note: a creak is wood slipping against itself in fits, so it is noise
/// with a bit of pitch rather than pitch with a bit of noise. Made here as a
/// low, very short clarinet-ish tone, which has only odd harmonics and so
/// sounds hollow rather than musical.
pub fn creak(strain: f64) -> Voice {
    let s = strain.clamp(0.0, 1.0);
    Voice::note(
        // Higher when it is bent further, because a stiffer, more loaded
        // member has a higher note — the same reason a tightened string does.
        Tone::pluck(90.0 + 160.0 * s)
            .with_timbre(Timbre::clarinet(4 + (s * 6.0) as usize))
            .with_decay(0.05 + 0.15 * s)
            .with_attack(0.004),
    )
    .at(0.15 + 0.35 * s)
}

/// A branch or trunk giving way. One heavier, lower, longer creak.
pub fn crack(strain: f64) -> Voice {
    let s = strain.clamp(0.0, 1.0);
    Voice::note(Tone::pluck(55.0 + 45.0 * s).with_timbre(Timbre::saw(12)).with_decay(0.35).with_attack(0.0005))
        .at(0.4 + 0.5 * s)
}

/// A click for anything the person did — a key, a tap on a shape.
///
/// Short and high on purpose. Feedback for your own action should be over
/// before you have finished noticing it, or it feels like lag.
pub fn tap() -> Voice {
    Voice::note(Tone::pluck(880.0).with_timbre(Timbre::pure()).with_decay(0.04).with_attack(0.0005)).at(0.3)
}

/// The wind itself, to be **held** rather than struck.
///
/// ```text
///     mixer.hold(kit::WIND, kit::wind(0.0));       once
///     mixer.level(kit::WIND, ..);                  every frame
///     mixer.colour(kit::WIND, kit::gustiness(v));  every frame
/// ```
pub fn wind(level: f64) -> Voice {
    Voice::rustle(gustiness(level)).at(level.clamp(0.0, 1.0))
}

/// How bright the wind should be at a given strength, in Hz.
///
/// The ear reads brightness as speed more than it reads volume — a gale
/// recorded quietly still sounds like a gale, and a breeze turned up still
/// sounds like a breeze. So this is the more important of the two numbers
/// pushed at the wind each frame.
pub fn gustiness(level: f64) -> f64 {
    200.0 + 3_000.0 * level.clamp(0.0, 1.0)
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::mixer::Mixer;

    /// Render one struck voice on its own and measure it.
    fn take(v: Voice, seconds: f64, rate: u32) -> Vec<f32> {
        let mut m = Mixer::new();
        m.strike(v);
        let mut out = vec![0.0f32; (seconds * f64::from(rate)) as usize];
        m.fill(&mut out, 1, rate);
        out
    }

    fn peak(b: &[f32]) -> f64 {
        b.iter().fold(0.0f64, |a, s| a.max(f64::from(s.abs())))
    }

    /// How bright a sound is, as a number.
    ///
    /// The energy of the **difference** between one sample and the next,
    /// against the energy of the samples themselves. Differencing is a crude
    /// high-pass — it barely touches a slow wave and doubles a fast one — so
    /// the ratio rises with harmonic content and is blind to volume.
    ///
    /// Counting zero crossings would be simpler and is wrong twice over here:
    /// a shorter note crosses fewer times in a fixed window merely for being
    /// shorter, and a triangle's harmonics fall off as 1/n², far too weakly to
    /// push the signal back across zero at all. It would read "brighter" as
    /// "longer and lower", which is the opposite of the claim.
    ///
    /// Measured over the first `LOOK` samples, while both sounds are still
    /// sounding — after one has died the comparison is against silence.
    fn brightness(b: &[f32]) -> f64 {
        const LOOK: usize = 800;
        let b = &b[..b.len().min(LOOK)];
        let energy: f64 = b.iter().map(|s| f64::from(*s) * f64::from(*s)).sum();
        let change: f64 = b.windows(2).map(|w| f64::from(w[1] - w[0]).powi(2)).sum();
        change / energy.max(1e-12)
    }

    /// How long until it has all but stopped.
    fn length(b: &[f32]) -> usize {
        b.iter().rposition(|s| s.abs() > 0.01).unwrap_or(0)
    }

    /// ★ A hard landing is not a loud soft landing. It is louder **and
    /// brighter and shorter**, and the middle one is what the ear actually
    /// reads as force — turn a gentle knock up and it stays a gentle knock.
    #[test]
    fn a_hard_landing_differs_from_a_soft_one_in_more_than_volume() {
        let soft = take(bounce(0.1), 0.5, 16_000);
        let hard = take(bounce(1.0), 0.5, 16_000);

        assert!(peak(&hard) > peak(&soft), "louder");
        // At a FIXED pitch, which is why `bounce` keeps one — move the
        // fundamental as well and this measure is reading two things at once.
        assert!(brightness(&hard) > brightness(&soft), "and brighter, which is the one that matters");
        assert!(length(&hard) < length(&soft), "and shorter, because it went in fast");
    }

    /// A bounce is low — it is a thud, not a ping. If this creeps up the whole
    /// thing sounds like a table-tennis ball whatever it is drawn as.
    #[test]
    fn a_bounce_is_low_and_a_tap_is_high() {
        let thud = take(bounce(0.5), 0.3, 16_000);
        let click = take(tap(), 0.3, 16_000);
        assert!(brightness(&click) > brightness(&thud) * 2.0, "a tap should be far higher");
    }

    /// Feedback for your own action must be over before you notice it, or it
    /// feels like the program is lagging behind your hand.
    #[test]
    fn a_tap_is_over_almost_at_once() {
        let click = take(tap(), 1.0, 16_000);
        assert!(length(&click) < 16_000 / 4, "a tap should not ring for a quarter of a second");
    }

    /// ★ Wood under more strain sounds higher, for the same reason a tightened
    /// string does: it is stiffer.
    #[test]
    fn wood_under_more_strain_sounds_tighter() {
        let easy = take(creak(0.05), 0.4, 16_000);
        let strained = take(creak(1.0), 0.4, 16_000);
        assert!(brightness(&strained) > brightness(&easy), "a bent branch should sound tighter");
        assert!(peak(&strained) > peak(&easy));
    }

    /// A crack is heavier than a creak — lower, longer, louder. Otherwise a
    /// tree going over sounds like a twig.
    #[test]
    fn a_crack_is_heavier_than_a_creak() {
        let c = take(creak(1.0), 1.0, 16_000);
        let big = take(crack(1.0), 1.0, 16_000);
        assert!(peak(&big) > peak(&c), "louder");
        assert!(length(&big) > length(&c), "and it lasts");
    }

    /// ★ Wind rising is heard as brightness more than as volume — a gale
    /// recorded quietly still sounds like a gale.
    #[test]
    fn wind_rising_is_heard_as_brightness() {
        assert!(gustiness(1.0) > gustiness(0.0) * 5.0, "it should hiss a great deal more when it is up");

        let hiss = |level: f64| {
            let mut m = Mixer::new();
            m.hold(WIND, wind(level));
            let mut b = vec![0.0f32; 8_000];
            m.fill(&mut b, 1, 16_000);
            brightness(&b)
        };
        assert!(hiss(1.0) > hiss(0.05) * 2.0, "a gale should hiss and a breeze should not");
    }

    /// Still wind is silence. A background sound held at zero must actually be
    /// nothing, or every quiet moment has a hiss under it.
    #[test]
    fn no_wind_is_silence() {
        let mut m = Mixer::new();
        m.hold(WIND, wind(0.0));
        let mut b = vec![0.0f32; 2_000];
        m.fill(&mut b, 1, 16_000);
        assert!(b.iter().all(|s| *s == 0.0), "calm should be silent");
    }

    /// Every sound in the kit stays inside the limit on its own, so the mixer's
    /// squashing is a safety net for a busy moment rather than something the
    /// sounds rely on.
    #[test]
    fn nothing_in_the_kit_clips_by_itself() {
        for (name, v) in [
            ("bounce", bounce(1.0)),
            ("creak", creak(1.0)),
            ("crack", crack(1.0)),
            ("tap", tap()),
            ("wind", wind(1.0)),
        ] {
            assert!(peak(&take(v, 0.5, 16_000)) <= 1.0, "{name} clipped");
        }
    }

    /// A strength outside 0..1 is clamped rather than rejected. These are fed
    /// straight from physics, where a number is occasionally larger than
    /// anybody expected, and a panic in the middle of an animation is a worse
    /// answer than a loud bounce.
    #[test]
    fn an_unreasonable_strength_is_clamped_not_a_panic() {
        for v in [bounce(50.0), creak(-3.0), crack(f64::MAX), wind(9.0)] {
            let b = take(v, 0.2, 16_000);
            assert!(b.iter().all(|s| s.is_finite()), "it should still be a sound");
            assert!(peak(&b) <= 1.0);
        }
    }
}

// ===========================================================================
// The game noises. Written as [`crate::noise::Grain`]s rather than as voices,
// because a grain is numbers and numbers are what a test can measure, a file
// can hold, and a browser can be told.

use crate::noise::Grain;

/// **A die thrown.** Not one hit — a die tumbling is a *series* of them, one
/// each time it goes over an edge, and the gaps between them grow as it slows.
///
/// The times are the same curve the die is drawn with. `plotkit::dice` turns
/// through `flips(t) = n·ease(t)` where `ease` is the scaled `1 − e^{−t/τ}`, so
/// the k-th contact is where `flips(t) = k`:
///
/// ```text
///     t_k = −τ · ln(1 − (k/n)·(1 − e^{−OVER/τ}))
/// ```
///
/// That is the same equation read backwards, and it is why the sound and the
/// picture cannot drift apart: they are one function, inverted.
///
/// Each contact is a **knock** and not a note. A tone with a decay is a pitched
/// thing being struck — a bell, a bar, a spoon on a saucepan — and it rings. A
/// die on card is broadband and the board swallows it, so `cut` is low and
/// there is no pitch at all. Getting that wrong is what made it sound metallic.
pub fn roll(flips: usize) -> Vec<Grain> {
    let n = flips.clamp(3, 24) as f64;
    let tau = plotkit::dice::REST;
    let span = 1.0 - (-plotkit::dice::OVER / tau).exp();
    (1..=flips.clamp(3, 24))
        .map(|k| {
            let part = k as f64 / n;
            let at = -tau * (1.0 - part * span).max(1e-6).ln();
            // Quieter as it settles, and duller: the last few are the die
            // rocking to a stop rather than bouncing.
            let left = 1.0 - part;
            // Low, because a die lands on a board and a board is wood. The
            // cut is the whole of the difference: above about a kilohertz what
            // comes through is hiss, and hiss is what the ear reads as metal.
            let knock = Grain::knock(at, 240.0 + 380.0 * left, 0.016 + 0.014 * left, 0.07 + 0.15 * left);
            // And a thump of the board itself under it. Wood has a body: a low
            // resonance that does not last long enough to be a pitch but
            // without which the contact is a click and not a knock.
            //
            // Short on purpose -- long enough and it starts to ring, which is
            // the fault being fixed. `tonality` is what says whether it has.
            let body = Grain::note(at, 132.0, 0.009 + 0.006 * left, 0.025 + 0.040 * left);
            [knock, body]
        })
        .flatten()
        .collect()
}

/// **A token on a square.** Short, soft, and quiet enough to hear ten of in a
/// row without it becoming a drum roll.
pub fn step() -> Vec<Grain> {
    // Measured rather than guessed: at a gain of 0.16 this came out at 0.39
    // peak, which is louder than a capture. A step has to sit UNDER everything
    // else in the game -- four of them go past in a second.
    //
    // And low, like everything else that touches this board. At 2400 it was a
    // tick on glass.
    vec![Grain::knock(0.0, 900.0, 0.007, 0.06)]
}

/// **A capture.** Low and with a bite on it: a knock for the contact and a
/// short low note under it, because something has been hit hard enough to make
/// the board itself sound.
pub fn cut() -> Vec<Grain> {
    vec![Grain::knock(0.0, 380.0, 0.035, 0.24), Grain::note(0.004, 88.0, 0.030, 0.07)]
}

/// **A token home.** Two notes going up a fifth — the one place a *note* is
/// right, because this is meant to sound like an announcement rather than like
/// something being hit.
pub fn home() -> Vec<Grain> {
    vec![Grain::note(0.0, 523.25, 0.10, 0.20), Grain::note(0.09, 783.99, 0.16, 0.20)]
}
