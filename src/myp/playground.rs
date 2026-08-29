//! # Playground — a scrollable graph
//!
//! A sandbox for plotting. Pan, zoom, and draw whatever you like in **world**
//! coordinates; the [`View`] turns those into pixels for you.
//!
//! ---
//!
//! ## The three layers, and which one you should be talking to
//!
//! ```text
//!   Canvas    raw pixels.  origin TOP-LEFT, y counts DOWN, units = pixels
//!     ^                    c.px / c.line / c.ring / c.text
//!     |
//!   View      world -> screen.  origin wherever you put it, y counts UP
//!     ^                    v.line / v.ring / v.disc / v.text
//!     |
//!   you       maths.  "a circle of radius 1 at the origin"
//! ```
//!
//! **Talk to the `View`, not the `Canvas`.** `c.line(8, 8, 7, 7, 9)` draws a
//! one-pixel line at the top-left corner in a colour that is almost black —
//! which is exactly what it looks like when nothing appears on screen.
//! `v.line(c, a, b, 2, WHITE)` takes two world points and does the conversion.
//!
//! The mapping itself is three multiplications:
//!
//! ```text
//! screen_x = origin_x + scale * world_x
//! screen_y = origin_y - scale * world_y      <- the minus IS the y flip
//! ```
//!
//! `View::new` (what the games use) puts the origin bottom-left at one pixel
//! per unit, so a unit circle would be **one pixel across**. For mathematics
//! you want the origin in the middle and a scale of a hundred-odd pixels per
//! unit — which is what this file builds every frame from its pan and zoom.
//!
//! ## Overlay
//!
//! `Overlay` draws nothing at all. It is a `u16` of toggle bits, and every
//! stage decides for itself what each bit means. Here:
//!
//! | key | |
//! |---|---|
//! | 1 LENGTHS | the sin and cos legs, as dimension lines |
//! | 2 ANGLES | the angle arc on the unit circle |
//! | 3 RADII | the radius line, marked |
//! | 4 VECTORS | the rotating radius as an arrow |
//! | 6 READOUTS | cursor position and the current numbers |
//! | 7 GRID | graph paper and tick labels |
//!
//! ## What is drawn, and where to add your own
//!
//! Everything is in [`Playground::draw`]. The helpers below take **world**
//! coordinates:
//!
//! * [`Playground::plot`] — `y = f(x)` across whatever is on screen
//! * [`Playground::param`] — a parametric curve `t -> (x, y)`
//! * [`Playground::polyline`] — an open path through points
//! * [`Playground::polygon`] — the same, closed

use crate::complex::Cx;
use crate::game::{pen, Input, Overlay, Readout, Stage, Status, View};
use crate::raster::{colour, Canvas};

pub struct Playground {
    title: String,
    /// World point sitting at the centre of the window.
    pan: Cx,
    /// Pixels per world unit.
    zoom: f64,
    /// The angle of the point running round the unit circle.
    theta: f64,
    running: bool,
    mouse: Option<Cx>,
}

impl Playground {
    pub fn new(title: String) -> Self {
        Playground { title, pan: Cx::new(3.0, 0.0), zoom: 90.0, theta: 0.0, running: true, mouse: None }
    }

    /// Build the graph's own view from the pan and zoom.
    ///
    /// The stage is handed the *game's* view — origin bottom-left, one pixel
    /// per unit. That is the wrong frame for plotting, so we make our own:
    /// put world `pan` at the centre of the window and scale by `zoom`.
    fn graph(&self, v: &View) -> View {
        View {
            w: v.w,
            h: v.h,
            origin: (
                v.w * 0.5 - self.pan.re * self.zoom,
                v.h * 0.5 + self.pan.im * self.zoom,
            ),
            scale: self.zoom,
        }
    }

    /// The world rectangle currently on screen, as `(min, max)`.
    fn bounds(&self, g: &View) -> (Cx, Cx) {
        let a = g.to_world(0.0, g.h);
        let b = g.to_world(g.w, 0.0);
        (a, b)
    }

    /// A "nice" grid step — 1, 2, 5, 10, 20, 50 ... — chosen so the lines land
    /// roughly `target` pixels apart at the current zoom.
    ///
    /// Take the raw spacing, split it into a power of ten and a leading digit,
    /// then round that digit to one of 1, 2, 5. It is the standard trick, and
    /// it is why an axis never reads 0, 0.37, 0.74.
    fn nice_step(&self, target_px: f64) -> f64 {
        let raw = target_px / self.zoom;
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

    // ---- plotting helpers, all in WORLD coordinates ---------------------

    /// `y = f(x)`, sampled once per pixel across the visible width.
    pub fn plot(&self, c: &mut Canvas, g: &View, f: impl Fn(f64) -> f64, col: u32) {
        let (lo, hi) = self.bounds(g);
        let mut prev: Option<Cx> = None;
        let steps = g.w as usize;
        for k in 0..=steps {
            let x = lo.re + (hi.re - lo.re) * k as f64 / steps as f64;
            let y = f(x);
            if !y.is_finite() {
                prev = None; // a pole: lift the pen rather than draw across it
                continue;
            }
            let p = Cx::new(x, y);
            if let Some(q) = prev {
                g.line(c, q, p, 2, col);
            }
            prev = Some(p);
        }
    }

    /// A parametric curve `t -> (x, y)` for `t` in `[t0, t1]`.
    pub fn param(&self, c: &mut Canvas, g: &View, f: impl Fn(f64) -> Cx, t0: f64, t1: f64, n: usize, col: u32) {
        let mut prev: Option<Cx> = None;
        for k in 0..=n {
            let t = t0 + (t1 - t0) * k as f64 / n as f64;
            let p = f(t);
            if let Some(q) = prev {
                g.line(c, q, p, 2, col);
            }
            prev = Some(p);
        }
    }

    /// An open path through the given world points.
    pub fn polyline(&self, c: &mut Canvas, g: &View, pts: &[Cx], col: u32) {
        for w in pts.windows(2) {
            g.line(c, w[0], w[1], 2, col);
        }
    }

    /// The same, closed back to the start.
    pub fn polygon(&self, c: &mut Canvas, g: &View, pts: &[Cx], col: u32) {
        self.polyline(c, g, pts, col);
        if pts.len() > 2 {
            g.line(c, pts[pts.len() - 1], pts[0], 2, col);
        }
    }

    /// A regular n-gon — `centre + r e^(i(2 pi k/n + phase))`, the roots of
    /// unity again.
    pub fn ngon(&self, centre: Cx, r: f64, n: usize, phase: f64) -> Vec<Cx> {
        (0..n)
            .map(|k| centre + Cx::expi(phase + std::f64::consts::TAU * k as f64 / n as f64).scale(r))
            .collect()
    }

    // ---- implicit curves:  F(x, y) = level ------------------------------

    /// Line segments approximating `F(x, y) = level`, by **marching squares**.
    ///
    /// ## Why this is needed at all
    ///
    /// `x^2 + y^2 = r^2` is an *implicit* equation. It tells you whether a
    /// point is **on** the curve; it does not tell you **where** the curve is.
    /// To draw it you have three choices:
    ///
    /// 1. **Solve for y** — `y = +/- sqrt(r^2 - x^2)`. Two branches to stitch
    ///    together, and the tangent goes vertical at `x = +/- r`, so the
    ///    sampling falls apart exactly at the sides.
    /// 2. **March the grid** — this function. Completely general: it will draw
    ///    any `F(x, y) = c` you can evaluate, including curves nobody has a
    ///    parameterisation for. It is also the most expensive and the least
    ///    smooth.
    /// 3. **Parameterise** — `z = r e^(i t)`. One line, exact, and perfectly
    ///    even. See [`Playground::curve`].
    ///
    /// **Use 3 whenever you can.** Use this for the curves where you cannot.
    ///
    /// ## The algorithm
    ///
    /// Sample `F` on a grid. On each little square, look at the sign of `F` at
    /// the four corners: wherever two adjacent corners disagree, the curve
    /// crosses that edge, and linear interpolation says where. Join the
    /// crossings up and you have the contour. A square with four crossings is
    /// a saddle and genuinely ambiguous — either pairing is a valid answer.
    pub fn contour(
        &self,
        f: impl Fn(f64, f64) -> f64,
        level: f64,
        lo: Cx,
        hi: Cx,
        res: usize,
    ) -> Vec<(Cx, Cx)> {
        let mut out = Vec::new();
        let (dx, dy) = ((hi.re - lo.re) / res as f64, (hi.im - lo.im) / res as f64);
        // one row of samples at a time, so F is evaluated once per grid point
        let sample = |i: usize, j: usize| {
            f(lo.re + i as f64 * dx, lo.im + j as f64 * dy) - level
        };
        for j in 0..res {
            for i in 0..res {
                let (x0, y0) = (lo.re + i as f64 * dx, lo.im + j as f64 * dy);
                let (x1, y1) = (x0 + dx, y0 + dy);
                // corners, anticlockwise from bottom-left
                let v = [sample(i, j), sample(i + 1, j), sample(i + 1, j + 1), sample(i, j + 1)];
                let c = [
                    (Cx::new(x0, y0), Cx::new(x1, y0)),
                    (Cx::new(x1, y0), Cx::new(x1, y1)),
                    (Cx::new(x1, y1), Cx::new(x0, y1)),
                    (Cx::new(x0, y1), Cx::new(x0, y0)),
                ];
                // where does the curve cross each edge?
                let mut hits: Vec<Cx> = Vec::new();
                for e in 0..4 {
                    let (a, b) = (v[e], v[(e + 1) % 4]);
                    if (a > 0.0) != (b > 0.0) {
                        // linear interpolation to the zero crossing
                        let t = a / (a - b);
                        let (p, q) = c[e];
                        hits.push(p + (q - p).scale(t));
                    }
                }
                match hits.len() {
                    2 => out.push((hits[0], hits[1])),
                    // a saddle: two branches pass through. Either pairing is
                    // defensible; this one is the usual choice.
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

    /// Draw `F(x, y) = level` over whatever is currently on screen.
    pub fn implicit(
        &self,
        c: &mut Canvas,
        g: &View,
        f: impl Fn(f64, f64) -> f64,
        level: f64,
        res: usize,
        col: u32,
    ) {
        let (lo, hi) = self.bounds(g);
        for (a, b) in self.contour(f, level, lo, hi, res) {
            g.line(c, a, b, 2, col);
        }
    }

    // ---- the complex way ------------------------------------------------

    /// A curve given as a **complex function of a parameter**: `t -> z(t)`.
    ///
    /// This is the form to reach for. The circle `x^2 + y^2 = r^2` is really
    /// the set `|z| = r`, and the natural way to write that set down is
    ///
    /// ```text
    /// z(t) = r e^(i t),   t in [0, 2 pi)
    /// ```
    ///
    /// — one multiplication per point, exactly on the curve, evenly spaced,
    /// and it never has to be told about branches or vertical tangents. Every
    /// awkwardness of the implicit form comes from throwing the parameter away.
    pub fn curve(&self, c: &mut Canvas, g: &View, z: impl Fn(f64) -> Cx, t0: f64, t1: f64, n: usize, col: u32) {
        self.param(c, g, z, t0, t1, n, col)
    }

    /// The unit circle scaled and moved: `centre + r e^(i t)`.
    pub fn circle_pts(&self, centre: Cx, r: f64, n: usize) -> Vec<Cx> {
        (0..n)
            .map(|k| centre + Cx::expi(std::f64::consts::TAU * k as f64 / n as f64).scale(r))
            .collect()
    }

    // ---- axes and graph paper -------------------------------------------

    fn draw_grid(&self, c: &mut Canvas, g: &View, labels: bool) {
        let (lo, hi) = self.bounds(g);
        let step = self.nice_step(78.0);
        let minor = step / 5.0;

        // minor lines first, so the major ones draw over them
        let mut x = (lo.re / minor).floor() * minor;
        while x < hi.re {
            g.line(c, Cx::new(x, lo.im), Cx::new(x, hi.im), 1, 0x141D26);
            x += minor;
        }
        let mut y = (lo.im / minor).floor() * minor;
        while y < hi.im {
            g.line(c, Cx::new(lo.re, y), Cx::new(hi.re, y), 1, 0x141D26);
            y += minor;
        }

        let mut x = (lo.re / step).floor() * step;
        while x < hi.re {
            g.line(c, Cx::new(x, lo.im), Cx::new(x, hi.im), 1, 0x1F2C38);
            x += step;
        }
        let mut y = (lo.im / step).floor() * step;
        while y < hi.im {
            g.line(c, Cx::new(lo.re, y), Cx::new(hi.re, y), 1, 0x1F2C38);
            y += step;
        }

        // the axes themselves, and the origin
        g.line(c, Cx::new(lo.re, 0.0), Cx::new(hi.re, 0.0), 2, 0x4A5B6B);
        g.line(c, Cx::new(0.0, lo.im), Cx::new(0.0, hi.im), 2, 0x4A5B6B);
        g.disc(c, Cx::ZERO, 4.0 / self.zoom, colour::INK);
        g.text(c, Cx::new(0.0, 0.0), " 0", colour::FAINT, 1);

        if !labels {
            return;
        }
        let fmt = |t: f64| {
            if step >= 1.0 { format!("{t:.0}") } else { format!("{t:.2}") }
        };
        let mut x = (lo.re / step).floor() * step;
        while x < hi.re {
            if x.abs() > step * 0.25 {
                let (sx, sy) = g.to_screen(Cx::new(x, 0.0));
                c.text(sx - 8, sy + 7, &fmt(x), colour::FAINT, 1);
            }
            x += step;
        }
        let mut y = (lo.im / step).floor() * step;
        while y < hi.im {
            if y.abs() > step * 0.25 {
                let (sx, sy) = g.to_screen(Cx::new(0.0, y));
                c.text(sx + 7, sy - 3, &fmt(y), colour::FAINT, 1);
            }
            y += step;
        }
    }
}

impl Stage for Playground {
    fn name(&self) -> &'static str {
        "PLAYGROUND"
    }
    fn goal(&self) -> &'static str {
        "A SCROLLABLE GRAPH - PAN, ZOOM, AND PLOT WHATEVER YOU LIKE"
    }
    fn controls(&self) -> &'static str {
        "ARROWS PAN  W/S ZOOM  SPACE RUN/STOP  R RESET  ESC MENU"
    }
    fn formula(&self) -> &'static [&'static str] {
        &[
            "SCREEN_X = ORIGIN_X + SCALE * WORLD_X",
            "SCREEN_Y = ORIGIN_Y - SCALE * WORLD_Y        (THE Y FLIP)",
            "UNIT CIRCLE   (COS T, SIN T)   =   E^(I T)",
            "AND SIN IS THE SHADOW OF THAT POINT, DRAGGED SIDEWAYS",
        ]
    }

    fn reset(&mut self) {
        self.pan = Cx::new(3.0, 0.0);
        self.zoom = 90.0;
        self.theta = 0.0;
        self.running = true;
    }

    fn update(&mut self, dt: f64, i: &Input) -> Status {
        // pan in WORLD units, so the speed feels the same at every zoom
        let speed = 320.0 / self.zoom;
        self.pan = self.pan + Cx::new(i.axis_x * speed * dt, i.axis_y * speed * dt);
        if i.action_pressed {
            self.running = !self.running;
        }
        if self.running {
            self.theta += 0.9 * dt;
        }
        self.mouse = i.mouse;
        Status::Playing
    }

    fn draw(&self, c: &mut Canvas, v: &View, ov: Overlay) {
        // OUR view: origin at the pan point, `zoom` pixels per unit.
        let g = &self.graph(v);

        if ov.on(Overlay::GRID) {
            self.draw_grid(c, g, true);
        } else {
            self.draw_grid(c, g, false);
        }

        // ---- the unit circle ---------------------------------------------
        // Radius 1 in WORLD units. It comes out `zoom` pixels across because
        // the view scales it - nothing here says "90 pixels".
        g.ring(c, Cx::ZERO, 1.0, 2, colour::IMAG);

        // ---- y = sin(x) ---------------------------------------------------
        self.plot(c, g, |x| x.sin(), colour::REAL);

        // ---- the same circle, drawn two ways, side by side ----------------
        //
        // LEFT, cyan: the implicit form x^2 + y^2 = r^2, marched over a grid.
        // It works, it is completely general, and it is visibly made of little
        // straight pieces because that is what marching squares produces.
        let r = 1.15;
        self.implicit(c, g, |x, y| {
            let (x, y) = (x + 2.4, y - 3.0);
            x * x + y * y
        }, r * r, 90, colour::IMAG);
        g.text_mid(c, Cx::new(-2.4, 3.0 - r - 0.3), "X2+Y2=R2  MARCHED", colour::IMAG, 1);

        // RIGHT, amber: the same circle as z = r e^(i t). One multiplication
        // per point, exactly on the curve, evenly spaced. This is what a
        // circle IS, once you stop throwing the parameter away.
        let mid = Cx::new(0.7, 3.0);
        self.curve(c, g, |t| mid + Cx::expi(t).scale(r), 0.0, std::f64::consts::TAU, 128, colour::REAL);
        g.text_mid(c, mid + Cx::new(0.0, -r - 0.28), "Z = R E^(IT)  EXACT", colour::REAL, 1);

        // ---- a curve with no easy parameterisation ------------------------
        // The lemniscate (x^2+y^2)^2 = 2a^2(x^2-y^2) - a figure of eight. This
        // is the case the implicit form earns its keep on.
        let a: f64 = 0.85;
        self.implicit(c, g, |x, y| {
            let (x, y) = (x - 3.9, y - 3.0);
            let s = x * x + y * y;
            s * s - 2.0 * a * a * (x * x - y * y)
        }, 0.0, 110, 0x9B7BD4);
        g.text_mid(c, Cx::new(3.9, 3.0 - 1.0), "LEMNISCATE  IMPLICIT ONLY", 0x9B7BD4, 1);

        // ---- polygons ------------------------------------------------------
        let hex = self.ngon(Cx::new(-3.2, -1.9), 0.85, 6, 0.3);
        self.polygon(c, g, &hex, colour::GOOD);
        let tri = self.ngon(Cx::new(-1.0, -1.9), 0.85, 3, 1.2);
        self.polygon(c, g, &tri, colour::GOOD);
        let star: Vec<Cx> = (0..10)
            .map(|k| {
                let rr = if k % 2 == 0 { 0.9 } else { 0.38 };
                Cx::new(1.2, -1.9) + Cx::expi(std::f64::consts::TAU * k as f64 / 10.0 + 1.57).scale(rr)
            })
            .collect();
        self.polygon(c, g, &star, colour::GOOD);
        g.text_mid(c, Cx::new(-1.0, -3.05), "NGON = ROOTS OF UNITY, SCALED", colour::GOOD, 1);

        // ---- the point that ties them together ----------------------------
        // On the circle it is e^(i theta). Its HEIGHT is sin(theta) - and the
        // sine wave is that height, carried sideways. Watch the two markers
        // stay level with each other.
        let t = self.theta;
        let on_circle = Cx::expi(t);
        let on_wave = Cx::new(t, t.sin());

        g.line(c, on_circle, on_wave, 1, 0x2B3945); // the level line
        g.disc(c, on_circle, 6.0 / self.zoom, colour::MOD);
        g.disc(c, on_wave, 6.0 / self.zoom, colour::MOD);

        // ---- annotations --------------------------------------------------
        if ov.on(Overlay::VECTORS) {
            pen::arrow(c, g, Cx::ZERO, on_circle, pen::VEC, Some("e^it"));
        }
        if ov.on(Overlay::RADII) {
            pen::radius(c, g, Cx::ZERO, 1.0, t + 0.9, "R 1");
        }
        if ov.on(Overlay::ANGLES) {
            pen::angle_arc(c, g, Cx::ZERO, 0.38, 0.0, t, &format!("{:.2} RAD", t % std::f64::consts::TAU));
        }
        if ov.on(Overlay::LENGTHS) {
            // cos along the axis, sin up to the point - the two legs of the
            // right triangle whose hypotenuse is the radius
            let foot = Cx::new(on_circle.re, 0.0);
            g.line(c, foot, on_circle, 1, 0x2B3945);
            pen::dimension(c, g, Cx::ZERO, foot, -0.22, &format!("COS {:.3}", t.cos()));
            pen::dimension(c, g, foot, on_circle, 0.18, &format!("SIN {:.3}", t.sin()));
        }
        if ov.on(Overlay::READOUTS) {
            let mut r = Readout::new(v);
            let m = self
                .mouse
                // the game hands us the mouse in ITS frame; convert back to
                // screen pixels, then into ours
                .map(|p| g.to_world(p.re, v.h - p.im))
                .map(|w| format!("({:+.3}, {:+.3})", w.re, w.im))
                .unwrap_or_else(|| "-".into());
            r.row(c, &format!("CURSOR {m}    ZOOM {:.0} PX/UNIT    STEP {:.3}",
                              self.zoom, self.nice_step(78.0)), colour::SOFT);
            r.row(c, &format!("THETA {:.3}   COS {:+.3}   SIN {:+.3}   {}",
                              t, t.cos(), t.sin(),
                              if self.running { "RUNNING" } else { "STOPPED" }), colour::SOFT);
        }

        let _ = &self.title;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    /// The mapping and its inverse must agree, or the mouse lands in the wrong
    /// place at every zoom but 1.
    #[test]
    fn world_and_screen_round_trip_at_any_zoom() {
        for zoom in [1.0, 37.0, 90.0, 400.0] {
            let p = Playground { zoom, pan: Cx::new(2.5, -1.25), ..Playground::new("t".into()) };
            let g = p.graph(&View::new(800, 600));
            for w in [Cx::ZERO, Cx::new(1.0, 1.0), Cx::new(-3.5, 2.25)] {
                let (sx, sy) = g.to_screen(w);
                let back = g.to_world(sx as f64, sy as f64);
                assert!((back - w).abs() < 1.5 / zoom, "zoom {zoom}: {w} -> {back}");
            }
        }
    }

    /// The pan point must land in the middle of the window - that is what pan
    /// means.
    #[test]
    fn the_pan_point_sits_at_the_centre() {
        let p = Playground { pan: Cx::new(4.0, -2.0), zoom: 55.0, ..Playground::new("t".into()) };
        let v = View::new(800, 600);
        let g = p.graph(&v);
        let (sx, sy) = g.to_screen(p.pan);
        assert_eq!((sx, sy), (400, 300));
    }

    /// World y must point UP: a larger y is a smaller screen row.
    #[test]
    fn world_y_points_up() {
        let p = Playground::new("t".into());
        let g = p.graph(&View::new(800, 600));
        let low = g.to_screen(Cx::new(0.0, 0.0)).1;
        let high = g.to_screen(Cx::new(0.0, 1.0)).1;
        assert!(high < low, "y is not flipped: {high} should be above {low}");
    }

    /// Grid steps must be 1, 2 or 5 times a power of ten - never 0.37.
    #[test]
    fn the_grid_step_is_always_a_nice_number() {
        for zoom in [3.0, 12.0, 55.0, 90.0, 260.0, 1400.0] {
            let p = Playground { zoom, ..Playground::new("t".into()) };
            let s = p.nice_step(78.0);
            let mag = 10f64.powf(s.log10().floor());
            let lead = s / mag;
            assert!(
                [1.0, 2.0, 5.0, 10.0].iter().any(|m| close(lead, *m)),
                "zoom {zoom} gave step {s} (lead {lead})"
            );
            // and it lands somewhere near the target spacing
            let px = s * zoom;
            assert!((25.0..260.0).contains(&px), "zoom {zoom}: {px} px between lines");
        }
    }

    /// ★ Marching squares must land on the curve: every endpoint it produces
    /// should satisfy the equation, to within the sampling error.
    #[test]
    fn the_contour_lands_on_the_curve() {
        let p = Playground::new("t".into());
        let r: f64 = 1.3;
        let segs = p.contour(
            |x, y| x * x + y * y,
            r * r,
            Cx::new(-2.0, -2.0),
            Cx::new(2.0, 2.0),
            140,
        );
        assert!(segs.len() > 100, "only {} segments", segs.len());
        for (a, b) in &segs {
            for q in [a, b] {
                assert!(
                    (q.abs() - r).abs() < 0.02,
                    "point at radius {} should be {r}",
                    q.abs()
                );
            }
        }
    }

    /// ...and the implicit circle should agree with the parametric one, which
    /// is the honest way to check both.
    #[test]
    fn implicit_and_parametric_circles_agree() {
        let p = Playground::new("t".into());
        let r: f64 = 0.9;
        let segs = p.contour(|x, y| x * x + y * y, r * r, Cx::new(-1.5, -1.5), Cx::new(1.5, 1.5), 160);
        // every marched point is close to SOME point of z = r e^(it)
        for (a, _) in &segs {
            let t = a.arg();
            let exact = Cx::expi(t).scale(r);
            assert!((*a - exact).abs() < 0.02, "{a} vs {exact}");
        }
        // and the parametric points are exact, by construction
        for q in p.circle_pts(Cx::ZERO, r, 64) {
            assert!(close(q.abs(), r), "parametric point off the circle");
        }
    }

    /// A closed curve with no crossings should produce a contour with no loose
    /// ends: every endpoint is shared by exactly two segments.
    #[test]
    fn a_closed_contour_has_no_loose_ends() {
        let p = Playground::new("t".into());
        let segs = p.contour(|x, y| x * x + y * y, 1.0, Cx::new(-2.0, -2.0), Cx::new(2.0, 2.0), 60);
        // count how many segment ends land near each end
        let mut lonely = 0;
        for (a, _) in &segs {
            let touching = segs
                .iter()
                .filter(|(p2, q2)| (*p2 - *a).abs() < 1e-9 || (*q2 - *a).abs() < 1e-9)
                .count();
            if touching < 2 {
                lonely += 1;
            }
        }
        assert_eq!(lonely, 0, "{lonely} endpoints were not shared");
    }

    /// The contour must be empty when the level is never reached.
    #[test]
    fn no_contour_where_the_curve_is_not() {
        let p = Playground::new("t".into());
        let segs = p.contour(|x, y| x * x + y * y, 100.0, Cx::new(-1.0, -1.0), Cx::new(1.0, 1.0), 40);
        assert!(segs.is_empty(), "found {} segments for a circle of radius 10", segs.len());
    }

    /// A regular n-gon is the roots of unity: every vertex the same distance
    /// out, evenly spaced.
    #[test]
    fn the_ngon_vertices_are_evenly_spaced_on_a_circle() {
        let p = Playground::new("t".into());
        let c = Cx::new(2.0, -1.0);
        let g = p.ngon(c, 1.7, 7, 0.4);
        assert_eq!(g.len(), 7);
        for q in &g {
            assert!(close((*q - c).abs(), 1.7));
        }
        let step = (g[1] - c).arg() - (g[0] - c).arg();
        assert!(close(step, std::f64::consts::TAU / 7.0));
    }
}
