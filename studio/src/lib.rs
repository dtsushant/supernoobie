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
//! ## Moving about the page
//!
//! **In every mode:** the wheel zooms about the pointer, dragging with the
//! **right button** slides the paper around, and `Home` puts it back where it
//! started. All on the mouse, so a sketch keeps every key for itself.
//!
//! Zooming keeps whatever is under the pointer under the pointer, rather than
//! zooming about the origin — see [`zoom_about`]. Without that, the thing you
//! are closing in on slides away from you as you approach it.
//!
//! ## Keys the graph handles for you
//!
//! In every mode: `Esc` quits, `G` toggles graph paper, `Home` resets the view.
//!
//! In `plot` and `animate` only, where the scene is not reading the keyboard:
//! `,` and `.` zoom, the arrow keys pan, `Space` pauses, and `S` saves a PNG.
//! In `play` those all reach your scene instead, because a sketch that responds
//! to keys needs them more than the graph does.
//!
//! ## Binding keys to handlers
//!
//! Give the graph some state with [`Graph::with`] and the keys can be bound
//! one by one, so the code reads like the table of controls it implements:
//!
//! ```no_run
//! # use studio::prelude::*;
//! # struct Game { n: u32, paused: bool }
//! # fn scene(_: &Game) -> Frame { Frame::new() }
//! Graph::new("game")
//!     .with(Game { n: 0, paused: false })
//!     .on('p', |g| g.paused = !g.paused)
//!     .on_digit(|g, d| g.n = d)
//!     .on_hold('w', |g| g.n += 1)
//!     .run(scene);
//! ```
//!
//! See [`Sketch`] for why it is written this way rather than as
//! `key('p').click(f)`.

use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Window, WindowOptions};
use plotkit::{plot, raster::Canvas, Cx, Frame, View};

/// Everything a sketch is likely to want, in one line.
///
/// ```no_run
/// use studio::prelude::*;
/// ```
pub mod prelude {
    pub use crate::{Graph, Keys, Sketch};
    pub use plotkit::{plot, Canvas, Cx, Frame, Shape, View};
    pub use shapes::{count, digit, face, fourier, glyph, grab, motion, troupe, wave};
    pub use shapes::{Actor, Disc, Draw, Motion, Place, Pose, Recipe, Series, Troupe, Wave};
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
        let home_view = view;
        let (mut t, mut paused, mut saves) = (0.0f64, false, 0);
        let (mut was_down, mut last_pan) = (false, None::<(f64, f64)>);

        while win.is_open() && !win.is_key_down(Key::Escape) {
            let keys = Keys::read(&win, &view, was_down);
            was_down = keys.down;

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

            // --- pan and zoom, in every mode -----------------------------
            //
            // On the mouse, never the keyboard, so a sketch keeps every key
            // for itself. The wheel zooms, the right button drags the paper
            // about, and Home puts it back.
            if keys.scroll().abs() > 1e-6 {
                zoom_about(&mut view, keys.at_px(), 1.0 + 0.14 * keys.scroll().clamp(-3.0, 3.0));
            }
            if keys.right_down() {
                if let Some((lx, ly)) = last_pan {
                    // The origin is in pixels, so moving it by the pointer's
                    // own delta makes the paper follow the hand exactly.
                    view.origin.0 += keys.at_px().0 - lx;
                    view.origin.1 += keys.at_px().1 - ly;
                }
                last_pan = Some(keys.at_px());
            } else {
                last_pan = None;
            }
            if keys.home {
                view = home_view;
            }

            // Keyboard pan and zoom last, so a sketch reading the same key
            // this frame is not fighting the graph over it.
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

    /// Hand the graph a piece of state to look after, so keys can be bound to
    /// handlers that change it. See [`Sketch`].
    ///
    /// Call this **after** the builders — `.size`, `.scale` and friends belong
    /// to the graph, and this hands the graph over.
    pub fn with<S>(self, state: S) -> Sketch<S> {
        Sketch { graph: self, state, bindings: Vec::new() }
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

// ===========================================================================
//  Binding keys to handlers
// ===========================================================================

/// A graph with a piece of state, and keys bound to handlers that change it.
///
/// ```no_run
/// # use studio::prelude::*;
/// # struct Game { score: u32, paused: bool }
/// # fn scene(_: &Game) -> Frame { Frame::new() }
/// # let game = Game { score: 0, paused: false };
/// Graph::new("game")
///     .with(game)
///     .each_frame(|g, t| g.score = t as u32)
///     .on('p', |g| g.paused = !g.paused)
///     .on_digit(|g, d| g.score += d)
///     .run(scene);
/// ```
///
/// ## Why not `key('p').click(f)`
///
/// In JavaScript every handler can reach the same object, because everything
/// is shared and mutable. Rust will not let two closures both hold `&mut game`
/// — one of them could invalidate what the other is looking at, and that is
/// the class of bug the language exists to stop.
///
/// So the state lives *here*, and each handler is lent it for the moment it
/// runs. You get the same shape of code — a list of bindings, read top to
/// bottom, each naming a key and what it does — with none of the aliasing.
pub struct Sketch<S> {
    graph: Graph,
    state: S,
    bindings: Vec<Binding<S>>,
}

enum Binding<S> {
    /// Pressed this frame.
    Press(char, Box<dyn FnMut(&mut S)>),
    /// Held down, fired every frame it stays down.
    Hold(char, Box<dyn FnMut(&mut S)>),
    Enter(Box<dyn FnMut(&mut S)>),
    Backspace(Box<dyn FnMut(&mut S)>),
    Digit(Box<dyn FnMut(&mut S, u32)>),
    /// The arrow keys, as a direction. Fires every frame, `0` when idle.
    Arrows(Box<dyn FnMut(&mut S, Cx)>),
    /// Every frame, whatever the keyboard is doing.
    Frame(Box<dyn FnMut(&mut S, f64)>),
    /// A mouse click, with where it landed in world coordinates.
    Click(Box<dyn FnMut(&mut S, Cx)>),
    /// The pointer every frame: where it is, and whether the button is down.
    Pointer(Box<dyn FnMut(&mut S, Cx, bool)>),
}

impl<S: 'static> Sketch<S> {
    /// A key press, once per press however long it is held.
    pub fn on(mut self, key: char, f: impl FnMut(&mut S) + 'static) -> Self {
        self.bindings.push(Binding::Press(key, Box::new(f)));
        self
    }

    /// A key held down, fired every frame it stays down — for movement and
    /// anything else that should be continuous rather than stepwise.
    pub fn on_hold(mut self, key: char, f: impl FnMut(&mut S) + 'static) -> Self {
        self.bindings.push(Binding::Hold(key, Box::new(f)));
        self
    }

    pub fn on_enter(mut self, f: impl FnMut(&mut S) + 'static) -> Self {
        self.bindings.push(Binding::Enter(Box::new(f)));
        self
    }

    pub fn on_backspace(mut self, f: impl FnMut(&mut S) + 'static) -> Self {
        self.bindings.push(Binding::Backspace(Box::new(f)));
        self
    }

    /// Each digit typed this frame, in the order it was typed.
    pub fn on_digit(mut self, f: impl FnMut(&mut S, u32) + 'static) -> Self {
        self.bindings.push(Binding::Digit(Box::new(f)));
        self
    }

    /// The arrow keys as a direction — right is `1`, up is `i`. Fires every
    /// frame, handing you `0` when nothing is held, so a single
    /// `z = z + dir.scale(speed)` is the whole of movement.
    pub fn on_arrows(mut self, f: impl FnMut(&mut S, Cx) + 'static) -> Self {
        self.bindings.push(Binding::Arrows(Box::new(f)));
        self
    }

    /// A mouse click, handed the position **in world coordinates** — the same
    /// numbers the scene is written in, so a hit test is just the mathematics:
    ///
    /// ```text
    ///     if (at - centre).abs() <= radius { ... }     // |z - c| <= r
    /// ```
    ///
    /// Fires once when the button goes down, not every frame it is held.
    pub fn on_click(mut self, f: impl FnMut(&mut S, Cx) + 'static) -> Self {
        self.bindings.push(Binding::Click(Box::new(f)));
        self
    }

    /// The pointer every frame — where it is and whether the button is down.
    ///
    /// [`Sketch::on_click`] is the edge; this is the whole state, which is what
    /// dragging needs. A drag is not a click, it is a press, some movement, and
    /// a release, and only something watching every frame can see the middle
    /// part.
    ///
    /// ```no_run
    /// # use studio::prelude::*;
    /// # use shapes::grab::Disc;
    /// # struct S { disc: Disc }
    /// # fn scene(_: &S) -> Frame { Frame::new() }
    /// Graph::new("x")
    ///     .with(S { disc: Disc::new(Cx::ZERO, 2.0) })
    ///     .on_pointer(|s, at, down| s.disc.drag(at, down))
    ///     .run(scene);
    /// ```
    pub fn on_pointer(mut self, f: impl FnMut(&mut S, Cx, bool) + 'static) -> Self {
        self.bindings.push(Binding::Pointer(Box::new(f)));
        self
    }

    /// Every frame, with the time in seconds. Runs **before** the key
    /// bindings, so the clock is already up to date when they fire.
    pub fn each_frame(mut self, f: impl FnMut(&mut S, f64) + 'static) -> Self {
        self.bindings.push(Binding::Frame(Box::new(f)));
        self
    }

    /// Fire everything one frame would fire, without a window.
    ///
    /// [`Sketch::run`] calls this; so can a test, which is the point — the
    /// bindings are ordinary code and do not need a screen to be checked.
    pub fn step(&mut self, t: f64, keys: &Keys) {
        // Frame handlers first, in registration order, then the rest — so
        // anything that updates a clock has done so before a key reads it.
        for b in &mut self.bindings {
            if let Binding::Frame(f) = b {
                f(&mut self.state, t);
            }
        }
        for b in &mut self.bindings {
            match b {
                Binding::Frame(_) => {}
                Binding::Press(k, f) => {
                    if keys.just(*k) {
                        f(&mut self.state);
                    }
                }
                Binding::Hold(k, f) => {
                    if keys.held(*k) {
                        f(&mut self.state);
                    }
                }
                Binding::Enter(f) => {
                    if keys.enter() {
                        f(&mut self.state);
                    }
                }
                Binding::Backspace(f) => {
                    if keys.backspace() {
                        f(&mut self.state);
                    }
                }
                Binding::Digit(f) => {
                    for d in keys.digits() {
                        f(&mut self.state, d);
                    }
                }
                Binding::Arrows(f) => f(&mut self.state, keys.arrows()),
                Binding::Click(f) => {
                    if keys.clicked() {
                        f(&mut self.state, keys.at());
                    }
                }
                Binding::Pointer(f) => f(&mut self.state, keys.at(), keys.down()),
            }
        }
    }

    pub fn state(&self) -> &S {
        &self.state
    }

    /// Open the window and go. `draw` turns the state into a picture and is
    /// the only part that sees it whole.
    pub fn run(self, mut draw: impl FnMut(&S) -> Frame + 'static) {
        let Sketch { graph, state, bindings } = self;
        let mut me = Sketch { graph: Graph::new(""), state, bindings };
        graph.play(move |t, keys| {
            me.step(t, keys);
            draw(&me.state)
        });
    }
}

/// Zoom a view by `factor`, keeping whatever is under `(px, py)` under it.
///
/// The obvious version — just multiply the scale — zooms about the origin, so
/// the thing you were looking at slides off the edge as you close in on it.
/// Instead: note the world point under the pointer, change the scale, then put
/// the origin wherever it has to be for that point to land back under the
/// pointer. From
///
/// ```text
///     px = origin.x + scale * world.x        py = origin.y - scale * world.y
/// ```
///
/// solving for the origin is one line each, and the minus on `y` is the same
/// minus that makes `y` count upwards.
pub fn zoom_about(view: &mut View, (px, py): (f64, f64), factor: f64) {
    let anchor = view.to_world(px, py);
    view.scale = (view.scale * factor).clamp(1e-4, 1e7);
    view.origin.0 = px - view.scale * anchor.re;
    view.origin.1 = py + view.scale * anchor.im;
}

fn slug(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect()
}

/// What the keyboard is doing, without minifb in your file.
pub struct Keys {
    pressed: Vec<Key>,
    held: Vec<Key>,
    /// Where the pointer is, **in world coordinates** — already through the
    /// view, so it can be compared with the numbers a scene is written in.
    at: Cx,
    /// The pointer in *screen* pixels, which is what zooming about it needs.
    at_px: (f64, f64),
    over: bool,
    down: bool,
    clicked: bool,
    right_down: bool,
    scroll: f64,
    home: bool,
}

impl Keys {
    fn none() -> Keys {
        Keys {
            pressed: Vec::new(),
            held: Vec::new(),
            at: Cx::ZERO,
            at_px: (0.0, 0.0),
            over: false,
            down: false,
            clicked: false,
            right_down: false,
            scroll: 0.0,
            home: false,
        }
    }

    /// Where the pointer is, in the same coordinates the scene is written in.
    ///
    /// So a hit test is the mathematics itself — `(keys.at() - c).abs() <= r`
    /// **is** the definition of the disc of radius `r` about `c`. For an
    /// arbitrary shape, [`plotkit::Shape::contains`] answers the same question.
    pub fn at(&self) -> Cx {
        self.at
    }

    /// Is the pointer over the window at all?
    pub fn over(&self) -> bool {
        self.over
    }

    /// The button, held down.
    pub fn down(&self) -> bool {
        self.down
    }

    /// The button, the moment it went down — once per click, not once per
    /// frame the finger is resting on it.
    pub fn clicked(&self) -> bool {
        self.clicked
    }

    /// A key state built by hand, for tests and for driving a sketch with no
    /// window. Everything listed counts as both pressed this frame and held.
    ///
    /// ```
    /// # use studio::Keys;
    /// let k = Keys::pressing("n7");
    /// assert!(k.just('n'));
    /// assert_eq!(k.digits(), vec![7]);
    /// ```
    pub fn pressing(keys: &str) -> Keys {
        let v: Vec<Key> = keys.chars().filter_map(key_of).collect();
        Keys { pressed: v.clone(), held: v, ..Keys::none() }
    }

    /// A click at a world position, for tests and headless use.
    pub fn clicking(at: Cx) -> Keys {
        Keys { at, over: true, down: true, clicked: true, ..Keys::none() }
    }

    /// Keys held but not newly pressed — so [`Keys::held`] answers yes and
    /// [`Keys::just`] answers no.
    pub fn holding(keys: &str) -> Keys {
        Keys { held: keys.chars().filter_map(key_of).collect(), ..Keys::none() }
    }

    /// The pointer in screen pixels.
    pub fn at_px(&self) -> (f64, f64) {
        self.at_px
    }

    /// The right button, held. Used by the graph for panning.
    pub fn right_down(&self) -> bool {
        self.right_down
    }

    /// How far the wheel turned this frame. Positive is away from you.
    pub fn scroll(&self) -> f64 {
        self.scroll
    }

    fn read(win: &Window, view: &View, was_down: bool) -> Keys {
        // Clamp for the position so it stays usable while dragging off the
        // edge; Discard only to ask whether the pointer is over us at all.
        let (mx, my) = win.get_mouse_pos(MouseMode::Clamp).unwrap_or((0.0, 0.0));
        let down = win.get_mouse_down(MouseButton::Left);
        let pressed = win.get_keys_pressed(KeyRepeat::No);
        Keys {
            home: pressed.contains(&Key::Home),
            pressed,
            held: win.get_keys(),
            at: view.to_world(mx as f64, my as f64),
            at_px: (mx as f64, my as f64),
            over: win.get_mouse_pos(MouseMode::Discard).is_some(),
            down,
            // Edge-triggered: a click is the transition, not the state.
            clicked: down && !was_down,
            right_down: win.get_mouse_down(MouseButton::Right),
            scroll: win.get_scroll_wheel().map_or(0.0, |(_, y)| y as f64),
        }
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
        let k = Keys { pressed: vec![], held: vec![Key::Right, Key::Up], ..Keys::none() };
        assert_eq!(k.arrows(), Cx::new(1.0, 1.0));
        let k = Keys { pressed: vec![], held: vec![Key::Left], ..Keys::none() };
        assert_eq!(k.arrows(), Cx::new(-1.0, 0.0));
        // Both at once cancel, rather than one winning arbitrarily.
        let k = Keys { pressed: vec![], held: vec![Key::Left, Key::Right], ..Keys::none() };
        assert_eq!(k.arrows(), Cx::ZERO);
    }

    #[test]
    fn just_and_held_are_different_questions() {
        let k = Keys { pressed: vec![Key::N], held: vec![Key::N, Key::E], ..Keys::none() };
        assert!(k.just('n') && k.held('n'));
        assert!(k.held('e') && !k.just('e'), "e is down but was not pressed this frame");
    }

    #[test]
    fn digits_come_back_in_order() {
        let k = Keys { pressed: vec![Key::Key4, Key::NumPad2], ..Keys::none() };
        assert_eq!(k.digits(), vec![4, 2]);
    }

    #[test]
    fn a_title_becomes_a_safe_filename() {
        assert_eq!(slug("My Sketch!"), "my-sketch-");
    }

    // ---- bindings --------------------------------------------------------

    struct Toy {
        n: i32,
        typed: String,
        at: Cx,
        t: f64,
        enters: i32,
    }

    impl Default for Toy {
        fn default() -> Toy {
            Toy { n: 0, typed: String::new(), at: Cx::ZERO, t: 0.0, enters: 0 }
        }
    }

    fn toy() -> Sketch<Toy> {
        Graph::new("t")
            .with(Toy::default())
            .each_frame(|s, t| s.t = t)
            .on('p', |s| s.n += 1)
            .on_hold('w', |s| s.n += 10)
            .on_enter(|s| s.enters += 1)
            .on_backspace(|s| {
                s.typed.pop();
            })
            .on_digit(|s, d| s.typed.push(char::from_digit(d, 10).expect("0..=9")))
            .on_arrows(|s, dir| s.at = s.at + dir)
    }

    /// ★ `on` fires once per press; `on_hold` fires every frame the key is
    /// down. Getting these the same way round would make a menu key repeat and
    /// a movement key stutter.
    #[test]
    fn a_press_fires_once_and_a_hold_fires_every_frame() {
        let mut s = toy();
        for _ in 0..5 {
            s.step(0.0, &Keys::holding("p")); // down, but not newly pressed
        }
        assert_eq!(s.state().n, 0, "a held key should not repeat an `on` binding");

        s.step(0.0, &Keys::pressing("p"));
        assert_eq!(s.state().n, 1);

        for _ in 0..3 {
            s.step(0.0, &Keys::holding("w"));
        }
        assert_eq!(s.state().n, 31, "an `on_hold` binding should fire every frame");
    }

    /// Digits arrive in the order they were typed, one call each.
    #[test]
    fn digits_reach_their_handler_in_order() {
        let mut s = toy();
        s.step(0.0, &Keys::pressing("42"));
        s.step(0.0, &Keys::pressing("7"));
        assert_eq!(s.state().typed, "427");
        s.step(0.0, &Keys::pressing("\n"));
        assert_eq!(s.state().enters, 1);
    }

    /// Arrows fire every frame, handing over `0` when nothing is held — so a
    /// single `z + dir.scale(speed)` is the whole of movement, with no special
    /// case for standing still.
    #[test]
    fn arrows_fire_every_frame_even_when_idle() {
        let mut s = toy();
        s.step(0.0, &Keys::none());
        assert_eq!(s.state().at, Cx::ZERO);
        s.step(0.0, &Keys { pressed: vec![], held: vec![Key::Right, Key::Up], ..Keys::none() });
        s.step(0.0, &Keys { pressed: vec![], held: vec![Key::Right], ..Keys::none() });
        assert_eq!(s.state().at, Cx::new(2.0, 1.0));
    }

    /// ★ `each_frame` runs before the key bindings. A handler that reads the
    /// clock — "how long ago did this happen" — would otherwise be a frame
    /// behind, and the bug would be invisible until something timed out early.
    #[test]
    fn the_clock_is_set_before_any_key_is_handled() {
        let seen = std::rc::Rc::new(std::cell::Cell::new(-1.0));
        let peek = seen.clone();
        let mut s = Graph::new("t")
            .with(Toy::default())
            .on('p', move |st: &mut Toy| peek.set(st.t))
            .each_frame(|st: &mut Toy, t| st.t = t); // registered LAST on purpose

        s.step(4.5, &Keys::pressing("p"));
        assert_eq!(seen.get(), 4.5, "the handler saw a stale clock");
    }

    /// Bindings fire in the order they were written, so two handlers on the
    /// same key compose predictably instead of racing.
    #[test]
    fn bindings_fire_in_the_order_written() {
        let mut s = Graph::new("t")
            .with(String::new())
            .on('p', |st: &mut String| st.push('a'))
            .on('p', |st: &mut String| st.push('b'));
        s.step(0.0, &Keys::pressing("p"));
        assert_eq!(s.state(), "ab");
    }

    /// ★ A click is the moment the button goes down, not every frame it is
    /// held. The other way round, one click on a colour swatch would cycle it
    /// sixty times a second.
    #[test]
    fn a_click_fires_once_per_press() {
        let hits = std::rc::Rc::new(std::cell::Cell::new(0));
        let count = hits.clone();
        let mut s = Graph::new("t").with(Cx::ZERO).on_click(move |st: &mut Cx, at| {
            count.set(count.get() + 1);
            *st = at;
        });

        // Down for three frames, but only the first is a click.
        let spot = Cx::new(2.0, -1.0);
        s.step(0.0, &Keys::clicking(spot));
        s.step(0.0, &Keys { down: true, clicked: false, at: spot, over: true, ..Keys::none() });
        s.step(0.0, &Keys { down: true, clicked: false, at: spot, over: true, ..Keys::none() });
        assert_eq!(hits.get(), 1, "a held button should not repeat");
        assert_eq!(*s.state(), spot, "the handler is told where it landed");

        s.step(0.0, &Keys::none());
        s.step(0.0, &Keys::clicking(spot));
        assert_eq!(hits.get(), 2, "releasing and pressing again is a second click");
    }

    /// The position handed to a click handler is in world coordinates, so a
    /// hit test can be written in the same numbers as the scene.
    #[test]
    fn a_click_arrives_in_world_coordinates() {
        let v = View::centred(400, 400, 50.0);
        // The middle of the window is the origin; 50 px right is 1 unit right.
        assert!((v.to_world(200.0, 200.0) - Cx::ZERO).abs() < 1e-12);
        assert!((v.to_world(250.0, 200.0) - Cx::new(1.0, 0.0)).abs() < 1e-12);
        // And up on screen is up in the world, which is the whole job of View.
        assert!((v.to_world(200.0, 150.0) - Cx::new(0.0, 1.0)).abs() < 1e-12);
    }

    /// ★ Zooming keeps whatever is under the pointer under the pointer.
    ///
    /// The naive version multiplies the scale and zooms about the ORIGIN, so
    /// the thing you are closing in on slides away from you — the single most
    /// annoying possible behaviour in a graph viewer.
    #[test]
    fn zooming_keeps_the_point_under_the_pointer() {
        let mut v = View::centred(800, 600, 50.0);
        let spot = (610.0, 190.0); // somewhere off-centre, on purpose
        let before = v.to_world(spot.0, spot.1);

        for f in [1.25, 1.25, 0.8, 4.0, 0.1] {
            zoom_about(&mut v, spot, f);
            let after = v.to_world(spot.0, spot.1);
            assert!((after - before).abs() < 1e-9, "the world slid: {before:?} -> {after:?}");
        }
    }

    #[test]
    fn zooming_really_changes_the_scale() {
        let mut v = View::centred(400, 400, 50.0);
        zoom_about(&mut v, (200.0, 200.0), 2.0);
        assert!((v.scale - 100.0).abs() < 1e-9);
        zoom_about(&mut v, (200.0, 200.0), 0.5);
        assert!((v.scale - 50.0).abs() < 1e-9);
    }

    /// Zoom cannot be driven to zero or to infinity, either of which would
    /// leave a view that can never be recovered by zooming back.
    #[test]
    fn zoom_stays_within_reach() {
        let mut v = View::centred(400, 400, 50.0);
        for _ in 0..400 {
            zoom_about(&mut v, (200.0, 200.0), 0.5);
        }
        assert!(v.scale > 0.0 && v.scale.is_finite(), "scale was {}", v.scale);
        for _ in 0..400 {
            zoom_about(&mut v, (200.0, 200.0), 2.0);
        }
        assert!(v.scale.is_finite(), "scale was {}", v.scale);
    }

    /// Panning by the pointer's own delta makes the paper follow the hand
    /// exactly — no scale factor, because the origin is already in pixels.
    #[test]
    fn panning_moves_the_world_by_exactly_the_hand() {
        let mut v = View::centred(400, 400, 37.0);
        let before = v.to_world(100.0, 100.0);
        v.origin.0 += 25.0;
        v.origin.1 += -13.0;
        // The same WORLD point is now 25 right and 13 up the screen.
        let (x, y) = v.to_screen(before);
        assert!((x - 125).abs() <= 1 && (y - 87).abs() <= 1, "landed at {x}, {y}");
    }

    #[test]
    fn keys_built_by_hand_behave_like_real_ones() {
        assert!(Keys::pressing("n").just('n') && Keys::pressing("n").held('n'));
        assert!(Keys::holding("n").held('n') && !Keys::holding("n").just('n'));
    }
}
