//! # Rung 6 — SPH fluid
//!
//!   cargo run --release --features window --bin fluid
//!
//! `fluid.rs` holds the physics and `grid.rs` the neighbour search; this file
//! is input and pixels only.
//!
//! **Release mode matters here.** SPH does real work per particle per step,
//! and a debug build is roughly twenty times slower.

use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Window, WindowOptions};
use recursion1::complex::Cx;
use recursion1::fluid::{Bound, Fluid};
use recursion1::raster::{colour, Canvas};
use std::time::Instant;

const W: usize = 1200;
const H: usize = 780;

const AX: f64 = 60.0;
const AY: f64 = 140.0; // floor clear of the HUD band
const AW: f64 = 1080.0;
const AH: f64 = 490.0;

const H_RADIUS: f64 = 22.0;
const SPACING: f64 = 11.0;

fn sx(p: Cx) -> i32 {
    p.re as i32
}
fn sy(p: Cx) -> i32 {
    (H as f64 - p.im) as i32
}
fn to_world(x: f32, y: f32) -> Cx {
    Cx::new(x as f64, H as f64 - y as f64)
}

fn tank() -> Fluid {
    let mut f = Fluid::new(H_RADIUS, SPACING);
    f.bounds.push(Bound::new(Cx::new(0.0, AY), Cx::new(0.0, 1.0)));
    f.bounds.push(Bound::new(Cx::new(0.0, AY + AH), Cx::new(0.0, -1.0)));
    f.bounds.push(Bound::new(Cx::new(AX, 0.0), Cx::new(1.0, 0.0)));
    f.bounds.push(Bound::new(Cx::new(AX + AW, 0.0), Cx::new(-1.0, 0.0)));
    f.tune_stiffness(AH, 0.02);
    f
}

fn scene_dam() -> Fluid {
    let mut f = tank();
    f.block(
        Cx::new(AX + 15.0, AY + 12.0),
        Cx::new(AX + 330.0, AY + AH - 40.0),
        SPACING,
    );
    f
}

fn scene_pool() -> Fluid {
    let mut f = tank();
    f.block(
        Cx::new(AX + 15.0, AY + 12.0),
        Cx::new(AX + AW - 15.0, AY + 180.0),
        SPACING,
    );
    f
}

fn scene_drop() -> Fluid {
    let mut f = tank();
    f.block(
        Cx::new(AX + 15.0, AY + 12.0),
        Cx::new(AX + AW - 15.0, AY + 120.0),
        SPACING,
    );
    // a blob about to land in the middle of it
    let c = Cx::new(AX + AW * 0.5, AY + 430.0);
    let mut y = -90.0;
    while y <= 90.0 {
        let mut x = -90.0;
        while x <= 90.0 {
            let q = Cx::new(x, y);
            if q.abs() <= 90.0 {
                f.add(c + q, Cx::new(0.0, -140.0));
            }
            x += SPACING;
        }
        y += SPACING;
    }
    f
}

const SCENES: [(&str, fn() -> Fluid); 3] =
    [("DAM BREAK", scene_dam), ("POOL - TILT TO SLOSH", scene_pool), ("DROP", scene_drop)];

/// Blue -> cyan -> white with speed. Fast fluid reads as spray.
fn speed_colour(s: f64, max: f64) -> u32 {
    let t = (s / max.max(1e-9)).clamp(0.0, 1.0).powf(0.6);
    let lerp = |a: f64, b: f64| (a + (b - a) * t) as u32;
    if t < 0.5 {
        let u = t * 2.0;
        let l = |a: f64, b: f64| (a + (b - a) * u) as u32;
        (l(30.0, 60.0) << 16) | (l(90.0, 190.0) << 8) | l(200.0, 220.0)
    } else {
        (lerp(60.0, 235.0) << 16) | (lerp(190.0, 245.0) << 8) | lerp(220.0, 255.0)
    }
}

fn main() {
    let mut scene = 0usize;
    let mut fluid = SCENES[0].1();
    let mut paused = false;
    let mut tilt = 0.0f64;

    if std::env::args().any(|a| a == "--snapshot") {
        let which: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        let secs: f64 = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(1.5);
        let mut f = SCENES[which % SCENES.len()].1();
        let dt = f.stable_dt();
        for _ in 0..(secs / dt) as usize {
            f.step(dt);
        }
        let mut c = Canvas::new(W, H);
        draw(&mut c, &f, SCENES[which % SCENES.len()].0, false, 0.0, 60.0, None);
        let out = format!("fluid{which}.png");
        c.write_png(&out).expect("could not write snapshot");
        println!("wrote {out}");
        return;
    }

    let mut canvas = Canvas::new(W, H);
    let mut window = Window::new(
        "Recursion I - SPH fluid (drag to stir, arrows to tilt)",
        W,
        H,
        WindowOptions::default(),
    )
    .expect("could not open a window");
    window.set_target_fps(60);

    let mut last = Instant::now();
    let (mut fps_t, mut frames, mut fps) = (Instant::now(), 0u32, 0.0f64);
    let mut prev_mouse: Option<Cx> = None;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let now = Instant::now();
        let frame = (now - last).as_secs_f64().min(0.1);
        last = now;

        for k in window.get_keys_pressed(KeyRepeat::No) {
            match k {
                Key::Space => paused = !paused,
                Key::R => fluid = SCENES[scene].1(),
                Key::Tab => {
                    scene = (scene + 1) % SCENES.len();
                    fluid = SCENES[scene].1();
                    tilt = 0.0;
                }
                Key::Key1 => fluid.viscosity = 0.0,
                Key::Key2 => fluid.viscosity = 60.0,
                Key::Key3 => fluid.viscosity = 250.0,
                Key::Key4 => fluid.viscosity = 900.0,
                _ => {}
            }
        }

        let rate = 1.6 * frame;
        if window.is_key_down(Key::Left) {
            tilt -= rate;
        }
        if window.is_key_down(Key::Right) {
            tilt += rate;
        }
        if window.is_key_down(Key::Down) {
            tilt = 0.0;
        }
        fluid.gravity = Cx::new(0.0, -600.0) * Cx::expi(tilt);

        // ---- stirring: shove nearby particles along the mouse's motion -----
        let mouse = window.get_mouse_pos(MouseMode::Discard).map(|(x, y)| to_world(x, y));
        if let (Some(m), Some(pm)) = (mouse, prev_mouse) {
            if window.get_mouse_down(MouseButton::Left) {
                let drag = (m - pm).scale(1.0 / frame.max(1e-3));
                for i in 0..fluid.len() {
                    let d = (fluid.p[i] - m).abs();
                    if d < 70.0 {
                        let w = 1.0 - d / 70.0;
                        fluid.v[i] = fluid.v[i] + drag.scale(0.35 * w);
                    }
                }
            }
        }
        prev_mouse = mouse;

        if !paused {
            // Fixed physics step from the CFL limit, spent in whole chunks.
            let dt = fluid.stable_dt();
            let steps = ((frame / dt) as usize).min(24);
            for _ in 0..steps {
                fluid.step(dt);
            }
        }

        frames += 1;
        if fps_t.elapsed().as_secs_f64() >= 0.5 {
            fps = frames as f64 / fps_t.elapsed().as_secs_f64();
            frames = 0;
            fps_t = Instant::now();
        }

        draw(&mut canvas, &fluid, SCENES[scene].0, paused, tilt, fps, mouse);
        window.update_with_buffer(&canvas.buf, W, H).expect("present failed");
    }
}

fn draw(c: &mut Canvas, f: &Fluid, scene: &str, paused: bool, tilt: f64, fps: f64, mouse: Option<Cx>) {
    c.clear(colour::BG);
    c.rect(
        sx(Cx::new(AX, 0.0)),
        sy(Cx::new(0.0, AY + AH)),
        AW as i32,
        AH as i32,
        colour::LINE,
    );

    let vmax = f.max_speed().max(120.0);
    for i in 0..f.len() {
        c.disc(sx(f.p[i]), sy(f.p[i]), 4, speed_colour(f.v[i].abs(), vmax));
    }

    if let Some(m) = mouse {
        c.circle(sx(m), sy(m), 70, colour::FAINT);
    }

    // gravity indicator
    let anchor = Cx::new(AX + AW - 80.0, AY + AH - 80.0);
    let tip = anchor + f.gravity.unit().scale(52.0);
    c.circle(sx(anchor), sy(anchor), 54, colour::LINE);
    c.thick_line(sx(anchor), sy(anchor), sx(tip), sy(tip), 3, colour::REAL);

    c.text(40, 18, "RECURSION I - SPH FLUID", colour::INK, 3);
    let help = "DRAG STIR   ARROWS TILT G   DOWN RESET G   TAB SCENE   1-4 VISCOSITY   R RESET   SPACE PAUSE";
    c.text(W as i32 - Canvas::text_w(help, 1) - 40, 22, help, colour::FAINT, 1);

    let mut y = H as i32 - 96;
    let mut row = |c: &mut Canvas, s: String| {
        c.text(40, y, &s, colour::SOFT, 2);
        y += 22;
    };
    row(c, format!("{}   {} PARTICLES   {} PER CELL", scene, f.len(), f.worst_bucket()));
    row(
        c,
        format!(
            "COMPRESSION {:+6.2}%   MEAN RHO {:.4}   REST {:.4}",
            f.compression() * 100.0,
            f.mean_density(),
            f.rest_density
        ),
    );
    row(
        c,
        format!(
            "VISC {:4.0}   DT {:.5}   MAX SPEED {:6.0}   {:.0} FPS{}",
            f.viscosity,
            f.stable_dt(),
            f.max_speed(),
            fps,
            if paused { "  [PAUSED]" } else { "" }
        ),
    );
    let _ = tilt;
}
