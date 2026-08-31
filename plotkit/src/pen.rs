//! # Engineering-drawing annotations
//!
//! Dimension lines with end ticks, angle arcs, radius leaders, arrows. The
//! notation technical drawings have used for a century, because it was built
//! for exactly this job and reads instantly.
//!
//! Everything here is decoration over a drawing that would be identical
//! without it.

/// Engineering-drawing annotations. Everything here is decoration over a
/// simulation that would run identically without it.
use crate::complex::Cx;
use crate::raster::Canvas;
use crate::view::View;

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
