//! # Rung 8 — a software renderer
//!
//!   cargo run --release --features window --bin render
//!
//! Filled, depth-buffered, lit triangles. Every pixel still written by the
//! CPU into a `Vec<u32>`; `render3.rs` holds the mathematics.
//!
//! The toggles are the point:
//!
//! * **P** turns off perspective-correct interpolation — watch the floor
//!   checker bend and swim, exactly as it did on a PlayStation 1.
//! * **Z** turns off the depth buffer — the painter's algorithm, and its
//!   failure, in one keypress.
//! * **C** turns off backface culling.
//! * **H** turns off shadows — the second render pass, from the light.
//! * **M** turns off the mirror — the third pass, from a reflected camera.
//!
//! Four passes go into a frame: depth-from-the-light, the reflection through
//! the floor plane, the translucent floor itself, then the opaque scene.

use minifb::{Key, KeyRepeat, Window, WindowOptions};
use recursion1::quat::Q;
use recursion1::raster::{colour, Canvas};
use recursion1::render3::{mirror_camera, Camera, Light, Mesh, Renderer, ShadowMap, Vert};
use recursion1::vec3::V3;
use std::time::Instant;

const W: usize = 1100;
const H: usize = 720;

struct Obj {
    mesh: Mesh,
    colour: u32,
    checker: bool,
    spin: V3,
    at: V3,
}

fn scene_hall() -> Vec<Obj> {
    vec![
        Obj {
            mesh: Mesh::plane(24.0, 12),
            colour: 0x3D5A73,
            checker: true,
            spin: V3::ZERO,
            at: V3::new(0.0, -1.6, 0.0),
        },
        Obj {
            mesh: Mesh::sphere(1.5, 22, 34),
            colour: 0xE0A44A,
            checker: false,
            spin: V3::ZERO,
            at: V3::new(-3.0, 0.2, 0.0),
        },
        Obj {
            mesh: Mesh::cube(2.2),
            colour: 0x4FBCD4,
            checker: false,
            spin: V3::new(0.3, 0.7, 0.2),
            at: V3::new(1.2, 0.0, -1.5),
        },
        Obj {
            mesh: Mesh::sphere(1.0, 18, 26),
            colour: 0xE585AC,
            checker: false,
            spin: V3::ZERO,
            at: V3::new(3.4, -0.4, 2.0),
        },
    ]
}

/// Two objects passing through each other — no back-to-front ordering of
/// whole objects can get this right, which is the case for a depth buffer.
fn scene_interpenetrate() -> Vec<Obj> {
    vec![
        Obj {
            mesh: Mesh::plane(24.0, 12),
            colour: 0x3D5A73,
            checker: true,
            spin: V3::ZERO,
            at: V3::new(0.0, -2.2, 0.0),
        },
        Obj {
            mesh: Mesh::cube(2.6),
            colour: 0xE0A44A,
            checker: false,
            spin: V3::new(0.0, 0.5, 0.0),
            at: V3::new(-0.7, 0.0, 0.0),
        },
        Obj {
            mesh: Mesh::cube(2.6),
            colour: 0x4FBCD4,
            checker: false,
            spin: V3::new(0.5, 0.0, 0.4),
            at: V3::new(0.7, 0.0, 0.0),
        },
    ]
}

const SCENES: [(&str, fn() -> Vec<Obj>); 2] =
    [("HALL", scene_hall), ("INTERPENETRATION", scene_interpenetrate)];

fn main() {
    let mut scene = 0usize;
    let mut objs = SCENES[0].1();
    let mut r = Renderer::new(W, H);
    let light = Light::default();
    let mut shadows = true;
    let mut mirror = true;
    let mut sm = ShadowMap::new(1024, light.dir, V3::ZERO, 9.0);
    let mut orbit = 0.7f64;
    let mut elev = 0.30f64;
    let mut dist = 12.0f64;
    let mut t = 0.0f64;
    let mut paused = false;

    if std::env::args().any(|a| a == "--snapshot") {
        let which: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        let pc: bool = std::env::args().nth(3).map(|s| s != "off").unwrap_or(true);
        let objs = SCENES[which % 2].1();
        let mut c = Canvas::new(W, H);
        r.perspective_correct = pc;
        let cam = camera(orbit, elev, dist);
        let mut sm = ShadowMap::new(1024, light.dir, V3::ZERO, 9.0);
        render(&mut c, &mut r, &mut sm, &cam, &objs, &light, 1.1, true, true);
        hud(&mut c, &r, SCENES[which % 2].0, 60.0, false);
        let out = format!("render{which}{}.png", if pc { "" } else { "_nopc" });
        c.write_png(&out).expect("write failed");
        println!("wrote {out}");
        return;
    }

    let mut canvas = Canvas::new(W, H);
    let mut window = Window::new(
        "Recursion I - software renderer",
        W,
        H,
        WindowOptions::default(),
    )
    .expect("could not open a window");
    window.set_target_fps(60);

    let mut last = Instant::now();
    let (mut fps_t, mut frames, mut fps) = (Instant::now(), 0u32, 0.0f64);

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let now = Instant::now();
        let frame = (now - last).as_secs_f64().min(0.1);
        last = now;
        if !paused {
            t += frame;
        }

        for k in window.get_keys_pressed(KeyRepeat::No) {
            match k {
                Key::Space => paused = !paused,
                Key::P => r.perspective_correct = !r.perspective_correct,
                Key::Z => r.depth_test = !r.depth_test,
                Key::C => r.cull = !r.cull,
                Key::H => shadows = !shadows,
                Key::M => mirror = !mirror,
                Key::Tab => {
                    scene = (scene + 1) % SCENES.len();
                    objs = SCENES[scene].1();
                }
                _ => {}
            }
        }
        let rate = 1.5 * frame;
        if window.is_key_down(Key::Left) {
            orbit -= rate;
        }
        if window.is_key_down(Key::Right) {
            orbit += rate;
        }
        if window.is_key_down(Key::Up) {
            elev = (elev + rate).min(1.35);
        }
        if window.is_key_down(Key::Down) {
            elev = (elev - rate).max(-0.2);
        }
        if window.is_key_down(Key::W) {
            dist = (dist - 9.0 * frame).max(3.5);
        }
        if window.is_key_down(Key::S) {
            dist += 9.0 * frame;
        }

        let cam = camera(orbit, elev, dist);
        render(&mut canvas, &mut r, &mut sm, &cam, &objs, &light, t, shadows, mirror);

        frames += 1;
        if fps_t.elapsed().as_secs_f64() >= 0.5 {
            fps = frames as f64 / fps_t.elapsed().as_secs_f64();
            frames = 0;
            fps_t = Instant::now();
        }
        hud(&mut canvas, &r, SCENES[scene].0, fps, paused);
        window.update_with_buffer(&canvas.buf, W, H).expect("present failed");
    }
}

fn camera(orbit: f64, elev: f64, dist: f64) -> Camera {
    let eye = V3::new(
        dist * elev.cos() * orbit.sin(),
        dist * elev.sin() + 1.0,
        dist * elev.cos() * orbit.cos(),
    );
    Camera { eye, target: V3::new(0.0, 0.0, 0.0), up: V3::Y, fov_y: 0.95, near: 0.15 }
}

#[allow(clippy::too_many_arguments)]
fn render(
    c: &mut Canvas,
    r: &mut Renderer,
    sm: &mut ShadowMap,
    cam: &Camera,
    objs: &[Obj],
    light: &Light,
    t: f64,
    shadows: bool,
    mirror: bool,
) {
    // Pose everything once, so all three passes see identical geometry.
    let mut world: Vec<(u32, bool, Vec<[Vert; 3]>)> = Vec::new();
    for o in objs {
        let q = if o.spin == V3::ZERO {
            Q::ONE
        } else {
            Q::from_axis_angle(o.spin, t * o.spin.norm())
        };
        let tris = o
            .mesh
            .tris
            .iter()
            .map(|tri| {
                let place = |v: &Vert| Vert {
                    pos: q.rotate(v.pos) + o.at,
                    normal: q.rotate(v.normal),
                    uv: v.uv,
                };
                [place(&tri[0]), place(&tri[1]), place(&tri[2])]
            })
            .collect();
        world.push((o.colour, o.checker, tris));
    }

    // ---- pass 1: depth from the light -----------------------------------
    if shadows {
        sm.begin();
        for (_, checker, tris) in &world {
            if *checker {
                continue; // the floor casts nothing useful onto itself
            }
            for tri in tris {
                sm.cast(*tri);
            }
        }
    }
    let shadow = if shadows { Some(&*sm) } else { None };

    c.clear(colour::BG);
    r.begin();

    // ---- pass 2: the reflection, seen by a mirrored camera ---------------
    // The floor at y = -1.6 acts as the mirror. Reflecting flips handedness,
    // so the winding reverses and culling has to be inverted for this pass.
    if mirror {
        let plane_y = -1.6;
        let mcam = mirror_camera(cam, V3::Y, plane_y);
        let keep = r.cull;
        r.cull = false; // simplest correct answer to the handedness flip
        for (col, checker, tris) in &world {
            if *checker {
                continue; // do not reflect the mirror in itself
            }
            for tri in tris {
                // dim the reflection so it reads as a reflection
                let dim = ((col >> 16 & 0xFF) / 3 << 16)
                    | ((col >> 8 & 0xFF) / 3 << 8)
                    | ((col & 0xFF) / 3);
                r.triangle_lit(c, &mcam, *tri, dim, light, false, None);
            }
        }
        r.cull = keep;
        // the reflection lives behind the floor, so clear depth before the
        // real scene is drawn over it
        r.begin();
    }

    // ---- pass 3: the mirror surface, translucent -------------------------
    // Drawn OVER the reflection at partial coverage, so the reflection shows
    // through. Without a stencil buffer this is the simplest honest way to
    // confine a planar reflection to the mirror.
    if mirror {
        r.alpha = 0.62;
    }
    for (col, checker, tris) in &world {
        if !*checker {
            continue;
        }
        for tri in tris {
            r.triangle_lit(c, cam, *tri, *col, light, true, shadow);
        }
    }
    r.alpha = 1.0;

    // ---- pass 4: everything else, opaque ---------------------------------
    for (col, checker, tris) in &world {
        if *checker {
            continue;
        }
        for tri in tris {
            r.triangle_lit(c, cam, *tri, *col, light, false, shadow);
        }
    }
}

fn hud(c: &mut Canvas, r: &Renderer, scene: &str, fps: f64, paused: bool) {
    c.text(30, 16, "RECURSION I - SOFTWARE RENDERER", colour::INK, 3);
    let help = "ARROWS ORBIT  W/S ZOOM  P PERSPECTIVE  Z DEPTH  C CULL  H SHADOWS  M MIRROR  TAB SCENE";
    c.text(W as i32 - Canvas::text_w(help, 1) - 30, 48, help, colour::FAINT, 1);

    let flag = |on: bool| if on { "ON " } else { "OFF" };
    let warn = |on: bool| if on { colour::SOFT } else { colour::WARN };

    let mut y = H as i32 - 74;
    c.text(30, y, &format!("{}   {} TRIS DRAWN   {} CULLED", scene, r.drawn, r.culled), colour::SOFT, 2);
    y += 24;
    c.text(30, y, "PERSPECTIVE-CORRECT ", warn(r.perspective_correct), 2);
    c.text(30 + Canvas::text_w("PERSPECTIVE-CORRECT ", 2), y, flag(r.perspective_correct), warn(r.perspective_correct), 2);
    let x2 = 30 + Canvas::text_w("PERSPECTIVE-CORRECT XXXX", 2);
    c.text(x2, y, "DEPTH ", warn(r.depth_test), 2);
    c.text(x2 + Canvas::text_w("DEPTH ", 2), y, flag(r.depth_test), warn(r.depth_test), 2);
    let x3 = x2 + Canvas::text_w("DEPTH XXXX", 2);
    c.text(x3, y, "CULL ", warn(r.cull), 2);
    c.text(x3 + Canvas::text_w("CULL ", 2), y, flag(r.cull), warn(r.cull), 2);

    c.text(
        W as i32 - 160,
        H as i32 - 26,
        &format!("{:.0} FPS{}", fps, if paused { " PAUSE" } else { "" }),
        colour::FAINT,
        1,
    );
}
