//! # world — a scene that both sees and sounds
//!
//! The playground's scene, moved out of its binary so that **two different
//! programs can drive the same world**: the one in this workspace, which draws
//! it, and the one outside it, which also plays it. The difference between
//! them is a sound card, and nothing else.
//!
//! ## How sound is bound to a shape
//!
//! Not by calling a speaker. A [`World`] **emits** sound the way it emits a
//! [`Frame`]: as a value, from what is going on.
//!
//! ```text
//!     the ball's height  --[ Trigger::falling ]-->  kit::bounce(how hard)  --> world.sounds
//!     the trunk's angle  --[ Trigger::rising  ]-->  kit::creak(how bent)   --> world.sounds
//!     the wind's push    ------ a level, not an edge ------->  world.gale
//! ```
//!
//! That split is the whole idea, and it is the same one as the event log:
//! **a state persists, an edge happens once.** Wind is a state — it has no
//! beginning, only a strength, so it is one held sound whose level is pushed
//! at it every frame. A bounce is an edge — it happens and is over, so it is
//! struck.
//!
//! Nothing here opens a device or blocks. [`World::advance`] fills
//! [`sounds`](World::sounds) and sets [`gale`](World::gale), and whoever wants
//! to hear it takes them:
//!
//! ```no_run
//! # use studio::world::World;
//! # use sound::{kit, Mixer};
//! # let (mut world, mut mixer) = (World::new(), Mixer::new());
//! # mixer.hold(kit::WIND, kit::wind(0.0));
//! world.advance(1.0);
//! for voice in world.sounds.drain(..) {
//!     mixer.strike(voice);          // the things that happened
//! }
//! mixer.level(kit::WIND, world.gale);          // the thing going on
//! mixer.colour(kit::WIND, kit::gustiness(world.gale));
//! ```
//!
//! A program that never takes them simply runs in silence, at no cost — which
//! is exactly what the playground does, so this workspace still links no audio
//! library at all.
//!
//! ## Why the triggers live on the world and not in the drawing
//!
//! Because [`scene`] is a **pure function of the world** and is called only to
//! draw. Skip a frame, draw twice, render a still for a document — nothing
//! about the world changes. Put an edge detector in there and the sound would
//! depend on how often somebody happened to redraw, which is a bug that only
//! appears on a slow machine.
//!
//! So all the deciding happens in [`World::advance`], once per step of the
//! clock, and drawing stays free.
//!
//! ## The tree is also a sum of waves
//!
//! Not decorated with them — **made** of them. A branch held at one end and
//! free at the other bends in the modes `sin((2n−1)πs/2L)`: a quarter wave,
//! three quarters, five quarters. Each is zero at the base, because that end
//! is held, and steepest at the tip, because that end is not. Its shape at any
//! instant is those added up.
//!
//! What the clock changes is **how much of each**, so the space part and the
//! time part separate and it stays a sum of waves instead of becoming a new
//! curve every frame. Amplitudes fall off as `1/n²`, which is why a branch
//! sways rather than buzzes.

use physics::{fall::gravity, Fall, Oscillator, Trigger};
use plotkit::{Cx, Frame, Shape};
use shapes::{bough, wave, Wave, Wind};
use sound::{kit, pitch, Timbre, Tone, Voice};
use std::f64::consts::PI;

pub const TREE: u32 = 0x8FBF6A;
pub const MODE: u32 = 0x4A6B56;
pub const GUST: u32 = 0x7FA6C4;
pub const BALL: u32 = 0xE0A44A;
pub const TONE: u32 = 0xE585AC;
pub const SPEC: u32 = 0x9B7BD4;

/// How stiff the tree is. Bigger resists the wind harder.
pub const STIFFNESS: f64 = 1.4;

/// Where the ball is let go, and the floor it lands on.
pub const DROP: (f64, f64) = (-1.6, 8.2);
pub const GROUND: f64 = -8.4;

/// The speed of a landing that counts as full strength.
///
/// A fixed number of units per second, so a landing on the moon and one on
/// Jupiter are compared against the same ruler — which is what makes one of
/// them audibly gentler.
const SMACK: f64 = 12.0;

/// The most sounds kept waiting for somebody who may never come.
///
/// A silent program never drains [`World::sounds`], and a list that only grows
/// is a leak with a slow fuse. Eight is more than a frame can produce, so
/// nothing is ever lost to a listener that is actually listening.
const KEPT: usize = 8;

/// A colour dimmed toward the background, for fading a gust in and out.
pub fn fade(c: u32, amount: f64) -> u32 {
    let k = amount.clamp(0.0, 1.0);
    let mix = |shift: u32| (((c >> shift) & 255) as f64 * k) as u32;
    (mix(16) << 16) | (mix(8) << 8) | mix(0)
}

/// Everything going on, and everything it has just made a noise about.
pub struct World {
    pub t: f64,
    /// The clock last frame, so the oscillator can be stepped by a real `dt`.
    was: f64,
    /// What you have asked for with the arrow keys.
    pub wind: f64,
    /// The tree's answer to the wind: a damped second-order system, so it
    /// overshoots, wobbles and SETTLES rather than snapping to the new angle.
    /// This is the Laplace part — the poles decide how it gets there.
    pub tree: Oscillator,
    /// Gravity, for the ball.
    pub g: f64,
    /// Which recipe of harmonics the drawn note is made of.
    pub voice: usize,

    // --- what it sounds like ------------------------------------------------
    /// Things that have just happened, waiting to be struck. Drained by
    /// whoever is listening; ignored, harmlessly, by whoever is not.
    pub sounds: Vec<Voice>,
    /// How hard the wind is blowing, 0 to 1 — a **level**, not an event,
    /// because wind does not happen, it goes on.
    pub gale: f64,
    /// The ball crossing the floor on its way down.
    landing: Trigger,
    /// The trunk passing a real bend. Rearms only when it comes back, so a
    /// tree held over by a steady wind creaks once rather than continuously.
    strain: Trigger,
    /// And the trunk going right over.
    flat: Trigger,
}

impl Default for World {
    fn default() -> World {
        World::new()
    }
}

impl World {
    pub fn new() -> World {
        World {
            t: 0.0,
            was: 0.0,
            wind: 2.2,
            // Lightly damped: a tree wobbles a few times before it stops.
            tree: Oscillator::new(2.6, 0.22),
            g: gravity::EARTH,
            voice: 3,

            sounds: Vec::new(),
            gale: 0.0,
            // Just above the floor, crossed downwards. Not *at* the floor: the
            // ball stops there and sits, and a trigger set exactly on a value
            // a thing comes to rest on is asking for trouble.
            landing: Trigger::falling(GROUND + 0.3, 0.6),
            // A quarter of a radian is a visible bend. The slack is what stops
            // a tree leaning steadily against the wind from creaking at sixty
            // a second — it must come back before it can creak again.
            strain: Trigger::rising(0.25, 0.12),
            flat: Trigger::rising(1.30, 0.25),
        }
    }

    /// Bring the world up to date, and say what it just did.
    ///
    /// The wind sets a target lean — the angle where the two moments balance.
    /// The oscillator is what actually gets there, and `rest_under` says a
    /// push of `f` settles at `f/ω²`, so pushing with `ω²·target` settles at
    /// the target. Take the wind away and it swings back and settles rather
    /// than snapping upright.
    pub fn advance(&mut self, t: f64) {
        let dt = (t - self.was).clamp(0.0, 1.0 / 20.0);
        (self.was, self.t) = (t, t);

        let w = self.blowing();
        let target = w.lean(STIFFNESS);
        let stiff = self.tree.omega * self.tree.omega;
        self.tree.step(dt, stiff * target);

        // --- the wind: a level, pushed every frame --------------------------
        // Pressure, not speed, because that is what you feel and what the tree
        // answers to — and it goes as v², so a doubling of the wind is four
        // times the sound.
        self.gale = (w.pressure() / 45.0).clamp(0.0, 1.0);

        // --- the ball: an edge, when it crosses the floor -------------------
        let ball = self.ball();
        if self.landing.saw(ball.im).is_some() {
            // How hard is taken from the physics, NOT from the trigger's
            // frame-to-frame change — that would be measured per frame, so the
            // same landing would sound softer on a machine drawing faster.
            //
            // Against a FIXED reference speed, not against terminal velocity.
            // Terminal velocity scales with gravity, so the fraction of it
            // reached says how *complete* the fall was, not how hard it hit —
            // and by that measure the moon wins, because a long slow drop gets
            // closer to its own small limit than a short fast one gets to its
            // large one. Which is exactly backwards.
            let hardness = self.falling().speed(0.0, self.landing_time()) / SMACK;
            self.say(kit::bounce(hardness));
        }

        // --- the tree: an edge when it bends, another when it goes over -----
        let bend = self.tree.x.abs();
        if self.strain.saw(bend).is_some() {
            self.say(kit::creak((bend / 1.2).min(1.0)));
        }
        if self.flat.saw(bend).is_some() {
            self.say(kit::crack(1.0));
        }
    }

    /// Something happened. Keep it for whoever is listening.
    fn say(&mut self, voice: Voice) {
        if self.sounds.len() >= KEPT {
            self.sounds.remove(0);
        }
        self.sounds.push(voice);
    }

    /// A sound for something the person did, rather than something the world
    /// did — a key, a tap on a shape.
    pub fn tapped(&mut self) {
        self.say(kit::tap());
    }

    /// The wind right now: what you asked for, gusting.
    ///
    /// Real wind is never steady, and a steady one leans the tree to an angle
    /// and leaves it there, which shows nothing.
    pub fn blowing(&self) -> Wind {
        let gust = 1.0 + 0.35 * (self.t * 0.9).sin() + 0.18 * (self.t * 2.3).sin();
        Wind::new(self.wind * gust)
    }

    /// The air the ball is falling through.
    pub fn falling(&self) -> Fall {
        Fall::in_air(self.g.max(0.01), 1.1)
    }

    /// How long the drop takes.
    pub fn landing_time(&self) -> f64 {
        self.falling().hits(Cx::new(DROP.0, DROP.1), 0.0, GROUND).unwrap_or(6.0)
    }

    /// How far through the drop it is: falling, then a pause on the floor,
    /// then back to the top.
    pub fn drop_phase(&self) -> f64 {
        self.t % (self.landing_time() + 0.9)
    }

    /// Where the ball is. **One value, two uses** — the trigger watches it and
    /// the drawing draws it, so what you hear cannot disagree with what you
    /// see.
    pub fn ball(&self) -> Cx {
        let landing = self.landing_time();
        self.falling().at(Cx::new(DROP.0, DROP.1), 0.0, self.drop_phase().min(landing))
    }

    /// The four recipes, and what each one is like.
    pub fn voices() -> [(&'static str, Timbre); 4] {
        [
            ("pure: one sine, nothing else", Timbre::pure()),
            ("triangle: odd harmonics, 1/n^2 -- soft", Timbre::triangle(15)),
            ("clarinet: odd harmonics, 1/n -- woody", Timbre::clarinet(15)),
            ("sawtooth: every harmonic, 1/n -- bright", Timbre::saw(15)),
        ]
    }

    /// The note being shown. One value; three views of it.
    pub fn tone(&self) -> Tone {
        let (_, timbre) = Self::voices()[self.voice % 4].clone();
        Tone::pluck(pitch::named("A3").unwrap_or(pitch::A4)).with_timbre(timbre).with_decay(1.4)
    }
}

/// Draw the world. A pure function of it — call it twice, or not at all.
pub fn scene(a: &World) -> Frame {
    let (t, w) = (a.t, a.blowing());
    let mut f = Frame::new();

    // --- the wind ----------------------------------------------------------
    // Gusts: short pieces of wave, drifting downwind and fading. A gust HAS
    // ends; a Wave does not. That is the whole difference between them.
    for (gust, bright) in w.gusts(14, Cx::new(-11.0, -8.0), Cx::new(11.0, 9.0), t) {
        f.add(gust).color(fade(GUST, bright)).width(1);
    }

    // --- the simplest thing there is --------------------------------------
    f.add(Wave::sine()).color(0x4FBCD4).width(2);

    // --- a tree, which is also a sum of waves ------------------------------
    // Not the static balance: where the OSCILLATOR has got to on its way
    // there. That is the difference between a tree that snaps to an angle and
    // one that swings past it and settles back.
    let upright = PI / 2.0;
    let angle = upright - a.tree.x;
    for (level, boughs) in
        bough::tree(Cx::new(1.5, GROUND), angle, 2.3, 6, 0.5, w.shake(0.06), t).into_iter().enumerate()
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

    f.pin(plotkit::Anchor::Bottom, 0.0, -34.0, "and a tree of those", TREE, 2);
    f.pin(plotkit::Anchor::BottomRight, -14.0, -14.0, "a wave has no ends -- it crosses everything", 0x46525E, 2);
    f.pin(
        plotkit::Anchor::TopRight,
        -14.0,
        12.0,
        format!("wind {:+.1}   push {:+.1} (goes as v^2)", w.speed, w.pressure()),
        GUST,
        2,
    );
    f.pin(
        plotkit::Anchor::TopRight,
        -14.0,
        30.0,
        format!(
            "leaning {:.0} deg from upright   trunk at {:.0} deg",
            w.lean(STIFFNESS).to_degrees().abs(),
            angle.to_degrees()
        ),
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
    let note = a.tone();
    let (voice, _) = World::voices()[a.voice % 4].clone();

    // The recipe. A clarinet's missing even harmonics are visibly missing.
    f.add(Shape::path(vec![Cx::new(-10.6, -4.4), Cx::new(-3.6, -4.4)])).color(0x3E4A55).width(1);
    f.place(note.spectrum().map(|z| Cx::new(z.re * 0.42, z.im * 2.2)), Cx::new(-10.6, -4.4)).color(SPEC).width(3);

    // What those add up to: two periods of the wave itself.
    f.place(note.waveform(2.0).map(|z| Cx::new(z.re * 3.3, z.im * 0.62)), Cx::new(-10.6, -6.4)).color(TONE).width(2);

    // --- a ball, with the gravity dial -------------------------------------
    // Galileo: distance goes as the SQUARE of the time, and mass does not come
    // into it. With air it stops speeding up and approaches a terminal
    // velocity -- the same e^(-t/tau) as the tree settling above.
    let air = a.falling();
    let from = Cx::new(DROP.0, DROP.1);
    let landing = a.landing_time();
    let travelled = a.drop_phase().min(landing);

    f.add(air.path(from, 0.0, travelled)).color(0x3E4A55).width(1);
    f.add(Shape::circle(a.ball(), 0.22)).color(BALL).width(2);
    f.pin(
        plotkit::Anchor::BottomLeft,
        14.0,
        -54.0,
        format!(
            "g {:.2}   falls in {:.1}s   terminal {:.1}/s   (U/J, M moon, E earth)",
            a.g,
            landing,
            air.terminal_velocity()
        ),
        BALL,
        2,
    );
    f.pin(plotkit::Anchor::BottomLeft, 14.0, -74.0, "s = 1/2 g t^2 -- Galileo. Mass does not come into it.", BALL, 2);
    f.pin(plotkit::Anchor::BottomLeft, 14.0, -114.0, format!("1-4 timbre: {voice}"), TONE, 2);
    f.pin(
        plotkit::Anchor::BottomLeft,
        14.0,
        -94.0,
        "bars are the harmonics, the curve is their sum -- press 5 to hear it",
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

    /// A world wound forward to `t`, in steps small enough to be real.
    fn run_to(w: &mut World, t: f64) {
        let mut clock = w.t;
        while clock < t {
            clock = (clock + 1.0 / 60.0).min(t);
            w.advance(clock);
        }
    }

    #[test]
    fn the_scene_draws_something_that_moves() {
        let mut w = World::new();
        assert!(!scene(&w).is_empty());
        let v = View::centred(400, 400, 20.0);
        let ink = |w: &World| {
            let mut c = plotkit::Canvas::new(400, 400);
            c.clear(0);
            scene(w).draw(&mut c, &v);
            c.buf.iter().filter(|&&p| p != 0).count()
        };
        let still = ink(&w);
        run_to(&mut w, 0.7);
        assert_ne!(ink(&w), still, "the scene should have moved");
    }

    /// ★ Harder wind lays the tree further over, and no wind ever lays it past
    /// flat — the saturation comes from the geometry, not from a clamp.
    #[test]
    fn the_wind_lays_the_tree_over_but_never_past_flat() {
        let angle = |wind: f64| {
            let mut w = World::new();
            w.wind = wind;
            w.blowing().trunk_angle(STIFFNESS)
        };
        assert!((angle(0.0) - PI / 2.0).abs() < 1e-9, "calm leaves it upright");
        assert!(angle(3.0) < angle(1.0), "more wind, further over");
        assert!(angle(300.0) > 0.0, "but never past flat");
        assert!(angle(300.0) < 0.02, "though very nearly");
    }

    /// A gust is drawn only when there is wind to carry it.
    #[test]
    fn calm_air_has_no_gusts() {
        let mut w = World::new();
        w.wind = 0.0;
        w.t = 3.0;
        assert!(w.blowing().gusts(10, Cx::ZERO, Cx::new(1.0, 1.0), 3.0).is_empty());
    }

    /// ★ The ball makes a noise **when it lands**, not before and not twice.
    /// This is the whole claim of the module: an event in the physics, on the
    /// frame it happens.
    #[test]
    fn the_ball_sounds_once_when_it_lands() {
        let mut w = World::new();
        w.wind = 0.0; // a silent tree, so only the ball is heard

        let mut heard = Vec::new();
        let mut clock = 0.0;
        let landing = w.landing_time();
        while clock < landing + 0.5 {
            clock += 1.0 / 60.0;
            w.advance(clock);
            for _ in w.sounds.drain(..) {
                heard.push(clock);
            }
        }
        assert_eq!(heard.len(), 1, "one landing, one sound: {heard:?}");
        let when = heard[0];
        assert!((when - landing).abs() < 0.1, "it should sound at {landing:.2}s, not {when:.2}s");
    }

    /// And it does so again next time round, rather than firing once and
    /// falling silent for the rest of the run.
    #[test]
    fn it_keeps_bouncing() {
        let mut w = World::new();
        w.wind = 0.0;
        let mut count = 0;
        let cycle = w.landing_time() + 0.9;
        let mut clock = 0.0;
        while clock < cycle * 3.0 {
            clock += 1.0 / 60.0;
            w.advance(clock);
            count += w.sounds.drain(..).count();
        }
        assert_eq!(count, 3, "three cycles, three landings");
    }

    /// ★ A ball that falls faster lands harder. If this were taken from the
    /// trigger's frame-to-frame change instead of from the physics, the same
    /// landing would sound softer on a machine that draws faster — which is a
    /// bug you would never find by listening.
    #[test]
    fn a_heavier_gravity_lands_harder() {
        let hardness = |g: f64| {
            let mut w = World::new();
            w.wind = 0.0;
            w.g = g;
            let landing = w.landing_time();
            let mut clock = 0.0;
            while clock < landing + 0.3 {
                clock += 1.0 / 60.0;
                w.advance(clock);
                if let Some(v) = w.sounds.pop() {
                    return v.gain;
                }
            }
            panic!("it never landed");
        };
        assert!(hardness(gravity::EARTH) > hardness(gravity::MOON), "the moon should be gentler");
    }

    /// ★ A tree held over by a steady wind creaks **once**, not sixty times a
    /// second. Wood under a constant load is quiet; it is the *moving* that
    /// makes the noise.
    #[test]
    fn a_steadily_leaning_tree_does_not_creak_continuously() {
        let mut w = World::new();
        w.wind = 6.0;
        let mut creaks = 0;
        let mut clock = 0.0;
        while clock < 12.0 {
            clock += 1.0 / 60.0;
            w.advance(clock);
            creaks += w.sounds.drain(..).count();
        }
        // Twelve seconds is over seven hundred frames. A handful of creaks is
        // a tree; hundreds is a buzz.
        assert!(creaks < 60, "it creaked {creaks} times -- that is a buzz, not a tree");
        assert!(creaks > 0, "and it should creak at all");
    }

    /// ★ Wind is a **level**, not an event — it has no beginning, only a
    /// strength, and it must follow the wind while the wind is already
    /// blowing. That is the thing a recording cannot do.
    #[test]
    fn the_wind_level_follows_the_wind() {
        let mut w = World::new();
        w.wind = 0.0;
        w.advance(0.1);
        let calm = w.gale;

        w.wind = 8.0;
        run_to(&mut w, 1.0);
        let blowing = w.gale;

        w.wind = 0.0;
        run_to(&mut w, 2.0);

        assert!(calm < 0.01, "calm should be silent, not {calm}");
        assert!(blowing > calm * 10.0 + 0.05, "it should be heard to get up: {calm} -> {blowing}");
        assert!(w.gale < 0.01, "and to drop again");
    }

    /// The level is bounded, because it is handed straight to a volume. A gale
    /// off the end of the scale must not ask for an amplitude of nine.
    #[test]
    fn even_an_absurd_gale_stays_inside_the_scale() {
        let mut w = World::new();
        w.wind = 500.0;
        run_to(&mut w, 1.0);
        assert!((0.0..=1.0).contains(&w.gale), "{}", w.gale);
    }

    /// ★ Nobody listening must not mean a list that only grows. A silent
    /// program runs for hours, and a leak with a slow fuse is still a leak.
    #[test]
    fn a_world_nobody_is_listening_to_does_not_fill_up() {
        let mut w = World::new();
        w.wind = 5.0;
        run_to(&mut w, 60.0);
        assert!(w.sounds.len() <= KEPT, "it kept {} sounds", w.sounds.len());
    }

    /// Drawing must not change anything. `scene` is called to render a still,
    /// to make a document, twice in a frame while debugging — and if a trigger
    /// lived in there the sound would depend on how often somebody drew.
    #[test]
    fn drawing_the_scene_makes_no_sound() {
        let mut w = World::new();
        run_to(&mut w, 4.0);
        w.sounds.clear();
        for _ in 0..50 {
            let _ = scene(&w);
        }
        assert!(w.sounds.is_empty(), "drawing must not be an event");
    }
}
