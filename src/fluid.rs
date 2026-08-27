//! # Smoothed particle hydrodynamics — the mathematics
//!
//! As always: nothing here draws, reads a key, or opens a window.
//!
//! ---
//!
//! ## The problem SPH solves
//!
//! Fluid dynamics is written for *fields*: density, pressure and velocity
//! defined at every point of space. Particles are not a field — they are a
//! scatter of dots. SPH is the bridge:
//!
//! > **Read a field off a scatter of particles by smearing each one out into a
//! > soft blob and adding up the overlaps.**
//!
//! That blob is the **kernel** `W(r, h)` — a bump function, peaked at the
//! particle and exactly zero beyond radius `h`. Every field is then a weighted
//! sum over neighbours, and every derivative of a field becomes a sum over the
//! *derivative of the kernel*, which is known analytically. Calculus on a
//! point cloud, with no mesh.
//!
//! ## 1. Density
//!
//! ```text
//! rho_i = sum_j  m_j W(|r_i - r_j|, h)
//! ```
//!
//! Note a particle counts itself (`r = 0`, the peak of the kernel), so density
//! is never zero. Where particles crowd, the blobs overlap and density rises —
//! which is the whole mechanism.
//!
//! ## 2. Pressure, from an equation of state
//!
//! Real water is very nearly incompressible, which is expensive to enforce.
//! **Weakly compressible** SPH cheats: allow a little squashing and push back
//! in proportion.
//!
//! ```text
//! p_i = k (rho_i - rho_0)        clamped at zero
//! ```
//!
//! `k` is a stiffness, exactly like the spring in `dynamics.rs`, and it has
//! the same problem: stiffer means less compressible but a smaller stable
//! timestep. Clamping at zero matters — negative pressure would make particles
//! *attract*, and a fluid that pulls itself into clumps looks nothing like a
//! fluid.
//!
//! ## 3. Forces
//!
//! **Pressure** drives flow from high to low, so it follows the *negative
//! gradient* of pressure. The naive form is not symmetric, and an unsymmetric
//! force pair invents momentum from nothing — so the two pressures are
//! averaged:
//!
//! ```text
//! f_press_i = - sum_j  m_j (p_i + p_j) / (2 rho_j)  grad W_spiky
//! ```
//!
//! Now particle j pushes i exactly as hard as i pushes j, and total momentum
//! is conserved. There is a test for it.
//!
//! **Viscosity** is friction between neighbouring parcels of fluid — it pulls
//! velocities towards their local average, and only depends on *relative*
//! velocity, so it cannot move the fluid as a whole:
//!
//! ```text
//! f_visc_i = mu sum_j  m_j (v_j - v_i) / rho_j  laplacian W_visc
//! ```
//!
//! ## 4. Why three different kernels
//!
//! This looks like fussiness and is not:
//!
//! | kernel | used for | why that one |
//! |---|---|---|
//! | **poly6** | density | smooth, cheap (no square root — it is a function of `r^2`) |
//! | **spiky** | pressure gradient | poly6's gradient goes to **zero** at `r = 0`, so particles on top of each other feel no push apart and clump into piles. Spiky's gradient is largest there. |
//! | **viscosity** | the Laplacian | poly6's Laplacian goes negative near `h`, which *adds* energy and blows the simulation up |
//!
//! Each is chosen for the behaviour of a *derivative* it will be put through.
//! Getting this wrong does not produce a slightly worse fluid; it produces
//! clumping or an explosion.
//!
//! Note the constants below are the **two-dimensional** normalisations. Using
//! the 3-D ones in a flat world gives a fluid of the wrong density and is a
//! silent, maddening bug.
//!
//! ## 5. Neighbours
//!
//! Every sum above is "over j within h", so SPH lives or dies on neighbour
//! search. That is [`crate::grid::SpatialHash`], with the cell size set to
//! `h`.

use crate::complex::Cx;
use crate::grid::SpatialHash;

// ---- kernels (2-D normalisations) ----------------------------------------

/// Poly6, for density. A function of `r^2`, so no square root is needed.
#[inline]
pub fn w_poly6(r2: f64, h: f64) -> f64 {
    let h2 = h * h;
    if r2 >= h2 {
        return 0.0;
    }
    let d = h2 - r2;
    4.0 / (std::f64::consts::PI * h.powi(8)) * d * d * d
}

/// Magnitude of the spiky kernel's gradient, for pressure.
/// Largest at `r = 0` — which is exactly why it is used there.
#[inline]
pub fn grad_w_spiky(r: f64, h: f64) -> f64 {
    if r >= h || r <= 0.0 {
        return 0.0;
    }
    let d = h - r;
    -30.0 / (std::f64::consts::PI * h.powi(5)) * d * d
}

/// Laplacian of the viscosity kernel. Positive everywhere inside `h`, so it
/// can only ever remove energy.
#[inline]
pub fn lap_w_visc(r: f64, h: f64) -> f64 {
    if r >= h {
        return 0.0;
    }
    40.0 / (std::f64::consts::PI * h.powi(5)) * (h - r)
}

/// Density of an infinite regular lattice with this spacing — the honest way
/// to pick `rest_density`, rather than guessing a number and wondering why the
/// fluid inflates or collapses.
pub fn lattice_density(mass: f64, spacing: f64, h: f64) -> f64 {
    let n = (h / spacing).ceil() as i32 + 1;
    let mut rho = 0.0;
    for j in -n..=n {
        for i in -n..=n {
            let r2 = ((i * i + j * j) as f64) * spacing * spacing;
            rho += mass * w_poly6(r2, h);
        }
    }
    rho
}

/// A straight wall, solid on the side the normal points away from.
#[derive(Clone, Copy, Debug)]
pub struct Bound {
    pub n: Cx,
    pub offset: f64,
}

impl Bound {
    pub fn new(point: Cx, normal: Cx) -> Self {
        let n = normal.unit();
        Bound { n, offset: n.dot(point) }
    }
    pub fn gap(&self, p: Cx) -> f64 {
        self.n.dot(p) - self.offset
    }
}

pub struct Fluid {
    pub p: Vec<Cx>,
    pub v: Vec<Cx>,
    pub density: Vec<f64>,
    pub pressure: Vec<f64>,
    pub force: Vec<Cx>,

    pub h: f64,
    pub mass: f64,
    pub rest_density: f64,
    pub stiffness: f64,
    pub viscosity: f64,
    pub gravity: Cx,
    /// Speed retained when bouncing off a wall.
    pub restitution: f64,
    pub bounds: Vec<Bound>,

    hash: SpatialHash,
    scratch: Vec<usize>,
}

impl Fluid {
    pub fn new(h: f64, spacing: f64) -> Self {
        let mass = 1.0;
        Fluid {
            p: Vec::new(),
            v: Vec::new(),
            density: Vec::new(),
            pressure: Vec::new(),
            force: Vec::new(),
            h,
            mass,
            rest_density: lattice_density(mass, spacing, h),
            // g * depth / squash for a 300-deep column at 2% squash
            stiffness: 9.0e6,
            viscosity: 60.0,
            gravity: Cx::new(0.0, -600.0),
            restitution: 0.25,
            bounds: Vec::new(),
            hash: SpatialHash::new(h),
            scratch: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.p.len()
    }
    pub fn is_empty(&self) -> bool {
        self.p.is_empty()
    }

    pub fn add(&mut self, at: Cx, vel: Cx) {
        self.p.push(at);
        self.v.push(vel);
        self.density.push(self.rest_density);
        self.pressure.push(0.0);
        self.force.push(Cx::ZERO);
    }

    /// Fill a rectangle on a regular lattice, jittered slightly. Perfectly
    /// regular starting positions are a trap: the forces cancel exactly and
    /// the block can sit there, balanced, refusing to flow.
    pub fn block(&mut self, lo: Cx, hi: Cx, spacing: f64) {
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        let mut jitter = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            ((seed >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * spacing * 0.2
        };
        let mut y = lo.im;
        while y <= hi.im {
            let mut x = lo.re;
            while x <= hi.re {
                let j = (jitter(), jitter());
                self.add(Cx::new(x + j.0, y + j.1), Cx::ZERO);
                x += spacing;
            }
            y += spacing;
        }
    }

    // ---- the three passes -------------------------------------------------

    /// Pass 1: density from the kernel sum, then pressure from the equation of
    /// state.
    pub fn compute_density(&mut self) {
        self.hash.build(&self.p);
        let h2 = self.h * self.h;
        for i in 0..self.p.len() {
            self.hash.candidates(self.p[i], self.h, &mut self.scratch);
            let mut rho = 0.0;
            for &j in &self.scratch {
                let r2 = (self.p[j] - self.p[i]).abs_sq();
                if r2 < h2 {
                    rho += self.mass * w_poly6(r2, self.h);
                }
            }
            self.density[i] = rho.max(1e-12);
            // clamped at zero: negative pressure would make the fluid
            // attract itself into clumps
            self.pressure[i] = (self.stiffness * (rho - self.rest_density)).max(0.0);
        }
    }

    /// Pass 2: pressure and viscosity forces, plus gravity.
    pub fn compute_forces(&mut self) {
        for i in 0..self.p.len() {
            self.hash.candidates(self.p[i], self.h, &mut self.scratch);
            let mut f = Cx::ZERO;
            for &j in &self.scratch {
                if j == i {
                    continue;
                }
                let d = self.p[i] - self.p[j];
                let r = d.abs();
                if r >= self.h || r < 1e-9 {
                    continue;
                }
                let dir = d.scale(1.0 / r);

                // symmetric pressure: i pushes j exactly as hard as j pushes i
                let shared = self.mass * (self.pressure[i] + self.pressure[j])
                    / (2.0 * self.density[j]);
                f = f - dir.scale(shared * grad_w_spiky(r, self.h));

                // viscosity: depends only on RELATIVE velocity
                let dv = self.v[j] - self.v[i];
                f = f + dv.scale(
                    self.viscosity * self.mass / self.density[j] * lap_w_visc(r, self.h),
                );
            }
            // gravity acts on mass, so as an acceleration it is scaled by rho
            self.force[i] = f + self.gravity.scale(self.density[i]);
        }
    }

    /// Pass 3: semi-implicit Euler, then push anything back inside the walls.
    pub fn integrate(&mut self, dt: f64) {
        for i in 0..self.p.len() {
            // a = f / rho  (force here is per unit volume, not per particle)
            let a = self.force[i].scale(1.0 / self.density[i]);
            self.v[i] = self.v[i] + a.scale(dt);
            self.p[i] = self.p[i] + self.v[i].scale(dt);

            for b in &self.bounds {
                let gap = b.gap(self.p[i]);
                if gap < 0.0 {
                    self.p[i] = self.p[i] + b.n.scale(-gap);
                    let vn = b.n.dot(self.v[i]);
                    if vn < 0.0 {
                        self.v[i] = self.v[i] - b.n.scale(vn * (1.0 + self.restitution));
                    }
                }
            }
        }
    }

    pub fn step(&mut self, dt: f64) {
        self.compute_density();
        self.compute_forces();
        self.integrate(dt);
    }

    // ---- diagnostics ------------------------------------------------------

    pub fn mean_density(&self) -> f64 {
        if self.density.is_empty() {
            return 0.0;
        }
        self.density.iter().sum::<f64>() / self.density.len() as f64
    }
    pub fn max_density(&self) -> f64 {
        self.density.iter().cloned().fold(0.0, f64::max)
    }
    pub fn max_speed(&self) -> f64 {
        self.v.iter().map(|v| v.abs()).fold(0.0, f64::max)
    }
    pub fn momentum(&self) -> Cx {
        self.v.iter().fold(Cx::ZERO, |a, v| a + v.scale(self.mass))
    }
    /// How far the fluid has been squashed, as a fraction. Weakly compressible
    /// SPH aims to keep this within a few per cent.
    pub fn compression(&self) -> f64 {
        (self.max_density() - self.rest_density) / self.rest_density
    }
    pub fn worst_bucket(&self) -> usize {
        self.hash.worst_bucket()
    }

    /// Speed of sound in this fluid.
    ///
    /// With `p = k(rho - rho_0)` the wave speed is `c = sqrt(dp/drho) =
    /// sqrt(k)` — it does **not** involve the density. Getting that wrong
    /// makes `stable_dt` return something absurd (seconds instead of
    /// milliseconds) and the fluid detonates on the first step while the
    /// diagnostic insists everything is fine.
    pub fn sound_speed(&self) -> f64 {
        self.stiffness.max(0.0).sqrt()
    }

    /// The CFL limit: a pressure wave must not cross a smoothing radius in
    /// less than one step.
    pub fn stable_dt(&self) -> f64 {
        let c = self.sound_speed().max(1e-9);
        0.25 * self.h / c
    }

    /// Choose a stiffness from what the fluid actually has to hold up.
    ///
    /// At the bottom of a column of depth `d` the pressure is `rho_0 g d`. We
    /// want that reached at a compression of `squash` (say 2%), so
    ///
    /// ```text
    /// k * squash * rho_0 = rho_0 * g * d      ->      k = g d / squash
    /// ```
    ///
    /// The rest density cancels, which is why a stiffness guessed without
    /// reference to gravity and depth is almost always wrong by orders of
    /// magnitude.
    pub fn tune_stiffness(&mut self, depth: f64, squash: f64) {
        self.stiffness = self.gravity.abs() * depth / squash.max(1e-6);
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    /// A kernel is a probability-like bump: it must integrate to 1 over the
    /// plane, or every density you compute is scaled wrong. Checked by
    /// numerical quadrature, which also catches using a 3-D constant in 2-D.
    #[test]
    fn the_density_kernel_integrates_to_one() {
        let h = 1.0;
        let n = 900;
        let step = 2.0 * h / n as f64;
        let mut total = 0.0;
        for j in 0..n {
            for i in 0..n {
                let x = -h + (i as f64 + 0.5) * step;
                let y = -h + (j as f64 + 0.5) * step;
                total += w_poly6(x * x + y * y, h) * step * step;
            }
        }
        assert!(close(total, 1.0, 2e-3), "kernel integrates to {total}, want 1");
    }

    /// Compact support: beyond `h` a kernel must be exactly zero, not merely
    /// small. That is what makes neighbour search finite.
    #[test]
    fn kernels_vanish_outside_the_smoothing_radius() {
        let h = 12.0;
        assert_eq!(w_poly6(h * h, h), 0.0);
        assert_eq!(w_poly6(h * h * 4.0, h), 0.0);
        assert_eq!(grad_w_spiky(h, h), 0.0);
        assert_eq!(grad_w_spiky(h * 2.0, h), 0.0);
        assert_eq!(lap_w_visc(h, h), 0.0);
    }

    /// THE reason spiky exists. Poly6's gradient dies at r = 0, so coincident
    /// particles feel no push apart and pile up. Spiky's is strongest there.
    #[test]
    fn spiky_pushes_hardest_where_poly6_gives_up() {
        let h = 10.0;
        // poly6 gradient magnitude, by finite difference on r
        let poly6_grad = |r: f64| {
            let e = 1e-6;
            ((w_poly6((r + e) * (r + e), h) - w_poly6((r - e) * (r - e), h)) / (2.0 * e)).abs()
        };
        assert!(poly6_grad(0.01) < poly6_grad(5.0), "poly6 gradient should vanish at 0");
        assert!(
            grad_w_spiky(0.01, h).abs() > grad_w_spiky(5.0, h).abs(),
            "spiky gradient should be strongest at 0"
        );
    }

    /// Viscosity may only ever take energy out, so its Laplacian must not go
    /// negative anywhere inside the support.
    #[test]
    fn the_viscosity_laplacian_never_goes_negative() {
        let h = 8.0;
        for k in 0..200 {
            let r = h * k as f64 / 200.0;
            assert!(lap_w_visc(r, h) >= 0.0, "negative at r={r}");
        }
    }

    /// A lattice at the spacing used to derive `rest_density` must actually
    /// measure that density. This is the calibration test.
    #[test]
    fn a_lattice_at_rest_spacing_has_the_rest_density() {
        let (h, spacing) = (20.0, 10.0);
        let mut f = Fluid::new(h, spacing);
        f.block(Cx::new(-100.0, -100.0), Cx::new(100.0, 100.0), spacing);
        f.compute_density();
        // sample the middle, away from the edges where the sum is truncated
        let mid = f
            .p
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
            .unwrap()
            .0;
        let got = f.density[mid];
        assert!(
            close(got / f.rest_density, 1.0, 0.05),
            "interior density {got:.4} vs rest {:.4}",
            f.rest_density
        );
    }

    /// The symmetric pressure term exists so that internal forces cancel in
    /// pairs. With gravity off and no walls, total momentum must not move.
    #[test]
    fn internal_forces_conserve_momentum() {
        let (h, spacing) = (20.0, 10.0);
        let mut f = Fluid::new(h, spacing);
        f.gravity = Cx::ZERO;
        f.viscosity = 0.0;
        // squash a block so there is real pressure to push with
        f.block(Cx::new(-60.0, -60.0), Cx::new(60.0, 60.0), spacing * 0.8);
        let p0 = f.momentum();
        for _ in 0..200 {
            f.step(f.stable_dt() * 0.5);
        }
        let p1 = f.momentum();
        assert!(close(p0.re, p1.re, 1e-6), "momentum drifted {} -> {}", p0.re, p1.re);
        assert!(close(p0.im, p1.im, 1e-6));
    }

    /// Viscosity is a function of RELATIVE velocity, so it must not slow down
    /// a fluid that is drifting as a rigid block.
    #[test]
    fn viscosity_does_not_brake_uniform_motion() {
        let (h, spacing) = (20.0, 10.0);
        let mut f = Fluid::new(h, spacing);
        f.gravity = Cx::ZERO;
        f.viscosity = 500.0;
        f.block(Cx::new(-50.0, -50.0), Cx::new(50.0, 50.0), spacing);
        for v in f.v.iter_mut() {
            *v = Cx::new(40.0, 0.0);
        }
        for _ in 0..100 {
            f.step(f.stable_dt() * 0.5);
        }
        let mean = f.momentum().re / f.len() as f64 / f.mass;
        assert!(close(mean, 40.0, 1.0), "uniform drift was damped to {mean}");
    }

    /// ...but it must flatten a shear: two layers sliding past each other
    /// should be dragged towards a common velocity.
    #[test]
    fn viscosity_flattens_a_shear() {
        let spread = |mu: f64| {
            let (h, spacing) = (20.0, 10.0);
            let mut f = Fluid::new(h, spacing);
            f.gravity = Cx::ZERO;
            f.stiffness = 0.0; // isolate viscosity from pressure
            f.viscosity = mu;
            f.block(Cx::new(-60.0, -30.0), Cx::new(60.0, 30.0), spacing);
            for i in 0..f.len() {
                f.v[i] = Cx::new(if f.p[i].im > 0.0 { 30.0 } else { -30.0 }, 0.0);
            }
            for _ in 0..300 {
                f.step(1e-4);
            }
            let hi: f64 = f.v.iter().map(|v| v.re).fold(f64::MIN, f64::max);
            let lo: f64 = f.v.iter().map(|v| v.re).fold(f64::MAX, f64::min);
            hi - lo
        };
        let inviscid = spread(0.0);
        let thick = spread(400.0);
        assert!(close(inviscid, 60.0, 1e-6), "shear changed without viscosity");
        assert!(thick < inviscid * 0.95, "viscosity did not smooth the shear");
    }

    /// A dam break must settle instead of exploding, and must stay roughly
    /// incompressible while doing it.
    #[test]
    fn a_dam_break_settles_without_exploding() {
        let (h, spacing) = (20.0, 10.0);
        let mut f = Fluid::new(h, spacing);
        f.bounds.push(Bound::new(Cx::new(0.0, 0.0), Cx::new(0.0, 1.0)));
        f.bounds.push(Bound::new(Cx::new(-200.0, 0.0), Cx::new(1.0, 0.0)));
        f.bounds.push(Bound::new(Cx::new(200.0, 0.0), Cx::new(-1.0, 0.0)));
        f.tune_stiffness(260.0, 0.02);
        f.block(Cx::new(-190.0, 10.0), Cx::new(-60.0, 260.0), spacing);

        let dt = f.stable_dt();
        for _ in 0..4000 {
            f.step(dt);
        }
        assert!(f.max_speed() < 3000.0, "detonated: max speed {}", f.max_speed());
        assert!(f.compression() < 0.6, "compressed by {:.2}", f.compression());
        for (i, p) in f.p.iter().enumerate() {
            assert!(p.im > -1.0, "particle {i} fell through the floor at {}", p.im);
            assert!(p.re > -201.0 && p.re < 201.0, "particle {i} escaped sideways");
        }
    }

    /// Stiffer fluid means a shorter stable step - the same trade-off the
    /// pulley's spring had, and the reason SPH is expensive.
    #[test]
    fn stiffer_fluid_needs_a_shorter_timestep() {
        let mut soft = Fluid::new(20.0, 10.0);
        soft.stiffness = 100.0;
        let mut hard = Fluid::new(20.0, 10.0);
        hard.stiffness = 10_000.0;
        assert!(hard.stable_dt() < soft.stable_dt());
    }

    /// Sanity: the 2-D constants really are 2-D. A 3-D poly6 constant would be
    /// 315/(64 pi h^9) and give a wildly different peak value.
    #[test]
    fn the_kernel_uses_two_dimensional_constants() {
        let h = 2.0;
        let peak = w_poly6(0.0, h);
        let expect_2d = 4.0 / (PI * h.powi(8)) * h.powi(6);
        assert!(close(peak, expect_2d, 1e-12));
    }
}
