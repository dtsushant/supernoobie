//! # Rung 4 — a rigid-body sandbox
//!
//!   cargo run --release --features play --bin bodies
//!
//! All the physics is in `rigid.rs` and none of it is here. This file only
//! turns key presses into world changes and world state into pixels — which
//! is the point of keeping the two apart.
//!
//! Arrow keys **tilt gravity**, so you are steering the whole arena rather
//! than any one body.

use minifb::{Key, KeyRepeat, Window, WindowOptions};
use recursion1::complex::Cx;
use recursion1::raster::{colour, Canvas};
use recursion1::rigid::{Body, Wall, World};
use std::time::Instant;

const W: usize = 1200;
const H: usize = 780;
const DT: f64 = 1.0 / 600.0;
const G: f64 = 1400.0;

// arena, in world units; +y is up, so the screen flip happens in `sy`
const AX: f64 = 60.0;
const AY: f64 = 120.0; // floor sits above the HUD band
const AW: f64 = 1080.0;
const AH: f64 = 510.0;

fn sx(p: Cx) -> i32 {
    p.re as i32
}
fn sy(p: Cx) -> i32 {
    (H as f64 - p.im) as i32
}

/// Deterministic noise, so a scene can be reproduced exactly. Same SplitMix64
/// idea the lending crate uses for its forests.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn f(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (self.next() >> 11) as f64 / (1u64 << 53) as f64 * (hi - lo)
    }
}

fn arena() -> World {
    let mut w = World { gravity: Cx::new(0.0, -G), iterations: 12, ..World::default() };
    // four inward-facing walls
    w.walls.push(Wall::new(Cx::new(0.0, AY), Cx::new(0.0, 1.0))); // floor
    w.walls.push(Wall::new(Cx::new(0.0, AY + AH), Cx::new(0.0, -1.0))); // ceiling
    w.walls.push(Wall::new(Cx::new(AX, 0.0), Cx::new(1.0, 0.0))); // left
    w.walls.push(Wall::new(Cx::new(AX + AW, 0.0), Cx::new(-1.0, 0.0))); // right
    w
}

fn scene_pile(w: &mut World, rng: &mut Rng) {
    for _ in 0..26 {
        let r = rng.f(12.0, 30.0);
        let mut b = Body::disc(
            Cx::new(rng.f(AX + 60.0, AX + AW - 60.0), rng.f(AY + 260.0, AY + AH - 40.0)),
            r,
            1.0,
        );
        b.v = Cx::new(rng.f(-160.0, 160.0), 0.0);
        b.restitution = 0.32;
        b.friction = 0.45;
        w.add(b);
    }
}

fn scene_stack(w: &mut World, _rng: &mut Rng) {
    for col in 0..5 {
        for k in 0..6 {
            let r = 26.0;
            let x = AX + 220.0 + col as f64 * 150.0;
            let mut b = Body::disc(Cx::new(x, AY + r + k as f64 * (2.0 * r + 1.0)), r, 1.0);
            b.restitution = 0.0;
            b.friction = 0.7;
            w.add(b);
        }
    }
}

fn scene_newton(w: &mut World, _rng: &mut Rng) {
    // five equal discs in a row; hit the left one and the impulse travels
    for k in 0..5 {
        let r = 30.0;
        let mut b = Body::disc(Cx::new(AX + 460.0 + k as f64 * (2.0 * r + 0.5), AY + r), r, 1.0);
        b.restitution = 1.0;
        b.friction = 0.0;
        w.add(b);
    }
    let mut hit = Body::disc(Cx::new(AX + 180.0, AY + 30.0), 30.0, 1.0);
    hit.v = Cx::new(700.0, 0.0);
    hit.restitution = 1.0;
    hit.friction = 0.0;
    w.add(hit);
}

const SCENES: [(&str, fn(&mut World, &mut Rng)); 3] = [
    ("PILE", scene_pile),
    ("STACK", scene_stack),
    ("NEWTON ROW", scene_newton),
];

fn build(which: usize, seed: u64) -> World {
    let mut w = arena();
    let mut rng = Rng(seed);
    SCENES[which % SCENES.len()].1(&mut w, &mut rng);
    w
}

fn main() {
    let mut scene = 0usize;
    let mut seed = 7u64;
    let mut world = build(scene, seed);
    let mut paused = false;
    let mut tilt = 0.0f64; // radians; arrows rotate gravity
    let mut show_contacts = true;

    if std::env::args().any(|a| a == "--snapshot") {
        let which: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        let secs: f64 = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(2.5);
        let mut w = build(which, seed);
        for _ in 0..(secs / DT) as usize {
            w.step(DT);
        }
        let mut c = Canvas::new(W, H);
        draw(&mut c, &w, SCENES[which % SCENES.len()].0, false, 0.0, 60.0, true);
        let out = format!("bodies{which}.png");
        c.write_png(&out).expect("could not write snapshot");
        println!("wrote {out}");
        return;
    }

    let mut canvas = Canvas::new(W, H);
    let mut window = Window::new(
        "Recursion I - rigid bodies (arrows tilt gravity)",
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
                Key::R => world = build(scene, seed),
                Key::Tab => {
                    scene = (scene + 1) % SCENES.len();
                    world = build(scene, seed);
                }
                Key::N => {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    world = build(scene, seed);
                }
                Key::C => show_contacts = !show_contacts,
                Key::S => {
                    let mut rng = Rng(seed ^ world.bodies.len() as u64);
                    let r = rng.f(14.0, 34.0);
                    let mut b = Body::disc(Cx::new(rng.f(AX + 80.0, AX + AW - 80.0), AY + AH - 60.0), r, 1.0);
                    b.v = Cx::new(rng.f(-200.0, 200.0), 0.0);
                    world.add(b);
                }
                Key::Key1 => world.iterations = 1,
                Key::Key2 => world.iterations = 4,
                Key::Key3 => world.iterations = 12,
                Key::Key4 => world.iterations = 40,
                _ => {}
            }
        }

        // ---- arrows tilt gravity -----------------------------------------
        let rate = 2.2 * frame;
        if window.is_key_down(Key::Left) {
            tilt -= rate;
        }
        if window.is_key_down(Key::Right) {
            tilt += rate;
        }
        if window.is_key_down(Key::Down) {
            tilt = 0.0;
        }
        // gravity is "straight down, rotated by tilt" - one multiplication
        world.gravity = Cx::new(0.0, -G) * Cx::expi(tilt);

        if !paused {
            acc += frame;
            let mut guard = 0;
            while acc >= DT && guard < 4000 {
                world.step(DT);
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

        draw(&mut canvas, &world, SCENES[scene].0, paused, tilt, fps, show_contacts);
        window.update_with_buffer(&canvas.buf, W, H).expect("present failed");
    }
}

fn draw(c: &mut Canvas, w: &World, scene: &str, paused: bool, tilt: f64, fps: f64, contacts: bool) {
    c.clear(colour::BG);

    // arena
    let (x0, y0) = (sx(Cx::new(AX, 0.0)), sy(Cx::new(0.0, AY + AH)));
    c.rect(x0, y0, AW as i32, AH as i32, colour::LINE);

    // bodies: rim plus a spoke, so rotation is visible
    for b in &w.bodies {
        let col = if b.inv_m == 0.0 { colour::FAINT } else { colour::IMAG };
        c.ring(sx(b.p), sy(b.p), b.r as i32, 2, col);
        let tip = b.p + Cx::expi(b.angle).scale(b.r);
        c.thick_line(sx(b.p), sy(b.p), sx(tip), sy(tip), 2, colour::MOD);
    }

    // contact points and their normals
    if contacts {
        for k in &w.contacts {
            let tip = k.point + k.normal.scale(18.0);
            c.thick_line(sx(k.point), sy(k.point), sx(tip), sy(tip), 1, colour::WARN);
            c.disc(sx(k.point), sy(k.point), 2, colour::WARN);
        }
    }

    // gravity arrow, drawn from a fixed anchor
    let anchor = Cx::new(AX + AW - 90.0, AY + AH - 90.0);
    let g_tip = anchor + w.gravity.unit().scale(58.0);
    c.ring(sx(anchor), sy(anchor), 60, 1, colour::LINE);
    c.thick_line(sx(anchor), sy(anchor), sx(g_tip), sy(g_tip), 3, colour::REAL);
    c.text(sx(anchor) - 12, sy(anchor) + 66, "G", colour::REAL, 2);

    // HUD
    c.text(40, 20, "RECURSION I - RIGID BODIES", colour::INK, 3);
    let help = "ARROWS TILT G   DOWN RESET G   S SPAWN   TAB SCENE   N NEW SEED   1-4 ITERATIONS   C CONTACTS   SPACE PAUSE";
    c.text(W as i32 - Canvas::text_w(help, 1) - 40, 24, help, colour::FAINT, 1);

    let mut y = H as i32 - 96;
    let mut row = |c: &mut Canvas, s: String| {
        c.text(40, y, &s, colour::SOFT, 2);
        y += 22;
    };
    row(
        c,
        format!(
            "{}   {} BODIES   {} CONTACTS   {} ITER",
            scene,
            w.bodies.len(),
            w.contacts.len(),
            w.iterations
        ),
    );
    row(
        c,
        format!(
            "MOMENTUM {:+8.0} {:+8.0}   ENERGY {:11.0}",
            w.momentum().re,
            w.momentum().im,
            w.kinetic_energy()
        ),
    );
    row(
        c,
        format!(
            "TILT {:+6.2} RAD   MAX PENETRATION {:5.2}   {:.0} FPS{}",
            tilt,
            w.max_penetration(),
            fps,
            if paused { "   [PAUSED]" } else { "" }
        ),
    );
}
