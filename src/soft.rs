//! # Soft bodies, cloth and rope — the mathematics
//!
//! As with `rigid.rs`: nothing here draws, reads a key, or opens a window.
//!
//! ---
//!
//! ## A different way to think about motion
//!
//! Everything so far has been *force-based*: work out the forces, divide by
//! mass to get acceleration, integrate that to velocity, integrate that to
//! position. It works, and `dynamics.rs` does exactly that.
//!
//! For cloth it falls apart. A sheet of fabric is thousands of tiny stiff
//! springs, and stiff springs need either a very small timestep or an implicit
//! solver — otherwise they explode, exactly as explicit Euler did on the
//! pulley's spring.
//!
//! So invert the problem. Work **directly with positions**, and express the
//! fabric not as forces but as **constraints** that must hold:
//!
//! > *"these two particles are 10 units apart."*
//!
//! Move the particles until that is true. No spring constant, no stiffness
//! limit, nothing to blow up. This is **position-based dynamics**.
//!
//! ## 1. Verlet integration — where the velocity went
//!
//! Store each particle's *previous* position instead of its velocity:
//!
//! ```text
//! next  = p + (p - prev) * damping + a * dt^2
//! prev  = p
//! p     = next
//! ```
//!
//! The term `(p - prev)` **is** the velocity, times `dt`. It is never stored,
//! only inferred — and that single choice is what makes the whole method work:
//!
//! > **If you move a particle, you have changed its velocity.**
//!
//! A constraint that yanks a particle sideways automatically gives it sideways
//! momentum on the next step, with no bookkeeping at all. Collisions become
//! "put the particle back outside the wall" and the bounce falls out for free.
//!
//! Note the `dt^2` on acceleration — this is a second-order form, not the
//! first-order `v += a*dt` you are used to.
//!
//! ## 2. The distance constraint
//!
//! Two particles that should be `L` apart. Let `d = pb - pa` and
//! `dist = |d|`. The error is `dist - L`, and we split the correction between
//! them in proportion to **inverse** mass — so a pinned particle
//! (`w = 0`) never moves and its partner takes the whole correction:
//!
//! ```text
//! corr = d/dist * (dist - L) * k / (wa + wb)
//! pa  += corr * wa
//! pb  -= corr * wb
//! ```
//!
//! That is the entire physics of cloth. Everything else is which pairs you
//! choose to constrain.
//!
//! ## 3. Relaxation, and why stiffness needs correcting
//!
//! Satisfying one constraint breaks its neighbours, so we sweep the list
//! repeatedly — the same Gauss-Seidel loop as the contact solver in
//! `rigid.rs`.
//!
//! But that means a link with stiffness `k` gets applied `n` times per step,
//! so the fabric gets stiffer as you add iterations. To keep the *feel*
//! constant, apply the standard correction:
//!
//! ```text
//! k' = 1 - (1 - k)^(1/n)
//! ```
//!
//! Now `iterations` buys accuracy, not stiffness — which is what you want
//! from a knob.
//!
//! ## 4. Which pairs to link
//!
//! For a grid of particles, the choice of links *is* the material:
//!
//! | links | behaviour |
//! |---|---|
//! | **structural** — N/S/E/W neighbours | holds length, but shears freely into a parallelogram |
//! | **shear** — the diagonals | resists shearing; now it behaves like fabric |
//! | **bend** — neighbours two apart | resists folding; cloth becomes card |
//!
//! Leave out the diagonals and your "cloth" collapses sideways like a
//! rhombus. There is a test for exactly that.
//!
//! ## 5. Tearing
//!
//! Because a link is just an entry in a list, tearing is deleting it. When
//! strain `(dist - L) / L` exceeds a threshold, kill the link. No special
//! case anywhere else in the solver.
//!
//! ## 6. What this deliberately does not do
//!
//! **No self-collision.** Nothing stops the fabric passing through itself, or
//! one part of a scene falling through another. That is not an oversight, it
//! is the genuinely expensive half of cloth simulation: you need a spatial
//! hash, particle-versus-*edge* tests rather than particle-versus-particle,
//! and continuous detection to stop fast nodes tunnelling. Collision here is
//! only against explicit `obstacles` and `bounds`.
//!
//! **A two-dimensional sheet cannot drape.** Draping is fabric buckling *out
//! of plane*, and a flat world has no out-of-plane to buckle into. Fully
//! triangulate a 2-D grid at full stiffness and you have built a rigid truss
//! that tumbles like a dinner tray. Hence the soft diagonals in
//! [`Fabric::cloth`], and hence the honest 2-D analogue of "cloth over a
//! sphere" being a **slack rope** over one.
//!
//! ## 7. Where this rejoins the pulley
//!
//! The pulley in `pulley.rs` treats its rope as a single scalar constraint:
//! `h1 + fixed + h2 = L`. That is exactly one distance constraint, solved
//! algebraically. Here the same idea is applied to twenty particles instead of
//! two ends — so the rope stops being a formula and becomes something that
//! swings, goes slack, and whips.

use crate::complex::Cx;

/// A point with mass, and a memory of where it just was.
#[derive(Clone, Copy, Debug)]
pub struct Particle {
    pub p: Cx,
    /// Position at the previous step. `p - prev` is the velocity times `dt`.
    pub prev: Cx,
    /// Inverse mass. **Zero pins the particle in place.**
    pub w: f64,
}

impl Particle {
    pub fn new(p: Cx, w: f64) -> Self {
        Particle { p, prev: p, w }
    }
    pub fn pinned(p: Cx) -> Self {
        Particle { p, prev: p, w: 0.0 }
    }
    /// The implied velocity. Only meaningful divided by the step that produced
    /// it, but useful for diagnostics and for launching a particle.
    pub fn implied_velocity(&self) -> Cx {
        self.p - self.prev
    }
    /// Give the particle motion by moving where it *was*.
    pub fn set_velocity(&mut self, v: Cx) {
        self.prev = self.p - v;
    }
}

/// "Keep these two particles `rest` apart."
#[derive(Clone, Copy, Debug)]
pub struct Link {
    pub a: usize,
    pub b: usize,
    pub rest: f64,
    /// 0 = ignored, 1 = fully enforced each sweep.
    pub stiffness: f64,
    /// Strain `(dist - rest)/rest` at which the link fails.
    pub tear: f64,
    pub alive: bool,
}

impl Link {
    pub fn new(a: usize, b: usize, rest: f64) -> Self {
        Link { a, b, rest, stiffness: 1.0, tear: f64::INFINITY, alive: true }
    }
}

/// A circular obstacle the fabric drapes over.
#[derive(Clone, Copy, Debug)]
pub struct Obstacle {
    pub c: Cx,
    pub r: f64,
}

pub struct Fabric {
    pub particles: Vec<Particle>,
    pub links: Vec<Link>,
    pub obstacles: Vec<Obstacle>,
    pub gravity: Cx,
    /// Velocity retained **per second**. 1.0 = frictionless vacuum.
    ///
    /// Per *second*, not per step. Getting this wrong is an easy and very
    /// confusing bug: a plausible-looking 0.995 applied at every one of 600
    /// steps a second leaves `0.995^600 = 0.05` of the motion, so a sheet of
    /// cloth falls at a fifth of the right speed and looks like it is
    /// suspended in treacle. `integrate` converts with `damping^dt`, which
    /// also makes the behaviour independent of the step size.
    pub damping: f64,
    pub iterations: usize,
    /// Axis-aligned box the fabric is confined to, as (min, max).
    pub bounds: Option<(Cx, Cx)>,
    /// Steady sideways force, scaled per particle. Cloth in a breeze.
    pub wind: Cx,
}

impl Default for Fabric {
    fn default() -> Self {
        Fabric {
            particles: Vec::new(),
            links: Vec::new(),
            obstacles: Vec::new(),
            gravity: Cx::new(0.0, -1200.0),
            damping: 0.995,
            iterations: 8,
            bounds: None,
            wind: Cx::ZERO,
        }
    }
}

impl Fabric {
    // ---- construction -----------------------------------------------------

    /// A chain of `segments + 1` particles from `a` to `b`. `pin` says which
    /// ends are nailed down.
    pub fn rope(a: Cx, b: Cx, segments: usize, pin_start: bool, pin_end: bool) -> Self {
        let mut f = Fabric::default();
        let step = (b - a).scale(1.0 / segments as f64);
        let rest = step.abs();
        for k in 0..=segments {
            let p = a + step.scale(k as f64);
            let pinned = (k == 0 && pin_start) || (k == segments && pin_end);
            f.particles
                .push(if pinned { Particle::pinned(p) } else { Particle::new(p, 1.0) });
        }
        for k in 0..segments {
            f.links.push(Link::new(k, k + 1, rest));
        }
        f
    }

    /// A rectangular sheet. `structural` links are always added; the others
    /// are what turn a floppy mesh into a material.
    pub fn cloth(
        origin: Cx,
        cols: usize,
        rows: usize,
        spacing: f64,
        pin_top: bool,
        shear: bool,
        bend: bool,
    ) -> Self {
        let mut f = Fabric::default();
        let idx = |c: usize, r: usize| r * cols + c;

        for r in 0..rows {
            for c in 0..cols {
                let p = origin + Cx::new(c as f64 * spacing, -(r as f64) * spacing);
                // pin the top row's corners and every fourth node
                let pinned = pin_top && r == 0 && (c == 0 || c == cols - 1 || c % 4 == 0);
                f.particles
                    .push(if pinned { Particle::pinned(p) } else { Particle::new(p, 1.0) });
            }
        }

        let diag = spacing * std::f64::consts::SQRT_2;
        for r in 0..rows {
            for c in 0..cols {
                // structural: right and down
                if c + 1 < cols {
                    f.links.push(Link::new(idx(c, r), idx(c + 1, r), spacing));
                }
                if r + 1 < rows {
                    f.links.push(Link::new(idx(c, r), idx(c, r + 1), spacing));
                }
                // Shear: both diagonals of each cell, but DELIBERATELY SOFT.
                //
                // At full stiffness this is a mistake in two dimensions. A
                // fully triangulated grid is a rigid truss - it cannot deform
                // at all without stretching a link, so the "cloth" becomes a
                // sheet of metal that tumbles instead of draping. Real fabric
                // escapes by bending *out of plane*, and a flat simulation has
                // no out-of-plane to bend into.
                //
                // A soft diagonal gives the right compromise: the sheet
                // resists shearing into a rhombus, but can still fold.
                if shear && c + 1 < cols && r + 1 < rows {
                    for (p, q) in [(idx(c, r), idx(c + 1, r + 1)), (idx(c + 1, r), idx(c, r + 1))] {
                        let mut l = Link::new(p, q, diag);
                        l.stiffness = 0.2;
                        f.links.push(l);
                    }
                }
                // bend: skip a neighbour, so folding costs something
                if bend {
                    if c + 2 < cols {
                        let mut l = Link::new(idx(c, r), idx(c + 2, r), spacing * 2.0);
                        l.stiffness = 0.25;
                        f.links.push(l);
                    }
                    if r + 2 < rows {
                        let mut l = Link::new(idx(c, r), idx(c, r + 2), spacing * 2.0);
                        l.stiffness = 0.25;
                        f.links.push(l);
                    }
                }
            }
        }
        f
    }

    /// A closed ring with spokes to its centre — a crude but effective soft
    /// disc that squashes on impact and springs back.
    pub fn blob(centre: Cx, radius: f64, points: usize) -> Self {
        let mut f = Fabric::default();
        f.particles.push(Particle::new(centre, 1.0)); // hub is index 0
        for k in 0..points {
            let a = 2.0 * std::f64::consts::PI * k as f64 / points as f64;
            f.particles.push(Particle::new(centre + Cx::expi(a).scale(radius), 1.0));
        }
        let rim = |k: usize| 1 + k % points;
        for k in 0..points {
            // rim to rim
            let seg = (f.particles[rim(k)].p - f.particles[rim(k + 1)].p).abs();
            f.links.push(Link::new(rim(k), rim(k + 1), seg));
            // spoke to the hub keeps it inflated
            f.links.push(Link::new(0, rim(k), radius));
            // a chord across two rim points resists denting
            let chord = (f.particles[rim(k)].p - f.particles[rim(k + 2)].p).abs();
            let mut l = Link::new(rim(k), rim(k + 2), chord);
            l.stiffness = 0.5;
            f.links.push(l);
        }
        f
    }

    pub fn set_tear(&mut self, strain: f64) {
        for l in &mut self.links {
            l.tear = strain;
        }
    }

    pub fn live_links(&self) -> usize {
        self.links.iter().filter(|l| l.alive).count()
    }

    // ---- the step ---------------------------------------------------------

    /// Integrate, then relax, then collide. Note the order: constraints are
    /// enforced *before* collisions, so the last word belongs to "do not be
    /// inside a wall".
    pub fn step(&mut self, dt: f64) {
        self.integrate(dt);
        for _ in 0..self.iterations {
            self.relax();
        }
        self.collide();
    }

    /// Verlet. Velocity is `p - prev`, so it never appears by name.
    pub fn integrate(&mut self, dt: f64) {
        let acc_common = self.gravity;
        // per-second -> per-step, so halving dt does not change the physics
        let d = self.damping.powf(dt);
        for q in &mut self.particles {
            if q.w == 0.0 {
                q.prev = q.p; // pinned: no drift, and no phantom velocity
                continue;
            }
            let a = acc_common + self.wind.scale(q.w);
            let v = (q.p - q.prev).scale(d);
            let next = q.p + v + a.scale(dt * dt);
            q.prev = q.p;
            q.p = next;
        }
    }

    /// One Gauss-Seidel sweep over the links.
    pub fn relax(&mut self) {
        // Keep the material's feel independent of the iteration count.
        let n = self.iterations.max(1) as f64;
        for li in 0..self.links.len() {
            let l = self.links[li];
            if !l.alive {
                continue;
            }
            let (pa, pb) = (self.particles[l.a], self.particles[l.b]);
            let wsum = pa.w + pb.w;
            if wsum == 0.0 {
                continue; // both pinned; nothing can move
            }
            let d = pb.p - pa.p;
            let dist = d.abs();
            if dist < 1e-12 {
                continue;
            }

            let strain = (dist - l.rest) / l.rest;
            if strain > l.tear {
                self.links[li].alive = false;
                continue;
            }

            let k = 1.0 - (1.0 - l.stiffness).powf(1.0 / n);
            let corr = d.scale((dist - l.rest) / dist * k / wsum);
            self.particles[l.a].p = pa.p + corr.scale(pa.w);
            self.particles[l.b].p = pb.p - corr.scale(pb.w);
        }
    }

    /// Push particles out of obstacles and back inside the bounds.
    ///
    /// Because velocity is implicit, simply *moving* a particle here already
    /// changes its motion — a particle shoved out of a wall loses exactly the
    /// component of velocity that drove it in. Friction and bounce come free.
    pub fn collide(&mut self) {
        for q in &mut self.particles {
            if q.w == 0.0 {
                continue;
            }
            for o in &self.obstacles {
                let d = q.p - o.c;
                let dist = d.abs();
                if dist < o.r && dist > 1e-12 {
                    q.p = o.c + d.scale(o.r / dist);
                }
            }
            if let Some((lo, hi)) = self.bounds {
                q.p = Cx::new(q.p.re.clamp(lo.re, hi.re), q.p.im.clamp(lo.im, hi.im));
            }
        }
    }

    // ---- interaction / diagnostics ---------------------------------------

    /// Index of the particle nearest `at`, if one is within `radius`.
    pub fn nearest(&self, at: Cx, radius: f64) -> Option<usize> {
        let mut best: Option<(usize, f64)> = None;
        for (i, q) in self.particles.iter().enumerate() {
            let d = (q.p - at).abs();
            if d <= radius && best.map_or(true, |(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        best.map(|(i, _)| i)
    }

    /// Kill every link whose midpoint lies within `radius` of `at` — a knife.
    pub fn cut(&mut self, at: Cx, radius: f64) -> usize {
        let mut n = 0;
        for li in 0..self.links.len() {
            if !self.links[li].alive {
                continue;
            }
            let l = self.links[li];
            let mid = (self.particles[l.a].p + self.particles[l.b].p).scale(0.5);
            if (mid - at).abs() <= radius {
                self.links[li].alive = false;
                n += 1;
            }
        }
        n
    }

    /// Largest `(dist - rest)/rest` over the live links. Near zero means the
    /// fabric is holding its shape.
    pub fn max_strain(&self) -> f64 {
        self.links
            .iter()
            .filter(|l| l.alive)
            .map(|l| {
                let d = (self.particles[l.b].p - self.particles[l.a].p).abs();
                ((d - l.rest) / l.rest).abs()
            })
            .fold(0.0, f64::max)
    }

    pub fn centre_of_mass(&self) -> Cx {
        let mut sum = Cx::ZERO;
        let mut n = 0.0;
        for q in &self.particles {
            sum = sum + q.p;
            n += 1.0;
        }
        if n == 0.0 { Cx::ZERO } else { sum.scale(1.0 / n) }
    }

    /// Total implied kinetic energy, for watching a scene settle.
    pub fn energy(&self, dt: f64) -> f64 {
        self.particles
            .iter()
            .filter(|q| q.w > 0.0)
            .map(|q| {
                let v = q.implied_velocity().scale(1.0 / dt);
                0.5 / q.w * v.abs_sq()
            })
            .sum()
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 1.0 / 600.0;

    fn settle(f: &mut Fabric, secs: f64) {
        for _ in 0..(secs / DT) as usize {
            f.step(DT);
        }
    }

    /// Velocity is never stored - it is the gap between `p` and `prev`.
    #[test]
    fn velocity_is_implicit_in_the_positions() {
        let mut q = Particle::new(Cx::ZERO, 1.0);
        q.set_velocity(Cx::new(3.0, -1.0));
        assert!((q.implied_velocity() - Cx::new(3.0, -1.0)).abs() < 1e-12);
    }

    /// Moving a particle by hand IS giving it momentum. This is the property
    /// the whole method leans on.
    #[test]
    fn displacing_a_particle_gives_it_momentum() {
        let mut f = Fabric { gravity: Cx::ZERO, damping: 1.0, ..Fabric::default() };
        f.particles.push(Particle::new(Cx::ZERO, 1.0));
        f.particles[0].p = Cx::new(10.0, 0.0); // a "constraint" yanked it
        f.step(DT);
        assert!(f.particles[0].p.re > 15.0, "it should have carried on moving");
    }

    /// A pinned particle never moves, whatever is attached to it.
    #[test]
    fn pinned_particles_never_move() {
        let mut f = Fabric::rope(Cx::new(0.0, 400.0), Cx::new(200.0, 400.0), 12, true, false);
        settle(&mut f, 3.0);
        assert!((f.particles[0].p - Cx::new(0.0, 400.0)).abs() < 1e-9);
        assert!(f.particles[12].p.im < 300.0, "the free end should have fallen");
    }

    /// The point of a distance constraint: the link keeps its length.
    #[test]
    fn links_hold_their_rest_length() {
        let mut f = Fabric::rope(Cx::new(0.0, 400.0), Cx::new(300.0, 400.0), 20, true, true);
        settle(&mut f, 4.0);
        assert!(f.max_strain() < 0.02, "strain reached {}", f.max_strain());
    }

    /// A rope pinned at both ends cannot fall further than its own length.
    #[test]
    fn a_slack_rope_hangs_but_does_not_fall_through() {
        let a = Cx::new(0.0, 400.0);
        let b = Cx::new(200.0, 400.0);
        let mut f = Fabric::rope(a, b, 20, true, true);
        // give it slack by shortening every link's rest length? no - the rope
        // is taut, so it should barely sag at all
        settle(&mut f, 4.0);
        let lowest = f.particles.iter().map(|q| q.p.im).fold(f64::MAX, f64::min);
        assert!(lowest > 400.0 - 210.0, "sagged to {lowest}, below its own length");
    }

    /// Higher stiffness must stretch less under the same load.
    #[test]
    fn stiffer_links_stretch_less() {
        let strain_at = |k: f64| {
            let mut f = Fabric::rope(Cx::new(0.0, 500.0), Cx::new(0.0, 100.0), 16, true, false);
            for l in &mut f.links {
                l.stiffness = k;
            }
            settle(&mut f, 3.0);
            f.max_strain()
        };
        let floppy = strain_at(0.05);
        let stiff = strain_at(1.0);
        assert!(stiff < floppy, "stiff {stiff:.4} should be under floppy {floppy:.4}");
    }

    /// Iterations must buy accuracy, not stiffness - that is what the
    /// `1 - (1-k)^(1/n)` correction is for.
    #[test]
    fn iteration_count_does_not_change_the_feel_much() {
        let strain_with = |n: usize| {
            let mut f = Fabric::rope(Cx::new(0.0, 500.0), Cx::new(0.0, 100.0), 16, true, false);
            f.iterations = n;
            for l in &mut f.links {
                l.stiffness = 0.5;
            }
            settle(&mut f, 3.0);
            f.max_strain()
        };
        let few = strain_with(2);
        let many = strain_with(20);
        assert!((few - many).abs() < 0.25, "feel changed: {few:.3} vs {many:.3}");
    }

    /// Structural links alone hold *lengths* but not *angles* - the sheet
    /// collapses sideways into a rhombus. Adding the diagonals fixes it.
    /// This is why "cloth" is a choice of links, not a kind of particle.
    #[test]
    fn shear_links_are_what_stop_a_sheet_collapsing() {
        let width = |shear: bool| {
            let mut f = Fabric::cloth(Cx::new(0.0, 500.0), 8, 8, 20.0, false, shear, false);
            // pin only the two top corners, then tug the sheet sideways
            f.particles[0] = Particle::pinned(f.particles[0].p);
            f.particles[7] = Particle::pinned(f.particles[7].p);
            f.wind = Cx::new(2500.0, 0.0);
            settle(&mut f, 3.0);
            // how far has the bottom row slid relative to the top?
            let top = f.particles[0].p.re;
            let bottom = f.particles[56].p.re;
            (bottom - top).abs()
        };
        let floppy = width(false);
        let braced = width(true);
        assert!(braced < floppy, "shear links did nothing: {braced:.1} vs {floppy:.1}");
    }

    /// Tearing is deleting a link, and needs no special case anywhere else.
    #[test]
    fn over_strained_links_tear() {
        let mut f = Fabric::rope(Cx::new(0.0, 600.0), Cx::new(0.0, 100.0), 10, true, false);
        f.set_tear(0.02);
        f.iterations = 1; // let it stretch before the solver catches up
        for l in &mut f.links {
            l.stiffness = 0.05;
        }
        let before = f.live_links();
        settle(&mut f, 3.0);
        assert!(f.live_links() < before, "nothing tore ({before} links intact)");
    }

    /// The knife: cut removes links near a point and nothing else.
    #[test]
    fn cutting_removes_only_nearby_links() {
        let mut f = Fabric::rope(Cx::new(0.0, 0.0), Cx::new(400.0, 0.0), 20, true, true);
        let before = f.live_links();
        let cut = f.cut(Cx::new(200.0, 0.0), 25.0);
        assert!(cut > 0 && cut < before, "cut {cut} of {before}");
        assert_eq!(f.live_links(), before - cut);
    }

    /// Cloth draped on a circle must end up outside it, not inside.
    #[test]
    fn fabric_stays_outside_an_obstacle() {
        let mut f = Fabric::cloth(Cx::new(-100.0, 400.0), 10, 8, 25.0, false, true, false);
        f.obstacles.push(Obstacle { c: Cx::new(20.0, 150.0), r: 90.0 });
        f.bounds = Some((Cx::new(-600.0, -200.0), Cx::new(600.0, 600.0)));
        settle(&mut f, 4.0);
        for q in &f.particles {
            let d = (q.p - f.obstacles[0].c).abs();
            assert!(d >= f.obstacles[0].r - 1e-6, "a particle got inside: {d}");
        }
    }

    /// A blob keeps its area rather than folding flat, because of the spokes.
    #[test]
    fn a_blob_stays_inflated_when_dropped() {
        let mut f = Fabric::blob(Cx::new(0.0, 300.0), 60.0, 16);
        f.bounds = Some((Cx::new(-400.0, 0.0), Cx::new(400.0, 600.0)));
        settle(&mut f, 4.0);
        let hub = f.particles[0].p;
        let mean = f.particles[1..].iter().map(|q| (q.p - hub).abs()).sum::<f64>()
            / (f.particles.len() - 1) as f64;
        assert!(mean > 35.0, "blob collapsed to mean radius {mean:.1}");
    }

    /// Damping below 1 must drain the motion; a scene has to settle.
    #[test]
    fn damping_settles_the_scene() {
        let mut f = Fabric::rope(Cx::new(0.0, 400.0), Cx::new(300.0, 400.0), 20, true, true);
        f.damping = 0.02; // heavy: 2% of the motion survives each second
        f.particles[10].set_velocity(Cx::new(0.0, -20.0)); // kick it
        let kicked = f.energy(DT);
        settle(&mut f, 6.0);
        assert!(f.energy(DT) < kicked * 0.01, "still ringing at {}", f.energy(DT));
    }

    /// Damping is per SECOND, so the same simulated time must produce the
    /// same result whatever step size you run it at. Pinning this stops the
    /// per-step/per-second confusion ever coming back.
    #[test]
    fn damping_is_independent_of_the_step_size() {
        let drop = |dt: f64| {
            let mut f = Fabric::default();
            f.damping = 0.5;
            f.particles.push(Particle::new(Cx::ZERO, 1.0));
            for _ in 0..(1.0 / dt) as usize {
                f.step(dt);
            }
            f.particles[0].p.im
        };
        let coarse = drop(1.0 / 120.0);
        let fine = drop(1.0 / 960.0);
        assert!(
            (coarse - fine).abs() / fine.abs() < 0.02,
            "step size changed the fall: {coarse:.2} vs {fine:.2}"
        );
    }

    /// With damping ~1 a particle must fall at very nearly `g t^2 / 2`.
    /// This is the test that would have caught the treacle bug immediately.
    #[test]
    fn a_free_particle_falls_at_the_right_rate() {
        let mut f = Fabric { damping: 1.0, ..Fabric::default() };
        f.particles.push(Particle::new(Cx::ZERO, 1.0));
        settle(&mut f, 2.0);
        let want = -0.5 * 1200.0 * 4.0; // -2400
        let got = f.particles[0].p.im;
        assert!((got - want).abs() / want.abs() < 0.01, "fell {got:.1}, expected {want:.1}");
    }
}
