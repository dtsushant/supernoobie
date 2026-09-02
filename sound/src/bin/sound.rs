//! # sound — write a demonstration to a WAV file
//!
//! ```text
//!     cargo run -p sound --release -- out.wav
//! ```
//!
//! About eight seconds: a pure tone, the same note on three timbres, the notes
//! of a chord one at a time, and then the chord. Play it with anything.
//!
//! ```text
//!     cargo run -p sound --release -- out.wav                 just writes it
//!     cargo run -p sound --features play --release -- --play  and plays it
//! ```
//!
//! Playing is behind a feature because it is the one part that needs a sound
//! card and a C library. A file, by contrast, can be listened to, looked at,
//! and checked.

use sound::{after, mix, pitch, wav, Timbre, Tone, RATE};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let wants_sound = args.iter().any(|a| a == "--play");
    let path = args.iter().find(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| "sound.wav".to_string());
    let note = |name: &str| pitch::named(name).unwrap_or(pitch::A4);

    // One sine and nothing else, so you know what "pure" sounds like before
    // anything is added to it.
    let pure = Tone::pluck(note("A4")).with_timbre(Timbre::pure()).with_decay(0.8).samples(1.0, RATE);

    // The same note, three ways. The pitch never changes — only the recipe of
    // harmonics — and that is the whole of timbre.
    let soft = Tone::bowed(note("A4")).with_timbre(Timbre::triangle(15)).samples(1.0, RATE);
    let woody = Tone::bowed(note("A4")).with_timbre(Timbre::clarinet(15)).samples(1.0, RATE);
    let bright = Tone::pluck(note("A4")).with_timbre(Timbre::saw(20)).samples(1.0, RATE);

    let chord_notes: Vec<Vec<f64>> = ["C4", "E4", "G4", "C5"]
        .iter()
        .map(|n| Tone::pluck(note(n)).with_timbre(Timbre::saw(14)).with_decay(1.6).samples(2.0, RATE))
        .collect();

    // One at a time first, so the chord can be heard as its parts.
    let one_by_one = after(
        &["C4", "E4", "G4", "C5"]
            .iter()
            .map(|n| Tone::pluck(note(n)).with_timbre(Timbre::saw(14)).with_decay(0.5).samples(0.35, RATE))
            .collect::<Vec<_>>(),
        0.02,
        RATE,
    );

    // Then together: adding the functions, not playing four things at once.
    let together = mix(&chord_notes);

    let all = after(&[pure, soft, woody, bright, one_by_one, together], 0.25, RATE);

    match wav::write(&path, &all, RATE) {
        Ok(()) => {
            println!("wrote {path}  --  {:.1} seconds at {RATE} Hz", all.len() as f64 / f64::from(RATE));
            println!("  a pure sine; then a triangle, a clarinet and a sawtooth on the SAME note;");
            println!("  then C E G C one at a time, then all four at once.");
        }
        Err(e) => eprintln!("could not write {path}: {e}"),
    }

    if wants_sound {
        play_it(&all);
    } else {
        println!("  (add --play to hear it, with: cargo run -p sound --features play)");
    }
}

#[cfg(feature = "play")]
fn play_it(samples: &[f64]) {
    println!("playing...");
    if let Err(e) = sound::speaker::play(samples, RATE) {
        eprintln!("could not play it: {e}");
    }
}

#[cfg(not(feature = "play"))]
fn play_it(_: &[f64]) {
    eprintln!("this build cannot play sound. Rebuild with:  --features play");
}
