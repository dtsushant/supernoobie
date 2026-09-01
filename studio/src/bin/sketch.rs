//! # sketch — a blank page
//!
//! ```text
//!     cargo run -p studio --release --bin sketch
//! ```
//!
//! Edit `scene`. Everything else is already done: the window, the canvas, the
//! view, the graph paper, the loop. You will not need to touch them.
//!
//! ```text
//!   Esc  quit          , .  zoom out / in
//!   G    graph paper   arrows  pan
//!   S    save a PNG    Space   pause
//! ```
//!
//! ## The three things to know
//!
//! **1. A `Frame` is a layer, not a moment.** So an animation is a plain
//! function `f(t) -> Frame`, a still is `t = 0`, and there is no second concept
//! to learn.
//!
//! **2. Every shape is built about its own origin.** So putting one somewhere
//! is one call — `f.place(thing, at)` — and the same shape can be placed
//! anywhere, any number of times.
//!
//! **3. Every position is a complex number.** A point, an offset and a plain
//! number are all `Cx`, so `a + b` is a translation, `a * b` is a rotation and
//! a scale at once, and `Cx::expi(t)` is a point going round a circle.
//!
//! ```text
//!     a + b        move                 a * b       turn AND stretch
//!     2z           twice as far out     z * i       a quarter turn
//!     Cx::expi(t)  round the circle     a*z + b     any similarity, in one line
//! ```
//!
//! Run `cargo run -p shapes -- list` to see what there is to draw, and
//! `cargo run -p shapes -- ghost --steps` to watch one being built.

use studio::prelude::*;

/// The circle you can click.
const CENTRE: Cx = Cx::new(0.0, 0.0);
const RADIUS: f64 = 2.0;

/// Whatever the sketch needs to remember between frames.
struct Doodle {
    /// Seconds since it started. `each_frame` keeps this up to date.
    t: f64,
    /// Where the colour is on the hue wheel, in `[0, 1)`.
    h: f64,
    colour: u32,
    clicks: u32,
    seed: u64,
}

fn main() {
    Graph::new("sketch")
        .with(Doodle { t: 0.0, h: 0.52, colour: hue(0.52), clicks: 0, seed: 0x2545_F491_4F6C_DD1D })
        .each_frame(|d, t| d.t = t)
        .on_click(|d, at| {
            d.click(at);
        })
        .run(scene);

    // Other ways to run a scene:
    //
    //   Graph::new("x").animate(|t| ...)        // no state, no keys
    //   Graph::new("x").plot(frame)             // one still
    //   Graph::new("x").print(frame)            // in the terminal
    //   Graph::new("x").png("out.png", frame)   // to a file, no window
    //
    // and more bindings, all optional:
    //
    //   .on('r', |d| d.colour = 0x4FBCD4)   // once per press
    //   .on_hold('w', |d| d.t += 0.1)       // every frame it is held
    //   .on_digit(|d, n| ...)               // each digit typed
    //   .on_arrows(|d, dir| ...)            // a direction, as a Cx
}

impl Doodle {
    /// A click at `at`, in world coordinates. Says whether it landed.
    ///
    /// The hit test **is** the definition of a disc — `at` arrives in the same
    /// coordinates the scene is written in, so there is nothing to convert:
    ///
    /// ```text
    ///     |z - c| <= r
    /// ```
    ///
    /// For a shape without such a tidy definition, `Shape::contains` answers
    /// the same question by counting how many times a ray out of the point
    /// crosses the outline.
    fn click(&mut self, at: Cx) -> bool {
        let hit = (at - CENTRE).abs() <= RADIUS;
        if hit {
            self.clicks += 1;
            self.colour = self.pick_colour();
        }
        hit
    }

    /// A new colour, guaranteed to look different from the current one.
    ///
    /// Two decisions, both to do with what "random colour" should mean:
    ///
    /// **A random hue, not a random RGB.** Most of the colour cube is somewhere
    /// near grey, so random RGB gives mud. The hue wheel passes only through
    /// the vivid ones — which is why a colour picker is a wheel and not a cube.
    ///
    /// **A random step, not a random position.** A random position can land
    /// next to where it already was, and then the click looks like it did
    /// nothing. Stepping between a quarter and three quarters of the way round
    /// is still unpredictable but can never be a near-miss.
    fn pick_colour(&mut self) -> u32 {
        self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let r = (self.seed >> 33) as f64 / 2147483648.0; // the top bits: an LCG's good ones
        self.h = (self.h + 0.25 + 0.5 * r).rem_euclid(1.0);
        hue(self.h)
    }
}

/// A vivid colour from a hue in `[0, 1)`. Saturation and brightness are fixed,
/// so every colour it returns is equally bright against the background.
fn hue(h: f64) -> u32 {
    let (s, v) = (0.55, 0.95);
    let f = |n: f64| {
        let k = (n + h * 6.0).rem_euclid(6.0);
        let c = v - v * s * k.min(4.0 - k).clamp(0.0, 1.0);
        (c * 255.0).round() as u32
    };
    (f(5.0) << 16) | (f(3.0) << 8) | f(1.0)
}

/// **This is the only function you need to edit.**
///
/// `t` is seconds since it started. Return whatever should be on screen then.
#[allow(unused_variables)] // a still picture is allowed to ignore the clock
fn scene(d: &Doodle) -> Frame {
    let t = d.t; // so everything below can just say `t`
    let mut f = Frame::new();

    // --- the circle you can click ----------------------------------------
    // Click inside it and it changes colour. The whole of "is that a hit?" is
    // `|z - c| <= r`, up in main().
    f.add(Shape::circle(CENTRE, RADIUS)).color(d.colour).width(3);
    f.label(Cx::new(0.0, RADIUS + 0.5), format!("click me  ({} so far)", d.clicks), 0x5A6774, 2);

    // --- a shape from the library, sitting still -------------------------
   // f.place(digit::glyph(7, 40), Cx::new(-3.0, 0.0)).color(0x4FBCD4).width(3);

    // --- the same shape, orbiting ----------------------------------------
    // Cx::expi(t) is the point at angle t on the unit circle. Scale it by 2
    // and it orbits at radius 2. That is the whole animation.
   // f.place(face::smiley(0.6), Cx::new(3.0, 0.0) + Cx::expi(t).scale(2.0)).color(0x6FCF97).width(2);

    // --- a curve of your own ---------------------------------------------
    // Shape::param is `t -> z(t)`: give it a journey and it draws the path.
    // This one is a rose, r = cos(5θ), which closes after one full turn.
   /* f.add(Shape::param(move |a| Cx::polar((5.0 * a).cos(), a + t * 0.3), 0.0, TAU, 600))
        .color(0xE585AC)
        .width(2);*/

    // --- something written as an equation --------------------------------
    // Shape::implicit draws the LEVEL SET F(x, y) = c — every point where F
    // takes the value c — without you having to solve for y.
    //
    // So the number is the value of F, NOT a radius. F here is x² + y², which
    // is r², so this is the circle of radius sqrt(c):
    //
    //      c = 1  ->  r = 1      c = 2  ->  r = 1.41      c = 4  ->  r = 2
    //
    // c = 1 matching radius 1 is a coincidence — 1 is the one number that is
    // its own square.
    f.add(Shape::implicit(|x, y| x * x + y * y, 9.0)).color(0x2C3742).width(1);
    //
    // If you would rather the number BE the radius, make F the radius:
    //     f.add(Shape::implicit(|x, y| (x * x + y * y).sqrt(), 2.0));   // |z| = 2
    // or just say what you mean:
    //     f.add(Shape::circle(Cx::ZERO, 2.0));

    // --- a label, pinned to a world position ------------------------------
    //f.label(Cx::new(0.0, -3.4), format!("t = {t:.1}s"), 0x5A6774, 2);

    f
}

// ===========================================================================
//  A few more things to try. Paste one into `scene` and see.
// ===========================================================================
//
//   // twelve of something round a circle — .at() is just addition
//   for k in 0..12 {
//       let spot = Cx::expi(TAU * k as f64 / 12.0).scale(3.0);
//       f.place(face::ghost(0.35), spot).color(0x9B7BD4);
//   }
//
//   // a shape turned by a value you can name, the way you would on paper
//   let r = Cx::expi(t);                       // let R be rotation by t
//   f.add(Shape::unit_square().map(move |z| r * z));   // apply R to the square
//
//   // a Fourier series: add sine waves and watch a square wave appear
//   let terms = 1 + (t as usize % 12) * 2;
//   f.add(Shape::graph(move |x| {
//       (1..=terms).step_by(2).map(|n| (n as f64 * x).sin() / n as f64).sum()
//   })).width(2);
//
//   // any closed curve, redrawn as a stack of rotating arrows
//   let s = digit::series(3);
//   f.add(s.curve(8)).color(0xE0A44A);
//   f.add(s.machine(8, t)).color(0x3B4A59).width(1);
//
//   // tally marks, and the roots of unity as a polygon
//   f.place(count::tally(7), Cx::new(0.0, -2.0));
//   f.add(Shape::ngon(Cx::new(4.0, 2.0), 1.2, 5));

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn doodle() -> Doodle {
        Doodle { t: 0.0, h: 0.52, colour: hue(0.52), clicks: 0, seed: 7 }
    }

    /// A sketch that draws nothing is almost always a mistake, and the window
    /// would just sit there empty telling you nothing about why.
    #[test]
    fn the_scene_actually_draws_something() {
        let f = scene(&doodle());
        assert!(!f.is_empty(), "scene() returned an empty frame");
        let v = View::centred(400, 400, 40.0);
        assert!(f.bounds(&v).is_some(), "nothing in the frame has a position");
    }

    /// ★ Clicking inside changes the colour; clicking outside leaves it alone.
    #[test]
    fn only_a_click_on_the_circle_changes_anything() {
        let mut d = doodle();
        let before = d.colour;

        assert!(!d.click(CENTRE + Cx::new(RADIUS + 0.3, 0.0)), "just outside the rim");
        assert_eq!(d.colour, before, "a miss should change nothing");
        assert_eq!(d.clicks, 0);

        assert!(d.click(CENTRE), "the middle is on the circle");
        assert_ne!(d.colour, before);
        assert_eq!(d.clicks, 1);

        // Anywhere inside counts, not only the middle — and the rim itself is
        // in, because the test is `<=`.
        assert!(d.click(CENTRE + Cx::polar(RADIUS * 0.99, 2.1)));
        assert!(d.click(CENTRE + Cx::polar(RADIUS, 0.0)));
    }

    /// The one-line test and the general one must agree, or the sketch and
    /// the library would disagree about what was clicked.
    #[test]
    fn the_one_liner_agrees_with_shape_contains() {
        let circle = Shape::circle(CENTRE, RADIUS);
        let (lo, hi) = (Cx::new(-9.0, -9.0), Cx::new(9.0, 9.0));
        for k in 0..120 {
            let p = CENTRE + Cx::polar(0.15 * k as f64 / 4.0, 0.9 * k as f64);
            let d = (p - CENTRE).abs();
            if (d - RADIUS).abs() > 0.05 {
                assert_eq!(d <= RADIUS, circle.contains(p, lo, hi, 600), "disagreed at |z-c| = {d}");
            }
        }
    }

    /// ★ A colour that came back the same, or nearly the same, or grey, would
    /// all look like the click had not registered. Stepping round the wheel
    /// rather than jumping to a random point on it is what rules that out.
    #[test]
    fn each_click_gives_a_new_vivid_colour() {
        let mut d = doodle();
        let mut seen = Vec::new();
        let mut last_h = d.h;
        for _ in 0..40 {
            d.click(CENTRE);
            let c = d.colour;
            // How far round the wheel it moved, the short way.
            let step = (d.h - last_h).rem_euclid(1.0).min((last_h - d.h).rem_euclid(1.0));
            assert!(step >= 0.24, "the hue barely moved: {step}");
            last_h = d.h;
            assert!(seen.last() != Some(&c), "the same colour twice running");
            let (r, g, b) = ((c >> 16) & 255, (c >> 8) & 255, c & 255);
            let spread = r.max(g).max(b) - r.min(g).min(b);
            assert!(spread > 60, "{c:06X} is too close to grey (spread {spread})");
            seen.push(c);
        }
        seen.sort_unstable();
        seen.dedup();
        assert!(seen.len() > 25, "only {} different colours in 40 clicks", seen.len());
    }

    /// Hue 0, 1/3 and 2/3 are red, green and blue — the check that the
    /// conversion has its channels the right way round.
    #[test]
    fn the_hue_wheel_starts_at_red_and_comes_back_to_it() {
        let big = |c: u32| {
            let (r, g, b) = ((c >> 16) & 255, (c >> 8) & 255, c & 255);
            if r >= g && r >= b {
                0
            } else if g >= b {
                1
            } else {
                2
            }
        };
        assert_eq!(big(hue(0.0)), 0, "hue 0 should be reddest");
        assert_eq!(big(hue(1.0 / 3.0)), 1, "hue 1/3 should be greenest");
        assert_eq!(big(hue(2.0 / 3.0)), 2, "hue 2/3 should be bluest");
        assert_eq!(hue(0.0), hue(1.0), "the wheel closes");
    }

    // There is deliberately no test that the scene MOVES. A sketch is a
    // scratchpad — a still picture is a perfectly good thing to be drawing,
    // and a test that failed the moment you commented out the moving parts
    // would be the test being wrong, not the sketch.


}

