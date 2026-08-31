//! # Playground — a scrollable graph driven by a script file
//!
//! Nothing is drawn from code here. The picture comes from
//! `scripts/playground.rec`, which is **reloaded whenever you save it** — edit
//! the file, alt-tab, and it has already changed.
//!
//! ---
//!
//! ## The three layers
//!
//! ```text
//!   Canvas    raw pixels.  origin TOP-LEFT, y counts DOWN
//!     ^
//!   View      world -> screen.  origin wherever you put it, y counts UP
//!     ^
//!   the script    maths.  "a circle of radius 1 at the origin"
//! ```
//!
//! All of that now lives in the `plotkit` crate; this file only decides where
//! the view is pointing and puts the errors on screen.
//!
//! ## The view
//!
//! The stage is handed the *game's* view — origin bottom-left, one pixel per
//! unit — which is the wrong frame for mathematics, since a unit circle would
//! come out one pixel across. So it builds its own each frame: world `pan` at
//! the centre of the window, `zoom` pixels per unit.
//!
//! ## Overlay
//!
//! `Overlay` draws nothing itself — it is a `u16` of toggle bits and every
//! stage decides what they mean. Here:
//!
//! | key | |
//! |---|---|
//! | 1 LENGTHS | the sin and cos legs, as dimension lines |
//! | 2 ANGLES | the angle arc on the unit circle |
//! | 3 RADII | the radius, marked |
//! | 4 VECTORS | the rotating radius as an arrow |
//! | 6 READOUTS | cursor position, zoom, the current numbers |
//! | 7 GRID | graph paper and tick labels |
//! | 8 FORMULAS | the mapping being used |

use crate::complex::Cx;
use crate::game::{pen, Input, Overlay, Readout, Stage, Status, View};
use crate::raster::{colour, Canvas};
use plotkit::plot::{self, GridStyle};
use plotkit::script;
use std::path::PathBuf;
use std::time::SystemTime;

pub struct Playground {
    path: PathBuf,
    src: String,
    stamp: Option<SystemTime>,
    /// Seconds until the next check for an edit.
    poll: f64,
    reloads: u32,

    pan: Cx,
    zoom: f64,
    theta: f64,
    running: bool,
    mouse: Option<Cx>,
}

impl Playground {
    pub fn new(_title: String) -> Self {
        let path = PathBuf::from("scripts/playground.rec");
        let mut p = Playground {
            path,
            src: String::new(),
            stamp: None,
            poll: 0.0,
            reloads: 0,
            pan: Cx::new(0.6, 0.4),
            zoom: 84.0,
            theta: 0.0,
            running: true,
            mouse: None,
        };
        p.reload();
        p
    }

    fn reload(&mut self) {
        match std::fs::read_to_string(&self.path) {
            Ok(s) => {
                self.src = s;
                self.reloads += 1;
            }
            Err(e) => {
                self.src = format!("# could not read {}\n# {e}\n", self.path.display());
            }
        }
        self.stamp = std::fs::metadata(&self.path).and_then(|m| m.modified()).ok();
    }

    /// Reload if the file has been written since we last looked. Checked a few
    /// times a second rather than every frame — a `stat` is cheap but not free.
    fn poll_file(&mut self, dt: f64) {
        self.poll -= dt;
        if self.poll > 0.0 {
            return;
        }
        self.poll = 0.25;
        let now = std::fs::metadata(&self.path).and_then(|m| m.modified()).ok();
        if now != self.stamp {
            self.reload();
        }
    }

    /// The graph's own view: `pan` at the centre, `zoom` pixels per unit.
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
}

impl Stage for Playground {
    fn name(&self) -> &'static str {
        "PLAYGROUND"
    }
    fn goal(&self) -> &'static str {
        "EDIT SCRIPTS/PLAYGROUND.REC AND SAVE - THE WINDOW RELOADS ITSELF"
    }
    fn controls(&self) -> &'static str {
        "ARROWS PAN  W/S ZOOM  SPACE RUN/STOP  R RELOAD  ESC MENU"
    }
    fn formula(&self) -> &'static [&'static str] {
        &[
            "SCREEN_X = ORIGIN_X + SCALE * WORLD_X",
            "SCREEN_Y = ORIGIN_Y - SCALE * WORLD_Y          (THE Y FLIP)",
            "A POINT AND AN OFFSET AND A NUMBER ARE ALL ONE COMPLEX VALUE",
            "SO A*Z + B IS ANY AFFINE MAP, WRITTEN EXACTLY LIKE THAT",
        ]
    }

    fn reset(&mut self) {
        self.pan = Cx::new(0.6, 0.4);
        self.zoom = 84.0;
        self.theta = 0.0;
        self.running = true;
        self.reload();
    }

    fn update(&mut self, dt: f64, i: &Input) -> Status {
        self.poll_file(dt);
        // pan in WORLD units, so it feels the same at every zoom
        let speed = 340.0 / self.zoom;
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
        let g = &self.graph(v);

        plot::grid(c, g, &GridStyle { labels: ov.on(Overlay::GRID), ..GridStyle::default() });

        // ---- everything visible comes from the file ----------------------
        let prog = plotkit::expr::run(&self.src);
        let rep = script::draw(c, g, &prog, &script::Style::default());

        // ---- the one live thing: the point going round --------------------
        // On the circle it is e^(i theta); its HEIGHT is sin(theta), and the
        // sine wave is that height carried sideways. The two markers stay
        // level with each other, which is the whole relationship.
        let t = self.theta;
        let on_circle = Cx::expi(t);
        let on_wave = Cx::new(t, t.sin());
        g.line(c, on_circle, on_wave, 1, 0x2B3945);
        g.disc(c, on_circle, 5.0 / self.zoom, colour::MOD);
        g.disc(c, on_wave, 5.0 / self.zoom, colour::MOD);

        if ov.on(Overlay::VECTORS) {
            pen::arrow(c, g, Cx::ZERO, on_circle, pen::VEC, Some("e^it"));
        }
        if ov.on(Overlay::RADII) {
            pen::radius(c, g, Cx::ZERO, 1.0, t + 0.9, "R 1");
        }
        if ov.on(Overlay::ANGLES) {
            pen::angle_arc(c, g, Cx::ZERO, 0.34, 0.0, t,
                           &format!("{:.2} RAD", t.rem_euclid(std::f64::consts::TAU)));
        }
        if ov.on(Overlay::LENGTHS) {
            let foot = Cx::new(on_circle.re, 0.0);
            g.line(c, foot, on_circle, 1, 0x2B3945);
            pen::dimension(c, g, Cx::ZERO, foot, -0.2, &format!("COS {:.3}", t.cos()));
            pen::dimension(c, g, foot, on_circle, 0.16, &format!("SIN {:.3}", t.sin()));
        }

        // ---- what the script did, and what went wrong ---------------------
        let mut y = 96;
        c.text(28, y, &format!("{}  -  {} SHAPES, {} NAMES, RELOAD {}",
                               self.path.display(), rep.drawn, prog.vars.len(), self.reloads),
               colour::FAINT, 1);
        y += 18;
        for (line, msg) in prog.errors.iter().take(6) {
            c.text(28, y, &format!("LINE {line}: {msg}"), colour::WARN, 1);
            y += 15;
        }
        for msg in rep.runtime_errors.iter().take(3) {
            c.text(28, y, msg, colour::WARN, 1);
            y += 15;
        }

        if ov.on(Overlay::READOUTS) {
            let mut r = Readout::new(v);
            let m = self
                .mouse
                // the shell hands the mouse over in ITS frame; back to screen
                // pixels, then into ours
                .map(|p| g.to_world(p.re, v.h - p.im))
                .map(|w| format!("({:+.3}, {:+.3})", w.re, w.im))
                .unwrap_or_else(|| "-".into());
            r.row(c, &format!("CURSOR {m}    ZOOM {:.0} PX/UNIT    STEP {:.3}",
                              self.zoom, plot::nice_step(self.zoom, 78.0)), colour::SOFT);
            r.row(c, &format!("THETA {:.3}   COS {:+.3}   SIN {:+.3}   {}",
                              t, t.cos(), t.sin(),
                              if self.running { "RUNNING" } else { "STOPPED" }), colour::SOFT);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pan point must land in the middle of the window — that is what pan
    /// means — and the mapping must invert exactly at any zoom.
    #[test]
    fn the_view_is_centred_on_the_pan_point_and_round_trips() {
        for zoom in [1.0, 37.0, 84.0, 400.0] {
            let mut p = Playground::new("t".into());
            p.zoom = zoom;
            p.pan = Cx::new(2.5, -1.25);
            let v = View::new(800, 600);
            let g = p.graph(&v);
            assert_eq!(g.to_screen(p.pan), (400, 300));
            for w in [Cx::ZERO, Cx::new(1.0, 1.0), Cx::new(-3.5, 2.25)] {
                let (sx, sy) = g.to_screen(w);
                let back = g.to_world(sx as f64, sy as f64);
                assert!((back - w).abs() < 1.5 / zoom, "zoom {zoom}: {w} -> {back}");
            }
        }
    }

    /// World y must point up: a larger y is a smaller screen row.
    #[test]
    fn world_y_points_up() {
        let p = Playground::new("t".into());
        let g = p.graph(&View::new(800, 600));
        assert!(g.to_screen(Cx::new(0.0, 1.0)).1 < g.to_screen(Cx::ZERO).1);
    }

    /// A missing script file must leave a readable message, not panic.
    #[test]
    fn a_missing_script_is_survivable() {
        let mut p = Playground::new("t".into());
        p.path = PathBuf::from("scripts/definitely-not-here.rec");
        p.reload();
        assert!(p.src.contains("could not read"));
        let prog = plotkit::expr::run(&p.src);
        assert!(prog.cmds.is_empty());
    }
}
