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

/// World to screen.
///
/// ```text
/// screen_x = origin_x + scale * world_x
/// screen_y = origin_y - scale * world_y        <- the flip
/// ```
///
/// The subtraction on `y` is the only surprising part, and it is there because
/// screen rows count **downward** while world `y` counts **up**. Every 2-D
/// demo in this crate makes that flip; this is the one place it lives.
///
/// `origin` and `scale` exist so the same code can draw two very different
/// things:
///
/// * [`View::new`] puts the origin at the **bottom-left** with one world unit
///   per pixel — what the games want, since they think in pixels already.
/// * [`View::centred`] puts the origin in the **middle** and lets you choose
///   how many pixels a unit is worth — what mathematics wants, since a unit
///   circle drawn at one pixel per unit is one pixel across.
#[derive(Debug, Clone, Copy)]
pub struct View {
    pub w: f64,
    pub h: f64,
    /// Where world `(0, 0)` lands, in screen pixels.
    pub origin: (f64, f64),
    /// Pixels per world unit.
    pub scale: f64,
}

impl View {
    /// Origin bottom-left, one pixel per unit. What the game stages use.
    pub fn new(w: usize, h: usize) -> Self {
        View { w: w as f64, h: h as f64, origin: (0.0, h as f64), scale: 1.0 }
    }

    /// Origin in the middle of the window, `scale` pixels per world unit.
    /// For drawing mathematics rather than pixels.
    pub fn centred(w: usize, h: usize, scale: f64) -> Self {
        View {
            w: w as f64,
            h: h as f64,
            origin: (w as f64 * 0.5, h as f64 * 0.5),
            scale,
        }
    }

    /// Move the origin without changing the scale.
    pub fn with_origin(mut self, x: f64, y: f64) -> Self {
        self.origin = (x, y);
        self
    }

    pub fn to_screen(&self, p: Cx) -> (i32, i32) {
        (
            (self.origin.0 + p.re * self.scale).round() as i32,
            (self.origin.1 - p.im * self.scale).round() as i32,
        )
    }
    /// Screen back to world — the exact inverse, so a mouse position can be
    /// compared against world coordinates.
    pub fn to_world(&self, x: f64, y: f64) -> Cx {
        Cx::new(
            (x - self.origin.0) / self.scale,
            (self.origin.1 - y) / self.scale,
        )
    }
    /// A world length in pixels.
    pub fn px(&self, world_len: f64) -> f64 {
        world_len * self.scale
    }

    pub fn line(&self, c: &mut Canvas, a: Cx, b: Cx, t: i32, col: u32) {
        let (p, q) = (self.to_screen(a), self.to_screen(b));
        c.thick_line(p.0, p.1, q.0, q.1, t, col);
    }
    /// `r` is a **world** radius; it is scaled like everything else.
    pub fn ring(&self, c: &mut Canvas, at: Cx, r: f64, t: i32, col: u32) {
        let p = self.to_screen(at);
        c.ring(p.0, p.1, self.px(r) as i32, t, col);
    }
    pub fn disc(&self, c: &mut Canvas, at: Cx, r: f64, col: u32) {
        let p = self.to_screen(at);
        c.disc(p.0, p.1, self.px(r).max(1.0) as i32, col);
    }
    pub fn text(&self, c: &mut Canvas, at: Cx, s: &str, col: u32, scale: i32) {
        let p = self.to_screen(at);
        c.text(p.0, p.1, s, col, scale);
    }
    /// Text centred horizontally on a world point.
    pub fn text_mid(&self, c: &mut Canvas, at: Cx, s: &str, col: u32, scale: i32) {
        let p = self.to_screen(at);
        c.text(p.0 - Canvas::text_w(s, scale) / 2, p.1, s, col, scale);
    }
}

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

/// Engineering-drawing annotations. Everything here is decoration over a
/// simulation that would run identically without it.
pub mod pen {
    use super::*;

    pub const DIM: u32 = 0xE0A44A; // dimensions, lengths
    pub const ANG: u32 = 0xE585AC; // angles
    pub const RAD: u32 = 0x4FBCD4; // radii
    pub const VEC: u32 = 0x6FCF97; // vectors
    pub const HIT: u32 = 0xE0704A; // contacts

    /// A fixed size in **pixels**, expressed in world units.
    ///
    /// Everything decorative here — arrowheads, tick marks, the dot on a
    /// radius — should be a constant size on screen no matter how far the view
    /// is zoomed in. But the pen is handed world coordinates, so those
    /// constants have to be divided by the scale on the way in.
    ///
    /// Skipping this is a real and confusing bug: at `scale = 1` (what the
    /// game stages use) pixels and world units coincide and everything looks
    /// right, and then at `scale = 90` a "2 pixel" dot arrives as a filled
    /// disc 180 pixels across, sitting on top of the drawing.
    #[inline]
    fn px(v: &View, pixels: f64) -> f64 {
        pixels / v.scale.max(1e-9)
    }

    /// A line with an arrowhead. Vectors are drawn scaled, since a velocity in
    /// world units is rarely a sensible number of pixels.
    pub fn arrow(c: &mut Canvas, v: &View, from: Cx, to: Cx, col: u32, label: Option<&str>) {
        let d = to - from;
        if d.abs() < 1.0 {
            return;
        }
        v.line(c, from, to, 2, col);
        let u = d.unit();
        let head = px(v, 9.0);
        // the two barbs are the direction rotated by +/- 155 degrees - a
        // rotation, so a complex multiplication
        for s in [2.7, -2.7] {
            let barb = to + (u * Cx::expi(s)).scale(head);
            v.line(c, to, barb, 2, col);
        }
        if let Some(t) = label {
            v.text(c, to + u.scale(px(v, 12.0)), t, col, 1);
        }
    }

    /// A dimension line: the measured span, offset to one side, with end ticks
    /// and the value in the middle. Straight out of a technical drawing.
    pub fn dimension(c: &mut Canvas, v: &View, a: Cx, b: Cx, offset: f64, label: &str) {
        let d = b - a;
        if d.abs() < 1e-6 {
            return;
        }
        // offset perpendicular to the span: multiply by i
        let n = (Cx::I * d.unit()).scale(offset);
        let (a2, b2) = (a + n, b + n);
        v.line(c, a2, b2, 1, DIM);
        // witness lines back to what is being measured
        v.line(c, a, a2, 1, DIM);
        v.line(c, b, b2, 1, DIM);
        // end ticks, at 45 degrees like a drafting slash
        let t = (d.unit() * Cx::expi(0.785)).scale(px(v, 6.0));
        v.line(c, a2 - t, a2 + t, 1, DIM);
        v.line(c, b2 - t, b2 + t, 1, DIM);
        let mid = (a2 + b2).scale(0.5) + (Cx::I * d.unit()).scale(px(v, 8.0));
        v.text_mid(c, mid, label, DIM, 1);
    }

    /// An arc from `a0` to `a1` about `centre`, with the angle in degrees.
    pub fn angle_arc(c: &mut Canvas, v: &View, centre: Cx, r: f64, a0: f64, a1: f64, label: &str) {
        let n = 28;
        let mut prev = centre + Cx::expi(a0).scale(r);
        for k in 1..=n {
            let t = a0 + (a1 - a0) * k as f64 / n as f64;
            let p = centre + Cx::expi(t).scale(r);
            v.line(c, prev, p, 1, ANG);
            prev = p;
        }
        // the two bounding rays, dashed-ish
        v.line(c, centre, centre + Cx::expi(a0).scale(r * 1.25), 1, ANG);
        v.line(c, centre, centre + Cx::expi(a1).scale(r * 1.25), 1, ANG);
        let mid = centre + Cx::expi((a0 + a1) * 0.5).scale(r + px(v, 16.0));
        v.text_mid(c, mid, label, ANG, 1);
    }

    /// A radius line from the centre to the rim, labelled.
    pub fn radius(c: &mut Canvas, v: &View, centre: Cx, r: f64, at_angle: f64, label: &str) {
        let rim = centre + Cx::expi(at_angle).scale(r);
        v.line(c, centre, rim, 1, RAD);
        v.disc(c, centre, px(v, 2.0), RAD);
        let mid = centre + Cx::expi(at_angle).scale(r * 0.55);
        v.text_mid(c, mid + Cx::new(0.0, px(v, 9.0)), label, RAD, 1);
    }

    /// A short leader line out to a label, for tagging a moving thing.
    pub fn tag(c: &mut Canvas, v: &View, at: Cx, away: Cx, label: &str, col: u32) {
        let end = at + away;
        v.line(c, at, end, 1, col);
        let (x, y) = v.to_screen(end);
        let dx = if away.re < 0.0 { -Canvas::text_w(label, 1) - 4 } else { 4 };
        c.text(x + dx, y - 3, label, col, 1);
    }

    /// A cross-hair marking a target position.
    pub fn crosshair(c: &mut Canvas, v: &View, at: Cx, r: f64, col: u32) {
        v.line(c, at - Cx::new(r, 0.0), at + Cx::new(r, 0.0), 1, col);
        v.line(c, at - Cx::new(0.0, r), at + Cx::new(0.0, r), 1, col);
        v.ring(c, at, r * 0.55, 1, col);
    }
}

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
