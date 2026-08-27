//! # Rung 7 — three dimensions
//!
//!   cargo run --release --features window --bin spin3d
//!
//! Wireframe, software-rasterised, still no GPU. `quat.rs` and `body3.rs`
//! hold the mathematics; this file projects and draws.
//!
//! Three things worth seeing:
//!
//! 1. **The Dzhanibekov effect** — a free box spinning about its middle axis
//!    flips end over end, forever, with no torque and no energy change.
//! 2. **Gimbal lock** — the three Euler axes drawn as they really are. At
//!    pitch 90 degrees two of them become parallel and a degree of freedom is
//!    simply gone.
//! 3. **Slerp against lerp** — same endpoints, same time, visibly different
//!    speed through the middle.

use minifb::{Key, KeyRepeat, Window, WindowOptions};
use recursion1::body3::Body3;
use recursion1::quat::Q;
use recursion1::raster::{colour, Canvas};
use recursion1::vec3::V3;
use std::f64::consts::PI;
use std::time::Instant;

const W: usize = 1200;
const H: usize = 780;
const DT: f64 = 1.0 / 900.0;

/// A camera that orbits the origin. Its own orientation is a quaternion, so
/// even the viewing transform is the subject of the file.
struct Cam {
    yaw: f64,
    pitch: f64,
    dist: f64,
    focal: f64,
}

impl Cam {
    fn orientation(&self) -> Q {
        Q::from_axis_angle(V3::X, self.pitch) * Q::from_axis_angle(V3::Y, self.yaw)
    }
    /// World point -> screen point, with perspective divide. `None` when the
    /// point is behind the camera.
    fn project(&self, p: V3) -> Option<(i32, i32)> {
        let c = self.orientation().rotate(p);
        let z = c.z + self.dist;
        if z < 0.2 {
            return None;
        }
        let f = self.focal / z;
        Some((
            (W as f64 * 0.5 + c.x * f) as i32,
            (H as f64 * 0.5 - c.y * f) as i32,
        ))
    }
    fn seg(&self, c: &mut Canvas, a: V3, b: V3, t: i32, col: u32) {
        if let (Some(p), Some(q)) = (self.project(a), self.project(b)) {
            c.thick_line(p.0, p.1, q.0, q.1, t, col);
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Scene {
    Racket,
    Gimbal,
    Slerp,
}

impl Scene {
    fn name(self) -> &'static str {
        match self {
            Scene::Racket => "DZHANIBEKOV - INTERMEDIATE AXIS FLIP",
            Scene::Gimbal => "GIMBAL LOCK - THE THREE EULER AXES",
            Scene::Slerp => "SLERP VS LERP",
        }
    }
    fn next(self) -> Scene {
        match self {
            Scene::Racket => Scene::Gimbal,
            Scene::Gimbal => Scene::Slerp,
            Scene::Slerp => Scene::Racket,
        }
    }
}

fn fresh_racket(axis: usize) -> Body3 {
    let mut b = Body3::box_body(V3::new(0.7, 2.0, 3.4), 2.0);
    let spin = 5.0;
    let n = 0.03; // the perturbation that decides everything
    b.omega = match axis {
        0 => V3::new(spin, n, n),
        1 => V3::new(n, spin, n),
        _ => V3::new(n, n, spin),
    };
    b
}

fn draw_box(c: &mut Canvas, cam: &Cam, b: &Body3, at: V3, col: u32) {
    let pts = b.corners();
    for (i, j) in Body3::EDGES {
        cam.seg(c, pts[i] + at, pts[j] + at, 2, col);
    }
    // body axes, coloured by which principal moment they carry
    let (ax, ay, az) = b.axes();
    let l = b.half;
    cam.seg(c, at, at + ax.scale(l.x * 1.9), 2, colour::REAL);
    cam.seg(c, at, at + ay.scale(l.y * 1.6), 2, colour::MOD);
    cam.seg(c, at, at + az.scale(l.z * 1.3), 2, colour::IMAG);
}

fn main() {
    let mut scene = Scene::Racket;
    let mut cam = Cam { yaw: 0.6, pitch: -0.35, dist: 14.0, focal: 900.0 };
    let mut paused = false;
    let mut spin_axis = 1usize;

    let mut body = fresh_racket(spin_axis);
    let mut t = 0.0f64;
    let mut flips = 0u32;
    let mut last_sign = 1.0f64;

    if std::env::args().any(|a| a == "--snapshot") {
        let which: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        let secs: f64 = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(2.0);
        let sc = [Scene::Racket, Scene::Gimbal, Scene::Slerp][which % 3];
        let mut b = fresh_racket(1);
        for _ in 0..(secs / DT) as usize {
            b.step(DT, V3::ZERO);
        }
        let mut c = Canvas::new(W, H);
        draw(&mut c, &cam, sc, &b, secs, spin_axis, 0, false, 60.0);
        let out = format!("spin{which}.png");
        c.write_png(&out).expect("could not write snapshot");
        println!("wrote {out}");
        return;
    }

    let mut canvas = Canvas::new(W, H);
    let mut window = Window::new(
        "Recursion I - 3D rotation (quaternions)",
        W,
        H,
        WindowOptions::default(),
    )
    .expect("could not open a window");
    window.set_target_fps(60);

    let mut last = Instant::now();
    let (mut fps_t, mut frames, mut fps) = (Instant::now(), 0u32, 0.0f64);
    let mut acc = 0.0f64;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let now = Instant::now();
        let frame = (now - last).as_secs_f64().min(0.2);
        last = now;

        for k in window.get_keys_pressed(KeyRepeat::No) {
            match k {
                Key::Space => paused = !paused,
                Key::R => {
                    body = fresh_racket(spin_axis);
                    t = 0.0;
                    flips = 0;
                }
                Key::Tab => {
                    scene = scene.next();
                    t = 0.0;
                    body = fresh_racket(spin_axis);
                    flips = 0;
                }
                Key::Key1 => {
                    spin_axis = 0;
                    body = fresh_racket(0);
                    t = 0.0;
                    flips = 0;
                }
                Key::Key2 => {
                    spin_axis = 1;
                    body = fresh_racket(1);
                    t = 0.0;
                    flips = 0;
                }
                Key::Key3 => {
                    spin_axis = 2;
                    body = fresh_racket(2);
                    t = 0.0;
                    flips = 0;
                }
                _ => {}
            }
        }

        // orbit the camera
        let r = 1.4 * frame;
        if window.is_key_down(Key::Left) {
            cam.yaw -= r;
        }
        if window.is_key_down(Key::Right) {
            cam.yaw += r;
        }
        if window.is_key_down(Key::Up) {
            cam.pitch = (cam.pitch - r).max(-1.4);
        }
        if window.is_key_down(Key::Down) {
            cam.pitch = (cam.pitch + r).min(1.4);
        }
        if window.is_key_down(Key::W) {
            cam.dist = (cam.dist - 8.0 * frame).max(4.0);
        }
        if window.is_key_down(Key::S) {
            cam.dist += 8.0 * frame;
        }

        if !paused {
            acc += frame;
            let mut n = 0;
            while acc >= DT && n < 4000 {
                if scene == Scene::Racket {
                    let before = body.omega.y;
                    body.step(DT, V3::ZERO);
                    let comp = [body.omega.x, body.omega.y, body.omega.z][spin_axis];
                    let _ = before;
                    if comp.signum() != last_sign && comp.abs() > 1.0 {
                        last_sign = comp.signum();
                        flips += 1;
                    }
                }
                t += DT;
                acc -= DT;
                n += 1;
            }
        }

        frames += 1;
        if fps_t.elapsed().as_secs_f64() >= 0.5 {
            fps = frames as f64 / fps_t.elapsed().as_secs_f64();
            frames = 0;
            fps_t = Instant::now();
        }

        draw(&mut canvas, &cam, scene, &body, t, spin_axis, flips, paused, fps);
        window.update_with_buffer(&canvas.buf, W, H).expect("present failed");
    }
}

#[allow(clippy::too_many_arguments)]
fn draw(
    c: &mut Canvas,
    cam: &Cam,
    scene: Scene,
    body: &Body3,
    t: f64,
    spin_axis: usize,
    flips: u32,
    paused: bool,
    fps: f64,
) {
    c.clear(colour::BG);

    // ground grid, for a sense of space
    for k in -5..=5 {
        let a = k as f64;
        cam.seg(c, V3::new(a, -3.0, -5.0), V3::new(a, -3.0, 5.0), 1, colour::GRID);
        cam.seg(c, V3::new(-5.0, -3.0, a), V3::new(5.0, -3.0, a), 1, colour::GRID);
    }

    let mut lines: Vec<String> = Vec::new();

    match scene {
        Scene::Racket => {
            draw_box(c, cam, body, V3::ZERO, colour::INK);

            // Angular momentum is FIXED in the world even while the body
            // tumbles. Drawing it makes that visible: the box thrashes, this
            // arrow never moves.
            let l = body.angular_momentum();
            cam.seg(c, V3::ZERO, l.unit().scale(4.2), 3, colour::GOOD);

            let names = ["X (LARGEST I)", "Y (INTERMEDIATE)", "Z (SMALLEST I)"];
            lines.push(format!(
                "SPINNING ABOUT {}   T {:6.2}S   FLIPS {}",
                names[spin_axis], t, flips
            ));
            lines.push(format!(
                "I = ({:.2}, {:.2}, {:.2})   OMEGA = ({:+.2}, {:+.2}, {:+.2})",
                body.inertia.x, body.inertia.y, body.inertia.z,
                body.omega.x, body.omega.y, body.omega.z
            ));
            lines.push(format!(
                "|L| {:8.4}  (GREEN, FIXED IN WORLD)    ENERGY {:8.4}",
                l.norm(),
                body.energy()
            ));
            lines.push(
                "1/2/3 PICK AXIS - ONLY THE MIDDLE ONE FLIPS, WITH NO TORQUE".into(),
            );
        }

        Scene::Gimbal => {
            // Drive pitch up through 90 degrees and back.
            let pitch = (t * 0.5).sin() * (PI / 2.0) * 1.02;
            let yaw = t * 0.35;
            let roll = 0.0;

            // The three Euler axes AS THEY REALLY ARE: each one is the
            // previous rotation applied to a fixed body axis.
            let qy = Q::from_axis_angle(V3::Y, yaw);
            let qp = qy * Q::from_axis_angle(V3::X, pitch);
            let yaw_axis = V3::Y;
            let pitch_axis = qy.rotate(V3::X);
            let roll_axis = qp.rotate(V3::Z);

            // Roll longest and drawn FIRST, yaw shortest and drawn LAST, so
            // that when they coincide you see magenta overlaid on cyan on the
            // same line - which is the entire point of the scene.
            cam.seg(c, V3::ZERO, roll_axis.scale(4.2), 3, colour::IMAG);
            cam.seg(c, V3::ZERO, pitch_axis.scale(3.4), 3, colour::REAL);
            cam.seg(c, V3::ZERO, yaw_axis.scale(2.6), 4, colour::MOD);

            let mut b = Body3::box_body(V3::new(2.2, 0.5, 1.2), 1.0);
            b.q = Q::from_euler(yaw, pitch, roll);
            draw_box(c, cam, &b, V3::ZERO, colour::SOFT);

            // How nearly parallel yaw and roll have become: 1 = fully locked
            let align = yaw_axis.dot(roll_axis).abs();
            lines.push(format!(
                "PITCH {:+6.1} DEG    YAW {:+6.1}    LOCK SEVERITY {:.3}",
                pitch.to_degrees(),
                yaw.to_degrees(),
                Q::gimbal_lock_severity(pitch)
            ));
            lines.push(format!(
                "YAW AXIS . ROLL AXIS = {:.4}   {}",
                align,
                if align > 0.97 { "<<< LOCKED - TWO AXES ARE THE SAME" } else { "" }
            ));
            lines.push("MAGENTA YAW   AMBER PITCH   CYAN ROLL".into());
            lines.push(
                "AT 90 DEG PITCH, YAW AND ROLL TURN THE SAME WAY - ONE DOF IS GONE".into(),
            );
        }

        Scene::Slerp => {
            let a = Q::ONE;
            let b = Q::from_axis_angle(V3::new(0.3, 1.0, 0.4), 2.6);
            let u = ((t * 0.4).sin() * 0.5 + 0.5).clamp(0.0, 1.0);

            let mut left = Body3::box_body(V3::new(2.0, 0.6, 1.2), 1.0);
            left.q = a.slerp(b, u);
            let mut right = Body3::box_body(V3::new(2.0, 0.6, 1.2), 1.0);
            right.q = a.scale(1.0 - u).add(b.scale(u)).unit(); // naive lerp

            draw_box(c, cam, &left, V3::new(-2.6, 0.0, 0.0), colour::IMAG);
            draw_box(c, cam, &right, V3::new(2.6, 0.0, 0.0), colour::REAL);

            let ang = |q: Q| 2.0 * a.dot(q).abs().clamp(-1.0, 1.0).acos();
            lines.push(format!("T {:.3}    LEFT SLERP     RIGHT NORMALISED LERP", u));
            lines.push(format!(
                "ANGLE TURNED   SLERP {:6.3} RAD    LERP {:6.3} RAD    DIFF {:+.3}",
                ang(left.q),
                ang(right.q),
                ang(left.q) - ang(right.q)
            ));
            lines.push("BOTH REACH THE SAME ENDS - LERP SURGES THROUGH THE MIDDLE".into());
        }
    }

    // world axes, faint
    cam.seg(c, V3::ZERO, V3::X.scale(5.5), 1, colour::LINE);
    cam.seg(c, V3::ZERO, V3::Y.scale(5.5), 1, colour::LINE);
    cam.seg(c, V3::ZERO, V3::Z.scale(5.5), 1, colour::LINE);

    c.text(40, 20, "RECURSION I - 3D", colour::INK, 3);
    c.text(40, 50, scene.name(), colour::FAINT, 2);
    let help = "ARROWS ORBIT   W/S ZOOM   TAB SCENE   1-3 AXIS   R RESET   SPACE PAUSE";
    c.text(W as i32 - Canvas::text_w(help, 1) - 40, 24, help, colour::FAINT, 1);

    let mut y = H as i32 - 24 - 22 * lines.len() as i32;
    for l in &lines {
        c.text(40, y, l, colour::SOFT, 2);
        y += 22;
    }
    c.text(
        W as i32 - 150,
        H as i32 - 30,
        &format!("{:.0} FPS{}", fps, if paused { " PAUSE" } else { "" }),
        colour::FAINT,
        1,
    );
}
