//! # RECURSION — four games over the same physics
//!
//!   cargo run --release --features window --bin recursion
//!
//! Each stage is one of the physics files with a goal attached. Keys **1-8**
//! toggle the mathematics overlay: lengths, angles, radii, vectors, contacts,
//! readouts, the spatial-hash grid, and the equation being solved.
//!
//! With the overlay off it is a game. With it on it is a diagram you can play.

use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Window, WindowOptions};
use recursion1::bike::{two_bone_ik, Bike, Terrain};
use recursion1::complex::Cx;
use recursion1::fluid::{Bound, Fluid};
use recursion1::game::{chrome, pen, Input, Overlay, Readout, Stage, Status, View};
use recursion1::raster::{colour, Canvas};
use recursion1::rigid::{Body, Wall, World};
use recursion1::soft::Fabric;
use std::time::Instant;
use recursion1::playground::Playground;

const W: usize = 1180;
const H: usize = 760;
const DT: f64 = 1.0 / 600.0;

// ===========================================================================
// 1. CRANE — the pulley, as a game
// ===========================================================================

/// Rope paid out is `r * theta`, exactly as in `pulley.rs`. Here the winch
/// drum is on screen and you can watch the arc length become rope.
struct Crane {
    world: World,
    trolley: f64,
    rope: f64,
    drum_r: f64,
    held: Option<(usize, f64, f64)>, // index, saved inv_m, saved inv_i
    target_y: f64,
    settled: f64,
}

const RAIL_Y: f64 = 620.0;
const FLOOR_Y: f64 = 132.0;

impl Crane {
    fn new() -> Self {
        let mut c = Crane {
            world: World::default(),
            trolley: 300.0,
            rope: 180.0,
            drum_r: 26.0,
            held: None,
            target_y: 300.0,
            settled: 0.0,
        };
        c.reset();
        c
    }
    fn hook(&self) -> Cx {
        Cx::new(self.trolley, RAIL_Y - self.rope)
    }
    /// The winch angle that has paid out this much rope: `theta = length / r`.
    fn theta(&self) -> f64 {
        self.rope / self.drum_r
    }
    fn stacked(&self) -> usize {
        self.world
            .bodies
            .iter()
            .filter(|b| b.inv_m > 0.0 && b.p.im > self.target_y)
            .count()
    }
}

impl Stage for Crane {
    fn name(&self) -> &'static str {
        "CRANE"
    }
    fn goal(&self) -> &'static str {
        "STACK THREE BLOCKS ABOVE THE LINE AND LET THEM SETTLE"
    }
    fn controls(&self) -> &'static str {
        "LEFT/RIGHT TROLLEY  UP/DOWN ROPE  SPACE GRAB  R RESET  ESC MENU"
    }
    fn formula(&self) -> &'static [&'static str] {
        &[
            "ROPE PAID OUT   S = R * THETA        (ARC LENGTH, RADIANS ONLY)",
            "CONTACTS        J = -(1+E) VN / K",
            "K = 1/MA + 1/MB + (RA X N)^2/IA + (RB X N)^2/IB",
        ]
    }
    fn reset(&mut self) {
        self.world = World { gravity: Cx::new(0.0, -1200.0), iterations: 14, ..World::default() };
        self.world.walls.push(Wall::new(Cx::new(0.0, FLOOR_Y), Cx::new(0.0, 1.0)));
        self.world.walls.push(Wall::new(Cx::new(40.0, 0.0), Cx::new(1.0, 0.0)));
        self.world.walls.push(Wall::new(Cx::new(W as f64 - 40.0, 0.0), Cx::new(-1.0, 0.0)));
        for k in 0..5 {
            let mut b = Body::disc(Cx::new(620.0 + k as f64 * 74.0, FLOOR_Y + 34.0), 32.0, 1.0);
            b.restitution = 0.05;
            b.friction = 0.85;
            self.world.add(b);
        }
        self.trolley = 300.0;
        self.rope = 180.0;
        self.held = None;
        self.settled = 0.0;
    }

    fn update(&mut self, dt: f64, i: &Input) -> Status {
        self.trolley = (self.trolley + i.axis_x * 320.0 * dt).clamp(90.0, W as f64 - 90.0);
        self.rope = (self.rope - i.axis_y * 220.0 * dt).clamp(40.0, RAIL_Y - FLOOR_Y - 20.0);

        if i.action_pressed {
            match self.held.take() {
                Some((k, im, ii)) => {
                    self.world.bodies[k].inv_m = im;
                    self.world.bodies[k].inv_i = ii;
                }
                None => {
                    let h = self.hook();
                    let mut best: Option<(usize, f64)> = None;
                    for (k, b) in self.world.bodies.iter().enumerate() {
                        let d = (b.p - h).abs();
                        if d < b.r + 26.0 && best.map_or(true, |(_, bd)| d < bd) {
                            best = Some((k, d));
                        }
                    }
                    if let Some((k, _)) = best {
                        let b = &mut self.world.bodies[k];
                        self.held = Some((k, b.inv_m, b.inv_i));
                        b.inv_m = 0.0;
                        b.inv_i = 0.0;
                    }
                }
            }
        }

        if let Some((k, _, _)) = self.held {
            // carried blocks are kinematic: pinned to the hook, no dynamics
            self.world.bodies[k].p = self.hook() + Cx::new(0.0, -32.0);
            self.world.bodies[k].v = Cx::ZERO;
            self.world.bodies[k].omega = 0.0;
        }
        self.world.step(dt);

        let quiet = self.world.bodies.iter().all(|b| b.v.abs() < 12.0);
        if self.stacked() >= 3 && quiet && self.held.is_none() {
            self.settled += dt;
        } else {
            self.settled = 0.0;
        }
        if self.settled > 0.8 {
            Status::Won
        } else {
            Status::Playing
        }
    }

    fn draw(&self, c: &mut Canvas, v: &View, ov: Overlay) {
        // rail, target line, floor
        v.line(c, Cx::new(60.0, RAIL_Y), Cx::new(W as f64 - 60.0, RAIL_Y), 2, colour::LINE);
        v.line(c, Cx::new(40.0, FLOOR_Y), Cx::new(W as f64 - 40.0, FLOOR_Y), 2, colour::LINE);
        for x in (60..(W - 60)).step_by(22) {
            let p = Cx::new(x as f64, self.target_y);
            v.line(c, p, p + Cx::new(11.0, 0.0), 1, colour::GOOD);
        }
        v.text(c, Cx::new(64.0, self.target_y + 8.0), "TARGET", colour::GOOD, 1);

        // the winch drum, and the rope
        let drum = Cx::new(self.trolley, RAIL_Y);
        v.ring(c, drum, self.drum_r, 2, colour::INK);
        // a spoke, so the rotation is visible: theta = rope / r
        let spoke = drum + Cx::expi(-self.theta()).scale(self.drum_r);
        v.line(c, drum, spoke, 2, colour::MOD);
        v.line(c, drum, self.hook(), 2, 0x8CA0B3);
        v.disc(c, self.hook(), 5.0, colour::MOD);

        for b in &self.world.bodies {
            let held = self.held.map_or(false, |(k, _, _)| self.world.bodies[k].p == b.p);
            v.ring(c, b.p, b.r, 2, if held { colour::MOD } else { colour::IMAG });
            let tip = b.p + Cx::expi(b.angle).scale(b.r);
            v.line(c, b.p, tip, 1, 0x2B3945);
        }

        // ---- annotations -------------------------------------------------
        if ov.on(Overlay::LENGTHS) {
            pen::dimension(c, v, drum, self.hook(), 34.0, &format!("L {:.0}", self.rope));
        }
        if ov.on(Overlay::RADII) {
            pen::radius(c, v, drum, self.drum_r, 0.6, &format!("R {:.0}", self.drum_r));
            for b in &self.world.bodies {
                pen::radius(c, v, b.p, b.r, 2.2, &format!("{:.0}", b.r));
            }
        }
        if ov.on(Overlay::ANGLES) {
            let t = self.theta() % (2.0 * std::f64::consts::PI);
            pen::angle_arc(c, v, drum, self.drum_r * 0.62, -t, 0.0,
                           &format!("{:.2} RAD", self.theta()));
        }
        if ov.on(Overlay::VECTORS) {
            for b in &self.world.bodies {
                if b.inv_m > 0.0 && b.v.abs() > 4.0 {
                    pen::arrow(c, v, b.p, b.p + b.v.scale(0.12), pen::VEC, None);
                }
            }
        }
        if ov.on(Overlay::CONTACTS) {
            for k in &self.world.contacts {
                pen::arrow(c, v, k.point, k.point + k.normal.scale(22.0), pen::HIT, None);
            }
        }
        if ov.on(Overlay::READOUTS) {
            let mut r = Readout::new(v);
            r.row(c, &format!("STACKED {} / 3   CONTACTS {}", self.stacked(), self.world.contacts.len()), colour::SOFT);
            r.row(c, &format!("ROPE {:.0}   THETA {:.2} RAD   R*THETA {:.0}", self.rope, self.theta(), self.drum_r * self.theta()), colour::SOFT);
        }
    }
}

// ===========================================================================
// 2. CUT — the rope, as a game
// ===========================================================================

struct Cut {
    rope: Fabric,
    basket: Cx,
    basket_r: f64,
    weight: usize,
    cuts: u32,
    time: f64,
}

impl Cut {
    fn new() -> Self {
        let mut s = Cut {
            rope: Fabric::default(),
            basket: Cx::new(880.0, 140.0),
            basket_r: 70.0,
            weight: 0,
            cuts: 0,
            time: 0.0,
        };
        s.reset();
        s
    }
    fn weight_pos(&self) -> Cx {
        self.rope.particles[self.weight].p
    }
}

impl Stage for Cut {
    fn name(&self) -> &'static str {
        "CUT"
    }
    fn goal(&self) -> &'static str {
        "CUT THE ROPE SO THE WEIGHT SWINGS INTO THE BASKET"
    }
    fn controls(&self) -> &'static str {
        "CLICK OR DRAG TO CUT   SPACE NUDGE   R RESET   ESC MENU"
    }
    fn formula(&self) -> &'static [&'static str] {
        &[
            "VERLET       X' = X + (X - XPREV)*D + A DT^2      (V IS IMPLICIT)",
            "CONSTRAINT   CORR = D/|D| * (|D| - L) * K / (WA + WB)",
        ]
    }
    fn reset(&mut self) {
        // a long rope pinned at both ends, sagging, with a heavy bead on it
        let mut f = Fabric::rope(Cx::new(180.0, 620.0), Cx::new(700.0, 620.0), 40, true, true);
        for l in &mut f.links {
            l.rest *= 1.25;
            l.stiffness = 0.95;
        }
        f.iterations = 14;
        f.damping = 0.4;
        f.bounds = Some((Cx::new(30.0, 60.0), Cx::new(W as f64 - 30.0, H as f64 - 60.0)));
        self.weight = 20;
        self.rope = f;
        self.cuts = 0;
        self.time = 0.0;
    }

    fn update(&mut self, dt: f64, i: &Input) -> Status {
        self.time += dt;
        if let Some(m) = i.mouse {
            if i.mouse_down {
                let n = self.rope.cut(m, 22.0);
                if n > 0 {
                    self.cuts += n as u32;
                }
            }
        }
        if i.action {
            // a puff of wind, so a stuck weight can be coaxed
            self.rope.wind = Cx::new(900.0, 0.0);
        } else {
            self.rope.wind = Cx::ZERO;
        }
        self.rope.step(dt);

        let w = self.weight_pos();
        if (w - self.basket).abs() < self.basket_r * 0.75 {
            return Status::Won;
        }
        if w.im < 70.0 && self.time > 1.0 {
            return Status::Lost;
        }
        Status::Playing
    }

    fn draw(&self, c: &mut Canvas, v: &View, ov: Overlay) {
        // basket
        pen::crosshair(c, v, self.basket, self.basket_r, colour::GOOD);
        v.ring(c, self.basket, self.basket_r, 2, colour::GOOD);
        v.text_mid(c, self.basket + Cx::new(0.0, -self.basket_r - 16.0), "BASKET", colour::GOOD, 1);

        for l in &self.rope.links {
            if !l.alive {
                continue;
            }
            let (a, b) = (self.rope.particles[l.a].p, self.rope.particles[l.b].p);
            let strain = (((b - a).abs() - l.rest) / l.rest).abs();
            let col = if strain > 0.08 { colour::WARN } else { colour::IMAG };
            v.line(c, a, b, 2, col);
        }
        for q in &self.rope.particles {
            if q.w == 0.0 {
                v.disc(c, q.p, 4.0, colour::MOD);
            }
        }
        v.disc(c, self.weight_pos(), 11.0, colour::REAL);

        // ---- annotations -------------------------------------------------
        if ov.on(Overlay::LENGTHS) {
            let total: f64 = self
                .rope
                .links
                .iter()
                .filter(|l| l.alive)
                .map(|l| {
                    (self.rope.particles[l.b].p - self.rope.particles[l.a].p).abs()
                })
                .sum();
            let a = self.rope.particles[0].p;
            let b = self.rope.particles[self.rope.particles.len() - 1].p;
            pen::dimension(c, v, a, b, 46.0, &format!("SPAN {:.0}", (b - a).abs()));
            pen::tag(c, v, self.weight_pos(), Cx::new(46.0, -30.0),
                     &format!("ROPE {total:.0}"), pen::DIM);
        }
        if ov.on(Overlay::VECTORS) {
            for (k, q) in self.rope.particles.iter().enumerate() {
                if k % 4 != 0 || q.w == 0.0 {
                    continue;
                }
                let vel = q.implied_velocity().scale(1.0 / DT);
                if vel.abs() > 40.0 {
                    pen::arrow(c, v, q.p, q.p + vel.scale(0.05), pen::VEC, None);
                }
            }
        }
        if ov.on(Overlay::RADII) {
            pen::radius(c, v, self.basket, self.basket_r, 1.0, &format!("R {:.0}", self.basket_r));
        }
        if ov.on(Overlay::READOUTS) {
            let mut r = Readout::new(v);
            r.row(c, &format!("LINKS {} ALIVE   {} CUT", self.rope.live_links(), self.cuts), colour::SOFT);
            r.row(c, &format!("MAX STRAIN {:.3}   WEIGHT AT ({:.0}, {:.0})",
                              self.rope.max_strain(), self.weight_pos().re, self.weight_pos().im), colour::SOFT);
        }
    }
}

// ===========================================================================
// 3. TILT — rigid bodies, as a game
// ===========================================================================

struct Tilt {
    world: World,
    tilt: f64,
    goal: Cx,
    goal_r: f64,
    ball: usize,
    time: f64,
}

impl Tilt {
    fn new() -> Self {
        let mut s = Tilt {
            world: World::default(),
            tilt: 0.0,
            goal: Cx::new(1010.0, 205.0),
            goal_r: 54.0,
            ball: 0,
            time: 0.0,
        };
        s.reset();
        s
    }
}

impl Stage for Tilt {
    fn name(&self) -> &'static str {
        "TILT"
    }
    fn goal(&self) -> &'static str {
        "TILT GRAVITY TO ROLL THE BALL THROUGH THE PEGS INTO THE GOAL"
    }
    fn controls(&self) -> &'static str {
        "LEFT/RIGHT TILT   DOWN LEVEL   R RESET   ESC MENU"
    }
    fn formula(&self) -> &'static [&'static str] {
        &[
            "GRAVITY      G' = G * E^(I*TILT)          (ONE MULTIPLICATION)",
            "IMPULSE      J = -(1+E) VN / K",
            "SPIN         DW = (R X J) / I",
        ]
    }
    fn reset(&mut self) {
        let mut w = World { gravity: Cx::new(0.0, -1100.0), iterations: 12, ..World::default() };
        w.walls.push(Wall::new(Cx::new(0.0, 140.0), Cx::new(0.0, 1.0)));
        w.walls.push(Wall::new(Cx::new(0.0, H as f64 - 150.0), Cx::new(0.0, -1.0)));
        w.walls.push(Wall::new(Cx::new(50.0, 0.0), Cx::new(1.0, 0.0)));
        w.walls.push(Wall::new(Cx::new(W as f64 - 50.0, 0.0), Cx::new(-1.0, 0.0)));
        // a lattice of pegs, offset row to row
        for row in 0..4 {
            for col in 0..8 {
                let x = 250.0 + col as f64 * 105.0 + if row % 2 == 0 { 0.0 } else { 52.0 };
                let y = 250.0 + row as f64 * 88.0;
                let mut p = Body::disc(Cx::new(x, y), 20.0, 1.0).pinned();
                p.restitution = 0.4;
                w.add(p);
            }
        }
        let mut b = Body::disc(Cx::new(140.0, H as f64 - 220.0), 24.0, 1.0);
        b.restitution = 0.35;
        b.friction = 0.35;
        self.ball = w.add(b);
        self.world = w;
        self.tilt = 0.0;
        self.time = 0.0;
    }

    fn update(&mut self, dt: f64, i: &Input) -> Status {
        self.time += dt;
        self.tilt += i.axis_x * 1.6 * dt;
        if i.down {
            self.tilt *= 1.0 - 4.0 * dt;
        }
        self.tilt = self.tilt.clamp(-1.2, 1.2);
        // gravity rotated: one complex multiplication
        self.world.gravity = Cx::new(0.0, -1100.0) * Cx::expi(self.tilt);
        self.world.step(dt);

        if (self.world.bodies[self.ball].p - self.goal).abs() < self.goal_r * 0.7 {
            Status::Won
        } else {
            Status::Playing
        }
    }

    fn draw(&self, c: &mut Canvas, v: &View, ov: Overlay) {
        v.ring(c, self.goal, self.goal_r, 2, colour::GOOD);
        pen::crosshair(c, v, self.goal, self.goal_r * 0.6, colour::GOOD);
        v.text_mid(c, self.goal + Cx::new(0.0, -self.goal_r - 16.0), "GOAL", colour::GOOD, 1);

        for (k, b) in self.world.bodies.iter().enumerate() {
            let is_ball = k == self.ball;
            v.ring(c, b.p, b.r, 2, if is_ball { colour::REAL } else { colour::FAINT });
            if is_ball {
                let tip = b.p + Cx::expi(b.angle).scale(b.r);
                v.line(c, b.p, tip, 2, colour::MOD);
            }
        }

        // the gravity dial, always useful
        let dial = Cx::new(120.0, H as f64 - 210.0);
        v.ring(c, dial, 46.0, 1, colour::LINE);
        pen::arrow(c, v, dial, dial + self.world.gravity.unit().scale(44.0), pen::DIM, None);

        // ---- annotations -------------------------------------------------
        let ball = self.world.bodies[self.ball];
        if ov.on(Overlay::ANGLES) {
            pen::angle_arc(c, v, dial, 30.0, -std::f64::consts::FRAC_PI_2,
                           -std::f64::consts::FRAC_PI_2 + self.tilt,
                           &format!("{:+.1} DEG", self.tilt.to_degrees()));
        }
        if ov.on(Overlay::VECTORS) {
            pen::arrow(c, v, ball.p, ball.p + ball.v.scale(0.16), pen::VEC,
                       Some(&format!("{:.0}", ball.v.abs())));
        }
        if ov.on(Overlay::RADII) {
            pen::radius(c, v, ball.p, ball.r, 1.9, &format!("R {:.0}", ball.r));
            pen::radius(c, v, self.goal, self.goal_r, 1.0, &format!("R {:.0}", self.goal_r));
        }
        if ov.on(Overlay::LENGTHS) {
            pen::dimension(c, v, ball.p, self.goal, 30.0,
                           &format!("D {:.0}", (self.goal - ball.p).abs()));
        }
        if ov.on(Overlay::CONTACTS) {
            for k in &self.world.contacts {
                pen::arrow(c, v, k.point, k.point + k.normal.scale(26.0), pen::HIT, None);
                v.disc(c, k.point, 3.0, pen::HIT);
            }
        }
        if ov.on(Overlay::READOUTS) {
            let mut r = Readout::new(v);
            r.row(c, &format!("SPEED {:6.0}   SPIN {:+6.2}   CONTACTS {}",
                              ball.v.abs(), ball.omega, self.world.contacts.len()), colour::SOFT);
            r.row(c, &format!("TILT {:+.2} RAD   G ({:+.0}, {:+.0})   T {:.1}S",
                              self.tilt, self.world.gravity.re, self.world.gravity.im, self.time), colour::SOFT);
        }
    }
}

// ===========================================================================
// 4. FLOW — the fluid, as a game
// ===========================================================================

struct Flow {
    fluid: Fluid,
    tilt: f64,
    basin: (Cx, Cx),
    need: usize,
    time: f64,
}

impl Flow {
    fn new() -> Self {
        let mut s = Flow {
            fluid: Fluid::new(22.0, 11.0),
            tilt: 0.0,
            basin: (Cx::new(880.0, 155.0), Cx::new(1110.0, 300.0)),
            need: 190,
            time: 0.0,
        };
        s.reset();
        s
    }
    fn caught(&self) -> usize {
        let (lo, hi) = self.basin;
        self.fluid
            .p
            .iter()
            .filter(|p| p.re > lo.re && p.re < hi.re && p.im > lo.im && p.im < hi.im)
            .count()
    }
}

impl Stage for Flow {
    fn name(&self) -> &'static str {
        "FLOW"
    }
    fn goal(&self) -> &'static str {
        "TILT THE TANK TO POUR THE FLUID INTO THE BASIN"
    }
    fn controls(&self) -> &'static str {
        "LEFT/RIGHT TILT   DOWN LEVEL   R RESET   ESC MENU"
    }
    fn formula(&self) -> &'static [&'static str] {
        &[
            "DENSITY    RHO = SUM MJ W(|RI - RJ|, H)",
            "PRESSURE   P = K (RHO - RHO0)          CLAMPED AT ZERO",
            "FORCE      F = -SUM MJ (PI+PJ)/(2 RHOJ) GRAD W",
        ]
    }
    fn reset(&mut self) {
        let mut f = Fluid::new(22.0, 11.0);
        f.bounds.push(Bound::new(Cx::new(0.0, 150.0), Cx::new(0.0, 1.0)));
        f.bounds.push(Bound::new(Cx::new(0.0, H as f64 - 90.0), Cx::new(0.0, -1.0)));
        f.bounds.push(Bound::new(Cx::new(60.0, 0.0), Cx::new(1.0, 0.0)));
        f.bounds.push(Bound::new(Cx::new(W as f64 - 60.0, 0.0), Cx::new(-1.0, 0.0)));
        f.tune_stiffness(450.0, 0.02);
        f.block(Cx::new(95.0, 162.0), Cx::new(400.0, 500.0), 11.0);
        self.fluid = f;
        self.tilt = 0.0;
        self.time = 0.0;
    }

    fn update(&mut self, dt: f64, i: &Input) -> Status {
        self.time += dt;
        self.tilt += i.axis_x * 1.1 * dt;
        if i.down {
            self.tilt *= 1.0 - 3.0 * dt;
        }
        self.tilt = self.tilt.clamp(-1.0, 1.0);
        self.fluid.gravity = Cx::new(0.0, -600.0) * Cx::expi(self.tilt);

        let sub = self.fluid.stable_dt();
        let steps = ((dt / sub) as usize).min(30);
        for _ in 0..steps {
            self.fluid.step(sub);
        }

        if self.caught() >= self.need {
            Status::Won
        } else {
            Status::Playing
        }
    }

    fn draw(&self, c: &mut Canvas, v: &View, ov: Overlay) {
        let (lo, hi) = self.basin;
        // basin
        let (a, b) = (v.to_screen(Cx::new(lo.re, hi.im)), v.to_screen(hi));
        let _ = b;
        c.rect(a.0, a.1, (hi.re - lo.re) as i32, (hi.im - lo.im) as i32, colour::GOOD);
        v.text(c, Cx::new(lo.re + 8.0, hi.im + 10.0), "BASIN", colour::GOOD, 1);

        let vmax = self.fluid.max_speed().max(120.0);
        for k in 0..self.fluid.len() {
            let s = (self.fluid.v[k].abs() / vmax).clamp(0.0, 1.0);
            let g = (120.0 + 120.0 * s) as u32;
            v.disc(c, self.fluid.p[k], 4.0, (g / 3 << 16) | (g << 8) | 220);
        }

        let dial = Cx::new(W as f64 - 120.0, 620.0);
        v.ring(c, dial, 44.0, 1, colour::LINE);
        pen::arrow(c, v, dial, dial + self.fluid.gravity.unit().scale(42.0), pen::DIM, None);

        // ---- annotations -------------------------------------------------
        if ov.on(Overlay::ANGLES) {
            pen::angle_arc(c, v, dial, 28.0, -std::f64::consts::FRAC_PI_2,
                           -std::f64::consts::FRAC_PI_2 + self.tilt,
                           &format!("{:+.1} DEG", self.tilt.to_degrees()));
        }
        if ov.on(Overlay::GRID) {
            // the spatial-hash cells the neighbour search actually uses
            let h = self.fluid.h;
            let mut x = 60.0;
            while x < W as f64 - 60.0 {
                v.line(c, Cx::new(x, 150.0), Cx::new(x, H as f64 - 90.0), 1, 0x1B2733);
                x += h;
            }
            let mut y = 150.0;
            while y < H as f64 - 90.0 {
                v.line(c, Cx::new(60.0, y), Cx::new(W as f64 - 60.0, y), 1, 0x1B2733);
                y += h;
            }
        }
        if ov.on(Overlay::RADII) && self.fluid.len() > 0 {
            // the smoothing radius, drawn on one particle
            let p = self.fluid.p[self.fluid.len() / 2];
            v.ring(c, p, self.fluid.h, 1, pen::RAD);
            pen::radius(c, v, p, self.fluid.h, 0.9, &format!("H {:.0}", self.fluid.h));
        }
        if ov.on(Overlay::VECTORS) {
            for k in (0..self.fluid.len()).step_by(18) {
                let vel = self.fluid.v[k];
                if vel.abs() > 60.0 {
                    pen::arrow(c, v, self.fluid.p[k], self.fluid.p[k] + vel.scale(0.08), pen::VEC, None);
                }
            }
        }
        if ov.on(Overlay::LENGTHS) {
            pen::dimension(c, v, Cx::new(lo.re, hi.im), Cx::new(hi.re, hi.im), 26.0,
                           &format!("W {:.0}", hi.re - lo.re));
        }
        if ov.on(Overlay::READOUTS) {
            let mut r = Readout::new(v);
            r.row(c, &format!("CAUGHT {} / {}   PARTICLES {}", self.caught(), self.need, self.fluid.len()), colour::SOFT);
            r.row(c, &format!("COMPRESSION {:+.2}%   MAX SPEED {:.0}   PER CELL {}",
                              self.fluid.compression() * 100.0, self.fluid.max_speed(), self.fluid.worst_bucket()), colour::SOFT);
        }
    }
}

// ===========================================================================
// 5. RIDE — the gears, as a bicycle
// ===========================================================================

/// A bicycle is the two-gear machine from `pulley.rs`, pedalled. The chain is
/// drawn with the same external-tangent construction the pulley rope uses.
struct Ride {
    t: Terrain,
    bike: Bike,
    cam: f64,
    finish: f64,
    time: f64,
}

impl Ride {
    fn new() -> Self {
        let t = Terrain::default();
        let mut r = Ride { t, bike: Bike::new(&t, 220.0), cam: 0.0, finish: 4200.0, time: 0.0 };
        r.reset();
        r
    }
    /// The chain: the two common tangents of chainring and sprocket, computed
    /// exactly as `pulley.rs` computes them for its rope.
    fn chain(&self) -> (Cx, Cx, Cx, Cx, Cx, f64, f64) {
        let c = self.bike.crank_centre();
        let s = self.bike.rear;
        let (rc, rs) = (self.bike.drive.chainring, self.bike.drive.sprocket);
        let d = s - c;
        let dist = d.abs().max(1e-6);
        let phi = d.arg();
        // alpha = acos((rc - rs)/dist); at equal radii it is pi/2 and the
        // offset becomes exactly i * dhat
        let alpha = ((rc - rs) / dist).clamp(-1.0, 1.0).acos();
        let up = Cx::expi(phi + alpha);
        let dn = Cx::expi(phi - alpha);
        (c, c + up.scale(rc), s + up.scale(rs), c + dn.scale(rc), s + dn.scale(rs), rc, rs)
    }
}

impl Stage for Ride {
    fn name(&self) -> &'static str {
        "RIDE"
    }
    fn goal(&self) -> &'static str {
        "PEDAL OVER THE HILLS TO THE FLAG - DROP A GEAR TO CLIMB, SHIFT UP TO FLY"
    }
    fn controls(&self) -> &'static str {
        "SPACE PEDAL  DOWN BRAKE  LEFT/RIGHT GEAR  R RESET  ESC MENU"
    }
    fn formula(&self) -> &'static [&'static str] {
        &[
            "CHAIN    W_WHEEL = W_CRANK * (R_CHAINRING / R_SPROCKET)",
            "GROUND   S = R_WHEEL * THETA_WHEEL            (ARC LENGTH)",
            "FORCE    F = TAU * R_SPROCKET / (R_CHAINRING * R_WHEEL)",
            "HILL     H(X) = SUM A SIN(F X + P)   SLOPE DIFFERENTIATED",
            "LEGS     TWO-BONE IK = TWO CIRCLES INTERSECTING",
        ]
    }
    fn reset(&mut self) {
        self.bike = Bike::new(&self.t, 220.0);
        self.cam = 0.0;
        self.time = 0.0;
    }

    fn update(&mut self, dt: f64, i: &Input) -> Status {
        self.time += dt;
        if i.left {
            self.bike.drive.chainring = (self.bike.drive.chainring - 26.0 * dt).max(12.0);
        }
        if i.right {
            self.bike.drive.chainring = (self.bike.drive.chainring + 26.0 * dt).min(46.0);
        }
        let pedal = if i.action { 1.0 } else { 0.0 };
        let brake = if i.down { 1.0 } else { 0.0 };
        self.bike.step(&self.t, dt, pedal, brake);
        self.cam += (self.bike.rear.re - 330.0 - self.cam) * (4.0 * dt).min(1.0);

        if self.bike.rear.re > self.finish {
            Status::Won
        } else {
            Status::Playing
        }
    }

    fn draw(&self, c: &mut Canvas, v: &View, ov: Overlay) {
        let b = &self.bike;
        let off = Cx::new(-self.cam, 0.0);
        let sh = |p: Cx| p + off;

        // the hill
        let mut prev: Option<Cx> = None;
        let mut x = self.cam - 20.0;
        while x < self.cam + W as f64 + 20.0 {
            let p = sh(self.t.at(x));
            if let Some(q) = prev {
                v.line(c, q, p, 2, 0x3D5A73);
            }
            if (x as i64) % 24 == 0 {
                v.line(c, p, p + Cx::new(-13.0, -20.0), 1, 0x1B2733);
            }
            prev = Some(p);
            x += 6.0;
        }

        // the flag
        let fp = sh(self.t.at(self.finish));
        v.line(c, fp, fp + Cx::new(0.0, 92.0), 2, colour::GOOD);
        v.line(c, fp + Cx::new(0.0, 92.0), fp + Cx::new(44.0, 76.0), 2, colour::GOOD);
        v.line(c, fp + Cx::new(44.0, 76.0), fp + Cx::new(0.0, 60.0), 2, colour::GOOD);

        // wheels, spun by the drivetrain
        let ang = b.drive.wheel_angle;
        for w in [b.rear, b.front] {
            v.ring(c, sh(w), b.wheel_r, 2, colour::INK);
            for k in 0..6 {
                let a = ang + k as f64 * std::f64::consts::PI / 3.0;
                v.line(c, sh(w), sh(w + Cx::expi(a).scale(b.wheel_r * 0.9)), 1, 0x2B3945);
            }
        }

        // frame and drivetrain
        let (crank, ct, st, cb, sb, rc, rs) = self.chain();
        for (a, z) in [
            (b.rear, b.seat),
            (b.front, b.seat),
            (b.rear, crank),
            (b.front, crank),
            (b.front, b.bars()),
        ] {
            v.line(c, sh(a), sh(z), 2, colour::IMAG);
        }
        v.ring(c, sh(crank), rc, 2, colour::REAL);
        v.ring(c, sh(b.rear), rs, 2, colour::REAL);
        v.line(c, sh(ct), sh(st), 2, colour::REAL);
        v.line(c, sh(cb), sh(sb), 2, colour::REAL);

        // the rider — legs follow the pedals, so the animation IS the gearing
        let (pa, pb) = b.drive.pedals(crank, 18.0);
        let up = (b.seat - b.rear).unit();
        let hip = b.seat + up.scale(6.0);
        let shoulder = hip + up.scale(38.0);
        let head = shoulder + up.scale(17.0);
        for foot in [pa, pb] {
            let knee = two_bone_ik(hip, foot, 40.0, 40.0, 1.0);
            v.line(c, sh(hip), sh(knee), 2, colour::MOD);
            v.line(c, sh(knee), sh(foot), 2, colour::MOD);
            v.disc(c, sh(foot), 3.0, colour::MOD);
        }
        v.line(c, sh(hip), sh(shoulder), 2, colour::MOD);
        let hand = b.bars();
        let elbow = two_bone_ik(shoulder, hand, 26.0, 26.0, -1.0);
        v.line(c, sh(shoulder), sh(elbow), 2, colour::MOD);
        v.line(c, sh(elbow), sh(hand), 2, colour::MOD);
        v.ring(c, sh(head), 11.0, 2, colour::MOD);

        // ---- annotations -------------------------------------------------
        if ov.on(Overlay::RADII) {
            pen::radius(c, v, sh(crank), rc, 1.9, &format!("RC {rc:.0}"));
            pen::radius(c, v, sh(b.rear), rs, 4.1, &format!("RS {rs:.0}"));
            pen::radius(c, v, sh(b.front), b.wheel_r, 0.5, &format!("RW {:.0}", b.wheel_r));
        }
        if ov.on(Overlay::ANGLES) {
            let t = b.drive.crank.rem_euclid(2.0 * std::f64::consts::PI);
            pen::angle_arc(c, v, sh(crank), rc * 0.5, 0.0, t, &format!("{t:.2} RAD"));
        }
        if ov.on(Overlay::LENGTHS) {
            pen::dimension(c, v, sh(b.rear), sh(b.front), 58.0, &format!("WB {:.0}", b.wheelbase));
            pen::tag(c, v, sh(head), Cx::new(44.0, 34.0),
                     &format!("{:.0} / {:.0}", b.rear.re, self.finish), pen::DIM);
        }
        if ov.on(Overlay::VECTORS) {
            let tan = self.t.tangent(b.rear.re);
            pen::arrow(c, v, sh(b.rear), sh(b.rear) + tan.scale(72.0), pen::VEC, Some("T"));
        }
        if ov.on(Overlay::CONTACTS) {
            for (w, on) in [(b.rear, b.rear_grounded), (b.front, b.front_grounded)] {
                if on {
                    let g = self.t.at(w.re);
                    v.disc(c, sh(g), 4.0, pen::HIT);
                    pen::arrow(c, v, sh(g), sh(g) + self.t.normal(w.re).scale(32.0), pen::HIT, None);
                }
            }
        }
        if ov.on(Overlay::GRID) {
            let mut gx = (self.cam / 200.0).floor() * 200.0;
            while gx < self.cam + W as f64 {
                let p = sh(Cx::new(gx, 0.0));
                v.line(c, p, p + Cx::new(0.0, H as f64), 1, 0x141D26);
                gx += 200.0;
            }
        }
        if ov.on(Overlay::READOUTS) {
            let mut r = Readout::new(v);
            r.row(c, &format!("GEAR {:.2}   TOP SPEED {:.0}   DRIVE FORCE {:.0}",
                              b.drive.ratio(), b.drive.top_speed(), b.drive.drive_force()), colour::SOFT);
            r.row(c, &format!("X {:.0} / {:.0}   SLOPE {:+.2}   {}",
                              b.rear.re, self.finish, self.t.slope(b.rear.re),
                              if b.rear_grounded { "GRIP" } else { "AIRBORNE" }), colour::SOFT);
        }
    }
}

// ===========================================================================
// the shell
// ===========================================================================

fn stages() -> Vec<Box<dyn Stage>> {
    vec![
        Box::new(Crane::new()),
        Box::new(Cut::new()),
        Box::new(Tilt::new()),
        Box::new(Flow::new()),
        Box::new(Ride::new()),
        Box::new(Playground::new("PLAYGROUND".parse().unwrap())),
    ]
}

const BLURB: [&str; 6] = [
    "PULLEY.RS + RIGID.RS   -   ARC LENGTH BECOMES ROPE",
    "SOFT.RS                -   VERLET AND DISTANCE CONSTRAINTS",
    "RIGID.RS               -   IMPULSES, FRICTION AND SPIN",
    "FLUID.RS + GRID.RS     -   SPH AND A SPATIAL HASH",
    "BIKE.RS                -   THE TWO-GEAR MACHINE, PEDALLED",
    "PLAYGROUND.rs        -    I DO MY LEARNING HERE",
];

fn main() {
    let mut games = stages();
    let mut picked: Option<usize> = None;
    let mut cursor = 0usize;
    let mut ov = Overlay { bits: Overlay::LENGTHS | Overlay::READOUTS };
    let mut status = Status::Playing;
    let v = View::new(W, H);

    if std::env::args().any(|a| a == "--snapshot") {
        let k: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        let secs: f64 = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(1.5);
        let mut c = Canvas::new(W, H);
        c.clear(colour::BG);
        if k == 9 {
            menu(&mut c, &v, cursor);
        } else {
            let n = games.len();
            let g = &mut games[k % n];
            // the snapshot pedals, so RIDE is actually moving
            let mut inp = Input { action: true, ..Input::default() };
            inp.resolve_axes();
            for _ in 0..(secs / DT) as usize {
                g.update(DT, &inp);
            }
            g.draw(&mut c, &v, Overlay::all_on());
            chrome(&mut c, &v, g.name(), g.goal(), g.controls(), Overlay::all_on(), Status::Playing);
            formulas(&mut c, &v, g.formula());
        }
        let out = format!("game{k}.png");
        c.write_png(&out).expect("write failed");
        println!("wrote {out}");
        return;
    }

    let mut canvas = Canvas::new(W, H);
    let mut window = Window::new("RECURSION", W, H, WindowOptions::default())
        .expect("could not open a window");
    window.set_target_fps(60);
    let mut last = Instant::now();
    let mut acc = 0.0f64;
    let mut was_down = false;

    while window.is_open() {
        let now = Instant::now();
        let frame = (now - last).as_secs_f64().min(0.1);
        last = now;

        let pressed: Vec<Key> = window.get_keys_pressed(KeyRepeat::No);
        let mut inp = Input {
            left: window.is_key_down(Key::Left),
            right: window.is_key_down(Key::Right),
            up: window.is_key_down(Key::Up),
            down: window.is_key_down(Key::Down),
            action: window.is_key_down(Key::Space),
            action_pressed: pressed.contains(&Key::Space),
            mouse: window
                .get_mouse_pos(MouseMode::Discard)
                .map(|(x, y)| v.to_world(x as f64, y as f64)),
            mouse_down: window.get_mouse_down(MouseButton::Left),
            mouse_pressed: window.get_mouse_down(MouseButton::Left) && !was_down,
            axis_x: 0.0,
            axis_y: 0.0,
        };
        inp.resolve_axes();
        was_down = inp.mouse_down;
        for (k, bit) in [
            (Key::Key1, Overlay::LENGTHS),
            (Key::Key2, Overlay::ANGLES),
            (Key::Key3, Overlay::RADII),
            (Key::Key4, Overlay::VECTORS),
            (Key::Key5, Overlay::CONTACTS),
            (Key::Key6, Overlay::READOUTS),
            (Key::Key7, Overlay::GRID),
            (Key::Key8, Overlay::FORMULAS),
        ] {
            if pressed.contains(&k) {
                ov.toggle(bit);
            }
        }
        if pressed.contains(&Key::Key0) {
            ov = if ov.count() > 0 { Overlay::none() } else { Overlay::all_on() };
        }
        match picked {
            None => {
                if pressed.contains(&Key::Down) {
                    cursor = (cursor + 1) % BLURB.len();
                }
                if pressed.contains(&Key::Up) {
                    cursor = (cursor + BLURB.len() - 1) % BLURB.len();
                }
                if pressed.contains(&Key::Enter) || pressed.contains(&Key::Space) {
                    games[cursor].reset();
                    status = Status::Playing;
                    picked = Some(cursor);
                }
                if pressed.contains(&Key::Escape) {
                    break;
                }
                canvas.clear(colour::BG);
                println!("the value being printed is {:?}", v);
                menu(&mut canvas, &v, cursor);
            }
            Some(i) => {
                if pressed.contains(&Key::Escape) {
                    picked = None;
                }
                if pressed.contains(&Key::R) {
                    games[i].reset();
                    status = Status::Playing;
                }
                if status == Status::Playing {
                    acc += frame;
                    let mut n = 0;
                    while acc >= DT && n < 4000 {
                        status = games[i].update(DT, &inp);
                        acc -= DT;
                        n += 1;
                        if status != Status::Playing {
                            break;
                        }
                    }
                } else {
                    acc = 0.0;
                }
                canvas.clear(colour::BG);
                games[i].draw(&mut canvas, &v, ov);
                chrome(&mut canvas, &v, games[i].name(), games[i].goal(), games[i].controls(), ov, status);
                if ov.on(Overlay::FORMULAS) {
                    formulas(&mut canvas, &v, games[i].formula());
                }
            }
        }

        window.update_with_buffer(&canvas.buf, W, H).expect("present failed");
    }
}

/// The equation each stage is really solving, right-aligned under the
/// controls so it never fights the scene for space.
fn formulas(c: &mut Canvas, v: &View, lines: &[&str]) {
    let width = lines
        .iter()
        .map(|l| Canvas::text_w(l, 1))
        .max()
        .unwrap_or(0);
    let x = v.w as i32 - width - 28;
    let mut y = 44;
    c.text(v.w as i32 - Canvas::text_w("SOLVING", 1) - 28, y, "SOLVING", 0x6B7987, 1);
    y += 16;
    for l in lines {
        c.text(x, y, l, colour::IMAG, 1);
        y += 15;
    }
}

fn menu(c: &mut Canvas, v: &View, cursor: usize) {
    c.text(90, 70, "RECURSION", colour::INK, 6);
    c.text(92, 124, "FOUR GAMES OVER THE SAME PHYSICS", colour::FAINT, 2);
    println!("loading the menu ...");
    let names = ["CRANE", "CUT", "TILT", "FLOW", "RIDE","PLAYGROUND"];
    for (k, n) in names.iter().enumerate() {
        println!("here the k is {k} and the name is {n} and the cursor is {cursor}");
        let y = 210 + k as i32 * 66;
        let on = k == cursor;
        if on {
            c.fill_rect(76, y - 14, 720, 58, 0x121C25);
            c.rect(76, y - 14, 720, 58, colour::IMAG);
        }
        c.text(100, y, n, if on { colour::IMAG } else { colour::SOFT }, 4);
        c.text(260, y + 8, BLURB[k], colour::FAINT, 1);
    }

    let mut y = v.h as i32 - 150;
    for l in [
        "UP / DOWN  CHOOSE      ENTER  PLAY      ESC  QUIT",
        "",
        "IN A GAME, KEYS 1-8 TOGGLE THE MATHEMATICS OVERLAY:",
        "1 LENGTHS   2 ANGLES   3 RADII   4 VECTORS",
        "5 CONTACTS  6 READOUTS 7 GRID    8 FORMULAS      0 ALL / NONE",
    ] {
        c.text(90, y, l, colour::FAINT, 1);
        y += 17;
    }
}
