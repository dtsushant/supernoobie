// A playground is half commented out most of the time, so an import waiting
// to be used again is the normal state rather than a mistake.
#![allow(unused_imports, unused_variables)]

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
use shapes::{bough, wave, Wave, Wind};
use shapes::digit::glyph;
use shapes::face::smiley;
use studio::Graph;

const TREE: u32 = 0x8FBF6A;
const MODE: u32 = 0x4A6B56;
const GUST: u32 = 0x7FA6C4;

/// A colour dimmed toward the background, for fading a gust in and out.
fn fade(c: u32, amount: f64) -> u32 {
    let k = amount.clamp(0.0, 1.0);
    let mix = |shift: u32| (((c >> shift) & 255) as f64 * k) as u32;
    (mix(16) << 16) | (mix(8) << 8) | mix(0)
}

/// How stiff the tree is. Bigger resists the wind harder.
const STIFFNESS: f64 = 1.4;

struct Air {
    t: f64,
    /// What you have asked for with the arrow keys.
    wind: f64,
}

impl Air {
    /// The wind right now: what you asked for, gusting.
    ///
    /// Real wind is never steady, and a steady one leans the tree to an angle
    /// and leaves it there, which shows nothing.
    fn blowing(&self) -> Wind {
        let gust = 1.0 + 0.35 * (self.t * 0.9).sin() + 0.18 * (self.t * 2.3).sin();
        Wind::new(self.wind * gust)
    }
}

fn main() {
    Graph::new("playground")
        .scale(44.0)
        .with(Air { t: 0.0, wind: 2.2 })
        .each_frame(|a, t| a.t = t)
        .on_hold('>', |a| a.wind = (a.wind + 0.06).min(30.0))
        .on_hold('<', |a| a.wind = (a.wind - 0.06).max(-30.0))
        .on('0', |a| a.wind = 0.0)
        .run(scene);
}

fn scene(a: &Air) -> Frame {
    let (t, w) = (a.t, a.blowing());
    let mut f = Frame::new();

    // --- the wind ----------------------------------------------------------
    // Gusts: short pieces of wave, drifting downwind and fading. A gust HAS
    // ends; a Wave does not. That is the whole difference between them.
    for (gust, bright) in w.gusts(14, Cx::new(-11.0, -8.0), Cx::new(11.0, 9.0), t) {
        f.add(gust).color(fade(GUST, bright)).width(1);
    }

    // --- the simplest thing there is --------------------------------------
    // sin(x), all the way across, at the origin.
    f.add(Wave::sine()).color(0x4FBCD4).width(2);

    // --- one that moves ----------------------------------------------------
    // A wave IS a value, so animating it is changing a number, not rebuilding
    // a curve. Here the phase runs, which is what makes it travel.
    // f.place(Wave::sine().amplitude(0.8).wavelength(2.5).phase(-t * 3.0), Cx::new(0.0, -3.5))
    //     .color(0xE585AC)
    //     .width(2);

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
    //f.place(smiley(2.0),Cx::new(1.0,3.0));
  //  f.place(glyph(7, 40),Cx::new(-3.0,3.0));
    //f.place(wave::sum(&square), Cx::new(0.0, -6.8)).color(0x6FCF97).width(2);

    // --- a tree, which is also a sum of waves ------------------------------
    // Not decorated with waves — MADE of them. A branch held at one end and
    // free at the other bends in the modes sin((2n-1) pi s / 2L): a quarter
    // wave, three quarters, five quarters. Its shape at any instant is those
    // added up, which is exactly `wave::total`. What the clock changes is how
    // much of each, so the space and the time separate and it stays a sum of
    // waves rather than a new curve every frame.
    // Upright in calm air, leaning by however far the wind wins the argument.
    // `lean` is the deflection, so it composes with whatever direction the
    // trunk grows in — turn `upright` and the whole thing tilts with it.
    let upright = PI / 2.0;
    let angle = upright - w.lean(STIFFNESS);
    for (level, boughs) in
        bough::tree(Cx::new(1.5, -8.4), angle, 2.3, 6, 0.5, w.shake(0.06), t).into_iter().enumerate()
    {
        // Thick trunk, thin twigs.
        f.add(boughs).color(TREE).width((6 - level as i32).max(1));
    }

    // The three modes one branch is bending in, drawn on their own so the
    // sum above has something to be the sum OF.
    let ms: Vec<Wave> = bough::modes(2.3, w.shake(0.06), 0.0, t).iter().map(|m| m.amplitude(m.a * 7.0)).collect();
    for (n, m) in ms.iter().enumerate() {
        f.place(*m, Cx::new(-11.0, 7.9 - n as f64 * 1.1)).color(MODE).width(1);
    }
    f.place(wave::sum(&ms), Cx::new(-11.0, 4.2)).color(TREE).width(2);

    // f.label(Cx::new(-6.0, 1.3), "Wave::sine()", 0x4FBCD4, 2);
    // f.label(Cx::new(-6.0, -2.2), "phase running: it travels", 0xE585AC, 2);
    // f.label(Cx::new(-6.0, -5.3), "sum of 1, 1/3, 1/5", 0x6FCF97, 2);
    // f.label(Cx::new(-6.6, 8.8), "the modes one branch bends in", MODE, 2);
    // f.label(Cx::new(-6.6, 3.2), "their sum: the bend", TREE, 2);
    // Pinned rather than laid on the drawing: a caption is not part of the
    // picture, and would slide off the edge as soon as you panned.
    f.pin(plotkit::Anchor::Bottom, 0.0, -34.0, "and a tree of those", TREE, 2);
    // The waves run straight through the tree, and that is the point.
    f.pin(plotkit::Anchor::BottomRight, -14.0, -14.0, "a wave has no ends -- it crosses everything", 0x46525E, 2);

    f.pin(plotkit::Anchor::TopRight, -14.0, 12.0, format!("wind {:+.1}   push {:+.1} (goes as v^2)", w.speed, w.pressure()), GUST, 2);
    f.pin(
        plotkit::Anchor::TopRight,
        -14.0,
        30.0,
        format!("leaning {:.0} deg from upright   trunk at {:.0} deg", w.lean(STIFFNESS).to_degrees().abs(), angle.to_degrees()),
        GUST,
        2,
    );
    f.pin(plotkit::Anchor::TopRight, -14.0, 48.0, "left / right for wind    0 for calm", 0x46525E, 2);
    f
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use plotkit::View;

    #[test]
    fn the_scene_draws_something_that_moves() {
        let air = |t: f64| Air { t, wind: 2.2 };
        assert!(!scene(&air(0.0)).is_empty());
        let v = View::centred(400, 400, 20.0);
        let ink = |t: f64| {
            let mut c = plotkit::Canvas::new(400, 400);
            c.clear(0);
            scene(&air(t)).draw(&mut c, &v);
            c.buf.iter().filter(|&&p| p != 0).count()
        };
        assert_ne!(ink(0.0), ink(0.7), "the travelling wave should have moved");
    }

    /// ★ Harder wind lays the tree further over, and no wind ever lays it past
    /// flat — the saturation comes from the geometry, not from a clamp.
    #[test]
    fn the_wind_lays_the_tree_over_but_never_past_flat() {
        let angle = |wind: f64| Air { t: 0.0, wind }.blowing().trunk_angle(STIFFNESS);
        assert!((angle(0.0) - PI / 2.0).abs() < 1e-9, "calm leaves it upright");
        assert!(angle(3.0) < angle(1.0), "more wind, further over");
        assert!(angle(300.0) > 0.0, "but never past flat");
        assert!(angle(300.0) < 0.02, "though very nearly");
    }

    /// A gust is drawn only when there is wind to carry it.
    #[test]
    fn calm_air_has_no_gusts() {
        assert!(Air { t: 3.0, wind: 0.0 }.blowing().gusts(10, Cx::ZERO, Cx::new(1.0, 1.0), 3.0).is_empty());
    }
}





