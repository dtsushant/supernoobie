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

fn main() {
    Graph::new("sketch").animate(scene);

    // Other ways to run the same scene:
    //
    //   Graph::new("sketch").scale(60.0).animate(scene);   // fixed zoom
    //   Graph::new("sketch").grid(false).animate(scene);   // no graph paper
    //   Graph::new("sketch").plot(scene(0.0));             // one still
    //   Graph::new("sketch").print(scene(0.0));            // in the terminal
    //   Graph::new("sketch").png("out.png", scene(0.0)).unwrap();
    //
    // and for something that answers the keyboard:
    //
    //   Graph::new("sketch").play(|t, keys| { ... });
}

/// **This is the only function you need to edit.**
///
/// `t` is seconds since it started. Return whatever should be on screen then.
#[allow(unused_variables)] // a still picture is allowed to ignore the clock
fn scene(t: f64) -> Frame {
    let mut f = Frame::new();

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
    f.add(Shape::implicit(|x, y| x * x + y * y, 4.0)).color(0x2C3742).width(1);
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

    /// A sketch that draws nothing is almost always a mistake, and the window
    /// would just sit there empty telling you nothing about why.
    #[test]
    fn the_scene_actually_draws_something() {
        let f = scene(0.0);
        assert!(!f.is_empty(), "scene() returned an empty frame");
        let v = View::centred(400, 400, 40.0);
        assert!(f.bounds(&v).is_some(), "nothing in the frame has a position");
    }

    // There is deliberately no test that the scene MOVES. A sketch is a
    // scratchpad — a still picture is a perfectly good thing to be drawing,
    // and a test that failed the moment you commented out the moving parts
    // would be the test being wrong, not the sketch.


}
