//! # playground — somewhere to try things
//!
//! ```text
//!     cargo run -p studio --release --bin playground
//! ```
//!
//! Two ways to get at the library, and they are equivalent:
//!
//! ```text
//!     use studio::prelude::*;                   // everything, one line
//!
//!     use plotkit::{Cx, Frame};                 // or name what you want
//!     use shapes::{digit, face, wave, Wave};
//!     use studio::Graph;
//! ```
//!
//! Where things live, since it is reasonable to wonder:
//!
//! | | |
//! |---|---|
//! | `plotkit` | the drawing — `Cx`, `Shape`, `Frame`, `View`, `Canvas` |
//! | `shapes` | things to draw — digits, faces, waves, cyclones |
//! | `studio` | the window — `Graph`, `Sketch`, `Keys`, `Tape` |
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

use plotkit::{Cx, Frame};
use shapes::{face, wave, Wave};
use studio::Graph;

fn main() {
    Graph::new("playground").scale(46.0).animate(scene);
}

fn scene(t: f64) -> Frame {
    let mut f = Frame::new();

    // --- the simplest thing there is --------------------------------------
    // sin(x), all the way across, at the origin.
    f.add(Wave::sine()).color(0x4FBCD4).width(2);

    // --- one you have described -------------------------------------------
    // Taller, longer, and waving about y = 3 starting from x = -6.
    f.place(Wave::sine().amplitude(1.5).wavelength(5.0), Cx::new(-6.0, 3.0)).color(0xE0A44A).width(2);

    // --- one that moves ----------------------------------------------------
    // A wave IS a value, so animating it is changing a number, not rebuilding
    // a curve. Here the phase runs, which is what makes it travel.
    f.place(Wave::sine().amplitude(0.8).wavelength(2.5).phase(-t * 3.0), Cx::new(0.0, -3.0))
        .color(0xE585AC)
        .width(2);

    // --- and adding them ---------------------------------------------------
    // `sum` adds the FUNCTIONS, not the pictures. Adding two plots would only
    // mean drawing both; adding two functions makes a third that neither one
    // is — and that is the whole of Fourier. These are the first three terms
    // of a square wave.
    let square = [
        Wave::sine().amplitude(1.0).frequency(1.0),
        Wave::sine().amplitude(1.0 / 3.0).frequency(3.0),
        Wave::sine().amplitude(1.0 / 5.0).frequency(5.0),
    ];
    f.place(wave::sum(&square), Cx::new(0.0, -6.5)).color(0x6FCF97).width(2);

    // --- something that is not a wave, for scale ---------------------------
    f.place(face::smiley(0.8), Cx::new(7.0, 6.0)).color(0x9B7BD4).width(2);

    f.label(Cx::new(-4.5, 1.3), "Wave::sine()", 0x4FBCD4, 2);
    f.label(Cx::new(-4.5, 4.8), "amplitude 1.5, wavelength 5", 0xE0A44A, 2);
    f.label(Cx::new(-4.5, -1.7), "phase running: it travels", 0xE585AC, 2);
    f.label(Cx::new(-4.5, -5.1), "sum of 1, 1/3, 1/5", 0x6FCF97, 2);
    f
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use plotkit::View;

    #[test]
    fn the_scene_draws_something_that_moves() {
        assert!(!scene(0.0).is_empty());
        let v = View::centred(400, 400, 20.0);
        let ink = |t: f64| {
            let mut c = plotkit::Canvas::new(400, 400);
            c.clear(0);
            scene(t).draw(&mut c, &v);
            c.buf.iter().filter(|&&p| p != 0).count()
        };
        assert_ne!(ink(0.0), ink(0.7), "the travelling wave should have moved");
    }
}

