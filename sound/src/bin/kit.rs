//! Write every noise in the kit to a `.wav`, and print what it measures.
//!
//! ```text
//!     cargo run -p sound --bin kit
//! ```
//!
//! Because a test can tell you a sound has no pitch and dies away in half a
//! second, and it still cannot tell you it sounds like a die. Both are needed:
//! the numbers catch the thing nobody would hear until it was shipped, and the
//! ear catches the thing no number thought to ask about.

use sound::kit;
use sound::noise::{brightness, fades, knocks, length, peak, render, tonality, Grain};

const RATE: u32 = 44_100;

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let all: [(&str, Vec<Grain>); 4] =
        [("roll", kit::roll(11)), ("step", kit::step()), ("cut", kit::cut()), ("home", kit::home())];

    println!("{:<6} {:>7} {:>7} {:>7} {:>7} {:>7}", "", "peak", "bright", "pitch", "hits", "secs");
    for (name, grains) in &all {
        let samples = render(grains, RATE);
        println!(
            "{name:<6} {:>7.3} {:>7.3} {:>7.3} {:>7} {:>7.2}",
            peak(&samples),
            brightness(&samples),
            tonality(&samples),
            knocks(&samples, RATE),
            length(grains)
        );
        let wide: Vec<f64> = samples.iter().map(|s| *s as f64).collect();
        let path = format!("{out}/{name}.wav");
        match sound::wav::write(&path, &wide, RATE) {
            Ok(()) => println!("       -> {path}"),
            Err(e) => println!("       -- could not write {path}: {e}"),
        }
    }

    // And a whole turn, so the sounds can be heard against each other rather
    // than one at a time. Four of anything in a row is where a noise that is a
    // shade too long stops being short.
    let mut turn: Vec<Grain> = kit::roll(11);
    let mut at = 2.7;
    for _ in 0..4 {
        turn.extend(kit::step().iter().map(|g| Grain { at: g.at + at, ..*g }));
        at += 0.14;
    }
    turn.extend(kit::cut().iter().map(|g| Grain { at: g.at + at + 0.1, ..*g }));
    turn.extend(kit::home().iter().map(|g| Grain { at: g.at + at + 0.9, ..*g }));

    let samples = render(&turn, RATE);
    println!(
        "\na whole turn: {:.2}s, peak {:.3}, fades to {:.3}",
        length(&turn),
        peak(&samples),
        fades(&samples)
    );
    let wide: Vec<f64> = samples.iter().map(|s| *s as f64).collect();
    let path = format!("{out}/turn.wav");
    match sound::wav::write(&path, &wide, RATE) {
        Ok(()) => println!("       -> {path}"),
        Err(e) => println!("       -- could not write {path}: {e}"),
    }
}
