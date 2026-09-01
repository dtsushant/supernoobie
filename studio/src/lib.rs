//! # studio — a graph to draw on, with the boilerplate taken away
//!
//! A sketch should be a scene function and nothing else. [`Graph`] owns the
//! canvas, the view, the window and the loop, so none of them appear in your
//! file:
//!
//! ```no_run
//! use studio::prelude::*;
//!
//! fn main() {
//!     Graph::new("my sketch").animate(scene);
//! }
//!
//! fn scene(t: f64) -> Frame {
//!     let mut f = Frame::new();
//!     f.place(face::smiley(1.0), Cx::polar(3.0, t));
//!     f.place(digit::glyph(7, 40), Cx::ZERO);
//!     f
//! }
//! ```
//!
//! That is the whole program. `cargo run -p studio --bin sketch` runs a copy of
//! it you can edit.
//!
//! ## The three ways to run a scene
//!
//! | | |
//! |---|---|
//! | [`Graph::plot`] | one still, in a window |
//! | [`Graph::animate`] | `f(t) -> Frame`, in a window |
//! | [`Graph::play`] | `f(t, keys) -> Frame`, for anything that responds |
//!
//! and two that need no window at all, so they work from a test or over ssh:
//!
//! | | |
//! |---|---|
//! | [`Graph::png`] | write a picture to a file |
//! | [`Graph::print`] | draw it in the terminal, in braille |
//!
//! ## Keys the graph handles for you
//!
//! In `plot` and `animate`: `Esc` quits, `G` toggles graph paper, `,` and `.`
//! zoom, the arrow keys pan, `Space` pauses, and `S` saves a PNG.
//!
//! In `play` only `Esc` and `G` are reserved — everything else reaches your
//! scene through [`Keys`], because a sketch that responds to keys needs them
//! more than the graph does.

use minifb::{Key, KeyRepeat, Window, WindowOptions};
use plotkit::{plot, raster::Canvas, Cx, Frame, View};

/// Everything a sketch is likely to want, in one line.
///
/// ```no_run
/// use studio::prelude::*;
/// ```
pub mod prelude {
    pub use crate::{Graph, Keys};
    pub use plotkit::{plot, Canvas, Cx, Frame, Shape, View};
    pub use shapes::{count, digit, face, fourier, glyph, wave};
    pub use shapes::{Draw, Place, Recipe, Series, Wave};
    pub use std::f64::consts::{PI, TAU};
}

/// A graph to draw on. Build it, then hand it a scene.
pub struct Graph {
    title: String,
    w: usize,
    h: usize,
    /// Pixels per unit. `None` means fit to the first frame.
    scale: Option<f64>,
    origin: Option<(f64, f64)>,
    grid: bool,
    background: u32,
}

impl Graph {
    /// 1100×700, graph paper on, fitted to whatever you draw.
    pub fn new(title: impl Into<String>) -> Graph {
        Graph { title: title.into(), w: 1100, h: 700, scale: None, origin: None, grid: true, background: 0x0B1017 }
    }

    pub fn size(mut self, w: usize, h: usize) -> Graph {
        (self.w, self.h) = (w, h);
        self
    }

    /// Pixels per unit, fixed. Without this the graph fits itself to the first
    /// frame — which is usually what a sketch wants, and never what an
    /// animation that grows wants.
    pub fn scale(mut self, k: f64) -> Graph {
        self.scale = Some(k);
        self
    }

    /// Where the origin sits, as a fraction of the window. `(0.5, 0.5)` is the
    /// middle, which is the default.
    pub fn origin(mut self, x: f64, y: f64) -> Graph {
        self.origin = Some((x, y));
        self
    }

    pub fn grid(mut self, on: bool) -> Graph {
        self.grid = on;
        self
    }

    pub fn background(mut self, c: u32) -> Graph {
        self.background = c;
        self
    }

    // ---- running it ------------------------------------------------------

    /// One still picture, in a window.
    pub fn plot(self, frame: Frame) {
        self.animate(move |_| frame.clone());
    }

    /// A film: `f(t) -> Frame`, with `t` in seconds.
    ///
    /// The graph keeps the pan, zoom and pause keys for itself here, since a
    /// scene that ignores the keyboard has no use for them.
    pub fn animate(self, mut scene: impl FnMut(f64) -> Frame) {
        self.run(move |t, _| scene(t), true);
    }

    /// A film that answers back: `f(t, keys) -> Frame`.
    ///
    /// Only `Esc` and `G` stay reserved — everything else reaches your scene,
    /// because a sketch that reads the keyboard needs it more than the graph
    /// does.
    pub fn play(self, scene: impl FnMut(f64, &Keys) -> Frame) {
        self.run(scene, false);
    }

    fn run(self, mut scene: impl FnMut(f64, &Keys) -> Frame, reserved: bool) {
        let (w, h) = (self.w, self.h);
        let mut c = Canvas::new(w, h);
        let mut win =
            Window::new(&self.title, w, h, WindowOptions::default()).expect("could not open a window");
        win.set_target_fps(60);

        let mut view = self.view_for(&scene(0.0, &Keys::none()));
        let mut grid = self.grid;
        let (mut t, mut paused, mut saves) = (0.0f64, false, 0);

        while win.is_open() && !win.is_key_down(Key::Escape) {
            let keys = Keys::read(&win);

            if keys.just('g') {
                grid = !grid;
            }
            if keys.just('s') {
                saves += 1;
                let path = format!("{}-{saves}.png", slug(&self.title));
                let mut shot = Canvas::new(w, h);
                shot.clear(self.background);
                scene(t, &keys).draw(&mut shot, &view);
                match shot.write_png(&path) {
                    Ok(()) => println!("wrote {path}"),
                    Err(e) => eprintln!("could not write {path}: {e}"),
                }
            }

            if !paused {
                t += 1.0 / 60.0;
            }

            c.clear(self.background);
            if grid {
                plot::grid(&mut c, &view, &plot::GridStyle::default());
            }
            scene(t, &keys).draw(&mut c, &view);
            win.update_with_buffer(&c.buf, w, h).expect("could not present the frame");

            // Pan and zoom last, so a sketch reading the same key this frame
            // is not fighting the graph over it.
            if reserved {
                if keys.just(' ') {
                    paused = !paused;
                }
                // Panning moves the ORIGIN in pixels, so it feels the same
                // however far you are zoomed in.
                let d = keys.arrows().scale(18.0);
                view.origin.0 -= d.re;
                view.origin.1 += d.im;
                if keys.just('.') {
                    view.scale *= 1.25;
                }
                if keys.just(',') {
                    view.scale /= 1.25;
                }
            }
        }
    }

    /// A still, written to a PNG. No window, so this works headless.
    pub fn png(self, path: &str, frame: Frame) -> std::io::Result<()> {
        let mut c = Canvas::new(self.w, self.h);
        c.clear(self.background);
        let view = self.view_for(&frame);
        if self.grid {
            plot::grid(&mut c, &view, &plot::GridStyle::default());
        }
        frame.draw(&mut c, &view);
        c.write_png(path)
    }

    /// A still, in the terminal, as braille. Also needs no window.
    pub fn print(self, frame: Frame) {
        // Square in pixels: a braille cell is 2 wide and 4 tall while a
        // terminal character is about twice as tall as it is wide, and the two
        // cancel. A canvas shaped like the window would come out flat.
        let side = self.w.min(160);
        let mut c = Canvas::new(side, side);
        c.clear(0x000000);
        let view = Graph { w: side, h: side, grid: false, ..self }.view_for(&frame);
        frame.draw(&mut c, &view);
        println!("{}", c.braille(0x000000, true));
    }

    /// The view to draw through: what was asked for, or one fitted to the
    /// frame with a margin.
    fn view_for(&self, frame: &Frame) -> View {
        let (ox, oy) = self.origin.unwrap_or((0.5, 0.5));
        let place = |scale: f64| View::centred(self.w, self.h, scale).with_origin(self.w as f64 * ox, self.h as f64 * oy);

        let Some(want) = self.scale else {
            // Measure through a deliberately wide view so nothing is clipped
            // out of the measurement, then fit to what came back.
            let rough = place(8.0);
            let Some((lo, hi)) = frame.bounds(&rough) else {
                return place(60.0); // nothing to measure
            };
            let reach = (hi.re - lo.re).max(hi.im - lo.im).max(1e-6);
            let mid = (lo + hi).scale(0.5);
            let fitted = (self.h.min(self.w) as f64 * 0.82) / reach;
            let mut v = place(fitted);
            // Centre on the drawing rather than on the origin.
            v.origin.0 -= mid.re * v.scale;
            v.origin.1 += mid.im * v.scale;
            return v;
        };
        place(want)
    }
}

fn slug(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect()
}

/// What the keyboard is doing, without minifb in your file.
pub struct Keys {
    pressed: Vec<Key>,
    held: Vec<Key>,
}

impl Keys {
    fn none() -> Keys {
        Keys { pressed: Vec::new(), held: Vec::new() }
    }

    fn read(win: &Window) -> Keys {
        Keys { pressed: win.get_keys_pressed(KeyRepeat::No), held: win.get_keys() }
    }

    /// Pressed this frame — for anything that should happen once per press.
    pub fn just(&self, c: char) -> bool {
        key_of(c).is_some_and(|k| self.pressed.contains(&k))
    }

    /// Held down — for anything that should happen continuously.
    pub fn held(&self, c: char) -> bool {
        key_of(c).is_some_and(|k| self.held.contains(&k))
    }

    /// The arrow keys as a direction, so `z + keys.arrows().scale(speed)`
    /// moves a thing about. Right is `1`, up is `i`, as they should be.
    pub fn arrows(&self) -> Cx {
        let axis = |neg: Key, pos: Key| {
            f64::from(self.held.contains(&pos) as i8) - f64::from(self.held.contains(&neg) as i8)
        };
        Cx::new(axis(Key::Left, Key::Right), axis(Key::Down, Key::Up))
    }

    /// Digits typed this frame, in order — for anything that takes a number.
    pub fn digits(&self) -> Vec<u32> {
        self.pressed.iter().filter_map(digit_of).collect()
    }

    pub fn enter(&self) -> bool {
        self.pressed.contains(&Key::Enter) || self.pressed.contains(&Key::NumPadEnter)
    }

    pub fn backspace(&self) -> bool {
        self.pressed.contains(&Key::Backspace)
    }

}

fn digit_of(k: &Key) -> Option<u32> {
    use Key::*;
    Some(match k {
        Key0 | NumPad0 => 0,
        Key1 | NumPad1 => 1,
        Key2 | NumPad2 => 2,
        Key3 | NumPad3 => 3,
        Key4 | NumPad4 => 4,
        Key5 | NumPad5 => 5,
        Key6 | NumPad6 => 6,
        Key7 | NumPad7 => 7,
        Key8 | NumPad8 => 8,
        Key9 | NumPad9 => 9,
        _ => return None,
    })
}

fn key_of(c: char) -> Option<Key> {
    use Key::*;
    Some(match c.to_ascii_lowercase() {
        'a' => A, 'b' => B, 'c' => C, 'd' => D, 'e' => E, 'f' => F, 'g' => G,
        'h' => H, 'i' => I, 'j' => J, 'k' => K, 'l' => L, 'm' => M, 'n' => N,
        'o' => O, 'p' => P, 'q' => Q, 'r' => R, 's' => S, 't' => T, 'u' => U,
        'v' => V, 'w' => W, 'x' => X, 'y' => Y, 'z' => Z,
        '0' => Key0, '1' => Key1, '2' => Key2, '3' => Key3, '4' => Key4,
        '5' => Key5, '6' => Key6, '7' => Key7, '8' => Key8, '9' => Key9,
        ' ' => Space,
        ',' => Comma,
        '.' => Period,
        '-' => Minus,
        '=' => Equal,
        '\t' => Tab,
        '\n' => Enter,
        _ => return None,
    })
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use plotkit::Shape;
    use shapes::{face, Draw};

    fn one_smiley_at(z: Cx) -> Frame {
        let mut f = Frame::new();
        f.place(face::smiley(1.0), z);
        f
    }

    /// ★ Fitting is what removes the last decision from a sketch: draw
    /// whatever you like and it appears at a sensible size. A shape ten times
    /// bigger must come out the same size on screen, at a tenth the scale.
    #[test]
    fn fitting_makes_any_size_of_drawing_fill_the_window() {
        let g = || Graph::new("t").size(400, 400);
        let small = g().view_for(&one_smiley_at(Cx::ZERO));

        let mut big = Frame::new();
        big.place(face::smiley(10.0), Cx::ZERO);
        let large = g().view_for(&big);

        assert!((small.scale / large.scale - 10.0).abs() < 0.1, "{} vs {}", small.scale, large.scale);
    }

    /// Fitting centres on the drawing, not on the origin — otherwise anything
    /// drawn off to one side would sit half out of the window.
    #[test]
    fn fitting_centres_on_the_drawing() {
        let v = Graph::new("t").size(400, 400).view_for(&one_smiley_at(Cx::new(50.0, -20.0)));
        let (x, y) = v.to_screen(Cx::new(50.0, -20.0));
        assert!((x - 200).abs() < 6 && (y - 200).abs() < 6, "the drawing landed at {x}, {y}");
    }

    /// An explicit scale is obeyed exactly. Fitting is a convenience, not a
    /// policy — a sketch that says 40 pixels per unit gets 40.
    #[test]
    fn an_explicit_scale_wins() {
        let v = Graph::new("t").size(400, 400).scale(40.0).view_for(&one_smiley_at(Cx::new(9.0, 9.0)));
        assert_eq!(v.scale, 40.0);
    }

    /// An empty frame has nothing to fit to, and must not divide by zero or
    /// hand back a view of scale NaN.
    #[test]
    fn an_empty_frame_still_gets_a_usable_view() {
        let v = Graph::new("t").size(300, 300).view_for(&Frame::new());
        assert!(v.scale.is_finite() && v.scale > 0.0, "scale was {}", v.scale);
    }

    /// A single point has zero extent. Fitting to it must not divide by zero.
    #[test]
    fn a_single_point_does_not_divide_by_zero() {
        let mut f = Frame::new();
        f.add(Shape::point(Cx::new(2.0, 2.0)));
        let v = Graph::new("t").size(300, 300).view_for(&f);
        assert!(v.scale.is_finite() && v.scale > 0.0, "scale was {}", v.scale);
    }

    #[test]
    fn arrows_read_as_a_direction() {
        let k = Keys { pressed: vec![], held: vec![Key::Right, Key::Up] };
        assert_eq!(k.arrows(), Cx::new(1.0, 1.0));
        let k = Keys { pressed: vec![], held: vec![Key::Left] };
        assert_eq!(k.arrows(), Cx::new(-1.0, 0.0));
        // Both at once cancel, rather than one winning arbitrarily.
        let k = Keys { pressed: vec![], held: vec![Key::Left, Key::Right] };
        assert_eq!(k.arrows(), Cx::ZERO);
    }

    #[test]
    fn just_and_held_are_different_questions() {
        let k = Keys { pressed: vec![Key::N], held: vec![Key::N, Key::E] };
        assert!(k.just('n') && k.held('n'));
        assert!(k.held('e') && !k.just('e'), "e is down but was not pressed this frame");
    }

    #[test]
    fn digits_come_back_in_order() {
        let k = Keys { pressed: vec![Key::Key4, Key::NumPad2], held: vec![] };
        assert_eq!(k.digits(), vec![4, 2]);
    }

    #[test]
    fn a_title_becomes_a_safe_filename() {
        assert_eq!(slug("My Sketch!"), "my-sketch-");
    }
}
