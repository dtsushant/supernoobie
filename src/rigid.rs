//! # Rigid bodies, collision, and impulses — the mathematics
//!
//! Nothing in this file draws, reads a key, or opens a window. It is the
//! physics on its own, so it can be read, tested and argued with in isolation.
//!
//! Every body here is a **disc**. That is not a limitation of the method —
//! the impulse mathematics below is identical for any shape — it just means
//! collision *detection* stays two lines instead of two hundred, and the
//! interesting part (collision *response*) is not buried.
//!
//! ---
//!
//! ## 1. What a rigid body is
//!
//! Six numbers, and two of them are complex:
//!
//! | | | |
//! |---|---|---|
//! | `p` | position of the centre of mass | `Cx` |
//! | `v` | linear velocity | `Cx` |
//! | `angle` | orientation | `f64` |
//! | `omega` | angular velocity | `f64` |
//! | `inv_m` | 1/mass — **zero means immovable** | `f64` |
//! | `inv_i` | 1/moment of inertia | `f64` |
//!
//! Storing the *inverse* mass rather than the mass is the standard trick: an
//! immovable wall is `inv_m = 0`, so "infinite mass" needs no special case
//! anywhere in the solver. It simply contributes nothing.
//!
//! For a uniform disc the moment of inertia is `I = m r^2 / 2` — the same
//! formula the pulley's gears used.
//!
//! ## 2. The velocity of a *point* on a body
//!
//! A spinning body does not move all at once. A point offset `r` from the
//! centre moves at
//!
//! ```text
//! v_point = v + omega * perp(r)          perp(r) = i * r
//! ```
//!
//! In three dimensions that term is the cross product `omega x r`. In the
//! plane it collapses to *rotate `r` a quarter turn and scale by `omega`* —
//! which is a multiplication by `i`. The complex type earns its place again.
//!
//! ## 3. Detecting a collision
//!
//! **Disc against disc.** Let `d = p_b - p_a`. They overlap when
//! `|d| < r_a + r_b`. The contact normal is `d / |d|` and the penetration
//! depth is `r_a + r_b - |d|`.
//!
//! **Disc against wall.** A wall is the set of points with `n . p = offset`,
//! solid on the side the normal points away from. The signed gap is
//! `n . p - offset - r`; negative means penetrating.
//!
//! ## 4. Responding — the impulse method  ★
//!
//! This is the heart of it. Do not push the bodies apart; **change their
//! velocities instantly**, as if a very large force acted for a very short
//! time. That instantaneous change of momentum is an *impulse*.
//!
//! Take one contact with normal `n`, contact point `c`, and offsets
//! `ra = c - p_a`, `rb = c - p_b`. The relative velocity *at the contact* is
//!
//! ```text
//! v_rel = (v_b + omega_b * perp(rb)) - (v_a + omega_a * perp(ra))
//! v_n   = v_rel . n
//! ```
//!
//! If `v_n > 0` the bodies are already separating — do nothing, or you will
//! glue them together.
//!
//! We apply `J = j n` to body B and `-J` to A. Each body's velocity and spin
//! change by
//!
//! ```text
//! dv     = +/- j n * inv_m
//! domega = +/- inv_i * (r x (j n))
//! ```
//!
//! Substituting those into `v_rel` and taking the normal component gives the
//! change in approach speed as `j K`, where
//!
//! ```text
//! K = inv_m_a + inv_m_b + inv_i_a (ra x n)^2 + inv_i_b (rb x n)^2
//! ```
//!
//! `K` is the **effective inverse mass at that contact, along that normal** —
//! the exact analogue of `M_eff` in the pulley, and it is bigger (so the
//! collision is softer) when the contact is far off-centre, because some of
//! the impulse goes into spin instead of translation.
//!
//! We want the bodies to leave at `-e` times the speed they arrived
//! (`e` = restitution, 1 perfectly bouncy, 0 dead). So we need
//! `v_n + j K = -e v_n`, giving the whole method in one line:
//!
//! ```text
//! j = -(1 + e) v_n / K
//! ```
//!
//! ## 5. Friction
//!
//! Identical, along the tangent `t = perp(n)`, aiming to kill the sliding
//! speed rather than reverse it (so no `1 + e` factor). Then Coulomb's law
//! caps it: `|j_t| <= mu |j_n|`. Below the cap the surfaces grip; at the cap
//! they slide.
//!
//! ## 6. Why iterate
//!
//! Each impulse is solved as if its contact were the only one. A stack of
//! discs has many, and fixing one disturbs the next — so we sweep the list
//! several times and let it settle. This is **projected Gauss-Seidel**, and
//! it is what every 2-D physics engine does. More iterations means firmer
//! stacks.
//!
//! ## 7. Sinking, and why position needs a separate nudge
//!
//! Impulses only fix *velocities*. A body resting under gravity accumulates a
//! little penetration each frame and slowly sinks. So after solving, we also
//! translate the pair apart by a fraction of the remaining depth, ignoring a
//! small tolerance ("slop") so resting contacts do not jitter forever.

use crate::complex::Cx;

/// A solid disc. Set `inv_m = 0` to pin it in place.
#[derive(Clone, Copy, Debug)]
pub struct Body {
    pub p: Cx,
    pub v: Cx,
    pub angle: f64,
    pub omega: f64,
    pub r: f64,
    pub inv_m: f64,
    pub inv_i: f64,
    pub restitution: f64,
    pub friction: f64,
}

impl Body {
    /// A disc of the given radius and density. Moment of inertia is
    /// `I = m r^2 / 2`, the uniform-disc value.
    pub fn disc(p: Cx, r: f64, density: f64) -> Self {
        let m = std::f64::consts::PI * r * r * density;
        let i = 0.5 * m * r * r;
        Body {
            p,
            v: Cx::ZERO,
            angle: 0.0,
            omega: 0.0,
            r,
            inv_m: 1.0 / m,
            inv_i: 1.0 / i,
            restitution: 0.35,
            friction: 0.35,
            }
    }

    /// Immovable: infinite mass and infinite inertia, expressed as zeros.
    pub fn pinned(mut self) -> Self {
        self.inv_m = 0.0;
        self.inv_i = 0.0;
        self
    }

    pub fn mass(&self) -> f64 {
        if self.inv_m == 0.0 { f64::INFINITY } else { 1.0 / self.inv_m }
    }

    /// Velocity of the material point currently at offset `r` from the centre.
    /// `v + omega * perp(r)` — see the module notes.
    pub fn point_velocity(&self, r: Cx) -> Cx {
        self.v + r.perp().scale(self.omega)
    }

    /// Apply an impulse `j` at offset `r`. Linear part changes `v`; the part
    /// that misses the centre of mass changes `omega`.
    pub fn apply_impulse(&mut self, j: Cx, r: Cx) {
        self.v = self.v + j.scale(self.inv_m);
        self.omega += self.inv_i * r.cross(j);
    }

    pub fn kinetic_energy(&self) -> f64 {
        if self.inv_m == 0.0 {
            return 0.0;
        }
        0.5 * self.mass() * self.v.abs_sq() + 0.5 * (1.0 / self.inv_i) * self.omega * self.omega
    }

    pub fn momentum(&self) -> Cx {
        if self.inv_m == 0.0 { Cx::ZERO } else { self.v.scale(self.mass()) }
    }
}

/// An infinite straight wall: every point `p` with `n . p = offset`.
/// Solid on the side the normal points *away* from.
#[derive(Clone, Copy, Debug)]
pub struct Wall {
    pub n: Cx,
    pub offset: f64,
    pub restitution: f64,
    pub friction: f64,
}

impl Wall {
    /// A wall through `point` whose inward normal is `normal`.
    pub fn new(point: Cx, normal: Cx) -> Self {
        let n = normal.unit();
        Wall { n, offset: n.dot(point), restitution: 0.3, friction: 0.5 }
    }
    /// Signed gap from the wall to the surface of a disc. Negative = overlap.
    pub fn gap(&self, b: &Body) -> f64 {
        self.n.dot(b.p) - self.offset - b.r
    }
}

/// One resolved touching pair. `b = None` means "against a wall".
#[derive(Clone, Copy, Debug)]
pub struct Contact {
    pub a: usize,
    pub b: Option<usize>,
    pub wall: usize,
    pub point: Cx,
    /// Points from A towards B (or out of the wall).
    pub normal: Cx,
    pub depth: f64,
    pub restitution: f64,
    pub friction: f64,
}

pub struct World {
    pub bodies: Vec<Body>,
    pub walls: Vec<Wall>,
    pub gravity: Cx,
    /// Gauss-Seidel sweeps per step. More = firmer stacks, more time.
    pub iterations: usize,
    /// Penetration ignored by positional correction, so resting contacts
    /// settle instead of buzzing.
    pub slop: f64,
    /// Fraction of the remaining penetration removed per step.
    pub correction: f64,
    pub contacts: Vec<Contact>,
}

impl Default for World {
    fn default() -> Self {
        World {
            bodies: Vec::new(),
            walls: Vec::new(),
            gravity: Cx::new(0.0, -900.0),
            iterations: 10,
            slop: 0.5,
            correction: 0.4,
            contacts: Vec::new(),
        }
    }
}

impl World {
    pub fn add(&mut self, b: Body) -> usize {
        self.bodies.push(b);
        self.bodies.len() - 1
    }

    /// One full step. The order matters:
    ///   1. gravity into velocity
    ///   2. find contacts
    ///   3. solve velocities (iterate)
    ///   4. velocity into position
    ///   5. push overlaps apart
    pub fn step(&mut self, dt: f64) {
        for b in &mut self.bodies {
            if b.inv_m != 0.0 {
                b.v = b.v + self.gravity.scale(dt);
            }
        }

        self.find_contacts();

        for _ in 0..self.iterations {
            self.solve_velocities();
        }

        for b in &mut self.bodies {
            if b.inv_m != 0.0 {
                b.p = b.p + b.v.scale(dt);
                b.angle += b.omega * dt;
            }
        }

        self.correct_positions();
    }

    /// Brute-force O(n^2) pair test. Fine to a few hundred bodies; past that
    /// you want a broadphase (spatial hash or sweep-and-prune) — which changes
    /// which pairs get tested, never the mathematics below.
    pub fn find_contacts(&mut self) {
        self.contacts.clear();

        for i in 0..self.bodies.len() {
            for (wi, w) in self.walls.iter().enumerate() {
                let gap = w.gap(&self.bodies[i]);
                if gap < 0.0 {
                    let b = &self.bodies[i];
                    self.contacts.push(Contact {
                        a: i,
                        b: None,
                        wall: wi,
                        // the point on the disc that is deepest into the wall
                        point: b.p - w.n.scale(b.r),
                        normal: -w.n, // from the disc into the wall
                        depth: -gap,
                        restitution: b.restitution.min(w.restitution),
                        friction: (b.friction * w.friction).sqrt(),
                    });
                }
            }
        }

        for i in 0..self.bodies.len() {
            for j in (i + 1)..self.bodies.len() {
                let (a, b) = (self.bodies[i], self.bodies[j]);
                if a.inv_m == 0.0 && b.inv_m == 0.0 {
                    continue; // two immovable things can never resolve
                }
                let d = b.p - a.p;
                let dist = d.abs();
                let sum = a.r + b.r;
                if dist >= sum || dist < 1e-12 {
                    continue;
                }
                let n = d.scale(1.0 / dist);
                self.contacts.push(Contact {
                    a: i,
                    b: Some(j),
                    wall: usize::MAX,
                    point: a.p + n.scale(a.r),
                    normal: n,
                    depth: sum - dist,
                    restitution: a.restitution.min(b.restitution),
                    friction: (a.friction * b.friction).sqrt(),
                });
            }
        }
    }

    /// One Gauss-Seidel sweep: visit every contact, apply the impulse that
    /// would fix it in isolation, and move on. Repeat and it converges.
    fn solve_velocities(&mut self) {
        for k in 0..self.contacts.len() {
            let c = self.contacts[k];
            let ra = c.point - self.bodies[c.a].p;

            // --- gather the two sides -----------------------------------
            let (va, ima, iia) = {
                let a = &self.bodies[c.a];
                (a.point_velocity(ra), a.inv_m, a.inv_i)
            };
            let (vb, rb, imb, iib) = match c.b {
                Some(j) => {
                    let b = &self.bodies[j];
                    let rb = c.point - b.p;
                    (b.point_velocity(rb), rb, b.inv_m, b.inv_i)
                }
                None => (Cx::ZERO, Cx::ZERO, 0.0, 0.0), // walls never move
            };

            let vrel = vb - va;
            let vn = vrel.dot(c.normal);
            if vn > 0.0 {
                continue; // already separating
            }

            // --- K: effective inverse mass at this contact --------------
            let ran = ra.cross(c.normal);
            let rbn = rb.cross(c.normal);
            let k_n = ima + imb + iia * ran * ran + iib * rbn * rbn;
            if k_n <= 0.0 {
                continue; // both immovable
            }

            // Ignore restitution for slow contacts, or a resting stack jitters
            // forever on the tiny velocity gravity adds each frame.
            let e = if vn > -20.0 { 0.0 } else { c.restitution };

            //  j = -(1 + e) * vn / K
            let jn = -(1.0 + e) * vn / k_n;
            let impulse = c.normal.scale(jn);
            self.bodies[c.a].apply_impulse(-impulse, ra);
            if let Some(bi) = c.b {
                self.bodies[bi].apply_impulse(impulse, rb);
            }

            // --- friction, along the tangent ----------------------------
            let t = c.normal.perp();
            let va2 = self.bodies[c.a].point_velocity(ra);
            let vb2 = match c.b {
                Some(j) => self.bodies[j].point_velocity(rb),
                None => Cx::ZERO,
            };
            let vt = (vb2 - va2).dot(t);
            let rat = ra.cross(t);
            let rbt = rb.cross(t);
            let k_t = ima + imb + iia * rat * rat + iib * rbt * rbt;
            if k_t <= 0.0 {
                continue;
            }
            // Coulomb: grip until the tangential impulse exceeds mu * normal.
            let jt = (-vt / k_t).clamp(-c.friction * jn.abs(), c.friction * jn.abs());
            let fimp = t.scale(jt);
            self.bodies[c.a].apply_impulse(-fimp, ra);
            if let Some(bi) = c.b {
                self.bodies[bi].apply_impulse(fimp, rb);
            }
        }
    }

    /// Impulses fix velocity, not overlap. Translate the pair apart by a
    /// fraction of whatever penetration is left, sharing the move in
    /// proportion to inverse mass so the heavier body moves less.
    fn correct_positions(&mut self) {
        for k in 0..self.contacts.len() {
            let c = self.contacts[k];
            let ima = self.bodies[c.a].inv_m;
            let imb = c.b.map_or(0.0, |j| self.bodies[j].inv_m);
            let total = ima + imb;
            if total <= 0.0 {
                continue;
            }
            let depth = (c.depth - self.slop).max(0.0);
            if depth <= 0.0 {
                continue;
            }
            let push = c.normal.scale(self.correction * depth / total);
            self.bodies[c.a].p = self.bodies[c.a].p - push.scale(ima);
            if let Some(j) = c.b {
                self.bodies[j].p = self.bodies[j].p + push.scale(imb);
            }
        }
    }

    pub fn kinetic_energy(&self) -> f64 {
        self.bodies.iter().map(|b| b.kinetic_energy()).sum()
    }

    pub fn momentum(&self) -> Cx {
        self.bodies.iter().fold(Cx::ZERO, |a, b| a + b.momentum())
    }

    /// Deepest overlap anywhere. Should stay near zero in a settled scene.
    pub fn max_penetration(&self) -> f64 {
        self.contacts.iter().map(|c| c.depth).fold(0.0, f64::max)
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    fn free_world() -> World {
        World { gravity: Cx::ZERO, ..World::default() }
    }

    /// A uniform disc has `I = m r^2 / 2` — the same formula as the gears.
    #[test]
    fn disc_inertia_is_half_m_r_squared() {
        let b = Body::disc(Cx::ZERO, 3.0, 2.0);
        let m = b.mass();
        assert!(close(m, std::f64::consts::PI * 9.0 * 2.0, 1e-9));
        assert!(close(1.0 / b.inv_i, 0.5 * m * 9.0, 1e-9));
    }

    /// A point on a spinning body moves at `omega * perp(r)`.
    #[test]
    fn point_velocity_is_omega_times_perp_r() {
        let mut b = Body::disc(Cx::ZERO, 1.0, 1.0);
        b.omega = 2.0;
        // a point one unit to the RIGHT of centre, spinning anticlockwise,
        // must be moving UP at speed omega * r
        let v = b.point_velocity(Cx::new(1.0, 0.0));
        assert!(close(v.re, 0.0, 1e-12));
        assert!(close(v.im, 2.0, 1e-12));
    }

    /// The headline conservation law: no gravity, no walls, no friction —
    /// total linear momentum is untouched by any collision.
    #[test]
    fn head_on_collision_conserves_momentum() {
        let mut w = free_world();
        for b in [
            (Cx::new(-20.0, 0.0), Cx::new(60.0, 0.0)),
            (Cx::new(20.0, 0.0), Cx::new(-40.0, 0.0)),
        ] {
            let mut d = Body::disc(b.0, 10.0, 1.0);
            d.v = b.1;
            d.restitution = 1.0;
            d.friction = 0.0;
            w.add(d);
        }
        let p0 = w.momentum();
        for _ in 0..400 {
            w.step(1.0 / 600.0);
        }
        let p1 = w.momentum();
        assert!(close(p0.re, p1.re, 1e-6), "{} vs {}", p0.re, p1.re);
        assert!(close(p0.im, p1.im, 1e-6));
    }

    /// With restitution 1 and no friction, kinetic energy survives too.
    #[test]
    fn perfectly_elastic_collision_conserves_energy() {
        let mut w = free_world();
        for b in [
            (Cx::new(-20.0, 0.0), Cx::new(60.0, 0.0)),
            (Cx::new(20.0, 0.0), Cx::new(-40.0, 0.0)),
        ] {
            let mut d = Body::disc(b.0, 10.0, 1.0);
            d.v = b.1;
            d.restitution = 1.0;
            d.friction = 0.0;
            w.add(d);
        }
        let e0 = w.kinetic_energy();
        for _ in 0..400 {
            w.step(1.0 / 600.0);
        }
        let e1 = w.kinetic_energy();
        assert!((e1 - e0).abs() / e0 < 1e-6, "{e0} -> {e1}");
    }

    /// Restitution below 1 must take energy out, never add it.
    #[test]
    fn inelastic_collision_loses_energy() {
        let mut w = free_world();
        for b in [
            (Cx::new(-20.0, 0.0), Cx::new(60.0, 0.0)),
            (Cx::new(20.0, 0.0), Cx::new(-40.0, 0.0)),
        ] {
            let mut d = Body::disc(b.0, 10.0, 1.0);
            d.v = b.1;
            d.restitution = 0.2;
            d.friction = 0.0;
            w.add(d);
        }
        let e0 = w.kinetic_energy();
        let p0 = w.momentum();
        for _ in 0..400 {
            w.step(1.0 / 600.0);
        }
        assert!(w.kinetic_energy() < e0 * 0.95, "energy was not lost");
        // ...but momentum is STILL exactly conserved. Restitution changes
        // ENERGY, never MOMENTUM. That distinction is the point of this pair
        // of tests, and it is why an inelastic collision is not a broken one.
        let p1 = w.momentum();
        assert!(close(p0.re, p1.re, 1e-6), "momentum {} -> {}", p0.re, p1.re);
        assert!(close(p0.im, p1.im, 1e-6));
    }

    /// An immovable body is unmoved by any impulse.
    #[test]
    fn pinned_bodies_never_move() {
        let mut w = free_world();
        let wall = w.add(Body::disc(Cx::ZERO, 30.0, 1.0).pinned());
        let mut bullet = Body::disc(Cx::new(-100.0, 0.0), 8.0, 1.0);
        bullet.v = Cx::new(400.0, 0.0);
        w.add(bullet);
        for _ in 0..600 {
            w.step(1.0 / 600.0);
        }
        assert!(close(w.bodies[wall].p.abs(), 0.0, 1e-12));
        assert!(close(w.bodies[wall].v.abs(), 0.0, 1e-12));
        assert!(w.bodies[1].v.re < 0.0, "the bullet should have bounced back");
    }

    /// Dropped under gravity onto a floor, a ball settles and stays put.
    #[test]
    fn a_ball_comes_to_rest_on_the_floor() {
        let mut w = World::default();
        w.walls.push(Wall::new(Cx::new(0.0, 0.0), Cx::new(0.0, 1.0)));
        let mut b = Body::disc(Cx::new(0.0, 300.0), 15.0, 1.0);
        b.restitution = 0.3;
        w.add(b);
        for _ in 0..4000 {
            w.step(1.0 / 600.0);
        }
        let b = w.bodies[0];
        assert!(b.v.abs() < 2.0, "still moving at {}", b.v.abs());
        assert!(close(b.p.im, 15.0, 1.5), "resting at {} not r=15", b.p.im);
    }

    /// It must not sink through the floor, however long it rests there.
    #[test]
    fn resting_contact_does_not_sink() {
        let mut w = World::default();
        w.walls.push(Wall::new(Cx::new(0.0, 0.0), Cx::new(0.0, 1.0)));
        w.add(Body::disc(Cx::new(0.0, 16.0), 15.0, 1.0));
        for _ in 0..12_000 {
            w.step(1.0 / 600.0);
        }
        assert!(w.bodies[0].p.im > 15.0 - w.slop - 0.5, "sank to {}", w.bodies[0].p.im);
        assert!(w.max_penetration() < 2.0);
    }

    /// A tower of discs must not collapse into itself.
    #[test]
    fn a_stack_stays_stacked() {
        let mut w = World { iterations: 20, ..World::default() };
        w.walls.push(Wall::new(Cx::new(0.0, 0.0), Cx::new(0.0, 1.0)));
        for k in 0..5 {
            let mut b = Body::disc(Cx::new(0.0, 15.0 + k as f64 * 30.0), 15.0, 1.0);
            b.restitution = 0.0;
            w.add(b);
        }
        for _ in 0..6000 {
            w.step(1.0 / 600.0);
        }
        for (k, b) in w.bodies.iter().enumerate() {
            let want = 15.0 + k as f64 * 30.0;
            assert!((b.p.im - want).abs() < 6.0, "disc {k} at {} want ~{want}", b.p.im);
        }
    }

    /// An off-centre hit must impart SPIN, not just translation. This is the
    /// `(r x n)` term in K doing its job.
    #[test]
    fn an_off_centre_hit_creates_spin() {
        let mut w = free_world();
        let target = w.add(Body::disc(Cx::ZERO, 20.0, 1.0));
        let mut bullet = Body::disc(Cx::new(-100.0, 15.0), 5.0, 1.0);
        bullet.v = Cx::new(300.0, 0.0);
        w.add(bullet);
        for _ in 0..400 {
            w.step(1.0 / 600.0);
        }
        assert!(w.bodies[target].omega.abs() > 1e-3, "no spin was imparted");
    }

    /// A dead-centre hit must impart NO spin — the same term, vanishing.
    #[test]
    fn a_centred_hit_creates_no_spin() {
        let mut w = free_world();
        let target = w.add(Body::disc(Cx::ZERO, 20.0, 1.0));
        let mut bullet = Body::disc(Cx::new(-100.0, 0.0), 5.0, 1.0);
        bullet.v = Cx::new(300.0, 0.0);
        bullet.friction = 0.0;
        w.bodies[target].friction = 0.0;
        w.add(bullet);
        for _ in 0..400 {
            w.step(1.0 / 600.0);
        }
        assert!(w.bodies[target].omega.abs() < 1e-9, "spin {} should be 0", w.bodies[target].omega);
    }

    /// Friction must slow a disc sliding along the ground; frictionless must
    /// not.
    #[test]
    fn friction_slows_a_sliding_disc() {
        let run = |mu: f64| {
            let mut w = World::default();
            let mut wall = Wall::new(Cx::new(0.0, 0.0), Cx::new(0.0, 1.0));
            wall.friction = mu;
            w.walls.push(wall);
            let mut b = Body::disc(Cx::new(0.0, 15.0), 15.0, 1.0);
            b.friction = mu;
            b.restitution = 0.0;
            b.v = Cx::new(300.0, 0.0);
            w.add(b);
            for _ in 0..1800 {
                w.step(1.0 / 600.0);
            }
            w.bodies[0].v.re
        };
        let slippery = run(0.0);
        let grippy = run(0.9);
        assert!(close(slippery, 300.0, 1e-6), "frictionless disc slowed to {slippery}");
        assert!(grippy < 250.0, "friction barely acted: {grippy}");
    }
}


#[cfg(test)]
mod cradle {
    use super::*;
    use crate::complex::Cx;

    /// **Newton's cradle.** Five touching discs in a line; strike the end.
    ///
    /// The striker must stop dead, the three in the middle must never move at
    /// all, and the far one must leave at exactly the speed that arrived.
    /// Momentum and energy both survive to machine precision.
    ///
    /// This works because the solver resolves contacts *sequentially*: the
    /// impulse arrives at one contact, passes through, and reaches the next on
    /// a later iteration. A solver with too few iterations smears the motion
    /// across the whole row instead - which is why `iterations` is a knob you
    /// can feel, and why the demo binds it to keys 1-4.
    #[test]
    fn newtons_cradle_passes_the_impulse_through_the_row() {
        let mut w = World { gravity: Cx::ZERO, iterations: 40, ..World::default() };
        for k in 0..5 {
            let mut b = Body::disc(Cx::new(k as f64 * 60.5, 0.0), 30.0, 1.0);
            b.restitution = 1.0;
            b.friction = 0.0;
            w.add(b);
        }
        let mut hit = Body::disc(Cx::new(-300.0, 0.0), 30.0, 1.0);
        hit.v = Cx::new(400.0, 0.0);
        hit.restitution = 1.0;
        hit.friction = 0.0;
        let striker = w.add(hit);

        let p0 = w.momentum().re;
        let e0 = w.kinetic_energy();
        for _ in 0..1200 {
            w.step(1.0 / 600.0);
        }

        assert!(w.bodies[striker].v.re.abs() < 1e-6, "striker kept {}", w.bodies[striker].v.re);
        for k in 0..4 {
            assert!(w.bodies[k].v.re.abs() < 1e-6, "ball {k} moved at {}", w.bodies[k].v.re);
        }
        assert!((w.bodies[4].v.re - 400.0).abs() < 1e-6, "far ball left at {}", w.bodies[4].v.re);
        assert!((w.momentum().re - p0).abs() < 1e-6);
        assert!((w.kinetic_energy() - e0).abs() / e0 < 1e-12);
    }

    /// What iteration count *actually* buys you.
    ///
    /// It does not change the cradle above - a single sweep still passes the
    /// impulse along, because the timestep is small enough that the impulse
    /// travels one contact per STEP. Where iterations matter is a **stack**,
    /// where many contacts must hold simultaneously against gravity: one sweep
    /// leaves the tower sagging into itself, many sweeps hold it apart.
    #[test]
    fn iterations_buy_firmer_stacks_not_better_collisions() {
        let sink = |iters: usize| {
            let mut w = World { iterations: iters, ..World::default() };
            w.walls.push(Wall::new(Cx::new(0.0, 0.0), Cx::new(0.0, 1.0)));
            for k in 0..8 {
                let mut b = Body::disc(Cx::new(0.0, 20.0 + k as f64 * 40.0), 20.0, 1.0);
                b.restitution = 0.0;
                w.add(b);
            }
            for _ in 0..2400 {
                w.step(1.0 / 600.0);
            }
            w.max_penetration()
        };
        let sloppy = sink(1);
        let firm = sink(40);
        assert!(firm < sloppy, "40 iterations ({firm:.3}) should beat 1 ({sloppy:.3})");
    }
}
