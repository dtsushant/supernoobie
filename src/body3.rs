//! # Rigid-body rotation in three dimensions
//!
//! Physics only — nothing here draws.
//!
//! ---
//!
//! ## Why spinning is harder than moving
//!
//! Linear motion is easy because mass is a single number: `F = m a`, and `m`
//! does not care which way you push.
//!
//! Rotation is not like that. How hard a body is to spin depends on the axis —
//! a pencil spins easily about its length and reluctantly end-over-end. So the
//! "mass" of rotation is a **tensor**, and in the body's own principal axes it
//! is three numbers:
//!
//! ```text
//! solid box a x b x c:   I = m/12 * (b^2+c^2,  a^2+c^2,  a^2+b^2)
//! ```
//!
//! ## Euler's equations
//!
//! Newton's law for rotation is `dL/dt = torque`, where `L = I omega` is
//! angular momentum. But `I` is only constant in the **body frame**, which is
//! itself turning — and differentiating in a rotating frame adds a term:
//!
//! ```text
//! I omega_dot  =  torque  -  omega x (I omega)
//!                            \______________/
//!                             the gyroscopic term
//! ```
//!
//! That cross product is the whole difficulty. It is zero when `omega` is
//! parallel to `I omega` — that is, when you spin about a **principal axis** —
//! and non-zero otherwise. It is why a wobbling body keeps wobbling, why a
//! gyroscope precesses instead of falling, and why the next result happens at
//! all.
//!
//! ## The intermediate axis theorem
//!
//! A body with three *different* moments of inertia has three axes it can spin
//! about cleanly. Spin it about the **largest** or the **smallest** and it
//! stays put. Spin it about the middle one and it is **unstable**: the
//! slightest wobble grows, the body flips end over end, settles, and flips
//! again — forever, with no torque and no energy change.
//!
//! Throw a book, a phone, or a tennis racket and you can see it. Cosmonaut
//! Vladimir Dzhanibekov noticed a wingnut doing it in orbit in 1985 and the
//! footage was classified for a decade, apparently because it looked like the
//! object was doing something impossible.
//!
//! It is not impossible; it is `omega x (I omega)` with a minus sign in the
//! wrong place. Linearise Euler's equations about each axis and the growth
//! rate is `sqrt((I2-I1)(I2-I3))/...`, which is real — exponential growth —
//! only for the middle axis. `intermediate_axis_is_unstable` asserts it.
//!
//! ## Integrating the orientation
//!
//! Angular velocity is not the derivative of any angle triple, so orientation
//! cannot simply be accumulated. The quaternion form is clean:
//!
//! ```text
//! q_dot = 1/2 * q * (0, omega_body)
//! ```
//!
//! Step it, renormalise, and that is the whole update. Compare the same job
//! with a rotation matrix: nine numbers drifting out of orthogonality, needing
//! Gram-Schmidt to repair. Four numbers and one square root wins.

use crate::quat::Q;
use crate::vec3::V3;

#[derive(Clone, Copy, Debug)]
pub struct Body3 {
    /// Orientation: body frame -> world frame.
    pub q: Q,
    /// Angular velocity, **in the body frame** — that is where `I` is
    /// constant, so that is where Euler's equations are written.
    pub omega: V3,
    /// Principal moments of inertia, along the body's own axes.
    pub inertia: V3,
    pub mass: f64,
    /// Half-extents, kept only so a renderer can draw the thing.
    pub half: V3,
}

impl Body3 {
    /// A solid rectangular box of the given full dimensions.
    pub fn box_body(size: V3, mass: f64) -> Self {
        let (a, b, c) = (size.x, size.y, size.z);
        let k = mass / 12.0;
        Body3 {
            q: Q::ONE,
            omega: V3::ZERO,
            inertia: V3::new(
                k * (b * b + c * c),
                k * (a * a + c * c),
                k * (a * a + b * b),
            ),
            mass,
            half: size.scale(0.5),
        }
    }

    /// `I omega`, in the body frame.
    pub fn angular_momentum_body(&self) -> V3 {
        self.inertia.mul_each(self.omega)
    }

    /// The same vector expressed in the world. **With no torque this is
    /// conserved exactly**, even while the body tumbles wildly — it is the
    /// sharpest check that the integration is honest.
    pub fn angular_momentum(&self) -> V3 {
        self.q.rotate(self.angular_momentum_body())
    }

    /// `T = 1/2 omega . (I omega)`. Also conserved without torque.
    pub fn energy(&self) -> f64 {
        0.5 * self.omega.dot(self.angular_momentum_body())
    }

    /// Euler's equations, rearranged for `omega_dot`.
    fn omega_dot(&self, w: V3, torque_body: V3) -> V3 {
        let l = self.inertia.mul_each(w);
        let rhs = torque_body - w.cross(l);
        V3::new(
            rhs.x / self.inertia.x,
            rhs.y / self.inertia.y,
            rhs.z / self.inertia.z,
        )
    }

    /// Advance by `dt`. `torque_body` is in body coordinates; pass `V3::ZERO`
    /// for free rotation.
    ///
    /// RK4 on `omega`, because the gyroscopic term is exactly the sort of
    /// curved trajectory a first-order method smears — and smearing it here
    /// silently destroys the conservation laws the tests rely on.
    pub fn step(&mut self, dt: f64, torque_body: V3) {
        let w = self.omega;
        let k1 = self.omega_dot(w, torque_body);
        let k2 = self.omega_dot(w + k1.scale(dt * 0.5), torque_body);
        let k3 = self.omega_dot(w + k2.scale(dt * 0.5), torque_body);
        let k4 = self.omega_dot(w + k3.scale(dt), torque_body);
        let dw = (k1 + k2.scale(2.0) + k3.scale(2.0) + k4).scale(dt / 6.0);

        // integrate the orientation with the average angular velocity
        let w_mid = w + dw.scale(0.5);
        // q_dot = 1/2 q (0, omega_body)
        let qd = (self.q * Q::from_vec(w_mid)).scale(0.5);
        self.q = self.q.add(qd.scale(dt)).unit();

        self.omega = w + dw;
    }

    /// Where the body's own axes point in the world.
    pub fn axes(&self) -> (V3, V3, V3) {
        self.q.basis()
    }

    /// The eight corners of the box, in world coordinates.
    pub fn corners(&self) -> [V3; 8] {
        let h = self.half;
        let mut out = [V3::ZERO; 8];
        for (n, c) in out.iter_mut().enumerate() {
            let s = |bit: usize| if n & (1 << bit) == 0 { -1.0 } else { 1.0 };
            *c = self.q.rotate(V3::new(h.x * s(0), h.y * s(1), h.z * s(2)));
        }
        out
    }

    /// Index pairs of the twelve box edges, for a wireframe.
    pub const EDGES: [(usize, usize); 12] = [
        (0, 1), (0, 2), (0, 4), (1, 3), (1, 5), (2, 3),
        (2, 6), (3, 7), (4, 5), (4, 6), (5, 7), (6, 7),
    ];

    /// Which principal axis is the intermediate one — the unstable one.
    pub fn intermediate_axis(&self) -> usize {
        let i = [self.inertia.x, self.inertia.y, self.inertia.z];
        let mut order = [0usize, 1, 2];
        order.sort_by(|&a, &b| i[a].partial_cmp(&i[b]).unwrap());
        order[1]
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    /// The textbook box formula, and the ordering that follows from it: the
    /// long axis is the easiest to spin about.
    #[test]
    fn box_inertia_matches_the_textbook() {
        let b = Body3::box_body(V3::new(2.0, 4.0, 6.0), 12.0);
        assert!(close(b.inertia.x, 16.0 + 36.0, 1e-9)); // m/12 = 1
        assert!(close(b.inertia.y, 4.0 + 36.0, 1e-9));
        assert!(close(b.inertia.z, 4.0 + 16.0, 1e-9));
        // longest side (z=6) -> smallest moment about... z? no: about the
        // axis ALONG which the body is longest, it is easiest to spin
        assert!(b.inertia.z < b.inertia.y && b.inertia.y < b.inertia.x);
    }

    /// A sphere-like body (all three moments equal) has no gyroscopic term at
    /// all, so it spins about any axis forever without wobbling.
    #[test]
    fn a_symmetric_body_never_wobbles() {
        let mut b = Body3::box_body(V3::new(2.0, 2.0, 2.0), 3.0);
        b.omega = V3::new(1.0, 2.0, -0.5);
        let w0 = b.omega;
        for _ in 0..20_000 {
            b.step(1e-3, V3::ZERO);
        }
        assert!(close(b.omega.x, w0.x, 1e-9));
        assert!(close(b.omega.y, w0.y, 1e-9));
        assert!(close(b.omega.z, w0.z, 1e-9));
    }

    /// ★ Angular momentum is conserved **in the world frame** with no torque —
    /// even while the body tumbles. In the body frame it moves; in the world
    /// it does not. This is the sharpest test of the whole file.
    #[test]
    fn angular_momentum_is_conserved_in_the_world_frame() {
        let mut b = Body3::box_body(V3::new(1.0, 3.0, 5.0), 2.0);
        b.omega = V3::new(0.3, 4.0, 0.25); // deliberately tumbling
        let l0 = b.angular_momentum();
        let e0 = b.energy();
        for _ in 0..60_000 {
            b.step(2e-4, V3::ZERO);
        }
        let l1 = b.angular_momentum();
        assert!(close(l0.x, l1.x, 1e-5), "Lx {} -> {}", l0.x, l1.x);
        assert!(close(l0.y, l1.y, 1e-5), "Ly {} -> {}", l0.y, l1.y);
        assert!(close(l0.z, l1.z, 1e-5), "Lz {} -> {}", l0.z, l1.z);
        assert!(close(e0, b.energy(), 1e-6), "energy {e0} -> {}", b.energy());
    }

    /// ...while in the BODY frame the same vector wanders. Both statements are
    /// true at once, and confusing them is the classic error.
    #[test]
    fn in_the_body_frame_the_momentum_vector_moves() {
        let mut b = Body3::box_body(V3::new(1.0, 3.0, 5.0), 2.0);
        b.omega = V3::new(0.3, 4.0, 0.25);
        let l0 = b.angular_momentum_body();
        for _ in 0..4000 {
            b.step(2e-4, V3::ZERO);
        }
        let l1 = b.angular_momentum_body();
        assert!((l1 - l0).norm() > 0.1, "body-frame L should not be constant");
    }

    /// Spinning about the LARGEST or SMALLEST moment is stable: a small wobble
    /// stays small.
    #[test]
    fn the_extreme_axes_are_stable() {
        for axis in [0usize, 2] {
            let mut b = Body3::box_body(V3::new(1.0, 3.0, 5.0), 2.0);
            let mut w = V3::new(0.02, 0.02, 0.02);
            match axis {
                0 => w.x = 5.0,
                _ => w.z = 5.0,
            }
            b.omega = w;
            let mut worst: f64 = 0.0;
            for _ in 0..40_000 {
                b.step(2e-4, V3::ZERO);
                let c = [b.omega.x, b.omega.y, b.omega.z][axis];
                worst = worst.max((5.0 - c).abs());
            }
            assert!(worst < 0.5, "axis {axis} drifted by {worst}");
        }
    }

    /// ★★ **The intermediate axis theorem.** Spin about the middle moment and
    /// the body flips right over: the dominant component of `omega` reverses
    /// sign, repeatedly, with no torque and no change in energy.
    #[test]
    fn intermediate_axis_is_unstable() {
        let mut b = Body3::box_body(V3::new(1.0, 3.0, 5.0), 2.0);
        assert_eq!(b.intermediate_axis(), 1, "y should be the middle moment");

        b.omega = V3::new(0.03, 5.0, 0.03); // spin about y, barely perturbed
        let e0 = b.energy();
        let l0 = b.angular_momentum().norm();

        let mut flipped = false;
        let mut min_y = f64::MAX;
        for _ in 0..120_000 {
            b.step(2e-4, V3::ZERO);
            min_y = min_y.min(b.omega.y);
            if b.omega.y < -1.0 {
                flipped = true;
            }
        }
        assert!(flipped, "the racket never flipped (min omega_y = {min_y})");
        // ...and it did so while conserving everything it should
        assert!(close(e0, b.energy(), 1e-5), "energy changed during the flip");
        assert!(close(l0, b.angular_momentum().norm(), 1e-5));
    }

    /// The orientation quaternion must not drift out of normalisation, however
    /// long it tumbles.
    #[test]
    fn the_orientation_stays_a_unit_quaternion() {
        let mut b = Body3::box_body(V3::new(1.0, 3.0, 5.0), 2.0);
        b.omega = V3::new(0.03, 5.0, 0.03);
        for _ in 0..200_000 {
            b.step(2e-4, V3::ZERO);
        }
        assert!(close(b.q.norm(), 1.0, 1e-12));
        let (ax, ay, az) = b.axes();
        // and the body frame is still orthonormal
        assert!(close(ax.norm(), 1.0, 1e-9));
        assert!(close(ax.dot(ay), 0.0, 1e-9));
        assert!(close(ax.cross(ay).dot(az), 1.0, 1e-9)); // still right-handed
    }

    /// A torque about a principal axis simply spins the body up, at
    /// `alpha = tau / I`.
    #[test]
    fn torque_about_a_principal_axis_spins_it_up() {
        let mut b = Body3::box_body(V3::new(2.0, 2.0, 2.0), 6.0);
        let i = b.inertia.z;
        let tau = 3.0;
        let t = 2.0;
        let dt = 1e-4;
        for _ in 0..(t / dt) as usize {
            b.step(dt, V3::new(0.0, 0.0, tau));
        }
        assert!(close(b.omega.z, tau / i * t, 1e-6));
    }
}
