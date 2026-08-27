//! # Rung 5 — cloth, rope and soft bodies
//!
//!   cargo run --release --features window --bin cloth
//!
//! The physics is all in `soft.rs`; this file only maps input to the world
//! and the world to pixels.
//!
//! **Left-drag** grabs the nearest particle. **Right-drag** is a knife.
//! Arrow keys tilt gravity, W cycles the wind.

use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Window, WindowOptions};
use recursion1::complex::Cx;
use recursion1::raster::{colour, Canvas};
use recursion1::soft::{Fabric, Obstacle, Particle};
use std::time::Instant;

const W: usize = 1200;
const H: usize = 780;
const DT: f64 = 1.0 / 600.0;

const AX: f64 = 60.0;
const AY: f64 = 120.0;
const AW: f64 = 1080.0;
const AH: f64 = 520.0;

fn sx(p: Cx) -> i32 {
    p.re as i32
}
fn sy(p: Cx) -> i32 {
    (H as f64 - p.im) as i32
}
/// Screen back to world, for the mouse.
fn to_world(x: f32, y: f32) -> Cx {
    Cx::new(x as f64, H as f64 - y as f64)
}

fn bounds(f: &mut Fabric) {
    f.bounds = Some((Cx::new(AX, AY), Cx::new(AX + AW, AY + AH)));
}

fn scene_cloth() -> Fabric {
    let mut f = Fabric::cloth(Cx::new(330.0, AY + AH - 30.0), 26, 18, 22.0, true, true, false);
    f.set_tear(0.35);
    f.iterations = 8;
    bounds(&mut f);
    f
}

fn scene_flag() -> Fabric {
    let mut f = Fabric::cloth(Cx::new(240.0, AY + AH - 40.0), 24, 15, 24.0, false, true, true);
    // pin the whole left edge to a flagpole
    for r in 0..15 {
        let i = r * 24;
        f.particles[i] = Particle::pinned(f.particles[i].p);
    }
    f.wind = Cx::new(2600.0, 260.0);
    f.iterations = 8;
    bounds(&mut f);
    f
}

fn scene_drape() -> Fabric {
    // A ROPE, not a sheet. A two-dimensional sheet cannot drape: draping is
    // fabric buckling out of plane, and a flat world has no out-of-plane to
    // buckle into. Fully triangulate a 2-D grid and you get a rigid truss that
    // tumbles like a dinner tray. The honest 2-D analogue of cloth falling
    // over a sphere is a slack rope falling over one.
    let mut f = Fabric::rope(
        Cx::new(AX + 70.0, AY + AH - 60.0),
        Cx::new(AX + AW - 70.0, AY + AH - 60.0),
        70,
        true,
        true,
    );
    // 55% more rope than the gap needs, so it sags deeply onto the obstacles
    for l in &mut f.links {
        l.rest *= 1.55;
        l.stiffness = 0.95;
    }
    f.obstacles.push(Obstacle { c: Cx::new(500.0, AY + 150.0), r: 130.0 });
    f.obstacles.push(Obstacle { c: Cx::new(830.0, AY + 100.0), r: 80.0 });
    f.iterations = 14;
    bounds(&mut f);
    f
}

fn scene_rope_and_blobs() -> Fabric {
    // A slack bridge and some soft blobs. NOTE: there is no self-collision,
    // so the blobs fall straight THROUGH the rope and land on the floor. That
    // is a real limitation of soft.rs, not a bug in the scene.
    let mut f = Fabric::rope(
        Cx::new(AX + 60.0, AY + 300.0),
        Cx::new(AX + AW - 60.0, AY + 300.0),
        44,
        true,
        true,
    );
    for l in &mut f.links {
        l.stiffness = 0.9;
    }
    for (k, cx) in [340.0, 620.0, 880.0].iter().enumerate() {
        let blob = Fabric::blob(Cx::new(*cx, AY + AH - 70.0 - k as f64 * 40.0), 52.0, 14);
        let base = f.particles.len();
        f.particles.extend(blob.particles);
        for mut l in blob.links {
            l.a += base;
            l.b += base;
            f.links.push(l);
        }
    }
    f.iterations = 10;
    bounds(&mut f);
    f
}

const SCENES: [(&str, fn() -> Fabric); 4] = [
    ("CLOTH - TEARS AT 35% STRAIN", scene_cloth),
    ("FLAG IN WIND", scene_flag),
    ("SLACK ROPE OVER OBSTACLES", scene_drape),
    ("SOFT BLOBS + ROPE - NO SELF-COLLISION", scene_rope_and_blobs),
];

/// Blend two colours, `t` in 0..=1.
fn mix(a: u32, b: u32, t: f64) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let ch = |sh: u32| {
        let x = ((a >> sh) & 0xFF) as f64;
        let y = ((b >> sh) & 0xFF) as f64;
        ((x + (y - x) * t) as u32) << sh
    };
    ch(16) | ch(8) | ch(0)
}

fn main() {
    let mut scene = 0usize;
    let mut fabric = SCENES[0].1();
    let mut paused = false;
    let mut tilt = 0.0f64;
    let mut wind_on = true;
    let mut grabbed: Option<usize> = None;

    if std::env::args().any(|a| a == "--snapshot") {
        let which: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        let secs: f64 = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(3.0);
        let mut f = SCENES[which % SCENES.len()].1();
        for _ in 0..(secs / DT) as usize {
            f.step(DT);
        }
        let mut c = Canvas::new(W, H);
        draw(&mut c, &f, SCENES[which % SCENES.len()].0, false, 0.0, 60.0, None);
        let out = format!("cloth{which}.png");
        c.write_png(&out).expect("could not write snapshot");
        println!("wrote {out}");
        return;
    }

    let mut canvas = Canvas::new(W, H);
    let mut window = Window::new(
        "Recursion I - cloth (drag to grab, right-drag to cut)",
        W,
        H,
        WindowOptions::default(),
    )
    .expect("could not open a window");
    window.set_target_fps(60);

    let mut acc = 0.0f64;
    let mut last = Instant::now();
    let (mut fps_t, mut frames, mut fps) = (Instant::now(), 0u32, 0.0f64);

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let now = Instant::now();
        let frame = (now - last).as_secs_f64().min(0.25);
        last = now;

        for k in window.get_keys_pressed(KeyRepeat::No) {
            match k {
                Key::Space => paused = !paused,
                Key::R => fabric = SCENES[scene].1(),
                Key::Tab => {
                    scene = (scene + 1) % SCENES.len();
                    fabric = SCENES[scene].1();
                    tilt = 0.0;
                }
                Key::W => wind_on = !wind_on,
                Key::T => {
                    let on = fabric.links.iter().any(|l| l.tear.is_finite());
                    fabric.set_tear(if on { f64::INFINITY } else { 0.35 });
                }
                Key::Key1 => fabric.iterations = 1,
                Key::Key2 => fabric.iterations = 4,
                Key::Key3 => fabric.iterations = 10,
                Key::Key4 => fabric.iterations = 30,
                _ => {}
            }
        }

        let rate = 2.0 * frame;
        if window.is_key_down(Key::Left) {
            tilt -= rate;
        }
        if window.is_key_down(Key::Right) {
            tilt += rate;
        }
        if window.is_key_down(Key::Down) {
            tilt = 0.0;
        }
        fabric.gravity = Cx::new(0.0, -1200.0) * Cx::expi(tilt);

        let base_wind = if scene == 1 { Cx::new(2600.0, 260.0) } else { Cx::new(900.0, 0.0) };
        fabric.wind = if wind_on { base_wind } else { Cx::ZERO };

        // ---- mouse: grab and cut -----------------------------------------
        let mouse = window.get_mouse_pos(MouseMode::Discard).map(|(x, y)| to_world(x, y));
        if let Some(m) = mouse {
            if window.get_mouse_down(MouseButton::Left) {
                if grabbed.is_none() {
                    grabbed = fabric.nearest(m, 40.0);
                }
                if let Some(i) = grabbed {
                    // Move it and let Verlet infer the velocity - dragging a
                    // particle really does throw it.
                    fabric.particles[i].p = m;
                }
            } else {
                grabbed = None;
            }
            if window.get_mouse_down(MouseButton::Right) {
                fabric.cut(m, 26.0);
            }
        } else {
            grabbed = None;
        }

        if !paused {
            acc += frame;
            let mut guard = 0;
            while acc >= DT && guard < 4000 {
                fabric.step(DT);
                acc -= DT;
                guard += 1;
            }
        }

        frames += 1;
        if fps_t.elapsed().as_secs_f64() >= 0.5 {
            fps = frames as f64 / fps_t.elapsed().as_secs_f64();
            frames = 0;
            fps_t = Instant::now();
        }

        draw(&mut canvas, &fabric, SCENES[scene].0, paused, tilt, fps, mouse);
        window.update_with_buffer(&canvas.buf, W, H).expect("present failed");
    }
}

fn draw(c: &mut Canvas, f: &Fabric, scene: &str, paused: bool, tilt: f64, fps: f64, mouse: Option<Cx>) {
    c.clear(colour::BG);
    c.rect(
        sx(Cx::new(AX, 0.0)),
        sy(Cx::new(0.0, AY + AH)),
        AW as i32,
        AH as i32,
        colour::LINE,
    );

    for o in &f.obstacles {
        c.ring(sx(o.c), sy(o.c), o.r as i32, 2, colour::FAINT);
    }

    // links, tinted by how far they are being stretched
    for l in &f.links {
        if !l.alive {
            continue;
        }
        let (a, b) = (f.particles[l.a].p, f.particles[l.b].p);
        let strain = (((b - a).abs() - l.rest) / l.rest).abs();
        let col = mix(colour::IMAG, colour::WARN, strain / 0.25);
        c.line(sx(a), sy(a), sx(b), sy(b), col);
    }

    // pinned particles only - drawing every node buries the fabric
    for q in &f.particles {
        if q.w == 0.0 {
            c.disc(sx(q.p), sy(q.p), 3, colour::MOD);
        }
    }

    if let Some(m) = mouse {
        c.circle(sx(m), sy(m), 26, colour::FAINT);
    }

    c.text(40, 20, "RECURSION I - CLOTH", colour::INK, 3);
    let help = "DRAG GRAB   RIGHT-DRAG CUT   ARROWS TILT G   W WIND   T TEAR   TAB SCENE   1-4 ITER   SPACE PAUSE";
    c.text(W as i32 - Canvas::text_w(help, 1) - 40, 24, help, colour::FAINT, 1);

    let mut y = H as i32 - 96;
    let mut row = |c: &mut Canvas, s: String| {
        c.text(40, y, &s, colour::SOFT, 2);
        y += 22;
    };
    row(c, format!("{}   {} NODES   {} LINKS", scene, f.particles.len(), f.live_links()));
    row(
        c,
        format!(
            "MAX STRAIN {:5.3}   {} ITER   ENERGY {:10.0}",
            f.max_strain(),
            f.iterations,
            f.energy(DT)
        ),
    );
    row(
        c,
        format!(
            "TILT {:+5.2} RAD   WIND {:5.0}   {:.0} FPS{}",
            tilt,
            f.wind.abs(),
            fps,
            if paused { "   [PAUSED]" } else { "" }
        ),
    );
}
