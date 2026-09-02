//! # playground — somewhere to try things
//!
//! ```text
//!     cargo run -p studio --release --bin playground
//! ```
//!
//! The scene itself lives in [`studio::world`], not in here. This file is only
//! the **window and the keys** — which is what made it possible for a second
//! program, outside this workspace, to drive exactly the same world with a
//! sound card attached. See `live/` at the top of the repository.
//!
//! ```text
//!     studio/src/world.rs      the world: what moves, and what it sounds like
//!     studio/src/bin/          this: a window, keys, and no audio device
//!     live/                    the same world, with a speaker
//! ```
//!
//! Two ways to get at the library, and they are equivalent:
//!
//! ```text
//!     use studio::prelude::*;                   // everything, one line
//!
//!     use plotkit::{Cx, Frame, Shape};          // or name what you want
//!     use physics::{Fall, Oscillator};
//!     use shapes::{bough, wave, Wave, Wind};
//!     use studio::Graph;
//! ```
//!
//! Where things live, since it is reasonable to wonder:
//!
//! | | |
//! |---|---|
//! | `plotkit` | the drawing — `Cx`, `Shape`, `Frame`, `View`, `Canvas` |
//! | `shapes` | things to draw — digits, faces, waves, cyclones |
//! | `physics` | how things move — `Oscillator`, `Fall`, `Trigger` |
//! | `sound` | how things sound — `Tone`, `Mixer`, `kit` |
//! | `studio` | the window — `Graph`, `Sketch`, `Keys`, `Tape`, `world` |
//!
//! `Frame` is plotkit's because a frame is a drawing. `Graph` is studio's
//! because it owns a window. Nothing is defined twice.
//!
//! ## Waves
//!
//! A wave has **no ends**. `Wave::shape` is a [`plotkit::Shape::graph`], which
//! is sampled against whatever is on screen — so it runs off both edges
//! however far you pan or zoom, and there is no start, no finish and no sample
//! count for anybody to pick.
//!
//! ```text
//!     Wave::sine()                    sin(x): amplitude 1, wavelength 2pi
//!         .amplitude(2.0)             how tall
//!         .wavelength(3.0)            one whole wave, end to end
//!         .phase(0.5)                 how far along it starts
//!         .from(Cx::new(1.0, 2.0))    where x is measured from, and the
//!                                     line it waves about
//! ```
//!
//! `f.place(wave, at)` does the last one for you. It **rebuilds** the wave
//! about that point rather than shifting it, because shifting a thing with no
//! ends drags the samples sideways and leaves a bare strip at one edge.
//!
//! ## Hearing it
//!
//! The world is **already making sound** as you watch — the ball's landing,
//! the tree's creaks, the wind's level are all being worked out every frame
//! and put in `world.sounds`. This program simply throws them away, because it
//! has nowhere to send them.
//!
//! Two ways to actually hear something:
//!
//! ```text
//!     press 5     writes playground.wav and hands it to whatever the machine
//!                 has. A recording: it cannot change once it has started, and
//!                 it cannot click on the frame the ball lands.
//!
//!     live/       the same world, live. Bounces happen ON the frame they
//!                 happen, and the wind changes while it is blowing.
//! ```
//!
//! **Nothing is linked for either of those from here.** Linking an audio
//! library would mean the whole repository needs a C library present before it
//! will compile, on every machine, for the sake of one file. `live/` is a
//! separate crate outside the workspace precisely so that cost lands only on
//! whoever wants to hear it.

use sound::{speaker, wav, RATE};
use studio::world::{scene, World};
use studio::Graph;

fn main() {
    Graph::new("playground")
        .scale(44.0)
        .with(World::new())
        .each_frame(World::advance)
        // Nothing drains `world.sounds` here, and that is the point: a world
        // that nobody is listening to costs nothing and keeps a bounded list.
        .on_hold('>', |a| a.wind = (a.wind + 0.06).min(30.0))
        .on_hold('<', |a| a.wind = (a.wind - 0.06).max(-30.0))
        .on('0', |a| a.wind = 0.0)
        .on_hold('u', |a| a.g = (a.g + 0.12).min(60.0))
        .on_hold('j', |a| a.g = (a.g - 0.12).max(0.0))
        .on('m', |a| a.g = physics::fall::gravity::MOON)
        .on('e', |a| a.g = physics::fall::gravity::EARTH)
        // The sound. Same recipe drawn and heard, so 1-4 change both at once.
        .on('1', |a| a.voice = 0)
        .on('2', |a| a.voice = 1)
        .on('3', |a| a.voice = 2)
        .on('4', |a| a.voice = 3)
        // Write the note, then hand it to whatever the machine has. Nothing
        // is linked for this: see `sound::speaker`.
        .on('5', |a| {
            let note = a.tone();
            let path = "playground.wav";
            if let Err(e) = wav::write(path, &note.samples(2.0, RATE), RATE) {
                eprintln!("could not write {path}: {e}");
                return;
            }
            // `false`: do not wait. The window has to keep drawing, and a
            // two-second note would freeze it solid.
            match speaker::play_file(path, false) {
                Ok(player) => println!("playing the note you are looking at ({player})"),
                Err(e) => {
                    eprintln!("{e}");
                    eprintln!("  the file is there though: {path}");
                }
            }
        })
        .run(scene);
}
