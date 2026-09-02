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
//!     use shapes::{bough, wave, Wave};
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
//!
//! ## The tree is also a sum of waves
//!
//! Not decorated with them — **made** of them, and that is not a trick. A
//! branch held at one end and free at the other bends in the modes
//! `sin((2n−1)πs/2L)`: a quarter wave, three quarters, five quarters. Each is
//! zero at the base, because that end is held, and steepest at the tip,
//! because that end is not. Its shape at any instant is those added up.
//!
//! What the clock changes is **how much of each**, so the space part and the
//! time part separate and it stays a sum of waves instead of becoming a new
//! curve every frame. Amplitudes fall off as `1/n²`, which is why a branch
//! sways rather than buzzes.
//!
//! Left of the tree you can see the three modes on their own, and under them
//! their sum — which is the bend the trunk is drawn with.

use plotkit::{Cx, Frame};
use std::f64::consts::PI;
use shapes::{bough, wave, Wave};
use studio::Graph;

const TREE: u32 = 0x8FBF6A;
const MODE: u32 = 0x4A6B56;

fn main() {
    Graph::new("playground").scale(46.0).animate(scene);
}

fn scene(t: f64) -> Frame {
    let mut f = Frame::new();

    // --- the simplest thing there is --------------------------------------
    // sin(x), all the way across, at the origin.
    f.add(Wave::sine()).color(0x4FBCD4).width(2);

    // --- one that moves ----------------------------------------------------
    // A wave IS a value, so animating it is changing a number, not rebuilding
    // a curve. Here the phase runs, which is what makes it travel.
    f.place(Wave::sine().amplitude(0.8).wavelength(2.5).phase(-t * 3.0), Cx::new(0.0, -3.5))
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
    f.place(wave::sum(&square), Cx::new(0.0, -6.8)).color(0x6FCF97).width(2);

    // --- a tree, which is also a sum of waves ------------------------------
    // Not decorated with waves — MADE of them. A branch held at one end and
    // free at the other bends in the modes sin((2n-1) pi s / 2L): a quarter
    // wave, three quarters, five quarters. Its shape at any instant is those
    // added up, which is exactly `wave::total`. What the clock changes is how
    // much of each, so the space and the time separate and it stays a sum of
    // waves rather than a new curve every frame.
    for (level, boughs) in bough::tree(Cx::new(6.6, -8.4), PI / 2.0, 2.3, 6, 0.5, 0.06, t).into_iter().enumerate() {
        // Thick trunk, thin twigs.
        f.add(boughs).color(TREE).width((6 - level as i32).max(1));
    }

    // The three modes one branch is bending in, drawn on their own so the
    // sum above has something to be the sum OF.
    let ms: Vec<Wave> = bough::modes(2.3, 0.06, 0.0, t).iter().map(|m| m.amplitude(m.a * 22.0)).collect();
    for (n, m) in ms.iter().enumerate() {
        f.place(*m, Cx::new(-11.0, 7.9 - n as f64 * 1.1)).color(MODE).width(1);
    }
    f.place(wave::sum(&ms), Cx::new(-11.0, 4.2)).color(TREE).width(2);

    f.label(Cx::new(-6.0, 1.3), "Wave::sine()", 0x4FBCD4, 2);
    f.label(Cx::new(-6.0, -2.2), "phase running: it travels", 0xE585AC, 2);
    f.label(Cx::new(-6.0, -5.3), "sum of 1, 1/3, 1/5", 0x6FCF97, 2);
    f.label(Cx::new(-6.6, 8.8), "the modes one branch bends in", MODE, 2);
    f.label(Cx::new(-6.6, 3.2), "their sum: the bend", TREE, 2);
    // Pinned rather than laid on the drawing: a caption is not part of the
    // picture, and would slide off the edge as soon as you panned.
    f.pin(plotkit::Anchor::BottomRight, -14.0, -34.0, "and a tree of those", TREE, 2);
    // The waves run straight through the tree, and that is the point.
    f.pin(plotkit::Anchor::BottomRight, -14.0, -14.0, "a wave has no ends -- it crosses everything", 0x46525E, 2);
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



