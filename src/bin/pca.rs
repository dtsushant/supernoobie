//! # C5 — eigenvectors and PCA, seen
//!
//!   cargo run --release --features window --bin pca
//!
//! A point cloud, its principal axes, and the two bounding boxes you can fit
//! around it. `eigen.rs` holds the mathematics.
//!
//! The picture is the argument: an **axis-aligned** box has to be big enough
//! to contain the cloud in the world's coordinates, while an **oriented** box
//! uses the cloud's *own* axes — the eigenvectors of its covariance — and is
//! dramatically tighter for anything elongated and rotated.
//!
//! That same decomposition, applied to a table of measurements instead of a
//! cloud of points, is dimensionality reduction.

use minifb::{Key, KeyRepeat, Window, WindowOptions};
use recursion1::eigen::{axis_aligned_bounds, oriented_bounds, pca, Pca};
use recursion1::quat::Q;
use recursion1::raster::{colour, Canvas};
use recursion1::render3::Camera;
use recursion1::vec3::V3;
use std::time::Instant;

const W: usize = 1150;
const H: usize = 760;

struct Rng(u64);
impl Rng {
    fn f(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f64 / (1u64 << 31) as f64) - 0.5
    }
    /// Sum of three uniforms - a cheap bell curve, so the cloud looks like
    /// data rather than a solid brick.
    fn g(&mut self) -> f64 {
        (self.f() + self.f() + self.f()) * 0.8
    }
}

fn cloud(kind: usize, seed: u64) -> Vec<V3> {
    let mut r = Rng(seed);
    let q = Q::from_axis_angle(V3::new(0.35, 1.0, -0.4), 0.85);
    (0..1400)
        .map(|_| match kind {
            // a long, flat, tilted slab - PCA has plenty to find
            0 => q.rotate(V3::new(r.g() * 5.2, r.g() * 1.6, r.g() * 0.5)),
            // a ring: the interesting structure is NOT a straight line, which
            // is exactly the case PCA cannot capture
            1 => {
                let t = (r.f() + 0.5) * std::f64::consts::TAU;
                q.rotate(V3::new(t.cos() * 3.0 + r.g() * 0.25, t.sin() * 3.0 + r.g() * 0.25, r.g() * 0.25))
            }
            // no preferred direction at all
            _ => V3::new(r.g() * 2.2, r.g() * 2.2, r.g() * 2.2),
        })
        .collect()
}

const NAMES: [&str; 3] = [
    "TILTED SLAB - PCA FINDS THE LONG AXIS",
    "RING - THE STRUCTURE IS NOT A DIRECTION",
    "ISOTROPIC - THERE IS NOTHING TO FIND",
];

fn seg(c: &mut Canvas, cam: &Camera, a: V3, b: V3, t: i32, col: u32) {
    if let (Some(p), Some(q)) = (cam.project(cam.to_view(a), W as f64, H as f64),
                                 cam.project(cam.to_view(b), W as f64, H as f64)) {
        c.thick_line(p.0 as i32, p.1 as i32, q.0 as i32, q.1 as i32, t, col);
    }
}

/// Wireframe box from a centre, three axes and three half-extents.
fn boxed(c: &mut Canvas, cam: &Camera, centre: V3, axes: [V3; 3], half: V3, col: u32) {
    let h = [half.x, half.y, half.z];
    let corner = |n: usize| {
        let s = |b: usize| if n & (1 << b) == 0 { -1.0 } else { 1.0 };
        centre + axes[0].scale(h[0] * s(0)) + axes[1].scale(h[1] * s(1)) + axes[2].scale(h[2] * s(2))
    };
    for (i, j) in [
        (0, 1), (0, 2), (0, 4), (1, 3), (1, 5), (2, 3),
        (2, 6), (3, 7), (4, 5), (4, 6), (5, 7), (6, 7),
    ] {
        seg(c, cam, corner(i), corner(j), 1, col);
    }
}

fn main() {
    let mut kind = 0usize;
    let mut seed = 7u64;
    let mut pts = cloud(kind, seed);
    let mut show_obb = true;
    let mut show_aabb = true;
    let (mut orbit, mut elev, mut dist) = (0.7f64, 0.35f64, 20.0f64);

    if std::env::args().any(|a| a == "--snapshot") {
        let k: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        let pts = cloud(k, seed);
        let mut c = Canvas::new(W, H);
        draw(&mut c, &camera(orbit, elev, dist), &pts, k, true, true, 60.0);
        let out = format!("pca{k}.png");
        c.write_png(&out).expect("write failed");
        println!("wrote {out}");
        return;
    }

    let mut canvas = Canvas::new(W, H);
    let mut window = Window::new("Recursion I - PCA", W, H, WindowOptions::default())
        .expect("could not open a window");
    window.set_target_fps(60);
    let mut last = Instant::now();
    let (mut fps_t, mut frames, mut fps) = (Instant::now(), 0u32, 0.0);

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let now = Instant::now();
        let frame = (now - last).as_secs_f64().min(0.1);
        last = now;

        for k in window.get_keys_pressed(KeyRepeat::No) {
            match k {
                Key::Tab => {
                    kind = (kind + 1) % 3;
                    pts = cloud(kind, seed);
                }
                Key::N => {
                    seed = seed.wrapping_add(0x9E3779B9);
                    pts = cloud(kind, seed);
                }
                Key::O => show_obb = !show_obb,
                Key::A => show_aabb = !show_aabb,
                _ => {}
            }
        }
        let r = 1.4 * frame;
        if window.is_key_down(Key::Left) { orbit -= r; }
        if window.is_key_down(Key::Right) { orbit += r; }
        if window.is_key_down(Key::Up) { elev = (elev + r).min(1.35); }
        if window.is_key_down(Key::Down) { elev = (elev - r).max(-1.35); }
        if window.is_key_down(Key::W) { dist = (dist - 12.0 * frame).max(6.0); }
        if window.is_key_down(Key::S) { dist += 12.0 * frame; }

        frames += 1;
        if fps_t.elapsed().as_secs_f64() >= 0.5 {
            fps = frames as f64 / fps_t.elapsed().as_secs_f64();
            frames = 0;
            fps_t = Instant::now();
        }

        draw(&mut canvas, &camera(orbit, elev, dist), &pts, kind, show_obb, show_aabb, fps);
        window.update_with_buffer(&canvas.buf, W, H).expect("present failed");
    }
}

fn camera(orbit: f64, elev: f64, dist: f64) -> Camera {
    Camera {
        eye: V3::new(
            dist * elev.cos() * orbit.sin(),
            dist * elev.sin(),
            dist * elev.cos() * orbit.cos(),
        ),
        target: V3::ZERO,
        up: V3::Y,
        fov_y: 0.95,
        near: 0.2,
    }
}

fn draw(c: &mut Canvas, cam: &Camera, pts: &[V3], kind: usize, obb: bool, aabb: bool, fps: f64) {
    c.clear(colour::BG);

    let p: Pca = pca(pts);
    let (oh, oc) = oriented_bounds(pts, &p);
    let (ah, ac) = axis_aligned_bounds(pts);
    let vol = |h: V3| 8.0 * h.x * h.y * h.z;

    // the cloud, dimmed with depth so it reads as a volume
    for &q in pts {
        if let Some((x, y, z)) = cam.project(cam.to_view(q), W as f64, H as f64) {
            let t = ((z - 8.0) / 22.0).clamp(0.0, 1.0);
            let g = (200.0 - 120.0 * t) as u32;
            c.disc(x as i32, y as i32, 2, (g / 3 << 16) | (g << 8) | (g + 25).min(255));
        }
    }

    if aabb {
        boxed(c, cam, ac, [V3::X, V3::Y, V3::Z], ah, colour::FAINT);
    }
    if obb {
        boxed(c, cam, oc, p.axes, oh, colour::GOOD);
    }

    // the three principal axes, scaled by the SPREAD (sqrt of the variance)
    let sd = [p.variance.x.sqrt(), p.variance.y.sqrt(), p.variance.z.sqrt()];
    for (k, col) in [(0, colour::REAL), (1, colour::MOD), (2, colour::IMAG)] {
        let a = p.axes[k].scale(sd[k] * 2.0);
        seg(c, cam, p.mean - a, p.mean + a, if k == 0 { 4 } else { 3 }, col);
    }

    c.text(30, 16, "RECURSION I - PCA", colour::INK, 3);
    c.text(30, 46, NAMES[kind], colour::FAINT, 2);
    let help = "ARROWS ORBIT  W/S ZOOM  TAB CLOUD  N RESEED  O OBB  A AABB";
    c.text(W as i32 - Canvas::text_w(help, 1) - 30, 20, help, colour::FAINT, 1);

    let mut y = H as i32 - 96;
    let mut row = |c: &mut Canvas, s: String, col: u32| {
        c.text(30, y, &s, col, 2);
        y += 23;
    };
    row(c, format!("VARIANCE  {:8.3} {:8.3} {:8.3}", p.variance.x, p.variance.y, p.variance.z), colour::SOFT);
    row(
        c,
        format!(
            "EXPLAINED  1 AXIS {:5.1}%   2 AXES {:5.1}%",
            p.explained(1) * 100.0,
            p.explained(2) * 100.0
        ),
        colour::SOFT,
    );
    row(
        c,
        format!(
            "BOX VOLUME  ORIENTED {:8.1}   AXIS-ALIGNED {:8.1}   RATIO {:.2}x",
            vol(oh),
            vol(ah),
            vol(ah) / vol(oh).max(1e-9)
        ),
        colour::GOOD,
    );
    c.text(W as i32 - 150, H as i32 - 26, &format!("{fps:.0} FPS"), colour::FAINT, 1);
}
