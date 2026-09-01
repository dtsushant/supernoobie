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
//! ## Crossing the ground
//!
//! The storm tracks over the plane, and that is **not** a translation on
//! screen. Going away from you climbs the page by only `sin(tilt)` of the
//! distance covered, while going sideways is one for one. Add a plain 2D
//! offset instead and it floats about on the glass — wrong, without ever
//! looking obviously wrong. Watch the trail recede.
//!
//! ## Crossing the land
//!
//! It knocks trees down, and **what falls is decided by the wind where each
//! tree stands** — `v = Γ/2πr` against what that tree can take — not by a
//! radius anybody chose. So the reach follows from the circulation.
//!
//! Which matters because the circulation **decays**, `e^{-t/life}`: a real
//! vortex loses itself to friction against the ground. As `Γ` falls the storm
//! thins, ropes out, lifts off the ground, and stops being able to flatten
//! anything — and not one of those is arranged separately. They all follow
//! from the same failing number.
//!
//! The trees are scattered by the R2 sequence, on the same principle as the
//! wander: an irrational never lands you back where you started, so the
//! scatter has no clumps and no gaps and needs no seed.
//!
//! The wander is a sum of sines at 1, φ, √2 and √3. A sum of sines repeats
//! only when every term comes back at once, and frequencies with no common
//! measure never do — so the path never retraces itself, with no random number
//! anywhere. Which also means a recorded run replays along exactly the same
//! path.
//!
//! ```text
//!   1 2 3 still / wander / wind    ↑ ↓   tilt the camera
//!   T     the track it has taken    ← →   circulation, Γ
//!   U J   how far it strays         W S   flare the funnel
//!   I K   how long it lives         E D   the eye (core radius)
//!   - =   fewer / more rings        R     a fresh storm on fresh ground
//!   Space pause                     Esc   quit
//!
//!   wheel zooms    right-drag slides the paper    Home resets the view
//! ```
//!
//! The cyclone itself lives in [`shapes::cyclone`], with tests for the
//! projection and for the vortex law. This file only says where it goes and
//! what the keys do.

use studio::prelude::*;

/// How the storm crosses the ground.
#[derive(Clone, Copy, PartialEq)]
enum Track {
    Still,
    /// A path made of sines whose frequencies have no common measure, so it
    /// never repeats — and needs no random number, so a taped run replays
    /// along exactly the same path.
    Wander,
    /// Carried by a steady wind, the way a real storm is steered by the flow
    /// it sits in.
    Wind,
}

impl Track {
    /// Where the storm stands on the **ground** at time `t`.
    ///
    /// `x` across, `y` into the distance. It is the cyclone's projection that
    /// decides what that looks like on the page.
    fn ground(self, spread: f64, t: f64) -> Cx {
        match self {
            Track::Still => Cx::ZERO,
            Track::Wander => motion::wander_at(spread, 0.35, t),
            // A steady drift, with a little wander on top so it is not a
            // ruled line. Two motions added, which is all "steered by the
            // mean wind, wobbling about it" means.
            Track::Wind => Cx::new(0.55, 0.18).scale(t) + motion::wander_at(spread * 0.35, 0.5, t),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Track::Still => "standing still",
            Track::Wander => "wander: sines at 1, phi, sqrt2, sqrt3 - no common measure, so it never repeats",
            Track::Wind => "steered by a steady wind, wobbling about it",
        }
    }
}

struct Sky {
    /// The graph's clock, which never stops.
    t: f64,
    storm: Cyclone,
    track: Track,
    /// How far it strays.
    spread: f64,
    trail: bool,
    /// What it is crossing, and knocking down.
    land: Field,
    /// The circulation it started with. It spends this as it goes.
    gamma0: f64,
    /// How long it takes to lose most of its strength, in seconds.
    life: f64,
    running: bool,
    /// The storm's own clock at the moment it was paused.
    frozen: f64,
    /// How much of the graph's clock to ignore, so resuming carries on rather
    /// than jumping forward by however long the pause lasted.
    held_at: f64,
}

impl Sky {
    fn new() -> Sky {
        Sky {
            t: 0.0,
            storm: Cyclone::new(),
            track: Track::Wander,
            spread: 3.0,
            trail: true,
            land: Field::new(220, 11.0),
            gamma0: 4.0,
            life: 30.0,
            running: true,
            frozen: 0.0,
            held_at: 0.0,
        }
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

    /// What is left of it.
    ///
    /// A vortex loses its circulation to friction against the ground, and
    /// exponential decay is the honest first guess: it always has some
    /// fraction of itself left, so it never quite stops — it just stops
    /// mattering. `e^{-t/life}`.
    fn strength(&self) -> f64 {
        (-self.when() / self.life).exp()
    }

    /// Bring the storm up to date: where it stands, how hard it is blowing,
    /// and — as it spends itself — how thin and how high off the ground it is.
    fn advance(&mut self) {
        let (t, left) = (self.when(), self.strength());

        self.storm.at = self.track.ground(self.spread, t);
        self.storm.circulation = self.gamma0 * left;

        // A dying tornado ropes out and lifts. Both follow from the same
        // number, so it thins and rises together rather than being two
        // effects that have to be kept in step by hand.
        self.storm.top = 2.6 * (0.25 + 0.75 * left);
        self.storm.tip = 0.22 * left.max(0.15);
        self.storm.lift = (1.0 - left) * 3.4;

        // And it flattens whatever it can still out-blow. The reach is not a
        // number here — it follows from the circulation, so a storm running
        // out of puff stops doing damage on its own.
        let storm = self.storm;
        self.land.blow(t, |p| storm.wind_at(p));
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
        .each_frame(|s, t| {
            s.t = t;
            s.advance();
        })
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
        .on('1', |s| s.track = Track::Still)
        .on('2', |s| s.track = Track::Wander)
        .on('3', |s| s.track = Track::Wind)
        .on('t', |s| s.trail = !s.trail)
        .on_hold('u', |s| s.spread = (s.spread + 0.03).min(8.0))
        .on_hold('j', |s| s.spread = (s.spread - 0.03).max(0.0))
        .on(' ', Sky::pause)
        .on_hold('i', |s| s.life = (s.life + 0.3).min(240.0))
        .on_hold('k', |s| s.life = (s.life - 0.3).max(2.0))
        .on('r', |s| *s = Sky { t: s.t, held_at: s.t, ..Sky::new() })
        .run(scene);
}

const GROUND: u32 = 0x2C3742;
const TRACK: u32 = 0x4A5B6B;
const TREES: u32 = 0x4E7A5A; // still standing
const FLAT: u32 = 0x3A3F34;  // knocked over
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
    // A horizon to measure the perspective against, and the track it has
    // taken. Both are drawn ON THE GROUND, so they recede with it — a trail
    // lying flat on the screen would undo the plane everything else builds.
    f.add(Shape::path(vec![c.project(-14.0, 0.0, 0.0), c.project(14.0, 0.0, 0.0)])).color(GROUND).width(1);
    // --- the land behind the storm ------------------------------------------
    // Trees stand UP out of the ground, so they project like everything else
    // and recede with the plane instead of being stuck on the glass.
    //
    // Split at the storm's own depth and drawn in two goes, so the funnel is
    // painted over what is behind it and under what is in front. All the
    // depth a painter needs, from one comparison.
    let trees = |keep: fn(&Tree, f64) -> bool| {
        let depth = c.at.im;
        s.land.shapes_if(t, |x, y, z| c.project(x, y, z), move |tr| keep(tr, depth))
    };
    let (up_far, down_far) = trees(|tr, d| tr.at.im >= d);
    f.add(down_far).color(FLAT).width(1);
    f.add(up_far).color(TREES).width(1);

    if s.trail && s.track != Track::Still {
        let (track, spread) = (s.track, s.spread);
        f.add(c.trail(move |u| track.ground(spread, u), (t - 22.0).max(0.0), t)).color(TRACK).width(1);
        f.add(Shape::point(c.foot())).color(TRACK).dot(4.0);
    }
    f.add(c.spine()).color(GROUND).width(1);

    // --- the funnel, in two halves ----------------------------------------
    // Far half dim, near half bright. Without that it is a stack of flat
    // hoops; with it, it is a solid.
    let (far, near) = c.halves(t);
    f.add(far).color(FAR).width(2);
    f.add(near).color(NEAR).width(2);

    // --- the air, winding up ----------------------------------------------
    // Shed rather than endless: the air is replaced, so the strands stay
    // readable instead of winding into a smear.
    f.add(c.shed(5, 2.5, t)).color(AIR).width(2);

    // --- the eye -----------------------------------------------------------
    f.add(c.eye()).color(EYE).width(2);

    // --- and the land in front of it ---------------------------------------
    let (up_near, down_near) = trees(|tr, d| tr.at.im < d);
    f.add(down_near).color(FLAT).width(1);
    f.add(up_near).color(TREES).width(1);

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
    f.pin(Anchor::TopLeft, 14.0, line(2.5), s.track.name(), INK, TEXT);
    f.pin(
        Anchor::TopLeft,
        14.0,
        line(3.5),
        format!("standing at ({:+.2}, {:+.2}) on the ground   spread {:.1}", c.at.re, c.at.im, s.spread),
        DIM,
        TEXT,
    );
    f.pin(Anchor::TopLeft, 14.0, line(5.0), "v(r) = Gamma / 2 pi r   so what falls follows from the wind, not a chosen radius", INK, TEXT);
    f.pin(
        Anchor::TopLeft,
        14.0,
        line(6.0),
        format!(
            "{} of {} trees still up   it can fell an average tree out to {:.1}",
            s.land.standing(),
            s.land.trees.len(),
            c.reach(0.37)
        ),
        DIM,
        TEXT,
    );
    f.pin(
        Anchor::TopLeft,
        14.0,
        line(7.5),
        format!("Gamma decays as e^(-t/{:.0})   {:.0}% left   lifted {:.1} off the ground", s.life, s.strength() * 100.0, c.lift),
        INK,
        TEXT,
    );
    f.pin(
        Anchor::TopLeft,
        14.0,
        line(8.5),
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
    f.pin(Anchor::BottomLeft, 14.0, foot(2.0), "1 still  2 wander  3 wind   T trail   U/J stray   I/K how long it lives", DIM, TEXT);
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



