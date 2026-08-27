//! # Iteration 3 — hands on the crank
//!
//!   cargo run --release --features play --bin play
//!
//! A window, a keyboard, and the same `dynamics.rs` that `cargo test` checks.
//! Every pixel is written by the CPU (see `raster.rs`) — no GPU, no shader,
//! no graphics API.
//!
//! ## The two ideas worth reading
//!
//! **1. Input is one term.** Making this interactive required adding
//! `input_torque` to the equation of motion and nothing else:
//!
//! ```text
//! M_eff * theta_ddot = gravity - k*theta - c*theta_dot + INPUT
//! ```
//!
//! **2. The physics step is decoupled from the frame rate.** A naive loop
//! steps the simulation by however long the last frame took — so the machine
//! behaves differently on a fast machine than a slow one, and a single hitch
//! can tunnel a weight straight through its end stop. Instead we accumulate
//! real time and spend it in fixed `DT` chunks:
//!
//! ```text
//! accumulator += frame_time
//! while accumulator >= DT { sim.step(DT); accumulator -= DT }
//! ```
//!
//! The simulation now sees an identical `DT` every step regardless of what
//! the renderer is doing. This is the standard fixed-timestep loop, and it is
//! the difference between a demo and something you can trust.

use minifb::{Key, KeyRepeat, Window, WindowOptions};
use recursion1::complex::Cx;
use recursion1::dynamics::{Integrator, Physics, Sim};
use recursion1::pulley::Config;
use recursion1::raster::{colour, Canvas};
use std::f64::consts::PI;
use std::time::Instant;

const W: usize = 1200;
const H: usize = 780;
const ORX: f64 = 600.0; // where the complex origin sits on screen
const ORY: f64 = 250.0;
const DT: f64 = 1.0 / 480.0; // fixed physics step — never varies
const CRANK: f64 = 3000.0; // torque applied while a key is held

fn sx(p: Cx) -> i32 {
    (ORX + p.re) as i32
}
fn sy(p: Cx) -> i32 {
    (ORY - p.im) as i32 // screen y grows downward, world y grows up
}

fn arc(c: Cx, r: f64, a0: f64, a1: f64, n: usize) -> Vec<(i32, i32)> {
    (0..=n)
        .map(|k| {
            let t = a0 + (a1 - a0) * k as f64 / n as f64;
            let p = c + Cx::expi(t).scale(r);
            (sx(p), sy(p))
        })
        .collect()
}

struct Preset {
    name: &'static str,
    cfg: Config,
    phys: Physics,
    theta0: f64,
}

fn presets() -> Vec<Preset> {
    let base = Config::default();
    let d = Physics::default();
    vec![
        Preset {
            name: "ATWOOD + BOUNCE",
            cfg: Config { m1: 2.0, m2: 5.0, ..base },
            phys: Physics { spring_k: 0.0, damping_c: 0.0, restitution: 0.55, ..d },
            theta0: 0.0,
        },
        Preset {
            name: "UNDAMPED SPRING",
            cfg: Config { m1: 3.0, m2: 1.0, ..base },
            phys: Physics { spring_k: 2.0e6, damping_c: 0.0, restitution: 0.0, ..d },
            theta0: 0.9,
        },
        Preset {
            name: "UNDERDAMPED",
            cfg: Config { m1: 3.0, m2: 1.0, ..base },
            phys: Physics { spring_k: 2.0e6, damping_c: 120_000.0, restitution: 0.0, ..d },
            theta0: 0.9,
        },
        Preset {
            name: "OVERDAMPED",
            cfg: Config { m1: 3.0, m2: 1.0, ..base },
            phys: Physics { spring_k: 2.0e6, damping_c: 900_000.0, restitution: 0.0, ..d },
            theta0: 0.9,
        },
        Preset {
            name: "BALANCED - ALL YOURS",
            cfg: Config { m1: 3.0, m2: 3.0, ..base },
            phys: Physics { spring_k: 0.0, damping_c: 20_000.0, restitution: 0.4, ..d },
            theta0: 0.0,
        },
    ]
}

fn main() {
    let sets = presets();

    // `--snapshot` renders one frame to PNG and exits. Works with no display
    // at all, which makes the renderer testable on a headless machine.
    if std::env::args().any(|a| a == "--snapshot") {
        let which: usize = std::env::args()
            .nth(2)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let p = &sets[which.min(sets.len() - 1)];
        let mut sim = Sim::new(p.cfg, p.phys, p.theta0, 0.0);
        for _ in 0..(1.4 / DT) as usize {
            sim.step(DT, Integrator::Verlet); // let it move off the initial pose
        }
        let mut canvas = Canvas::new(W, H);
        draw(&mut canvas, &sim, Integrator::Verlet, p.name, false, 0.6, 3.0, 12.0, 400.0, 60.0);
        let out = format!("frame{which}.png");
        canvas.write_png(&out).expect("could not write the snapshot");
        println!("wrote {out}");
        return;
    }

    let mut which = 0usize;
    let mut sim = Sim::new(sets[0].cfg, sets[0].phys, sets[0].theta0, 0.0);
    let mut integrator = Integrator::Verlet;
    let mut paused = false;
    let mut time_scale = 3.0f64;
    let mut drift_peak = 0.0f64;
    let mut ke_peak = 0.0f64;
    let energy0 = sim.energy();
    let mut energy0 = energy0;

    let mut canvas = Canvas::new(W, H);
    let mut window = Window::new(
        "Recursion I — hold LEFT / RIGHT to crank",
        W,
        H,
        WindowOptions::default(),
    )
    .expect("could not open a window");
    window.set_target_fps(60);

    let mut acc = 0.0f64;
    let mut last = Instant::now();
    let mut fps_t = Instant::now();
    let mut frames = 0u32;
    let mut fps = 0.0f64;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // ---- real time elapsed since the previous frame ------------------
        let now = Instant::now();
        // Clamped: after a breakpoint or a window drag, `frame` could be
        // seconds long, and we must not try to simulate all of it at once.
        let frame = (now - last).as_secs_f64().min(0.25);
        last = now;

        // ---- discrete key presses ----------------------------------------
        for k in window.get_keys_pressed(KeyRepeat::No) {
            match k {
                Key::Space => paused = !paused,
                Key::R => {
                    sim = Sim::new(sets[which].cfg, sets[which].phys, sets[which].theta0, 0.0);
                    energy0 = sim.energy();
                    drift_peak = 0.0;
                    ke_peak = 0.0;
                }
                Key::Tab => {
                    which = (which + 1) % sets.len();
                    sim = Sim::new(sets[which].cfg, sets[which].phys, sets[which].theta0, 0.0);
                    energy0 = sim.energy();
                    drift_peak = 0.0;
                    ke_peak = 0.0;
                }
                Key::Key1 => integrator = Integrator::ExplicitEuler,
                Key::Key2 => integrator = Integrator::SemiImplicitEuler,
                Key::Key3 => integrator = Integrator::Verlet,
                Key::Key4 => integrator = Integrator::Rk4,
                Key::Minus => time_scale = (time_scale / 1.5).max(0.1),
                Key::Equal => time_scale = (time_scale * 1.5).min(30.0),
                _ => {}
            }
        }

        // ---- held keys become a torque ------------------------------------
        let mut drive = 0.0;
        if window.is_key_down(Key::Left) || window.is_key_down(Key::A) {
            drive -= 1.0;
        }
        if window.is_key_down(Key::Right) || window.is_key_down(Key::D) {
            drive += 1.0;
        }
        sim.input_torque = drive * CRANK;

        // ---- FIXED TIMESTEP: the whole point ------------------------------
        if !paused {
            acc += frame * time_scale;
            let mut guard = 0;
            while acc >= DT && guard < 40_000 {
                sim.step(DT, integrator);
                acc -= DT;
                guard += 1;
            }
            let d = (sim.energy() - energy0).abs();
            if d > drift_peak {
                drift_peak = d;
            }
            let ke = sim.kinetic(sim.omega);
            if ke > ke_peak {
                ke_peak = ke;
            }
        }

        frames += 1;
        if fps_t.elapsed().as_secs_f64() >= 0.5 {
            fps = frames as f64 / fps_t.elapsed().as_secs_f64();
            frames = 0;
            fps_t = Instant::now();
        }

        draw(
            &mut canvas, &sim, integrator, sets[which].name, paused, drive, time_scale,
            drift_peak, ke_peak, fps,
        );
        window
            .update_with_buffer(&canvas.buf, W, H)
            .expect("could not present the frame");
    }
}

#[allow(clippy::too_many_arguments)]
fn draw(
    c: &mut Canvas,
    sim: &Sim,
    integ: Integrator,
    preset: &str,
    paused: bool,
    drive: f64,
    time_scale: f64,
    drift: f64,
    ke_peak: f64,
    fps: f64,
) {
    let cfg = sim.cfg;
    let st = cfg.solve(sim.theta);
    c.clear(colour::BG);

    // ---- grid and the two axes -------------------------------------------
    let mut gx = ORX as i32 % 60;
    while gx < c.w {
        c.line(gx, 0, gx, c.h, colour::GRID);
        gx += 60;
    }
    let mut gy = ORY as i32 % 60;
    while gy < c.h {
        c.line(0, gy, c.w, gy, colour::GRID);
        gy += 60;
    }
    c.line(0, ORY as i32, c.w, ORY as i32, colour::REAL); // real axis
    c.line(ORX as i32, 0, ORX as i32, c.h, colour::IMAG); // imaginary axis

    // ---- the rope, in its five pieces ------------------------------------
    c.thick_line(sx(st.w1), sy(st.w1), sx(st.pa), sy(st.pa), 3, colour::ROPE);
    c.polyline(&arc(st.a, cfg.r_a, PI, st.tangent_angle, 40), 3, colour::ROPE);
    c.thick_line(sx(st.ta), sy(st.ta), sx(st.tb), sy(st.tb), 3, colour::ROPE);
    c.polyline(&arc(st.b, cfg.r_b, st.tangent_angle, 0.0, 40), 3, colour::ROPE);
    c.thick_line(sx(st.pb), sy(st.pb), sx(st.w2), sy(st.w2), 3, colour::ROPE);

    // ---- gears: rim, then teeth as rotated roots of unity ------------------
    gear(c, st.a, cfg.r_a, st.theta, cfg.teeth);
    let nb = ((cfg.teeth as f64 * cfg.r_b / cfg.r_a).round() as usize).max(6);
    gear(c, st.b, cfg.r_b, st.gear_b_angle, nb);

    // tangent points, and the radius that proves the run is perpendicular
    c.thick_line(sx(st.a), sy(st.a), sx(st.ta), sy(st.ta), 1, colour::IMAG);
    c.disc(sx(st.ta), sy(st.ta), 4, colour::IMAG);
    c.disc(sx(st.tb), sy(st.tb), 4, colour::IMAG);

    // ---- the weights ------------------------------------------------------
    weight(c, st.w1, cfg.m1, colour::REAL, "M1");
    weight(c, st.w2, cfg.m2, colour::MOD, "M2");

    // ---- crank indicator: which way you are pushing ------------------------
    let bx = 40;
    let by = c.h - 150;
    c.rect(bx, by, 180, 16, colour::LINE);
    let mid = bx + 90;
    if drive != 0.0 {
        let len = (drive * 88.0) as i32;
        let (x0, wdt) = if len >= 0 { (mid, len) } else { (mid + len, -len) };
        c.fill_rect(x0, by + 1, wdt, 15, colour::MOD);
    }
    c.line(mid, by, mid, by + 16, colour::SOFT);
    c.text(bx, by - 14, "CRANK  (HOLD LEFT / RIGHT)", colour::FAINT, 1);

    // ---- readouts ---------------------------------------------------------
    let l = 22;
    let mut y = c.h - 120;
    let mut row = |c: &mut Canvas, s: String| {
        c.text(40, y, &s, colour::SOFT, 2);
        y += l;
    };
    row(c, format!("T {:7.2}S   THETA {:+7.3}   OMEGA {:+7.3}", sim.t, sim.theta, sim.omega));
    row(c, format!("H1 {:7.1}    H2 {:9.1}    ENERGY {:+10.1}", st.h1, st.h2, sim.energy()));
    let drift_txt = if ke_peak > 1e-9 {
        format!("{:.2}%", 100.0 * drift / ke_peak)
    } else {
        "-".into()
    };
    row(
        c,
        format!(
            "{}   DRIFT {}   M/EFF {:.0}",
            integ.name().to_uppercase(),
            drift_txt,
            sim.m_eff()
        ),
    );

    // lambda: the complex root, when the machine can oscillate at all
    let lam = match sim.lambda() {
        Some(l) => format!("LAMBDA {:+.3} {:+.3}I   DECAY X ROTATION", l.re, l.im),
        None if sim.phys.spring_k <= 0.0 => "LAMBDA -   NO SPRING, NO OSCILLATION".to_string(),
        None => "LAMBDA REAL   ROTATION GONE (OVERDAMPED)".to_string(),
    };
    c.text(40, y, &lam, colour::MOD, 2);

    // ---- header -----------------------------------------------------------
    c.text(40, 26, "RECURSION I", colour::INK, 3);
    c.text(
        40,
        56,
        &format!("{}   {:.0}X TIME   {:.0} FPS{}", preset, time_scale, fps,
                 if paused { "   [PAUSED]" } else { "" }),
        if paused { colour::WARN } else { colour::FAINT },
        1,
    );
    let help = "SPACE PAUSE   R RESET   TAB PRESET   1-4 INTEGRATOR   -/= TIME   ESC QUIT";
    c.text(c.w - Canvas::text_w(help, 1) - 40, 26, help, colour::FAINT, 1);
}

/// Rim, hub, teeth. The teeth are `centre + r*e^(i(angle + 2*pi*k/N))` — the
/// same roots-of-unity call the SVG renderer uses, from `pulley.rs`.
fn gear(c: &mut Canvas, centre: Cx, r: f64, angle: f64, n: usize) {
    c.ring(sx(centre), sy(centre), r as i32, 2, colour::INK);
    c.ring(sx(centre), sy(centre), (r * 0.16) as i32, 2, colour::INK);
    for k in 0..n {
        let t = angle + 2.0 * PI * k as f64 / n as f64;
        let e = Cx::expi(t);
        let p0 = centre + e.scale(r * 0.98);
        let p1 = centre + e.scale(r * 1.16);
        // tooth 0 is highlighted, so the rotation is unmistakable
        let (col, w) = if k == 0 { (colour::MOD, 4) } else { (colour::INK, 3) };
        c.thick_line(sx(p0), sy(p0), sx(p1), sy(p1), w, col);
    }
    c.disc(sx(centre), sy(centre), 4, colour::INK);
}

fn weight(c: &mut Canvas, p: Cx, mass: f64, col: u32, tag: &str) {
    let label = format!("{tag} {mass:.1}");
    // Size with the mass, but never narrower than the label it has to hold.
    let w = ((30.0 + mass * 4.2) as i32).max(Canvas::text_w(&label, 1) + 10);
    let h = (22.0 + mass * 2.6) as i32;
    let (x, y) = (sx(p) - w / 2, sy(p));
    c.fill_rect(x, y, w, h, colour::BG);
    c.rect(x, y, w, h, col);
    c.text(sx(p) - Canvas::text_w(&label, 1) / 2, y + h / 2 - 3, &label, col, 1);
}
