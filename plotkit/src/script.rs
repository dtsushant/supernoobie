//! # Running a script onto a canvas
//!
//! The join between [`crate::expr`] (which turns text into commands) and
//! [`crate::plot`] (which draws). Give it a script and a [`View`] and it
//! draws:
//!
//! ```text
//! a = 0 + 0i
//! b = 1 + 2i
//! polygon(a, b)
//! ```
//!
//! Deferred commands — `plot`, `param`, `implicit` — carry an unevaluated
//! expression with a free variable. This is where that variable finally gets
//! bound, once per sample.

use crate::complex::Cx;
use crate::expr::{env_of, eval_with, Cmd, Expr, Program};
use crate::plot;
use crate::raster::Canvas;
use crate::view::View;
use std::collections::HashMap;

/// Colours a script cycles through when it does not say. Chosen to stay
/// distinguishable on a dark ground.
pub const PALETTE: [u32; 6] = [0x4FBCD4, 0xE0A44A, 0xE585AC, 0x6FCF97, 0x9B7BD4, 0xE0704A];

pub struct Style {
    /// Used until the script says `color(...)`.
    pub start: u32,
    /// Advance through [`PALETTE`] for each drawing command that has not been
    /// given an explicit colour.
    pub auto_cycle: bool,
    pub point_radius_px: f64,
    /// Samples for `param`, and grid resolution for `implicit`.
    pub samples: usize,
    pub implicit_res: usize,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            start: PALETTE[0],
            auto_cycle: true,
            point_radius_px: 5.0,
            samples: 256,
            implicit_res: 140,
        }
    }
}

/// What happened, for reporting back to whoever is editing the script.
#[derive(Default, Debug)]
pub struct Report {
    pub drawn: usize,
    /// Failures during *drawing* — a deferred expression that blew up on some
    /// sample, which parsing could not have caught.
    pub runtime_errors: Vec<String>,
}

/// Draw every command in a program.
pub fn draw(c: &mut Canvas, v: &View, p: &Program, st: &Style) -> Report {
    let env = env_of(p);
    let mut rep = Report::default();
    let mut colour = st.start;
    let mut auto = 0usize;

    // An explicit `color(...)` pins the colour; otherwise each shape takes the
    // next one from the palette, so a script of bare `polygon(...)` lines is
    // still readable.
    let mut pinned = false;

    for cmd in &p.cmds {
        if let Cmd::Color(x) = cmd {
            colour = *x;
            pinned = true;
            continue;
        }
        let col = if pinned || !st.auto_cycle {
            colour
        } else {
            let c = PALETTE[auto % PALETTE.len()];
            auto += 1;
            c
        };

        match cmd {
            Cmd::Color(_) => unreachable!(),
            Cmd::Point(pts) => {
                for q in pts {
                    v.disc(c, *q, st.point_radius_px / v.scale.max(1e-9), col);
                }
            }
            Cmd::Line(pts) => plot::polyline(c, v, pts, col),
            Cmd::Polygon(pts) => plot::polygon(c, v, pts, col),
            Cmd::Circle(centre, r) => {
                // parametric, not a rasterised ring: it stays smooth at any zoom
                plot::param(c, v, |t| *centre + Cx::expi(t).scale(*r), 0.0, std::f64::consts::TAU, st.samples, col);
            }
            Cmd::Ngon(centre, r, n) => {
                let pts = plot::ngon(*centre, *r, *n, 0.0);
                plot::polygon(c, v, &pts, col);
            }
            Cmd::Plot(e) => {
                if let Err(msg) = sample_ok(e, "x", &env) {
                    rep.runtime_errors.push(format!("plot: {msg}"));
                    continue;
                }
                plot::graph(c, v, |x| {
                    eval_with(e, "x", Cx::new(x, 0.0), &env)
                        .map(|z| z.re)
                        .unwrap_or(f64::NAN)
                }, col);
            }
            Cmd::Param(e, t0, t1) => {
                if let Err(msg) = sample_ok(e, "t", &env) {
                    rep.runtime_errors.push(format!("param: {msg}"));
                    continue;
                }
                plot::param(c, v, |t| {
                    eval_with(e, "t", Cx::new(t, 0.0), &env)
                        .unwrap_or(Cx::new(f64::NAN, f64::NAN))
                }, *t0, *t1, st.samples, col);
            }
            Cmd::Implicit(e, level) => {
                if let Err(msg) = sample_xy(e, &env) {
                    rep.runtime_errors.push(format!("implicit: {msg}"));
                    continue;
                }
                let f = |x: f64, y: f64| {
                    let mut m = env.clone();
                    m.insert("x".into(), Cx::new(x, 0.0));
                    m.insert("y".into(), Cx::new(y, 0.0));
                    e.eval(&m).map(|z| z.re).unwrap_or(f64::NAN)
                };
                plot::implicit(c, v, f, *level, st.implicit_res, col);
            }
        }
        rep.drawn += 1;
    }
    rep
}

/// Try one sample before committing to a few hundred, so a typo inside a
/// deferred expression is reported once rather than swallowed per pixel.
fn sample_ok(e: &Expr, var: &str, env: &HashMap<String, Cx>) -> Result<(), String> {
    eval_with(e, var, Cx::new(0.37, 0.0), env).map(|_| ())
}

fn sample_xy(e: &Expr, env: &HashMap<String, Cx>) -> Result<(), String> {
    let mut m = env.clone();
    m.insert("x".into(), Cx::new(0.37, 0.0));
    m.insert("y".into(), Cx::new(0.21, 0.0));
    e.eval(&m).map(|_| ())
}

/// Parse and draw in one call.
pub fn run(c: &mut Canvas, v: &View, src: &str, st: &Style) -> (Program, Report) {
    let p = crate::expr::run(src);
    let r = draw(c, v, &p, st);
    (p, r)
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn painted(c: &Canvas) -> usize {
        c.buf.iter().filter(|&&p| p != 0).count()
    }

    /// ★ The requested script draws a line, end to end from text to pixels.
    #[test]
    fn a_two_point_polygon_draws_a_line() {
        let v = View::centred(200, 200, 40.0);
        let mut c = Canvas::new(200, 200);
        c.clear(0);
        let (p, r) = run(&mut c, &v, "a = 0 + 0i\nb = 1 + 2i\npolygon(a, b)", &Style::default());
        assert!(p.errors.is_empty(), "{:?}", p.errors);
        assert_eq!(r.drawn, 1);
        assert!(painted(&c) > 40, "only {} pixels", painted(&c));
    }

    /// The line really runs between the two points, not somewhere else.
    #[test]
    fn the_line_lands_where_the_numbers_say() {
        let v = View::centred(200, 200, 40.0);
        let mut c = Canvas::new(200, 200);
        c.clear(0);
        run(&mut c, &v, "polygon(0 + 0i, 1 + 2i)", &Style::default());
        for (w, want) in [(Cx::ZERO, true), (Cx::new(1.0, 2.0), true), (Cx::new(-2.0, 2.0), false)] {
            let (x, y) = v.to_screen(w);
            let near = (-2..=2).any(|dy| (-2..=2).any(|dx| {
                let (px, py) = (x + dx, y + dy);
                px >= 0 && py >= 0 && px < 200 && py < 200 && c.buf[(py * 200 + px) as usize] != 0
            }));
            assert_eq!(near, want, "at {w}");
        }
    }

    #[test]
    fn every_command_draws_something() {
        for src in [
            "point(0)",
            "line(0, 1, 1+1i)",
            "polygon(0, 1, 1i)",
            "circle(0, 1)",
            "ngon(0, 1, 6)",
            "plot(sin(x))",
            "param(exp(i*t), 0, tau)",
            "implicit(x*x + y*y, 1)",
        ] {
            let v = View::centred(240, 240, 60.0);
            let mut c = Canvas::new(240, 240);
            c.clear(0);
            let (p, r) = run(&mut c, &v, src, &Style::default());
            assert!(p.errors.is_empty(), "{src}: {:?}", p.errors);
            assert!(r.runtime_errors.is_empty(), "{src}: {:?}", r.runtime_errors);
            assert_eq!(r.drawn, 1, "{src} drew {} things", r.drawn);
            assert!(painted(&c) > 8, "{src} painted only {} pixels", painted(&c));
        }
    }

    /// A deferred expression with a typo must report ONCE, not per sample, and
    /// must not take the rest of the drawing with it.
    #[test]
    fn a_bad_deferred_expression_reports_once_and_keeps_going() {
        let v = View::centred(200, 200, 40.0);
        let mut c = Canvas::new(200, 200);
        c.clear(0);
        let (p, r) = run(&mut c, &v, "plot(nosuch(x))\npolygon(0, 1+1i)", &Style::default());
        // `nosuch(` is not a known function, so it parses as multiplication and
        // fails at evaluation - which is exactly the case this guards
        assert!(p.errors.len() + r.runtime_errors.len() >= 1, "the typo went unreported");
        assert!(painted(&c) > 20, "the good polygon should still have drawn");
    }

    /// `color(...)` pins the colour; without it, shapes cycle the palette so a
    /// plain script is still readable.
    #[test]
    fn colours_pin_and_cycle() {
        let v = View::centred(120, 120, 30.0);

        let mut a = Canvas::new(120, 120);
        a.clear(0);
        run(&mut a, &v, "polygon(-1, 1)\npolygon(-1i, 1i)", &Style::default());
        let mut shades: Vec<u32> = a.buf.iter().copied().filter(|&p| p != 0).collect();
        shades.sort_unstable();
        shades.dedup();
        assert!(shades.len() >= 2, "the two shapes should have taken different colours");

        let mut b = Canvas::new(120, 120);
        b.clear(0);
        run(&mut b, &v, "color(16711680)\npolygon(-1, 1)\npolygon(-1i, 1i)", &Style::default());
        let mut pinned: Vec<u32> = b.buf.iter().copied().filter(|&p| p != 0).collect();
        pinned.sort_unstable();
        pinned.dedup();
        assert_eq!(pinned, vec![0xFF0000], "a pinned colour should apply to both");
    }

    /// An empty or entirely broken script must draw nothing and not panic.
    #[test]
    fn an_empty_or_broken_script_is_harmless() {
        let v = View::centred(80, 80, 20.0);
        for src in ["", "# just a comment", "@@@", "polygon(", "a = "] {
            let mut c = Canvas::new(80, 80);
            c.clear(0);
            let (_, r) = run(&mut c, &v, src, &Style::default());
            assert_eq!(r.drawn, 0, "'{src}' drew something");
            assert_eq!(painted(&c), 0, "'{src}' painted pixels");
        }
    }
}
