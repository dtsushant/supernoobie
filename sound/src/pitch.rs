//! # pitch — how a note gets its number
//!
//! ## The formula
//!
//! ```text
//!     f(n)  =  440 · 2^(n/12)
//! ```
//!
//! `n` is how many semitones you are from the A above middle C. Twelve of them
//! multiply up to exactly `2`, which is an octave.
//!
//! ## What it does, and why it is a compromise
//!
//! Pitch is **multiplicative, not additive**. Going up an octave doubles the
//! frequency, so going up two octaves quadruples it. That is why the formula
//! has a power in it and not a sum, and it is why the piano keys get closer
//! together in frequency as you go down and further apart as you go up, while
//! *looking* evenly spaced.
//!
//! The ear likes **small whole-number ratios**. Two notes an octave apart are
//! `2:1`. A perfect fifth is `3:2`. A major third is `5:4`. Play those exactly
//! and they lock together, because their harmonics line up.
//!
//! The trouble is that you cannot have all of them at once. Stack twelve
//! perfect fifths of `3:2` and you should arrive back where you started, seven
//! octaves up. You do not:
//!
//! ```text
//!     (3/2)¹²  =  129.746…          2⁷  =  128
//! ```
//!
//! They miss by about a quarter of a semitone — the **Pythagorean comma** — and
//! no amount of cleverness closes the gap, because a power of 3 can never be a
//! power of 2. So a choice has to be made about where to hide the error.
//!
//! **Equal temperament** hides it evenly: make every semitone the same ratio,
//! `2^(1/12)`, and spread the mistake over all of them. Now nothing is exactly
//! right except the octave, but nothing is badly wrong either, and every key
//! sounds the same as every other. Bach's *Well-Tempered Clavier* — two books
//! of preludes and fugues in all twenty-four keys — is a demonstration that
//! this is worth the trade.
//!
//! Our fifth is `2^(7/12) = 1.4983` rather than `1.5`. Two cents flat, and
//! almost nobody can hear it.
//!
//! ## How it is used here
//!
//! To turn a note you can name into a frequency a [`crate::Tone`] can play.

/// Concert A — the note orchestras tune to, and the peg everything else hangs
/// on. Fixed at 440 Hz by international agreement in 1955; before that it
/// wandered from about 415 to 450 depending on the country and the century.
pub const A4: f64 = 440.0;

/// The ratio between neighbouring semitones: the twelfth root of two.
///
/// Twelve of them multiply to exactly 2, which is the entire design.
pub const SEMITONE: f64 = 1.059_463_094_359_295_3;

/// The frequency `n` semitones from concert A. Negative goes down.
pub fn from_a(n: f64) -> f64 {
    A4 * (n / 12.0).exp2()
}

/// The frequency of a named note, like `"A4"`, `"C#5"`, `"Eb3"`.
///
/// Octaves change at C, not at A — which is why A4 is 440 but C4 is below it.
/// That is a historical accident and catches everybody once.
pub fn named(name: &str) -> Option<f64> {
    let mut chars = name.chars();
    let letter = chars.next()?.to_ascii_uppercase();
    // Semitones from C, for each letter. The gaps of 1 are where there is no
    // black key: B–C and E–F.
    let base = match letter {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let rest: String = chars.collect();
    let (accidental, octave) = match rest.chars().next() {
        Some('#') => (1, &rest[1..]),
        Some('b') => (-1, &rest[1..]),
        _ => (0, &rest[..]),
    };
    let octave: i32 = octave.trim().parse().ok()?;

    // A4 is 9 semitones above C4, so C4 is -9 from our reference.
    let from_c4 = (octave - 4) * 12 + base + accidental;
    Some(from_a(f64::from(from_c4) - 9.0))
}

/// How far apart two frequencies are, in **cents** — hundredths of a
/// semitone, and roughly the smallest difference a good ear can pick out.
///
/// `1200 · log₂(b/a)`, because pitch is multiplicative: the *ratio* is what
/// the ear hears, so the difference is a logarithm.
pub fn cents(a: f64, b: f64) -> f64 {
    1200.0 * (b / a).log2()
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concert_a_is_where_everything_hangs_from() {
        assert!((from_a(0.0) - 440.0).abs() < 1e-12);
        assert!((named("A4").expect("A4") - 440.0).abs() < 1e-9);
    }

    /// ★ Twelve semitones make exactly one octave, and an octave is exactly
    /// double. That exactness is the whole point of equal temperament — it is
    /// the one interval that is not a compromise.
    #[test]
    fn twelve_semitones_double_the_frequency() {
        assert!((from_a(12.0) / from_a(0.0) - 2.0).abs() < 1e-12);
        assert!((from_a(-12.0) / from_a(0.0) - 0.5).abs() < 1e-12);
        assert!((SEMITONE.powi(12) - 2.0).abs() < 1e-12, "the twelfth root of two, twelve times");
    }

    /// ★ And the compromise, in one number: our perfect fifth is not perfect.
    ///
    /// The ear wants `3:2`. Equal temperament gives `2^(7/12) = 1.4983`, two
    /// cents flat. It has to: a power of three can never be a power of two, so
    /// twelve true fifths overshoot seven octaves by the Pythagorean comma and
    /// the error has to be hidden somewhere. Spreading it evenly is the deal
    /// that lets every key sound alike.
    #[test]
    fn the_fifth_is_deliberately_slightly_wrong() {
        let ours = from_a(7.0) / from_a(0.0);
        let pure = 3.0 / 2.0;
        assert!((ours - 1.498_3).abs() < 1e-4, "our fifth is {ours}");
        assert!(ours < pure, "it should be flat of pure");
        assert!(cents(pure, ours).abs() < 2.5, "but by only about two cents");
        assert!(cents(pure, ours).abs() > 1.0, "and it really is off, not exact");

        // The reason: twelve true fifths miss seven octaves. The comma.
        let comma = cents(2f64.powi(7), 1.5f64.powi(12));
        assert!((comma - 23.46).abs() < 0.1, "the Pythagorean comma is about 23.5 cents, got {comma}");
    }

    /// Octaves change at C, not at A. Everybody trips over this once.
    #[test]
    fn the_octave_number_changes_at_c() {
        let c4 = named("C4").expect("C4");
        let b3 = named("B3").expect("B3");
        assert!(c4 > b3, "C4 is just above B3");
        assert!((c4 / b3 - SEMITONE).abs() < 1e-9, "and one semitone above it");
        assert!(c4 < 440.0, "C4 is below concert A, despite the same number");
        assert!((named("C5").expect("C5") / c4 - 2.0).abs() < 1e-9);
    }

    #[test]
    fn sharps_and_flats_meet_in_the_middle() {
        // In equal temperament — and only in equal temperament — these are the
        // same note. In a tuning with true fifths they are not.
        assert!((named("C#4").expect("c sharp") - named("Db4").expect("d flat")).abs() < 1e-9);
    }

    #[test]
    fn nonsense_is_refused_rather_than_guessed_at() {
        assert!(named("H4").is_none());
        assert!(named("A").is_none());
        assert!(named("").is_none());
    }

    /// Cents are a logarithm because pitch is a ratio. A semitone is 100 of
    /// them by construction.
    #[test]
    fn a_semitone_is_a_hundred_cents() {
        assert!((cents(440.0, from_a(1.0)) - 100.0).abs() < 1e-9);
        assert!((cents(440.0, 880.0) - 1200.0).abs() < 1e-9);
        assert!(cents(440.0, 440.0).abs() < 1e-12);
    }
}
