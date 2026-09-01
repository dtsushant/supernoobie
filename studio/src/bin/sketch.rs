//! # sketch — a cyclone on flat paper
//!
//! ```text
//!     cargo run -p studio --release --bin sketch
//! ```
//!
//! Nothing here is 3D. Every point ends up as one `Cx` on the flat page. It
//! reads as a solid turning thing because of one fact:
//!
//! > **An ellipse is a circle seen at an angle.**
//!
//! ```text
//!     screen_x = x
//!     screen_y = y · sin(tilt)  +  z · cos(tilt)
//! ```
//!
//! Hold `↑` and `↓` and watch the two halves of that trade off: overhead
//! (`sin = 1`) the rings are circles and every height lands in the same place;
//! edge on (`sin = 0`) they collapse to lines and height is all you see.
//!
//! ## The part that is physics
//!
//! The funnel's outline is a drawing. **How fast each ring turns** is not.
//!
//! A vortex conserves circulation, so `ω(r) = Γ / 2πr²` — the narrow bottom
//! whips round while the wide top barely moves. That is why the strands *wind
//! up* as it runs instead of turning as one rigid piece, and it is the same law
//! as a skater pulling their arms in. Watch the bottom coil while the top is
//! still lazily turning.
//!
//! Taken literally `ω → ∞` at the middle. The **eye** stops that: inside the
//! core radius it turns as a solid body, which is what a real vortex does.
//!
//! ```text
//!   ↑ ↓   tilt the camera        W S   flare the funnel
//!   ← →   circulation, Γ         E D   the eye (core radius)
//!   - =   fewer / more rings     R     reset
//!   Space pause                  Esc   quit
//!
//!   wheel zooms    right-drag slides the paper    Home resets the view
//! ```
//!
//! The cyclone itself lives in [`shapes::cyclone`], with tests for the
//! projection and for the vortex law. This file only says where it goes and
//! what the keys do.

use studio::prelude::*;

struct Sky {
    /// The graph's clock, which never stops.
    t: f64,
    storm: Cyclone,
    running: bool,
    /// The storm's own clock at the moment it was paused.
    frozen: f64,
    /// How much of the graph's clock to ignore, so resuming carries on rather
    /// than jumping forward by however long the pause lasted.
    held_at: f64,
}

impl Sky {
    fn new() -> Sky {
        Sky { t: 0.0, storm: Cyclone::new(), running: true, frozen: 0.0, held_at: 0.0 }
    }

    /// The time the storm is drawn at.
    ///
    /// Not the graph's `t`. The graph's clock keeps running through a pause,
    /// so a storm drawn at `t` would leap forward the moment you resumed.
    fn when(&self) -> f64 {
        if self.running {
            self.t - self.held_at
        } else {
            self.frozen
        }
    }

    fn pause(&mut self) {
        if self.running {
            self.frozen = self.t - self.held_at;
        } else {
            self.held_at = self.t - self.frozen;
        }
        self.running = !self.running;
    }
}

fn main() {
    Graph::new("SKETCH  -  a cyclone on flat paper")
        .scale(58.0)
        .origin(0.5, 0.78)
        .with(Sky::new())
        .each_frame(|s, t| s.t = t)
        // Tilt is the one to play with: it is the whole illusion, in one key.
        .on_hold('^', |s| s.storm.tilt = (s.storm.tilt + 0.012).min(PI / 2.0))
        .on_hold('!', |s| s.storm.tilt = (s.storm.tilt - 0.012).max(0.0))
        .on_hold('>', |s| s.storm.circulation = (s.storm.circulation + 0.15).min(60.0))
        .on_hold('<', |s| s.storm.circulation = (s.storm.circulation - 0.15).max(0.0))
        .on_hold('w', |s| s.storm.flare = (s.storm.flare + 0.02).min(6.0))
        .on_hold('s', |s| s.storm.flare = (s.storm.flare - 0.02).max(0.4))
        .on_hold('e', |s| s.storm.core = (s.storm.core + 0.01).min(s.storm.top))
        .on_hold('d', |s| s.storm.core = (s.storm.core - 0.01).max(0.02))
        .on('=', |s| s.storm.rings = (s.storm.rings + 2).min(60))
        .on('-', |s| s.storm.rings = s.storm.rings.saturating_sub(2).max(2))
        .on(' ', Sky::pause)
        .on('r', |s| *s = Sky { t: s.t, held_at: s.t, ..Sky::new() })
        .run(scene);
}

const GROUND: u32 = 0x2C3742;
const FAR: u32 = 0x2E4756; // the half turned away from you
const NEAR: u32 = 0x6FA8C4; // the half facing you
const AIR: u32 = 0xE0A44A;
const EYE: u32 = 0xE585AC;
const INK: u32 = 0x9AA7B4;
const DIM: u32 = 0x5A6774;

fn scene(s: &Sky) -> Frame {
    let (c, t) = (&s.storm, s.when());
    let mut f = Frame::new();

    // --- the ground it stands on ------------------------------------------
    // Something for the perspective to be measured against.
    f.add(Shape::path(vec![Cx::new(-9.0, 0.0), Cx::new(9.0, 0.0)])).color(GROUND).width(1);
    f.add(c.spine()).color(GROUND).width(1);

    // --- the funnel, in two halves ----------------------------------------
    // Far half dim, near half bright. Without that it is a stack of flat
    // hoops; with it, it is a solid.
    let (far, near) = c.halves(t);
    f.add(far).color(FAR).width(2);
    f.add(near).color(NEAR).width(2);

    // --- the air, winding up ----------------------------------------------
    f.add(c.streamers(5, t)).color(AIR).width(2);

    // --- the eye -----------------------------------------------------------
    f.add(c.eye()).color(EYE).width(2);

    // --- what it is doing --------------------------------------------------
    const TEXT: i32 = 2;
    let line = |n: f64| 12.0 + 9.0 * TEXT as f64 * n;
    f.pin(Anchor::TopLeft, 14.0, line(0.0), "screen = ( x , y sin(tilt) + z cos(tilt) )", INK, TEXT);
    f.pin(
        Anchor::TopLeft,
        14.0,
        line(1.0),
        format!("tilt {:.2} rad   circles squashed to {:.0}%", c.tilt, c.foreshortening() * 100.0),
        DIM,
        TEXT,
    );
    f.pin(Anchor::TopLeft, 14.0, line(2.5), "omega(r) = Gamma / 2 pi r^2   so the bottom outruns the top", INK, TEXT);
    f.pin(
        Anchor::TopLeft,
        14.0,
        line(3.5),
        format!(
            "Gamma {:.1}   eye {:.2}   bottom {:.2} rad/s   top {:.2} rad/s",
            c.circulation,
            c.core,
            c.spin_at(c.radius_at(0.0)),
            c.spin_at(c.radius_at(1.0))
        ),
        DIM,
        TEXT,
    );

    let foot = |n: f64| -14.0 - 9.0 * TEXT as f64 * n;
    f.pin(Anchor::BottomLeft, 14.0, foot(1.0), "up/down tilt   left/right Gamma   W/S flare   E/D eye   -/= rings", DIM, TEXT);
    f.pin(
        Anchor::BottomLeft,
        14.0,
        foot(0.0),
        if s.running {
            "space pauses   R resets   wheel zooms   right-drag slides the paper"
        } else {
            "PAUSED   space to go on"
        },
        0x46525E,
        TEXT,
    );
    f
}

// ===========================================================================
//  Things to try, once the storm is boring.
// ===========================================================================
//
//   // a shape from the library, and the same one orbiting
//   f.place(digit::glyph(7, 40), Cx::new(-3.0, 2.0)).color(0x4FBCD4).width(3);
//   f.place(face::smiley(0.6), Cx::new(3.0, 2.0) + Cx::expi(t).scale(2.0)).color(0x6FCF97);
//
//   // a rose, r = cos(5 theta), turning
//   f.add(Shape::param(move |a| Cx::polar((5.0 * a).cos(), a + t * 0.3), 0.0, TAU, 600));
//
//   // the LEVEL SET F(x,y) = c. The number is the value of F, not a radius:
//   // with F = x^2 + y^2 that is r^2, so 4 is the circle of radius 2.
//   f.add(Shape::implicit(|x, y| x * x + y * y, 4.0));
//   f.add(Shape::circle(Cx::ZERO, 2.0));            // or just say what you mean
//
//   // a Fourier series: add sine waves and watch a square wave appear
//   let terms = 1 + (t as usize % 12) * 2;
//   f.add(Shape::graph(move |x| (1..=terms).step_by(2).map(|n| (n as f64 * x).sin() / n as f64).sum()));
//
//   // a closed curve redrawn as a stack of rotating arrows
//   let series = digit::series(3);
//   f.add(series.curve(8));
//   f.add(series.machine(8, t)).color(0x3B4A59).width(1);
//
//   // the draggable disc this sketch used to hold, if you want it back
//   let mut disc = Disc::new(Cx::ZERO, 2.0);   // .drag(at, down) from on_pointer
//   f.add(disc.shape());

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// A sketch that draws nothing is almost always a mistake, and the window
    /// would just sit there empty telling you nothing about why.
    #[test]
    fn the_scene_actually_draws_something() {
        let f = scene(&Sky::new());
        assert!(!f.is_empty(), "scene() returned an empty frame");
        assert!(f.bounds(&View::centred(400, 400, 40.0)).is_some(), "nothing in the frame has a position");
    }

    /// ★ Pausing has to hold the picture where it is. The naive version — stop
    /// advancing the storm — would still leap forward on resume, because the
    /// graph's clock never stopped while you were paused.
    #[test]
    fn pausing_holds_the_picture_and_resuming_carries_on() {
        let mut s = Sky::new();
        s.t = 4.0;
        let at_pause = s.when();

        s.pause();
        for k in 1..30 {
            s.t = 4.0 + k as f64 * 0.1; // the graph's clock keeps running
            assert!((s.when() - at_pause).abs() < 1e-12, "the picture moved while paused");
        }

        s.pause();
        assert!((s.when() - at_pause).abs() < 1e-12, "it should resume where it stopped");
        s.t += 1.0;
        assert!((s.when() - (at_pause + 1.0)).abs() < 1e-12, "and then carry on");
    }

    /// Reset must not rewind the graph's clock either — same trap.
    #[test]
    fn reset_starts_the_storm_again_without_rewinding_the_clock() {
        let mut s = Sky::new();
        s.t = 30.0;
        s = Sky { t: s.t, held_at: s.t, ..Sky::new() };
        assert!(s.when().abs() < 1e-12, "a fresh storm starts at zero however late it is");
    }

    /// The readout says what the projection actually does, so the number on
    /// screen and the shape on screen cannot drift apart.
    #[test]
    fn the_readout_matches_the_projection() {
        let mut s = Sky::new();
        s.storm.tilt = 0.5;
        assert!((s.storm.foreshortening() - 0.5f64.sin()).abs() < 1e-12);
    }
}


