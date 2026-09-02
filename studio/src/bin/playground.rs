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
//! ## There is no speaker
//!
//! This draws sound and can save it. It does not **play** it, and nothing in
//! this repository does. Getting audio out of a machine needs a platform
//! library and a callback thread running at a rate nothing else here cares
//! about — a real dependency, and the first one past a window.
//!
//! Press `5` and it writes `playground.wav` into the working directory, which
//! any player will open. That is the whole of the audio output for now.
//!
//! ## One screen, six ideas
//!
//! This has outgrown a single view, and that is a fair sign it is working. If
//! it gets in your way, the honest fix is to split it: one binary per idea,
//! the way `waves`, `stage` and `sketch` already are. Nothing here would have
//! to change to do it, because every part is a value that draws itself.
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

use plotkit::{Cx, Frame, Shape};
use std::f64::consts::PI;
use physics::{fall::gravity, Fall, Oscillator};
use sound::{pitch, wav, Timbre, Tone, RATE};
use shapes::{bough, wave, Wave, Wind};
use shapes::digit::glyph;
use shapes::face::smiley;
use studio::Graph;

const TREE: u32 = 0x8FBF6A;
const MODE: u32 = 0x4A6B56;
const GUST: u32 = 0x7FA6C4;
const BALL: u32 = 0xE0A44A;
const TONE: u32 = 0xE585AC;
const SPEC: u32 = 0x9B7BD4;

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
    /// The clock last frame, so the oscillator can be stepped by a real `dt`.
    was: f64,
    /// What you have asked for with the arrow keys.
    wind: f64,
    /// The tree's answer to the wind: a damped second-order system, so it
    /// overshoots, wobbles and SETTLES rather than snapping to the new angle.
    /// This is the Laplace part — the poles decide how it gets there.
    tree: Oscillator,
    /// Gravity, for the ball.
    g: f64,
    /// Which recipe of harmonics the note is made of. The same list draws the
    /// spectrum, draws the waveform, and makes the sound.
    voice: usize,
}

impl Air {
    /// The wind right now: what you asked for, gusting.
    ///
    /// Real wind is never steady, and a steady one leans the tree to an angle
    /// and leaves it there, which shows nothing.
        /// Bring the tree up to date.
    ///
    /// The wind sets a target lean — the angle where the two moments balance.
    /// The oscillator is what actually gets there, and `rest_under` says a
    /// push of `f` settles at `f/ω²`, so pushing with `ω²·target` settles at
    /// the target. Take the wind away and it swings back and settles rather
    /// than snapping upright.
    fn advance(&mut self, t: f64) {
        let dt = (t - self.was).clamp(0.0, 1.0 / 20.0);
        (self.was, self.t) = (t, t);
        let target = self.blowing().lean(STIFFNESS);
        let stiff = self.tree.omega * self.tree.omega;
        self.tree.step(dt, stiff * target);
    }

    /// The four recipes, and what each one is like.
    fn voices() -> [(&'static str, Timbre); 4] {
        [
            ("pure: one sine, nothing else", Timbre::pure()),
            ("triangle: odd harmonics, 1/n^2 -- soft", Timbre::triangle(15)),
            ("clarinet: odd harmonics, 1/n -- woody", Timbre::clarinet(15)),
            ("sawtooth: every harmonic, 1/n -- bright", Timbre::saw(15)),
        ]
    }

    /// The note being shown. One value; three views of it.
    fn tone(&self) -> Tone {
        let (_, timbre) = Self::voices()[self.voice % 4].clone();
        Tone::pluck(pitch::named("A3").unwrap_or(pitch::A4)).with_timbre(timbre).with_decay(1.4)
    }

    fn blowing(&self) -> Wind {
        let gust = 1.0 + 0.35 * (self.t * 0.9).sin() + 0.18 * (self.t * 2.3).sin();
        Wind::new(self.wind * gust)
    }
}

fn main() {
    Graph::new("playground")
        .scale(44.0)
        .with(Air {
            t: 0.0,
            was: 0.0,
            wind: 2.2,
            // Lightly damped: a tree wobbles a few times before it stops.
            tree: Oscillator::new(2.6, 0.22),
            g: gravity::EARTH,
            voice: 3,
        })
        .each_frame(Air::advance)
        .on_hold('>', |a| a.wind = (a.wind + 0.06).min(30.0))
        .on_hold('<', |a| a.wind = (a.wind - 0.06).max(-30.0))
        .on('0', |a| a.wind = 0.0)
        .on_hold('u', |a| a.g = (a.g + 0.12).min(60.0))
        .on_hold('j', |a| a.g = (a.g - 0.12).max(0.0))
        .on('m', |a| a.g = gravity::MOON)
        .on('e', |a| a.g = gravity::EARTH)
        // The sound. Same recipe drawn and heard, so 1-4 change both at once.
        .on('1', |a| a.voice = 0)
        .on('2', |a| a.voice = 1)
        .on('3', |a| a.voice = 2)
        .on('4', |a| a.voice = 3)
        // NOTE: this writes a file. Nothing here plays audio — see the header.
        .on('5', |a| {
            let note = a.tone();
            match wav::write("playground.wav", &note.samples(2.0, RATE), RATE) {
                Ok(()) => {
                    let full = std::env::current_dir()
                        .map(|d| d.join("playground.wav").display().to_string())
                        .unwrap_or_else(|_| "playground.wav".to_string());
                    println!("wrote {full}");
                    println!("  nothing here plays it -- open it with any player to hear the note you are looking at");
                }
                Err(e) => eprintln!("could not write it: {e}"),
            }
        })
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
    // Not the static balance: where the OSCILLATOR has got to on its way
    // there. That is the difference between a tree that snaps to an angle and
    // one that swings past it and settles back.
    let upright = PI / 2.0;
    let angle = upright - a.tree.x;
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
    f.pin(
        plotkit::Anchor::TopRight,
        -14.0,
        48.0,
        format!(
            "zeta {:.2}  poles {:+.2}{:+.2}i  settles in {:.1}s",
            a.tree.zeta,
            a.tree.poles().0.re,
            a.tree.poles().0.im,
            a.tree.settling_time()
        ),
        0x9B7BD4,
        2,
    );

    // --- the sound, which is the same mathematics ---------------------------
    // One recipe, three views: the harmonics as uprights, the waveform they
    // add up to, and -- press 5 -- a file you can listen to. Adding the
    // harmonics is the same `wave::sum` that draws the square wave above.
    let note = a.tone();
    let (voice, _) = Air::voices()[a.voice % 4].clone();

    // The recipe. A clarinet's missing even harmonics are visibly missing.
    f.add(Shape::path(vec![Cx::new(-10.6, -4.4), Cx::new(-3.6, -4.4)])).color(0x3E4A55).width(1);
    f.place(note.spectrum().map(|z| Cx::new(z.re * 0.42, z.im * 2.2)), Cx::new(-10.6, -4.4)).color(SPEC).width(3);

    // What those add up to: two periods of the wave itself.
    f.place(note.waveform(2.0).map(|z| Cx::new(z.re * 3.3, z.im * 0.62)), Cx::new(-10.6, -6.4)).color(TONE).width(2);

    // --- a ball, with the gravity dial -------------------------------------
    // Galileo: distance goes as the SQUARE of the time, and mass does not come
    // into it. With air it stops speeding up and approaches a terminal
    // velocity -- the same e^(-t/tau) as the tree settling above.
    let air = Fall::in_air(a.g.max(0.01), 1.1);
    let from = Cx::new(-1.6, 8.2);
    let landing = air.hits(from, 0.0, -8.4).unwrap_or(6.0);
    let phase = t % (landing + 0.9);
    let ball = air.at(from, 0.0, phase.min(landing));

    f.add(air.path(from, 0.0, phase.min(landing))).color(0x3E4A55).width(1);
    f.add(Shape::circle(ball, 0.22)).color(BALL).width(2);
    f.pin(
        plotkit::Anchor::BottomLeft,
        14.0,
        -54.0,
        format!("g {:.2}   falls in {:.1}s   terminal {:.1}/s   (U/J, M moon, E earth)", a.g, landing, air.terminal_velocity()),
        BALL,
        2,
    );
    f.pin(plotkit::Anchor::BottomLeft, 14.0, -74.0, "s = 1/2 g t^2 -- Galileo. Mass does not come into it.", BALL, 2);
    f.pin(plotkit::Anchor::BottomLeft, 14.0, -114.0, format!("1-4 timbre: {voice}"), TONE, 2);
    f.pin(
        plotkit::Anchor::BottomLeft,
        14.0,
        -94.0,
        "bars are the harmonics, the curve is their sum -- 5 SAVES a wav (nothing here plays it)",
        SPEC,
        2,
    );

    f
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use plotkit::View;

    #[test]
    fn the_scene_draws_something_that_moves() {
        let air = |t: f64| Air { t, was: t, wind: 2.2, tree: Oscillator::new(2.6, 0.22), g: gravity::EARTH, voice: 3 };
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
        let angle = |wind: f64| {
            Air { t: 0.0, was: 0.0, wind, tree: Oscillator::new(2.6, 0.22), g: gravity::EARTH, voice: 3 }
                .blowing()
                .trunk_angle(STIFFNESS)
        };
        assert!((angle(0.0) - PI / 2.0).abs() < 1e-9, "calm leaves it upright");
        assert!(angle(3.0) < angle(1.0), "more wind, further over");
        assert!(angle(300.0) > 0.0, "but never past flat");
        assert!(angle(300.0) < 0.02, "though very nearly");
    }

    /// A gust is drawn only when there is wind to carry it.
    #[test]
    fn calm_air_has_no_gusts() {
        let calm = Air { t: 3.0, was: 3.0, wind: 0.0, tree: Oscillator::new(2.6, 0.22), g: gravity::EARTH, voice: 3 };
        assert!(calm.blowing().gusts(10, Cx::ZERO, Cx::new(1.0, 1.0), 3.0).is_empty());
    }
}








