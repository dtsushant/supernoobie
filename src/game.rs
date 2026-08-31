//! # The game shell — shared parts, and the mathematics overlay
//!
//! Four small games sit on top of the physics files. Everything they have in
//! common lives here: the input they read, the stage lifecycle they follow,
//! and — the point of the exercise — the **annotation pen** that draws the
//! mathematics on top of the picture.
//!
//! ## The overlay
//!
//! Every game is a physics file with a goal bolted on. The risk in dressing a
//! simulation up as a game is that the mathematics becomes invisible: a rope
//! stops being a chain of distance constraints and becomes a squiggle.
//!
//! So each stage annotates itself, and each *kind* of annotation is a separate
//! toggle:
//!
//! | key | shows |
//! |---|---|
//! | 1 | **lengths** — rope length, distances, dimension lines |
//! | 2 | **angles** — orientation, arcs, degrees |
//! | 3 | **radii** — circles marked with `r = ...` |
//! | 4 | **vectors** — velocity and force arrows |
//! | 5 | **contacts** — contact points and their normals |
//! | 6 | **readouts** — energy, momentum, counts |
//! | 7 | **grid** — the spatial hash cells |
//! | 8 | **formulas** — the equation each stage is actually solving |
//!
//! With all eight off you have a game. With all eight on you have a diagram
//! that happens to be playable. Neither view is the real one; they are the
//! same numbers drawn twice.
//!
//! The drawing style is deliberately that of an engineering drawing —
//! dimension lines with end ticks, angle arcs, leader lines — because that is
//! a notation built for exactly this job and it reads instantly.

use crate::complex::Cx;
use crate::raster::{colour, Canvas};

// ---------------------------------------------------------------------------
// input
// ---------------------------------------------------------------------------

/// Keyboard and mouse, in terms the library can express.
///
/// Deliberately *not* minifb's `Key` type: the binary translates. That keeps
/// this file — and the whole library — free of the windowing dependency.
#[derive(Clone, Copy, Default)]
pub struct Input {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    /// Space, held.
    pub action: bool,
    /// Space, on the frame it went down.
    pub action_pressed: bool,
    pub mouse: Option<Cx>,
    pub mouse_down: bool,
    /// Mouse button, on the frame it went down.
    pub mouse_pressed: bool,
    /// Left/right as -1, 0 or +1.
    pub axis_x: f64,
    pub axis_y: f64,
}

impl Input {
    pub fn resolve_axes(&mut self) {
        self.axis_x = (self.right as i32 - self.left as i32) as f64;
        self.axis_y = (self.up as i32 - self.down as i32) as f64;
    }
}

// ---------------------------------------------------------------------------
// stage lifecycle
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Playing,
    Won,
    Lost,
}

/// One playable stage. Four implement this; the shell knows nothing else.
pub trait Stage {
    fn name(&self) -> &'static str;
    /// One line, shown under the title — what you are trying to do.
    fn goal(&self) -> &'static str;
    /// The controls, shown in the corner.
    fn controls(&self) -> &'static str;
    /// The equation this stage is actually solving, shown by overlay 8.
    fn formula(&self) -> &'static [&'static str];
    fn reset(&mut self);
    fn update(&mut self, dt: f64, input: &Input) -> Status;
    fn draw(&self, c: &mut Canvas, v: &View, ov: Overlay);
}

// ---------------------------------------------------------------------------
// view
// ---------------------------------------------------------------------------

pub use plotkit::pen;
pub use plotkit::view::View;

// ---------------------------------------------------------------------------
// the overlay
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Default)]
pub struct Overlay {
    pub bits: u16,
}

impl Overlay {
    pub const LENGTHS: u16 = 1 << 0;
    pub const ANGLES: u16 = 1 << 1;
    pub const RADII: u16 = 1 << 2;
    pub const VECTORS: u16 = 1 << 3;
    pub const CONTACTS: u16 = 1 << 4;
    pub const READOUTS: u16 = 1 << 5;
    pub const GRID: u16 = 1 << 6;
    pub const FORMULAS: u16 = 1 << 7;

    pub const ALL: [(u16, &'static str); 8] = [
        (Self::LENGTHS, "LENGTHS"),
        (Self::ANGLES, "ANGLES"),
        (Self::RADII, "RADII"),
        (Self::VECTORS, "VECTORS"),
        (Self::CONTACTS, "CONTACTS"),
        (Self::READOUTS, "READOUTS"),
        (Self::GRID, "GRID"),
        (Self::FORMULAS, "FORMULAS"),
    ];

    pub fn on(self, f: u16) -> bool {
        self.bits & f != 0
    }
    pub fn toggle(&mut self, f: u16) {
        self.bits ^= f;
    }
    pub fn all_on() -> Overlay {
        Overlay { bits: 0xFF }
    }
    pub fn none() -> Overlay {
        Overlay { bits: 0 }
    }
    pub fn count(self) -> u32 {
        self.bits.count_ones()
    }
}

// ---------------------------------------------------------------------------
// the annotation pen
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// chrome
// ---------------------------------------------------------------------------

/// Title, goal line, overlay strip and controls. Identical in every stage, so
/// the games differ only in the part that is actually the game.
pub fn chrome(
    c: &mut Canvas,
    v: &View,
    title: &str,
    goal: &str,
    controls: &str,
    ov: Overlay,
    status: Status,
) {
    c.text(28, 16, title, colour::INK, 3);
    c.text(28, 46, goal, colour::FAINT, 1);
    c.text(
        v.w as i32 - Canvas::text_w(controls, 1) - 28,
        20,
        controls,
        colour::FAINT,
        1,
    );

    // the overlay strip: which annotations are live
    let mut x = 28;
    let y = v.h as i32 - 26;
    c.text(x, y, "OVERLAY", colour::FAINT, 1);
    x += Canvas::text_w("OVERLAY  ", 1);
    for (k, (bit, name)) in Overlay::ALL.iter().enumerate() {
        let on = ov.on(*bit);
        let lbl = format!("{}:{}", k + 1, name);
        c.text(x, y, &lbl, if on { colour::IMAG } else { 0x2B3945 }, 1);
        x += Canvas::text_w(&lbl, 1) + 10;
    }

    match status {
        Status::Won => banner(c, v, "SOLVED", colour::GOOD),
        Status::Lost => banner(c, v, "FAILED  -  R TO RETRY", colour::WARN),
        Status::Playing => {}
    }
}

fn banner(c: &mut Canvas, v: &View, text: &str, col: u32) {
    let w = Canvas::text_w(text, 4);
    let x = (v.w as i32 - w) / 2;
    let y = v.h as i32 / 2 - 30;
    c.fill_rect(x - 24, y - 16, w + 48, 56, 0x0B1017);
    c.rect(x - 24, y - 16, w + 48, 56, col);
    c.text(x, y, text, col, 4);
}

/// A labelled numeric readout, stacked from the bottom-left.
pub struct Readout {
    pub x: i32,
    pub y: i32,
}

impl Readout {
    pub fn new(v: &View) -> Self {
        Readout { x: 28, y: v.h as i32 - 76 }
    }
    pub fn row(&mut self, c: &mut Canvas, s: &str, col: u32) {
        c.text(self.x, self.y, s, col, 2);
        self.y -= 22;
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_view_flips_y_and_round_trips() {
        let v = View::new(800, 600);
        let p = Cx::new(120.0, 90.0);
        let (x, y) = v.to_screen(p);
        assert_eq!((x, y), (120, 510));
        let back = v.to_world(x as f64, y as f64);
        assert!((back - p).abs() < 1e-9);
    }

    #[test]
    fn overlay_bits_toggle_independently() {
        let mut o = Overlay::none();
        assert_eq!(o.count(), 0);
        o.toggle(Overlay::LENGTHS);
        o.toggle(Overlay::RADII);
        assert!(o.on(Overlay::LENGTHS) && o.on(Overlay::RADII));
        assert!(!o.on(Overlay::ANGLES));
        assert_eq!(o.count(), 2);
        o.toggle(Overlay::LENGTHS);
        assert!(!o.on(Overlay::LENGTHS));
        assert_eq!(Overlay::all_on().count(), 8);
    }

    /// Every flag in the strip must be distinct, or two toggles fight.
    #[test]
    fn every_overlay_flag_is_a_distinct_bit() {
        let mut seen = 0u16;
        for (bit, _) in Overlay::ALL {
            assert_eq!(bit.count_ones(), 1, "flags must be single bits");
            assert_eq!(seen & bit, 0, "duplicate flag");
            seen |= bit;
        }
        assert_eq!(seen.count_ones(), 8);
    }

    #[test]
    fn input_axes_resolve_to_minus_one_zero_or_one() {
        let mut i = Input { left: true, ..Input::default() };
        i.resolve_axes();
        assert_eq!(i.axis_x, -1.0);
        i.right = true;
        i.resolve_axes();
        assert_eq!(i.axis_x, 0.0, "both held should cancel");
        i.left = false;
        i.resolve_axes();
        assert_eq!(i.axis_x, 1.0);
    }

    /// The annotations must not panic on degenerate input - a zero-length
    /// dimension, an arrow of no length, a zero radius.
    #[test]
    fn annotations_survive_degenerate_geometry() {
        let v = View::new(200, 200);
        let mut c = Canvas::new(200, 200);
        let p = Cx::new(50.0, 50.0);
        pen::arrow(&mut c, &v, p, p, pen::VEC, Some("v"));
        pen::dimension(&mut c, &v, p, p, 10.0, "0");
        pen::angle_arc(&mut c, &v, p, 0.0, 0.0, 0.0, "0");
        pen::radius(&mut c, &v, p, 0.0, 0.0, "r=0");
        pen::crosshair(&mut c, &v, p, 0.0, pen::HIT);
    }

    /// Drawing off the edge of the canvas must be clipped, not wrapped.
    #[test]
    fn annotations_off_screen_do_not_wrap() {
        let v = View::new(64, 64);
        let mut c = Canvas::new(64, 64);
        c.clear(0);
        pen::arrow(&mut c, &v, Cx::new(-500.0, 32.0), Cx::new(-400.0, 32.0), pen::VEC, None);
        // everything was off to the left, so nothing should have been drawn
        assert!(c.buf.iter().all(|&p| p == 0), "something leaked on screen");
    }
}
