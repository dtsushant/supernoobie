//! # Plotting — curves, graph paper, and implicit equations
//!
//! Everything here takes **world** coordinates and a [`View`]. Nothing takes
//! pixels.
//!
//! ## Three ways to draw a curve, and when each is right
//!
//! | | form | use it when |
//! |---|---|---|
//! | [`graph`] | `y = f(x)` | you have an explicit function of x |
//! | [`param`] | `t -> z(t)` | **the default.** One point per sample, exact, evenly spaced |
//! | [`implicit`] | `F(x, y) = c` | there is no parameterisation, or you do not know one |
//!
//! A circle makes the difference concrete. `x^2 + y^2 = r^2` tells you whether
//! a point is **on** the curve; it does not tell you **where** the curve is. To
//! draw from that form you either solve for `y` (two branches, and the tangent
//! goes vertical exactly at the sides, so the sampling falls apart there) or
//! march a grid ([`contour`]) — general, but blocky and slow.
//!
//! Parameterise instead and it is `z = r e^(i t)`: one multiplication per
//! point, exactly on the curve, perfectly even, no branches. Every
//! awkwardness of the implicit form comes from having thrown the parameter
//! away.

use crate::complex::Cx;
use crate::raster::Canvas;
use crate::view::View;

/// The world rectangle currently visible, as `(min, max)`.
pub fn bounds(v: &View) -> (Cx, Cx) {
    (v.to_world(0.0, v.h), v.to_world(v.w, 0.0))
}

/// `y = f(x)`, sampled once per pixel of width.
///
/// Non-finite values lift the pen rather than drawing a line across the gap,
/// so `tan` gets asymptotes instead of vertical streaks.
pub fn graph(c: &mut Canvas, v: &View, f: impl Fn(f64) -> f64, col: u32) {
    let (lo, hi) = bounds(v);
    let steps = (v.w as usize).max(2);
    let mut prev: Option<Cx> = None;
    for k in 0..=steps {
        let x = lo.re + (hi.re - lo.re) * k as f64 / steps as f64;
        let y = f(x);
        if !y.is_finite() {
            prev = None;
            continue;
        }
        let p = Cx::new(x, y);
        if let Some(q) = prev {
            v.line(c, q, p, 2, col);
        }
        prev = Some(p);
    }
}

/// A parametric curve `t -> z(t)` over `[t0, t1]`. The form to reach for.
pub fn param(c: &mut Canvas, v: &View, f: impl Fn(f64) -> Cx, t0: f64, t1: f64, n: usize, col: u32) {
    let mut prev: Option<Cx> = None;
    for k in 0..=n.max(1) {
        let t = t0 + (t1 - t0) * k as f64 / n.max(1) as f64;
        let p = f(t);
        if !p.re.is_finite() || !p.im.is_finite() {
            prev = None;
            continue;
        }
        if let Some(q) = prev {
            v.line(c, q, p, 2, col);
        }
        prev = Some(p);
    }
}

/// An open path through world points.
pub fn polyline(c: &mut Canvas, v: &View, pts: &[Cx], col: u32) {
    for w in pts.windows(2) {
        v.line(c, w[0], w[1], 2, col);
    }
}

/// A closed path. **Two points make a straight line**, which is why
/// `polygon(a, b)` does the obvious thing.
pub fn polygon(c: &mut Canvas, v: &View, pts: &[Cx], col: u32) {
    polyline(c, v, pts, col);
    if pts.len() > 2 {
        v.line(c, pts[pts.len() - 1], pts[0], 2, col);
    }
}

/// Vertices of a regular n-gon: `centre + r e^(i(2 pi k/n + phase))` — the
/// roots of unity, scaled and moved.
pub fn ngon(centre: Cx, r: f64, n: usize, phase: f64) -> Vec<Cx> {
    (0..n.max(3))
        .map(|k| centre + Cx::expi(phase + std::f64::consts::TAU * k as f64 / n.max(3) as f64).scale(r))
        .collect()
}

/// Points on a circle — the same thing, with enough sides to look round.
pub fn circle_pts(centre: Cx, r: f64, n: usize) -> Vec<Cx> {
    ngon(centre, r, n.max(3), 0.0)
}

// ---- implicit curves ------------------------------------------------------

/// Line segments approximating `F(x, y) = level`, by **marching squares**.
///
/// Sample `F` on a grid. On each little square, look at the sign at the four
/// corners: where two adjacent corners disagree the curve crosses that edge,
/// and linear interpolation says where. Join the crossings up.
///
/// A square with four crossings is a **saddle** and genuinely ambiguous —
/// either pairing is a defensible answer, and this takes the usual one.
pub fn contour(
    f: impl Fn(f64, f64) -> f64,
    level: f64,
    lo: Cx,
    hi: Cx,
    res: usize,
) -> Vec<(Cx, Cx)> {
    let res = res.max(2);
    let mut out = Vec::new();
    let (dx, dy) = ((hi.re - lo.re) / res as f64, (hi.im - lo.im) / res as f64);
    let at = |i: usize, j: usize| f(lo.re + i as f64 * dx, lo.im + j as f64 * dy) - level;

    for j in 0..res {
        for i in 0..res {
            let (x0, y0) = (lo.re + i as f64 * dx, lo.im + j as f64 * dy);
            let (x1, y1) = (x0 + dx, y0 + dy);
            // corners, anticlockwise from bottom-left
            let v = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
            if v.iter().any(|x| !x.is_finite()) {
                continue;
            }
            let edges = [
                (Cx::new(x0, y0), Cx::new(x1, y0)),
                (Cx::new(x1, y0), Cx::new(x1, y1)),
                (Cx::new(x1, y1), Cx::new(x0, y1)),
                (Cx::new(x0, y1), Cx::new(x0, y0)),
            ];
            let mut hits: Vec<Cx> = Vec::new();
            for e in 0..4 {
                let (a, b) = (v[e], v[(e + 1) % 4]);
                if (a > 0.0) != (b > 0.0) {
                    let t = a / (a - b);
                    let (p, q) = edges[e];
                    hits.push(p + (q - p).scale(t));
                }
            }
            match hits.len() {
                2 => out.push((hits[0], hits[1])),
                4 => {
                    out.push((hits[0], hits[1]));
                    out.push((hits[2], hits[3]));
                }
                _ => {}
            }
        }
    }
    out
}

/// Draw `F(x, y) = level` over the visible area.
pub fn implicit(c: &mut Canvas, v: &View, f: impl Fn(f64, f64) -> f64, level: f64, res: usize, col: u32) {
    let (lo, hi) = bounds(v);
    for (a, b) in contour(f, level, lo, hi, res) {
        v.line(c, a, b, 2, col);
    }
}

// ---- graph paper ----------------------------------------------------------

/// A "nice" axis step — 1, 2, 5, 10, 20, 50 … — landing roughly `target_px`
/// apart at the view's current scale.
///
/// Split the raw spacing into a power of ten and a leading digit, then round
/// that digit to 1, 2 or 5. It is the standard trick, and it is why an axis
/// never reads 0, 0.37, 0.74.
pub fn nice_step(scale: f64, target_px: f64) -> f64 {
    let raw = (target_px / scale.max(1e-9)).max(1e-300);
    let mag = 10f64.powf(raw.log10().floor());
    let lead = raw / mag;
    let m = if lead < 1.5 {
        1.0
    } else if lead < 3.5 {
        2.0
    } else if lead < 7.5 {
        5.0
    } else {
        10.0
    };
    m * mag
}

pub struct GridStyle {
    pub minor: u32,
    pub major: u32,
    pub axis: u32,
    pub label: u32,
    pub labels: bool,
    /// Roughly how far apart the major lines should be, in pixels.
    pub target_px: f64,
}

impl Default for GridStyle {
    fn default() -> Self {
        GridStyle {
            minor: 0x141D26,
            major: 0x1F2C38,
            axis: 0x4A5B6B,
            label: 0x6B7987,
            labels: true,
            target_px: 78.0,
        }
    }
}

/// Graph paper: minor lines, major lines, the two axes, and tick labels.
pub fn grid(c: &mut Canvas, v: &View, s: &GridStyle) {
    let (lo, hi) = bounds(v);
    let step = nice_step(v.scale, s.target_px);
    let minor = step / 5.0;

    let mut x = (lo.re / minor).floor() * minor;
    while x < hi.re {
        v.line(c, Cx::new(x, lo.im), Cx::new(x, hi.im), 1, s.minor);
        x += minor;
    }
    let mut y = (lo.im / minor).floor() * minor;
    while y < hi.im {
        v.line(c, Cx::new(lo.re, y), Cx::new(hi.re, y), 1, s.minor);
        y += minor;
    }
    let mut x = (lo.re / step).floor() * step;
    while x < hi.re {
        v.line(c, Cx::new(x, lo.im), Cx::new(x, hi.im), 1, s.major);
        x += step;
    }
    let mut y = (lo.im / step).floor() * step;
    while y < hi.im {
        v.line(c, Cx::new(lo.re, y), Cx::new(hi.re, y), 1, s.major);
        y += step;
    }

    v.line(c, Cx::new(lo.re, 0.0), Cx::new(hi.re, 0.0), 2, s.axis);
    v.line(c, Cx::new(0.0, lo.im), Cx::new(0.0, hi.im), 2, s.axis);

    if !s.labels {
        return;
    }
    let fmt = |t: f64| if step >= 1.0 { format!("{t:.0}") } else { format!("{t:.2}") };
    let mut x = (lo.re / step).floor() * step;
    while x < hi.re {
        if x.abs() > step * 0.25 {
            let (sx, sy) = v.to_screen(Cx::new(x, 0.0));
            c.text(sx - 8, sy + 7, &fmt(x), s.label, 1);
        }
        x += step;
    }
    let mut y = (lo.im / step).floor() * step;
    while y < hi.im {
        if y.abs() > step * 0.25 {
            let (sx, sy) = v.to_screen(Cx::new(0.0, y));
            c.text(sx + 7, sy - 3, &fmt(y), s.label, 1);
        }
        y += step;
    }
    let (ox, oy) = v.to_screen(Cx::ZERO);
    c.text(ox + 5, oy + 7, "0", s.label, 1);
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    /// Marching squares must land on the curve: every endpoint should satisfy
    /// the equation, to within the sampling error.
    #[test]
    fn the_contour_lands_on_the_curve() {
        let r: f64 = 1.3;
        let segs = contour(|x, y| x * x + y * y, r * r, Cx::new(-2.0, -2.0), Cx::new(2.0, 2.0), 140);
        assert!(segs.len() > 100, "only {} segments", segs.len());
        for (a, b) in &segs {
            for q in [a, b] {
                assert!((q.abs() - r).abs() < 0.02, "radius {} should be {r}", q.abs());
            }
        }
    }

    /// ...and it must agree with the parametric form, which is the honest way
    /// to check both at once.
    #[test]
    fn implicit_and_parametric_circles_agree() {
        let r: f64 = 0.9;
        let segs = contour(|x, y| x * x + y * y, r * r, Cx::new(-1.5, -1.5), Cx::new(1.5, 1.5), 160);
        for (a, _) in &segs {
            let exact = Cx::expi(a.arg()).scale(r);
            assert!((*a - exact).abs() < 0.02, "{a} vs {exact}");
        }
        for q in circle_pts(Cx::ZERO, r, 64) {
            assert!(close(q.abs(), r));
        }
    }

    /// A closed curve should have no loose ends — every endpoint shared.
    #[test]
    fn a_closed_contour_has_no_loose_ends() {
        let segs = contour(|x, y| x * x + y * y, 1.0, Cx::new(-2.0, -2.0), Cx::new(2.0, 2.0), 60);
        for (a, _) in &segs {
            let touching = segs
                .iter()
                .filter(|(p, q)| (*p - *a).abs() < 1e-9 || (*q - *a).abs() < 1e-9)
                .count();
            assert!(touching >= 2, "loose endpoint at {a}");
        }
    }

    #[test]
    fn no_contour_where_the_curve_is_not() {
        let segs = contour(|x, y| x * x + y * y, 100.0, Cx::new(-1.0, -1.0), Cx::new(1.0, 1.0), 40);
        assert!(segs.is_empty(), "{} phantom segments", segs.len());
    }

    /// Axis steps must be 1, 2 or 5 times a power of ten — never 0.37.
    #[test]
    fn the_grid_step_is_always_a_nice_number() {
        for scale in [3.0, 12.0, 55.0, 90.0, 260.0, 1400.0] {
            let s = nice_step(scale, 78.0);
            let mag = 10f64.powf(s.log10().floor());
            let lead = s / mag;
            assert!(
                [1.0, 2.0, 5.0, 10.0].iter().any(|m| close(lead, *m)),
                "scale {scale} gave {s}"
            );
            assert!((25.0..260.0).contains(&(s * scale)), "scale {scale}: {} px", s * scale);
        }
    }

    /// An n-gon is the roots of unity: same radius, even spacing.
    #[test]
    fn ngon_vertices_are_evenly_spaced_on_a_circle() {
        let c = Cx::new(2.0, -1.0);
        let g = ngon(c, 1.7, 7, 0.4);
        assert_eq!(g.len(), 7);
        for q in &g {
            assert!(close((*q - c).abs(), 1.7));
        }
        let step = (g[1] - c).arg() - (g[0] - c).arg();
        assert!(close(step, std::f64::consts::TAU / 7.0));
    }

    /// Degenerate input must not panic or hang.
    #[test]
    fn degenerate_input_is_survivable() {
        let v = View::centred(64, 64, 10.0);
        let mut c = Canvas::new(64, 64);
        graph(&mut c, &v, |_| f64::NAN, 0xFFFFFF);
        graph(&mut c, &v, |x| 1.0 / x, 0xFFFFFF); // a pole at 0
        param(&mut c, &v, |_| Cx::ZERO, 0.0, 0.0, 0, 0xFFFFFF);
        polygon(&mut c, &v, &[], 0xFFFFFF);
        polygon(&mut c, &v, &[Cx::ZERO], 0xFFFFFF);
        implicit(&mut c, &v, |x, y| x + y, 0.0, 2, 0xFFFFFF);
        assert_eq!(ngon(Cx::ZERO, 1.0, 0, 0.0).len(), 3, "n is clamped to a triangle");
    }
}
