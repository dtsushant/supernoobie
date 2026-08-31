//! # World-to-screen mapping
//!
//! One small struct that stands between the mathematics and the pixels.
//! Everything above it talks in world units; everything below it is a
//! `Vec<u32>`.

use crate::complex::Cx;
use crate::raster::Canvas;

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
