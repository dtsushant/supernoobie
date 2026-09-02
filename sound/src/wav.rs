//! # wav — writing a sound to a file, by hand
//!
//! ## What a WAV file is
//!
//! Almost nothing. A little header saying how the numbers are arranged, and
//! then the numbers:
//!
//! ```text
//!     "RIFF"  <how many bytes follow>  "WAVE"
//!     "fmt "  16  1  channels  rate  byterate  align  bits
//!     "data"  <how many bytes of sound>  then the samples
//! ```
//!
//! That is the whole format. Written out here rather than pulled in, for the
//! same reason [`plotkit::raster`](../../plotkit/raster/index.html) writes its
//! own PNGs: a file format you have written once is a thing you understand,
//! and this one is forty lines.
//!
//! ## RIFF, and the chunks
//!
//! **RIFF** — Resource Interchange File Format — was Microsoft and IBM's 1991
//! answer to a real problem: how do you add something to a file format later
//! without breaking every program that reads it? The answer is to make a file
//! a list of **chunks**, each one a four-letter name and a length. A reader
//! that meets a chunk it does not know skips over it by its length and carries
//! on. AVI, WAV and several others are all RIFF underneath.
//!
//! It is the same idea as PNG's chunks, arrived at independently, and it is
//! why files from 1991 still open.
//!
//! ## Why the samples are integers
//!
//! Sound here is computed as `f64` between −1 and 1, because that is
//! convenient to think in. A WAV stores **16-bit integers**, −32768 to 32767 —
//! 65536 steps, which is about 96 dB between the quietest step and the loudest
//! sound, comfortably past what an ear can pick out in one sitting.
//!
//! Converting is one multiply, and one piece of care: anything past ±1 has to
//! be **clamped**, not allowed to wrap. A sample that wraps does not get
//! quieter, it jumps to full volume the other way, and that is a vicious
//! crack rather than a gentle distortion.

use std::fs::File;
use std::io::{BufWriter, Write};

/// Write samples, each between −1 and 1, as a 16-bit mono WAV.
pub fn write(path: &str, samples: &[f64], rate: u32) -> std::io::Result<()> {
    let mut w = BufWriter::new(File::create(path)?);
    for part in chunks(samples, rate) {
        w.write_all(&part)?;
    }
    w.flush()
}

/// The bytes of the file, in order — split out from the writing so the format
/// can be checked without touching a disk.
pub fn chunks(samples: &[f64], rate: u32) -> Vec<Vec<u8>> {
    const CHANNELS: u16 = 1;
    const BITS: u16 = 16;

    let data_len = (samples.len() * 2) as u32;
    let align = CHANNELS * BITS / 8;
    let byte_rate = rate * u32::from(align);

    let mut header = Vec::new();
    header.extend(b"RIFF");
    // Everything after this field: 4 for "WAVE", 24 for the fmt chunk, 8 for
    // the data header, then the sound.
    header.extend((36 + data_len).to_le_bytes());
    header.extend(b"WAVE");

    header.extend(b"fmt ");
    header.extend(16u32.to_le_bytes()); // how long this chunk is
    header.extend(1u16.to_le_bytes()); // 1 means plain uncompressed samples
    header.extend(CHANNELS.to_le_bytes());
    header.extend(rate.to_le_bytes());
    header.extend(byte_rate.to_le_bytes());
    header.extend(align.to_le_bytes());
    header.extend(BITS.to_le_bytes());

    header.extend(b"data");
    header.extend(data_len.to_le_bytes());

    let body: Vec<u8> = samples.iter().flat_map(|s| to_i16(*s).to_le_bytes()).collect();
    vec![header, body]
}

/// One sample, as the integer that goes in the file.
///
/// **Clamped, not wrapped.** A sample past ±1 that wraps does not get quieter,
/// it leaps to full volume the other way — a crack rather than a distortion,
/// and the single nastiest sound a synthesiser can make.
fn to_i16(s: f64) -> i16 {
    (s.clamp(-1.0, 1.0) * f64::from(i16::MAX)) as i16
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(samples: &[f64], rate: u32) -> Vec<u8> {
        chunks(samples, rate).concat()
    }

    fn word(b: &[u8], at: usize) -> u32 {
        u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
    }

    /// ★ The lengths in the header have to match the file that follows. Get
    /// one wrong and players do one of three things — refuse it, play silence,
    /// or play the header as a burst of noise — and which one tells you
    /// nothing about what went wrong.
    #[test]
    fn the_header_describes_the_file_that_follows() {
        let b = bytes(&[0.0; 100], 44_100);
        assert_eq!(&b[0..4], b"RIFF");
        assert_eq!(&b[8..12], b"WAVE");
        assert_eq!(&b[12..16], b"fmt ");
        assert_eq!(&b[36..40], b"data");

        assert_eq!(b.len(), 44 + 200, "44 bytes of header, two per sample");
        assert_eq!(word(&b, 4) as usize, b.len() - 8, "the RIFF size counts everything after itself");
        assert_eq!(word(&b, 40) as usize, 200, "the data size counts the samples");
    }

    /// The rate and the byte rate have to agree, or it plays at the wrong
    /// speed — which sounds like the wrong notes, not like a broken file.
    #[test]
    fn the_rate_is_written_consistently() {
        for rate in [8_000u32, 22_050, 44_100, 48_000] {
            let b = bytes(&[0.0; 8], rate);
            assert_eq!(word(&b, 24), rate, "the sample rate");
            assert_eq!(word(&b, 28), rate * 2, "and the byte rate: one 16-bit mono sample each");
        }
    }

    /// ★ Loud samples are clamped, never wrapped. A wrapped sample leaps to
    /// full volume the other way — the nastiest sound a synthesiser makes, and
    /// it comes from one missing `clamp`.
    #[test]
    fn too_loud_is_clamped_and_never_wrapped() {
        assert_eq!(to_i16(2.0), i16::MAX, "way over should sit at the top");
        assert_eq!(to_i16(-2.0), -i16::MAX, "and way under at the bottom");
        assert_eq!(to_i16(1.0), i16::MAX);
        assert_eq!(to_i16(0.0), 0);

        // The thing that must never happen: loud coming out quiet, or the
        // wrong sign.
        for over in [1.01, 1.5, 9.0, 1e9] {
            assert!(to_i16(over) > 30_000, "{over} came out as {}", to_i16(over));
            assert!(to_i16(-over) < -30_000);
        }
    }

    /// Quiet samples keep their detail rather than being rounded to nothing.
    #[test]
    fn quiet_samples_survive() {
        assert!(to_i16(0.5) > 16_000);
        assert!(to_i16(0.001) > 30, "a thousandth should still be tens of steps");
        assert_eq!(to_i16(-0.5), -to_i16(0.5), "and be symmetric about silence");
    }

    /// An empty sound is a valid file, not a crash — a note of zero length is
    /// an easy thing to ask for by accident.
    #[test]
    fn a_silent_file_is_still_a_file() {
        let b = bytes(&[], 44_100);
        assert_eq!(b.len(), 44);
        assert_eq!(word(&b, 40), 0);
        assert_eq!(word(&b, 4) as usize, 36);
    }

    #[test]
    fn it_actually_writes_to_disk() {
        let path = format!("{}/plotkit-test.wav", std::env::temp_dir().display());
        write(&path, &[0.0, 0.5, -0.5, 1.0], 8_000).expect("write");
        let back = std::fs::read(&path).expect("read");
        assert_eq!(back, bytes(&[0.0, 0.5, -0.5, 1.0], 8_000));
        let _ = std::fs::remove_file(&path);
    }
}
