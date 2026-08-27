//! # Rasterisation — turning triangles into pixels
//!
//! Physics files never drew; this one only draws. It is still the same
//! `Vec<u32>` from `raster.rs` underneath — no GPU, no shader language.
//!
//! ---
//!
//! ## The pipeline
//!
//! ```text
//! model space  --(place the object)-->  world space
//!              --(move the camera)---->  view space      z = depth ahead
//!              --(clip the near plane)->  clipped
//!              --(divide by z)-------->  screen space
//!              --(fill)--------------->  pixels
//! ```
//!
//! Every stage is a few lines. The interesting parts are the last two.
//!
//! ## 1. Perspective is a division
//!
//! That is the whole of it. Things far away look small because you divide by
//! how far away they are:
//!
//! ```text
//! screen_x = focal * view_x / view_z
//! ```
//!
//! Everything awkward about 3-D rendering — clipping, the depth buffer,
//! perspective-correct interpolation — is a consequence of that one division.
//! Divide by zero, or by a negative, and geometry behind your head gets drawn
//! in front of you, mirrored. Hence clipping.
//!
//! ## 2. Which pixels are inside a triangle?
//!
//! The **edge function**: for an edge `a -> b` and a point `p`,
//!
//! ```text
//! E(a, b, p) = (b - a) x (p - a)        the 2-D cross product
//! ```
//!
//! Positive on one side, negative on the other, zero exactly on the line. A
//! point is inside the triangle when all three edge functions share a sign.
//!
//! That is the same scalar cross product from `complex.rs` —
//! `Im(conj(a) * b)`, the signed area — doing a completely different job. And
//! the sum of the three edge functions is twice the triangle's area, so
//! dividing by it turns them into **barycentric coordinates**: the weights
//! that blend the three vertices' attributes across the face.
//!
//! ## 3. The depth buffer
//!
//! Painting far things first and near things over them (the "painter's
//! algorithm") fails as soon as triangles interpenetrate or form a cycle. So
//! instead keep a depth per *pixel* and write only when the new fragment is
//! nearer. Per-pixel, no sorting, order-independent.
//!
//! ## 4. Why you interpolate 1/z and not z  ★
//!
//! **The classic rasteriser bug.** Screen space is not linear in world space —
//! the perspective divide saw to that. Depth and every other attribute vary
//! linearly in *world* space, so what varies linearly across the *screen* is
//! their reciprocal:
//!
//! ```text
//! WRONG:  attr = w0*a0 + w1*a1 + w2*a2
//! RIGHT:  attr = (w0*a0/z0 + w1*a1/z1 + w2*a2/z2) / (w0/z0 + w1/z1 + w2/z2)
//! ```
//!
//! Get it wrong and a texture on a floor stretching away from the camera
//! visibly bends and swims — the artefact every PlayStation 1 game has, because
//! that hardware genuinely could not do the division. There is a test that
//! measures the error.
//!
//! ## 5. Light
//!
//! **Lambert**: a surface facing the light is bright, one edge-on is not, and
//! the measure of facing is a dot product — `max(0, N . L)`. Clamping at zero
//! matters: without it, surfaces pointing away get *negative* light.
//!
//! **Blinn-Phong** adds a highlight, using the half-vector between the light
//! and the eye: `max(0, N . H)^shininess`. Not physically derived, but it has
//! been good enough since 1977.

use crate::raster::Canvas;
use crate::vec3::V3;

/// A camera looking from `eye` at `target`.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub eye: V3,
    pub target: V3,
    pub up: V3,
    /// Vertical field of view, radians.
    pub fov_y: f64,
    /// Nothing closer than this is drawn — the plane we clip against.
    pub near: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            eye: V3::new(0.0, 2.0, 8.0),
            target: V3::ZERO,
            up: V3::Y,
            fov_y: 1.0,
            near: 0.1,
        }
    }
}

impl Camera {
    /// Right, up, forward — an orthonormal frame built with two cross
    /// products. No matrix type needed.
    ///
    /// The order of these crosses is not free. `forward x up` gives `-right`,
    /// which quietly **mirrors the image** and reverses every triangle's
    /// winding — so backface culling then keeps precisely the wrong half of
    /// every mesh. `up x forward` is the one that yields `(X, Y, Z)` for a
    /// camera looking down +Z with +Y up.
    pub fn basis(&self) -> (V3, V3, V3) {
        let f = (self.target - self.eye).unit();
        let r = self.up.cross(f).unit();
        let u = f.cross(r);
        (r, u, f)
    }

    /// World -> view. `z` becomes distance straight ahead of the camera.
    pub fn to_view(&self, p: V3) -> V3 {
        let (r, u, f) = self.basis();
        let d = p - self.eye;
        V3::new(d.dot(r), d.dot(u), d.dot(f))
    }

    /// View -> screen. Returns `(x, y, z)` with `z` kept as the view depth,
    /// because the depth buffer needs it. `None` if behind the near plane.
    pub fn project(&self, v: V3, w: f64, h: f64) -> Option<(f64, f64, f64)> {
        if v.z < self.near {
            return None;
        }
        let focal = (h * 0.5) / (self.fov_y * 0.5).tan();
        Some((w * 0.5 + v.x * focal / v.z, h * 0.5 - v.y * focal / v.z, v.z))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Light {
    /// Direction the light travels *towards* the scene.
    pub dir: V3,
    pub ambient: f64,
    pub diffuse: f64,
    pub specular: f64,
    pub shininess: f64,
}

impl Default for Light {
    fn default() -> Self {
        Light {
            dir: V3::new(-0.4, -1.0, -0.5).unit(),
            ambient: 0.18,
            diffuse: 0.85,
            specular: 0.45,
            shininess: 24.0,
        }
    }
}

impl Light {
    /// Lambert plus a Blinn-Phong highlight.
    ///
    /// `n` and `view_dir` are in world space; `view_dir` points from the
    /// surface towards the eye.
    pub fn shade(&self, base: u32, n: V3, view_dir: V3) -> u32 {
        let l = -self.dir; // towards the light
        // max(0, ...) - without the clamp, back-facing surfaces get negative
        // light and wrap around to bright
        let lambert = n.dot(l).max(0.0);
        let half = (l + view_dir).unit();
        let spec = if lambert > 0.0 {
            n.dot(half).max(0.0).powf(self.shininess) * self.specular
        } else {
            0.0
        };
        let k = self.ambient + self.diffuse * lambert;
        let ch = |sh: u32| {
            let c = ((base >> sh) & 0xFF) as f64;
            (((c * k + 255.0 * spec).clamp(0.0, 255.0)) as u32) << sh
        };
        ch(16) | ch(8) | ch(0)
    }
}

/// One corner of a triangle, ready to be interpolated across.
#[derive(Clone, Copy, Debug)]
pub struct Vert {
    pub pos: V3,
    pub normal: V3,
    /// Surface coordinates, so a procedural pattern can show whether the
    /// interpolation is perspective-correct.
    pub uv: (f64, f64),
}

impl Vert {
    pub fn new(pos: V3, normal: V3, uv: (f64, f64)) -> Self {
        Vert { pos, normal, uv }
    }
}

/// `E(a, b, p)` — the signed area of the parallelogram, twice the triangle.
/// Positive when `p` is to the left of `a -> b`.
#[inline]
pub fn edge(ax: f64, ay: f64, bx: f64, by: f64, px: f64, py: f64) -> f64 {
    (bx - ax) * (py - ay) - (by - ay) * (px - ax)
}

/// Clip a triangle against the near plane `z = near`, in view space.
///
/// Returns 0, 1 or 2 triangles. Two is the interesting case: cutting a corner
/// off a triangle leaves a quadrilateral, which needs splitting back into
/// triangles. Skipping this is the usual cause of geometry smearing across the
/// screen when you walk into a wall.
pub fn clip_near(tri: [Vert; 3], near: f64) -> Vec<[Vert; 3]> {
    let inside: Vec<bool> = tri.iter().map(|v| v.pos.z >= near).collect();
    let n_in = inside.iter().filter(|&&b| b).count();

    let lerp = |a: &Vert, b: &Vert| -> Vert {
        let t = (near - a.pos.z) / (b.pos.z - a.pos.z);
        Vert {
            pos: a.pos + (b.pos - a.pos).scale(t),
            normal: (a.normal + (b.normal - a.normal).scale(t)).unit(),
            uv: (
                a.uv.0 + (b.uv.0 - a.uv.0) * t,
                a.uv.1 + (b.uv.1 - a.uv.1) * t,
            ),
        }
    };

    match n_in {
        0 => Vec::new(),
        3 => vec![tri],
        1 => {
            let i = inside.iter().position(|&b| b).unwrap();
            let (a, b, c) = (tri[i], tri[(i + 1) % 3], tri[(i + 2) % 3]);
            vec![[a, lerp(&a, &b), lerp(&a, &c)]]
        }
        _ => {
            let i = inside.iter().position(|&b| !b).unwrap();
            let (out, a, b) = (tri[i], tri[(i + 1) % 3], tri[(i + 2) % 3]);
            let p = lerp(&a, &out);
            let q = lerp(&b, &out);
            vec![[a, b, q], [a, q, p]]
        }
    }
}

/// Colour and depth for one frame.
pub struct Renderer {
    pub w: usize,
    pub h: usize,
    pub depth: Vec<f32>,
    /// Skip triangles whose screen winding is clockwise — they face away.
    pub cull: bool,
    /// Turn OFF to see the classic swimming-texture artefact.
    pub perspective_correct: bool,
    /// Turn OFF to see why the painter's algorithm is not enough.
    pub depth_test: bool,
    pub drawn: usize,
    pub culled: usize,
}

impl Renderer {
    pub fn new(w: usize, h: usize) -> Self {
        Renderer {
            w,
            h,
            depth: vec![f32::INFINITY; w * h],
            cull: true,
            perspective_correct: true,
            depth_test: true,
            drawn: 0,
            culled: 0,
        }
    }

    pub fn begin(&mut self) {
        self.depth.fill(f32::INFINITY);
        self.drawn = 0;
        self.culled = 0;
    }

    /// Draw one world-space triangle.
    pub fn triangle(
        &mut self,
        c: &mut Canvas,
        cam: &Camera,
        tri: [Vert; 3],
        base: u32,
        light: &Light,
        checker: bool,
    ) {
        // to view space, then clip
        let view = [
            Vert { pos: cam.to_view(tri[0].pos), ..tri[0] },
            Vert { pos: cam.to_view(tri[1].pos), ..tri[1] },
            Vert { pos: cam.to_view(tri[2].pos), ..tri[2] },
        ];
        for piece in clip_near(view, cam.near) {
            self.raster(c, cam, piece, tri, base, light, checker);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn raster(
        &mut self,
        c: &mut Canvas,
        cam: &Camera,
        v: [Vert; 3],
        world: [Vert; 3],
        base: u32,
        light: &Light,
        checker: bool,
    ) {
        let (wf, hf) = (self.w as f64, self.h as f64);
        let mut s = [(0.0, 0.0, 0.0); 3];
        for k in 0..3 {
            match cam.project(v[k].pos, wf, hf) {
                Some(p) => s[k] = p,
                None => return,
            }
        }

        // Twice the signed screen area. Its SIGN is the winding, so this one
        // number both culls back faces and normalises the barycentrics.
        let area = edge(s[0].0, s[0].1, s[1].0, s[1].1, s[2].0, s[2].1);
        if area.abs() < 1e-9 {
            return; // degenerate, edge-on
        }
        // Cull when the screen winding comes out NEGATIVE. The sign is not
        // obvious from first principles, because `project` flips y (screen
        // rows count downward, world y counts up) and that reverses the
        // handedness of every triangle on the way in. Pinned by
        // `culling_keeps_front_faces_and_drops_back_faces`, which builds a
        // triangle whose geometric normal provably faces the camera.
        if self.cull && area <= 0.0 {
            self.culled += 1;
            return;
        }
        self.drawn += 1;

        let min_x = s.iter().map(|p| p.0).fold(f64::MAX, f64::min).floor().max(0.0) as i32;
        let max_x = s.iter().map(|p| p.0).fold(f64::MIN, f64::max).ceil().min(wf - 1.0) as i32;
        let min_y = s.iter().map(|p| p.1).fold(f64::MAX, f64::min).floor().max(0.0) as i32;
        let max_y = s.iter().map(|p| p.1).fold(f64::MIN, f64::max).ceil().min(hf - 1.0) as i32;

        let inv_z = [1.0 / s[0].2, 1.0 / s[1].2, 1.0 / s[2].2];
        let eye_dir_of = |p: V3| (cam.eye - p).unit();

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
                // three edge functions; same sign everywhere inside
                let w0 = edge(s[1].0, s[1].1, s[2].0, s[2].1, px, py) / area;
                let w1 = edge(s[2].0, s[2].1, s[0].0, s[0].1, px, py) / area;
                let w2 = edge(s[0].0, s[0].1, s[1].0, s[1].1, px, py) / area;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }

                // Depth: 1/z is what varies linearly across the screen.
                let inv = w0 * inv_z[0] + w1 * inv_z[1] + w2 * inv_z[2];
                let z = 1.0 / inv;
                let idx = y as usize * self.w + x as usize;
                if self.depth_test && z as f32 >= self.depth[idx] {
                    continue;
                }

                // Perspective-correct weights: divide each by its own z, then
                // renormalise. With `perspective_correct` off we use the raw
                // screen-space weights instead, which is the PS1 wobble.
                let (b0, b1, b2) = if self.perspective_correct {
                    (
                        w0 * inv_z[0] / inv,
                        w1 * inv_z[1] / inv,
                        w2 * inv_z[2] / inv,
                    )
                } else {
                    (w0, w1, w2)
                };

                let n = (world[0].normal.scale(b0)
                    + world[1].normal.scale(b1)
                    + world[2].normal.scale(b2))
                .unit();
                let p = world[0].pos.scale(b0) + world[1].pos.scale(b1) + world[2].pos.scale(b2);

                let mut col = base;
                if checker {
                    let u = world[0].uv.0 * b0 + world[1].uv.0 * b1 + world[2].uv.0 * b2;
                    let vv = world[0].uv.1 * b0 + world[1].uv.1 * b1 + world[2].uv.1 * b2;
                    let sq = ((u * 8.0).floor() as i64 + (vv * 8.0).floor() as i64) & 1;
                    col = if sq == 0 { base } else { 0x1B2733 };
                }

                if self.depth_test {
                    self.depth[idx] = z as f32;
                }
                c.px(x, y, light.shade(col, n, eye_dir_of(p)));
            }
        }
    }
}

// ---- a couple of meshes to point the camera at ---------------------------

pub struct Mesh {
    pub tris: Vec<[Vert; 3]>,
}

impl Mesh {
    /// Unit-ish cube, flat normals per face.
    pub fn cube(size: f64) -> Mesh {
        let h = size * 0.5;
        let mut tris = Vec::new();
        // (normal, four corners in winding order)
        let faces: [(V3, [V3; 4]); 6] = [
            (V3::Z, [V3::new(-h, -h, h), V3::new(h, -h, h), V3::new(h, h, h), V3::new(-h, h, h)]),
            (-V3::Z, [V3::new(h, -h, -h), V3::new(-h, -h, -h), V3::new(-h, h, -h), V3::new(h, h, -h)]),
            (V3::X, [V3::new(h, -h, h), V3::new(h, -h, -h), V3::new(h, h, -h), V3::new(h, h, h)]),
            (-V3::X, [V3::new(-h, -h, -h), V3::new(-h, -h, h), V3::new(-h, h, h), V3::new(-h, h, -h)]),
            (V3::Y, [V3::new(-h, h, h), V3::new(h, h, h), V3::new(h, h, -h), V3::new(-h, h, -h)]),
            (-V3::Y, [V3::new(-h, -h, -h), V3::new(h, -h, -h), V3::new(h, -h, h), V3::new(-h, -h, h)]),
        ];
        for (n, q) in faces {
            let v = |i: usize, uv: (f64, f64)| Vert::new(q[i], n, uv);
            tris.push([v(0, (0.0, 0.0)), v(1, (1.0, 0.0)), v(2, (1.0, 1.0))]);
            tris.push([v(0, (0.0, 0.0)), v(2, (1.0, 1.0)), v(3, (0.0, 1.0))]);
        }
        Mesh { tris }
    }

    /// UV sphere with smooth normals — a sphere's normal is just its position.
    pub fn sphere(r: f64, rings: usize, segs: usize) -> Mesh {
        let mut tris = Vec::new();
        let at = |i: usize, j: usize| -> Vert {
            let phi = std::f64::consts::PI * i as f64 / rings as f64;
            let th = 2.0 * std::f64::consts::PI * j as f64 / segs as f64;
            let n = V3::new(phi.sin() * th.cos(), phi.cos(), phi.sin() * th.sin());
            Vert::new(n.scale(r), n, (j as f64 / segs as f64, i as f64 / rings as f64))
        };
        for i in 0..rings {
            for j in 0..segs {
                let (a, b, c, d) = (at(i, j), at(i, j + 1), at(i + 1, j + 1), at(i + 1, j));
                tris.push([a, b, c]);
                tris.push([a, c, d]);
            }
        }
        Mesh { tris }
    }

    /// A flat floor, big enough to show perspective.
    pub fn plane(size: f64, tiles: usize) -> Mesh {
        let mut tris = Vec::new();
        let s = size / tiles as f64;
        for i in 0..tiles {
            for j in 0..tiles {
                let (x0, z0) = (-size * 0.5 + i as f64 * s, -size * 0.5 + j as f64 * s);
                let p = |dx: f64, dz: f64| V3::new(x0 + dx, 0.0, z0 + dz);
                let uv = |dx: f64, dz: f64| ((i as f64 + dx) / 2.0, (j as f64 + dz) / 2.0);
                let v = |dx: f64, dz: f64| Vert::new(p(dx * s, dz * s), V3::Y, uv(dx, dz));
                tris.push([v(0.0, 0.0), v(0.0, 1.0), v(1.0, 1.0)]);
                tris.push([v(0.0, 0.0), v(1.0, 1.0), v(1.0, 0.0)]);
            }
        }
        Mesh { tris }
    }

    pub fn translated(&self, by: V3) -> Mesh {
        Mesh {
            tris: self
                .tris
                .iter()
                .map(|t| {
                    [
                        Vert { pos: t[0].pos + by, ..t[0] },
                        Vert { pos: t[1].pos + by, ..t[1] },
                        Vert { pos: t[2].pos + by, ..t[2] },
                    ]
                })
                .collect(),
        }
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    /// The edge function is positive on one side and negative on the other,
    /// and exactly zero on the line. That sign IS the inside test.
    #[test]
    fn the_edge_function_separates_the_two_sides() {
        // edge from (0,0) to (4,0)
        assert!(edge(0.0, 0.0, 4.0, 0.0, 2.0, 3.0) > 0.0);
        assert!(edge(0.0, 0.0, 4.0, 0.0, 2.0, -3.0) < 0.0);
        assert!(close(edge(0.0, 0.0, 4.0, 0.0, 2.0, 0.0), 0.0, 1e-12));
    }

    /// Barycentric weights sum to one everywhere, and pick out one vertex at a
    /// time at the corners.
    #[test]
    fn barycentric_weights_sum_to_one_and_isolate_the_corners() {
        let (a, b, c) = ((0.0, 0.0), (6.0, 0.0), (0.0, 4.0));
        let area = edge(a.0, a.1, b.0, b.1, c.0, c.1);
        let bary = |p: (f64, f64)| {
            (
                edge(b.0, b.1, c.0, c.1, p.0, p.1) / area,
                edge(c.0, c.1, a.0, a.1, p.0, p.1) / area,
                edge(a.0, a.1, b.0, b.1, p.0, p.1) / area,
            )
        };
        for p in [(1.0, 1.0), (3.0, 0.5), (0.5, 3.0)] {
            let (w0, w1, w2) = bary(p);
            assert!(close(w0 + w1 + w2, 1.0, 1e-12));
        }
        let (w0, w1, w2) = bary(a);
        assert!(close(w0, 1.0, 1e-12) && close(w1, 0.0, 1e-12) && close(w2, 0.0, 1e-12));
    }

    /// Outside the triangle at least one weight goes negative - which is the
    /// whole inside/outside test in the inner loop.
    #[test]
    fn points_outside_have_a_negative_weight() {
        let (a, b, c) = ((0.0, 0.0), (6.0, 0.0), (0.0, 4.0));
        let area = edge(a.0, a.1, b.0, b.1, c.0, c.1);
        let w0 = edge(b.0, b.1, c.0, c.1, 5.0, 5.0) / area;
        let w1 = edge(c.0, c.1, a.0, a.1, 5.0, 5.0) / area;
        let w2 = edge(a.0, a.1, b.0, b.1, 5.0, 5.0) / area;
        assert!(w0 < 0.0 || w1 < 0.0 || w2 < 0.0);
    }

    /// Winding order flips the sign of the area, which is exactly what
    /// backface culling reads.
    #[test]
    fn winding_order_flips_the_sign_of_the_area() {
        let fwd = edge(0.0, 0.0, 4.0, 0.0, 0.0, 3.0);
        let rev = edge(0.0, 0.0, 0.0, 3.0, 4.0, 0.0);
        assert!(close(fwd, -rev, 1e-12));
    }

    /// Perspective divide: something twice as far looks half as big.
    #[test]
    fn perspective_halves_the_size_at_twice_the_distance() {
        let cam = Camera { eye: V3::ZERO, target: V3::Z, up: V3::Y, ..Camera::default() };
        let near = cam.project(V3::new(1.0, 0.0, 5.0), 800.0, 600.0).unwrap();
        let far = cam.project(V3::new(1.0, 0.0, 10.0), 800.0, 600.0).unwrap();
        let (dn, df) = (near.0 - 400.0, far.0 - 400.0);
        assert!(close(dn / df, 2.0, 1e-9), "{dn} vs {df}");
    }

    /// Anything behind the near plane must not project at all.
    #[test]
    fn geometry_behind_the_camera_is_rejected() {
        let cam = Camera { eye: V3::ZERO, target: V3::Z, up: V3::Y, ..Camera::default() };
        assert!(cam.project(V3::new(0.0, 0.0, -3.0), 800.0, 600.0).is_none());
        assert!(cam.project(V3::new(0.0, 0.0, 0.01), 800.0, 600.0).is_none());
        assert!(cam.project(V3::new(0.0, 0.0, 3.0), 800.0, 600.0).is_some());
    }

    /// Near-plane clipping: cutting one corner off leaves a quad, so it must
    /// come back as TWO triangles, and everything must end up in front.
    #[test]
    fn clipping_produces_geometry_entirely_in_front() {
        let v = |z: f64| Vert::new(V3::new(0.0, 0.0, z), V3::Z, (0.0, 0.0));
        assert_eq!(clip_near([v(5.0), v(6.0), v(7.0)], 1.0).len(), 1);
        assert_eq!(clip_near([v(-1.0), v(-2.0), v(-3.0)], 1.0).len(), 0);
        // one vertex in front -> one smaller triangle
        assert_eq!(clip_near([v(5.0), v(-1.0), v(-2.0)], 1.0).len(), 1);
        // two in front -> a quad -> two triangles
        let two = clip_near([v(5.0), v(6.0), v(-2.0)], 1.0);
        assert_eq!(two.len(), 2);
        for t in clip_near([v(5.0), v(6.0), v(-2.0)], 1.0) {
            for vert in t {
                assert!(vert.pos.z >= 1.0 - 1e-9, "clipped vertex at z={}", vert.pos.z);
            }
        }
    }

    /// ★ **The classic bug.** Interpolating an attribute linearly in screen
    /// space is wrong under perspective; you must interpolate `attr/z` and
    /// `1/z` and divide. This measures the error on a steeply receding edge.
    #[test]
    fn screen_space_interpolation_is_wrong_under_perspective() {
        // an edge from z=1 to z=9, sampled at the screen midpoint
        let (z0, z1) = (1.0, 9.0);
        let (a0, a1) = (0.0, 1.0); // the attribute at each end

        // screen-space half-way is NOT world half-way
        let naive = 0.5 * a0 + 0.5 * a1;

        let inv = 0.5 / z0 + 0.5 / z1;
        let correct = (0.5 * a0 / z0 + 0.5 * a1 / z1) / inv;

        // the true world-space parameter at the screen midpoint
        let z_at_mid = 1.0 / inv;
        let t_world = (z_at_mid - z0) / (z1 - z0);
        let truth = a0 + (a1 - a0) * t_world;

        assert!(close(correct, truth, 1e-12), "the corrected form should be exact");
        assert!(
            (naive - truth).abs() > 0.3,
            "naive should be badly wrong, was off by {}",
            (naive - truth).abs()
        );
    }

    /// Lambert must clamp at zero. Without the clamp a surface facing away
    /// gets negative light and wraps round to bright.
    #[test]
    fn light_never_goes_negative_behind_a_surface() {
        let l = Light { dir: -V3::Y, ambient: 0.0, diffuse: 1.0, specular: 0.0, ..Light::default() };
        let facing = l.shade(0xFFFFFF, V3::Y, V3::Y);
        let away = l.shade(0xFFFFFF, -V3::Y, V3::Y);
        assert_eq!(facing, 0xFFFFFF);
        assert_eq!(away, 0x000000, "back face should be black, not wrapped");
    }

    /// Brightness should fall off as the surface turns away, monotonically.
    #[test]
    fn brightness_falls_off_with_the_cosine() {
        let l = Light { dir: -V3::Y, ambient: 0.0, diffuse: 1.0, specular: 0.0, ..Light::default() };
        let at = |deg: f64| {
            let a = deg.to_radians();
            let n = V3::new(a.sin(), a.cos(), 0.0);
            (l.shade(0xFFFFFF, n, V3::Y) & 0xFF) as i32
        };
        assert!(at(0.0) > at(45.0) && at(45.0) > at(80.0) && at(80.0) > at(95.0));
        assert_eq!(at(95.0), 0);
    }

    /// The depth buffer must keep the nearer fragment however the triangles
    /// are ordered - that is the point of it over painting back to front.
    #[test]
    fn the_depth_buffer_is_order_independent() {
        let mut a = Canvas::new(64, 64);
        let mut b = Canvas::new(64, 64);
        let cam = Camera { eye: V3::new(0.0, 0.0, -6.0), target: V3::ZERO, up: V3::Y, ..Camera::default() };
        let light = Light { ambient: 1.0, diffuse: 0.0, specular: 0.0, ..Light::default() };

        let quad = |z: f64| {
            let n = V3::new(0.0, 0.0, -1.0);
            [
                Vert::new(V3::new(-2.0, -2.0, z), n, (0.0, 0.0)),
                Vert::new(V3::new(2.0, -2.0, z), n, (1.0, 0.0)),
                Vert::new(V3::new(0.0, 2.0, z), n, (0.5, 1.0)),
            ]
        };

        let mut r = Renderer::new(64, 64);
        r.cull = false;
        r.begin();
        a.clear(0);
        r.triangle(&mut a, &cam, quad(0.0), 0xFF0000, &light, false); // near
        r.triangle(&mut a, &cam, quad(3.0), 0x00FF00, &light, false); // far

        r.begin();
        b.clear(0);
        r.triangle(&mut b, &cam, quad(3.0), 0x00FF00, &light, false); // far first
        r.triangle(&mut b, &cam, quad(0.0), 0xFF0000, &light, false);

        assert_eq!(a.buf, b.buf, "result depended on submission order");
        let centre = a.buf[32 * 64 + 32];
        assert_eq!(centre & 0xFF0000, 0xFF0000, "the far triangle won");
    }

    /// The camera frame must be right-handed and unmirrored. Cross the two
    /// vectors the other way round and every winding flips, so culling keeps
    /// the far side of every object and the picture is a mirror image.
    #[test]
    fn the_camera_basis_is_right_handed() {
        let cam = Camera { eye: V3::ZERO, target: V3::Z, up: V3::Y, ..Camera::default() };
        let (r, u, f) = cam.basis();
        assert!(close(r.x, 1.0, 1e-12), "right should be +X, got {r}");
        assert!(close(u.y, 1.0, 1e-12), "up should be +Y, got {u}");
        assert!(close(f.z, 1.0, 1e-12));
        // right x up = forward  <=>  right-handed
        assert!(close(r.cross(u).dot(f), 1.0, 1e-12), "frame is mirrored");
        // and a point to the world's right lands right of centre on screen
        let p = cam.project(V3::new(1.0, 0.0, 5.0), 800.0, 600.0).unwrap();
        assert!(p.0 > 400.0, "world +X should appear right of centre");
    }

    /// The decisive culling test: one triangle, facing the camera, must be
    /// DRAWN; the identical triangle with its winding reversed must be
    /// CULLED. Counting how much of a sphere survives is a statistic and
    /// depends on the mesh; this is the actual rule.
    #[test]
    fn culling_keeps_front_faces_and_drops_back_faces() {
        let cam = Camera {
            eye: V3::new(0.0, 0.0, -6.0),
            target: V3::ZERO,
            up: V3::Y,
            ..Camera::default()
        };
        // Wind the triangle so its GEOMETRIC normal - (b-a) x (c-a) - points
        // at the eye. Getting this consistent matters: my first attempt gave
        // the vertices a normal of -Z while winding them so the cross product
        // came out +Z, and the test then happily enshrined the wrong sign.
        let (a, b, c) = (
            V3::new(-1.0, -1.0, 0.0),
            V3::new(0.0, 1.0, 0.0),
            V3::new(1.0, -1.0, 0.0),
        );
        let n = (b - a).cross(c - a).unit();
        assert!(n.z < -0.99, "the test triangle should face the eye, n = {n}");
        let vert = |p: V3| Vert::new(p, n, (0.0, 0.0));

        let run = |tri: [Vert; 3]| {
            let mut canvas = Canvas::new(80, 80);
            canvas.clear(0);
            let mut r = Renderer::new(80, 80);
            r.begin();
            r.triangle(&mut canvas, &cam, tri, 0xFFFFFF, &Light::default(), false);
            (r.drawn, r.culled, canvas.buf.iter().filter(|&&p| p != 0).count())
        };

        let (drawn, culled, pixels) = run([vert(a), vert(b), vert(c)]);
        assert_eq!((drawn, culled), (1, 0), "the front face was culled");
        assert!(pixels > 100, "the front face drew nothing");

        let (drawn, culled, pixels) = run([vert(c), vert(b), vert(a)]);
        assert_eq!((drawn, culled), (0, 1), "the back face was not culled");
        assert_eq!(pixels, 0, "the back face drew {pixels} pixels");
    }

    /// Culling must remove a substantial fraction of a closed mesh - the
    /// point of it - without removing so much that the object disappears.
    #[test]
    fn culling_removes_a_large_share_of_a_closed_mesh() {
        let mut c = Canvas::new(200, 200);
        let cam = Camera {
            eye: V3::new(0.0, 0.0, -6.0),
            target: V3::ZERO,
            up: V3::Y,
            ..Camera::default()
        };
        let mesh = Mesh::sphere(1.5, 12, 18);

        let mut r = Renderer::new(200, 200);
        r.begin();
        c.clear(0);
        for t in &mesh.tris {
            r.triangle(&mut c, &cam, *t, 0x8899AA, &Light::default(), false);
        }
        let ratio = r.culled as f64 / (r.drawn + r.culled) as f64;
        assert!(r.drawn > 0 && r.culled > 0, "culling did nothing useful");
        assert!((0.25..0.75).contains(&ratio), "culled {ratio:.2} of the mesh");
        // and the sphere is still visibly there
        assert!(c.buf.iter().filter(|&&p| p != 0).count() > 2000);
    }

    /// Something must actually appear on screen - the cheapest guard against
    /// a pipeline that silently rejects everything.
    #[test]
    fn a_lit_sphere_actually_draws_pixels() {
        let mut c = Canvas::new(160, 160);
        c.clear(0);
        let cam = Camera { eye: V3::new(0.0, 0.0, -6.0), target: V3::ZERO, up: V3::Y, ..Camera::default() };
        let mut r = Renderer::new(160, 160);
        r.begin();
        for t in &Mesh::sphere(2.0, 16, 24).tris {
            r.triangle(&mut c, &cam, *t, 0xAABBCC, &Light::default(), false);
        }
        let lit = c.buf.iter().filter(|&&p| p != 0).count();
        assert!(lit > 2000, "only {lit} pixels were drawn");
        // and it should be shaded, not flat: many distinct brightnesses
        let mut shades: Vec<u32> = c.buf.iter().filter(|&&p| p != 0).map(|&p| p & 0xFF).collect();
        shades.sort_unstable();
        shades.dedup();
        assert!(shades.len() > 8, "only {} distinct shades - is the light working?", shades.len());
    }
}
