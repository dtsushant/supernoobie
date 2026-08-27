//! # Quaternions — rotation in three dimensions
//!
//! The sequel to `complex.rs`, and the reason this project began with complex
//! numbers rather than with vectors.
//!
//! ---
//!
//! ## 1. The question
//!
//! A complex number rotates the **plane**, and does it with a single
//! multiplication: `z' = e^(i theta) z`. Multiply lengths, add angles. Two
//! numbers, one multiply, done.
//!
//! So: what rotates **space**?
//!
//! The obvious answer is three angles — yaw, pitch and roll. It is the wrong
//! answer, and the fact that it is wrong is not obvious until it bites. Three
//! angles suffer **gimbal lock**: at certain orientations two of the three
//! stop being independent and a whole degree of freedom vanishes. There is a
//! test for it below, and Apollo 11's inertial platform had the same problem
//! with real hardware.
//!
//! Hamilton spent thirteen years looking for a three-number system that
//! multiplied properly, and there isn't one. In 1843 he realised you need
//! **four**, and that the multiplication cannot commute. He carved the
//! defining relation into Broom Bridge:
//!
//! ```text
//! i^2 = j^2 = k^2 = i j k = -1
//! ```
//!
//! Everything else follows from that single line — including, if you expand
//! it, `ij = k`, `jk = i`, `ki = j`, and crucially `ji = -k`. **Order matters.**
//! It has to: rotating about x then y genuinely is not the same as y then x.
//! Try it with a book. Non-commutativity is not an inconvenience of the
//! algebra, it is the physics being reported accurately.
//!
//! ## 2. A quaternion as a rotation
//!
//! ```text
//! q = cos(theta/2) + sin(theta/2) * (unit axis)
//! ```
//!
//! Note the **half** angle. It is not a convention you could drop; it falls
//! out of how the rotation is applied:
//!
//! ```text
//! v' = q v q*          "the sandwich product"
//! ```
//!
//! The vector is multiplied on both sides, so the rotation is applied twice —
//! and the half-angle exactly compensates. In return you get something the
//! plane never needed: a form that composes cleanly and never degenerates.
//!
//! ## 3. Composition is just multiplication
//!
//! Rotate by `q`, then by `p`, and the combined rotation is `p * q`. Chain a
//! thousand of them and it is still one quaternion — four numbers, no drift in
//! structure, and renormalising costs one square root.
//!
//! ## 4. The double cover, and why 720 degrees
//!
//! `q` and `-q` describe the **same rotation** — the sandwich has `q` on both
//! sides, so both signs cancel. Turn a full 360 degrees and the quaternion is
//! at `-1`, not `1`; you need **720** degrees to return to the identity.
//!
//! This is not a quirk of the notation. It is the fact that the group of 3-D
//! rotations is *not simply connected*, and it is measurable in the physical
//! world — it is why an electron must be turned twice to come back to itself,
//! and why you can untwist your arm by rotating a held glass through 720
//! degrees but not 360. Try it; it works.
//!
//! ## 5. Where the complex numbers went
//!
//! Restrict a quaternion to `w + z k` and you have exactly `complex.rs`, doing
//! exactly what it did: rotating one plane about one axis. A complex number is
//! a quaternion that has only ever heard of the z-axis.
//!
//! | | plane | space |
//! |---|---|---|
//! | rotate | `z' = e^(i t) z` | `v' = q v q*` |
//! | numbers | 2 | 4 |
//! | commutes? | yes | **no** |
//! | angle | `theta` | `theta/2` |
//! | degenerates? | never | never (Euler angles do) |

use crate::vec3::V3;
use std::ops::Mul;

/// `w + x i + y j + z k`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Q {
    pub w: f64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Q {
    pub const ONE: Q = Q { w: 1.0, x: 0.0, y: 0.0, z: 0.0 };
    pub const I: Q = Q { w: 0.0, x: 1.0, y: 0.0, z: 0.0 };
    pub const J: Q = Q { w: 0.0, x: 0.0, y: 1.0, z: 0.0 };
    pub const K: Q = Q { w: 0.0, x: 0.0, y: 0.0, z: 1.0 };

    pub const fn new(w: f64, x: f64, y: f64, z: f64) -> Self {
        Q { w, x, y, z }
    }

    /// A quaternion with zero real part — how a vector enters the algebra.
    pub fn from_vec(v: V3) -> Self {
        Q::new(0.0, v.x, v.y, v.z)
    }
    pub fn vec(self) -> V3 {
        V3::new(self.x, self.y, self.z)
    }

    /// **The rotation constructor.** Turn by `angle` about `axis`.
    /// Note `angle / 2` — see the module notes.
    pub fn from_axis_angle(axis: V3, angle: f64) -> Self {
        let a = axis.unit();
        if a == V3::ZERO {
            return Q::ONE;
        }
        let (s, c) = (angle * 0.5).sin_cos();
        Q::new(c, a.x * s, a.y * s, a.z * s)
    }

    /// The axis and angle this rotation represents. Inverse of the above,
    /// modulo the double cover.
    pub fn to_axis_angle(self) -> (V3, f64) {
        let q = self.unit();
        let s = q.vec().norm();
        if s < 1e-12 {
            return (V3::Z, 0.0);
        }
        (q.vec().scale(1.0 / s), 2.0 * s.atan2(q.w))
    }

    /// Conjugate: negate the vector part. For a **unit** quaternion this is
    /// also the inverse, which is why rotation never needs a division.
    pub fn conj(self) -> Q {
        Q::new(self.w, -self.x, -self.y, -self.z)
    }

    pub fn norm_sq(self) -> f64 {
        self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z
    }
    pub fn norm(self) -> f64 {
        self.norm_sq().sqrt()
    }

    pub fn scale(self, s: f64) -> Q {
        Q::new(self.w * s, self.x * s, self.y * s, self.z * s)
    }

    /// Renormalise. Numerical drift slowly inflates or shrinks a quaternion;
    /// one square root per step fixes it forever. Compare a rotation *matrix*,
    /// which drifts out of orthogonality and needs Gram-Schmidt to repair.
    pub fn unit(self) -> Q {
        let n = self.norm();
        if n < 1e-15 { Q::ONE } else { self.scale(1.0 / n) }
    }

    pub fn add(self, o: Q) -> Q {
        Q::new(self.w + o.w, self.x + o.x, self.y + o.y, self.z + o.z)
    }

    /// 4-D dot product. Its sign says whether two quaternions are on the same
    /// side of the double cover, which `slerp` needs to know.
    pub fn dot(self, o: Q) -> f64 {
        self.w * o.w + self.x * o.x + self.y * o.y + self.z * o.z
    }

    /// **Rotate a vector: `q v q*`.**
    ///
    /// Written out rather than as three multiplications, because the expanded
    /// form is what every engine actually ships:
    ///
    /// ```text
    /// v' = v + 2 * ( qv x (qv x v + w v) )
    /// ```
    pub fn rotate(self, v: V3) -> V3 {
        let u = self.vec();
        let t = u.cross(v).scale(2.0);
        v + t.scale(self.w) + u.cross(t)
    }

    /// Rotate the other way.
    pub fn rotate_inv(self, v: V3) -> V3 {
        self.conj().rotate(v)
    }

    /// **Spherical linear interpolation** — the shortest arc between two
    /// orientations, traversed at constant angular speed.
    ///
    /// Plain component-wise interpolation would cut *through* the sphere: the
    /// path is still a valid rotation once renormalised, but the speed
    /// surges in the middle. Slerp stays on the surface.
    ///
    /// The sign flip matters. `q` and `-q` are the same rotation, so without
    /// it you have a 50% chance of taking the 300-degree route instead of the
    /// 60-degree one.
    pub fn slerp(self, other: Q, t: f64) -> Q {
        let a = self.unit();
        let mut b = other.unit();
        let mut d = a.dot(b);
        if d < 0.0 {
            b = b.scale(-1.0); // take the short way round
            d = -d;
        }
        if d > 0.9995 {
            // nearly parallel: the arc is indistinguishable from the chord,
            // and the formula below would divide by ~0
            return a.add(b.add(a.scale(-1.0)).scale(t)).unit();
        }
        let theta = d.clamp(-1.0, 1.0).acos();
        let s = theta.sin();
        a.scale(((1.0 - t) * theta).sin() / s)
            .add(b.scale((t * theta).sin() / s))
            .unit()
    }

    /// Yaw-pitch-roll (intrinsic Z-Y-X) into a quaternion.
    ///
    /// Provided mainly so that [`Q::gimbal_lock_severity`] can demonstrate
    /// what is wrong with it.
    pub fn from_euler(yaw: f64, pitch: f64, roll: f64) -> Q {
        let (sy, cy) = (yaw * 0.5).sin_cos();
        let (sp, cp) = (pitch * 0.5).sin_cos();
        let (sr, cr) = (roll * 0.5).sin_cos();
        Q::new(
            cr * cp * cy + sr * sp * sy,
            sr * cp * cy - cr * sp * sy,
            cr * sp * cy + sr * cp * sy,
            cr * cp * sy - sr * sp * cy,
        )
    }

    /// Back to yaw, pitch, roll. Note the clamp: at pitch = +/-90 degrees the
    /// recovery is genuinely ambiguous, and this is where gimbal lock lives.
    pub fn to_euler(self) -> (f64, f64, f64) {
        let q = self.unit();
        let sinp = 2.0 * (q.w * q.y - q.z * q.x);
        let pitch = sinp.clamp(-1.0, 1.0).asin();
        let roll = (2.0 * (q.w * q.x + q.y * q.z))
            .atan2(1.0 - 2.0 * (q.x * q.x + q.y * q.y));
        let yaw = (2.0 * (q.w * q.z + q.x * q.y))
            .atan2(1.0 - 2.0 * (q.y * q.y + q.z * q.z));
        (yaw, pitch, roll)
    }

    /// How close a set of Euler angles is to losing a degree of freedom.
    ///
    /// Returns 0 when yaw and roll are fully independent, and 1 at the lock,
    /// where turning yaw and turning roll produce *the same motion* and the
    /// three angles can only reach a two-dimensional set of orientations.
    ///
    /// It is simply `|sin(pitch)|` — but computed by measuring how nearly
    /// parallel the two axes have become, which is what actually goes wrong.
    pub fn gimbal_lock_severity(pitch: f64) -> f64 {
        pitch.sin().abs()
    }

    /// Columns of the equivalent rotation matrix — where the basis vectors go.
    pub fn basis(self) -> (V3, V3, V3) {
        (self.rotate(V3::X), self.rotate(V3::Y), self.rotate(V3::Z))
    }
}

impl Mul for Q {
    type Output = Q;
    /// **The Hamilton product.** Written in vector form it is
    ///
    /// ```text
    /// (w1, v1)(w2, v2) = (w1 w2 - v1 . v2,  w1 v2 + w2 v1 + v1 x v2)
    /// ```
    ///
    /// The cross product is the whole story: it is the only term that changes
    /// when you swap the arguments, and so it is precisely why quaternion
    /// multiplication does not commute — and why it can express rotation at
    /// all.
    fn mul(self, o: Q) -> Q {
        Q::new(
            self.w * o.w - self.x * o.x - self.y * o.y - self.z * o.z,
            self.w * o.x + self.x * o.w + self.y * o.z - self.z * o.y,
            self.w * o.y - self.x * o.z + self.y * o.w + self.z * o.x,
            self.w * o.z + self.x * o.y - self.y * o.x + self.z * o.w,
        )
    }
}

impl std::fmt::Display for Q {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.4} {:+.4}i {:+.4}j {:+.4}k", self.w, self.x, self.y, self.z)
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }
    fn close_v(a: V3, b: V3) -> bool {
        close(a.x, b.x) && close(a.y, b.y) && close(a.z, b.z)
    }
    fn close_q(a: Q, b: Q) -> bool {
        close(a.w, b.w) && close(a.x, b.x) && close(a.y, b.y) && close(a.z, b.z)
    }

    /// Hamilton's bridge inscription, as an assertion.
    #[test]
    fn i_squared_j_squared_k_squared_and_ijk_are_all_minus_one() {
        let minus_one = Q::new(-1.0, 0.0, 0.0, 0.0);
        assert!(close_q(Q::I * Q::I, minus_one));
        assert!(close_q(Q::J * Q::J, minus_one));
        assert!(close_q(Q::K * Q::K, minus_one));
        assert!(close_q(Q::I * Q::J * Q::K, minus_one));
    }

    /// Everything else follows from that one line — including the sign flips.
    #[test]
    fn the_multiplication_table_follows() {
        assert!(close_q(Q::I * Q::J, Q::K));
        assert!(close_q(Q::J * Q::K, Q::I));
        assert!(close_q(Q::K * Q::I, Q::J));
        assert!(close_q(Q::J * Q::I, Q::K.scale(-1.0)));
        assert!(close_q(Q::K * Q::J, Q::I.scale(-1.0)));
        assert!(close_q(Q::I * Q::K, Q::J.scale(-1.0)));
    }

    /// Non-commutativity is not a defect. Rotating x-then-y really does differ
    /// from y-then-x, and the algebra has to say so.
    #[test]
    fn rotation_does_not_commute_and_the_algebra_agrees() {
        let rx = Q::from_axis_angle(V3::X, PI / 2.0);
        let ry = Q::from_axis_angle(V3::Y, PI / 2.0);
        assert!(!close_q(rx * ry, ry * rx));
        // and the two orders really do send a vector to different places
        let v = V3::Z;
        assert!(!close_v((rx * ry).rotate(v), (ry * rx).rotate(v)));
    }

    /// A quarter turn about z must send x to y.
    #[test]
    fn a_quarter_turn_about_z_sends_x_to_y() {
        let q = Q::from_axis_angle(V3::Z, PI / 2.0);
        assert!(close_v(q.rotate(V3::X), V3::Y));
        assert!(close_v(q.rotate(V3::Y), -V3::X));
        assert!(close_v(q.rotate(V3::Z), V3::Z)); // the axis is fixed
    }

    /// Rotation is rigid: lengths and angles survive it.
    #[test]
    fn rotation_preserves_lengths_and_angles() {
        let q = Q::from_axis_angle(V3::new(1.0, 2.0, -3.0), 1.234);
        let a = V3::new(3.0, -1.0, 2.0);
        let b = V3::new(0.5, 4.0, 1.0);
        assert!(close(q.rotate(a).norm(), a.norm()));
        assert!(close(q.rotate(a).dot(q.rotate(b)), a.dot(b)));
        // and it preserves handedness - a reflection would flip this sign
        assert!(close_v(q.rotate(a).cross(q.rotate(b)), q.rotate(a.cross(b))));
    }

    /// For a unit quaternion the conjugate IS the inverse, so rotating and
    /// un-rotating costs no division.
    #[test]
    fn the_conjugate_is_the_inverse_for_unit_quaternions() {
        let q = Q::from_axis_angle(V3::new(2.0, -1.0, 0.5), 2.1);
        assert!(close_q(q * q.conj(), Q::ONE));
        let v = V3::new(1.0, 2.0, 3.0);
        assert!(close_v(q.rotate_inv(q.rotate(v)), v));
    }

    /// Composition is multiplication, and the order is "second * first".
    #[test]
    fn composition_is_multiplication_in_reverse_order() {
        let a = Q::from_axis_angle(V3::X, 0.7);
        let b = Q::from_axis_angle(V3::Y, -1.3);
        let v = V3::new(1.0, -2.0, 0.5);
        assert!(close_v((b * a).rotate(v), b.rotate(a.rotate(v))));
    }

    /// **The double cover.** A full turn lands on -1, not 1; two full turns
    /// are needed to come home. Both signs still rotate identically.
    #[test]
    fn a_full_turn_gives_minus_one_and_two_turns_give_one() {
        let full = Q::from_axis_angle(V3::Z, 2.0 * PI);
        let twice = Q::from_axis_angle(V3::Z, 4.0 * PI);
        assert!(close_q(full, Q::ONE.scale(-1.0)), "{full}");
        assert!(close_q(twice, Q::ONE), "{twice}");

        // ...yet -q and q are the same rotation, because the sandwich has q
        // on both sides and the signs cancel
        let q = Q::from_axis_angle(V3::new(1.0, 1.0, 0.0), 0.9);
        let v = V3::new(2.0, 0.0, -1.0);
        assert!(close_v(q.rotate(v), q.scale(-1.0).rotate(v)));
    }

    /// The half angle is real: turning by `theta` puts `cos(theta/2)` in the
    /// real part, not `cos(theta)`.
    #[test]
    fn the_stored_angle_is_halved() {
        let q = Q::from_axis_angle(V3::Z, PI / 3.0); // 60 degrees
        assert!(close(q.w, (PI / 6.0).cos()));
        let (axis, angle) = q.to_axis_angle();
        assert!(close_v(axis, V3::Z));
        assert!(close(angle, PI / 3.0));
    }

    /// ★ **Gimbal lock.** At pitch = 90 degrees, yaw and roll stop being
    /// independent: only their difference matters, so a whole degree of
    /// freedom is gone. Two very different-looking angle triples produce the
    /// identical orientation.
    #[test]
    fn euler_angles_lose_a_degree_of_freedom_at_ninety_degrees() {
        let a = Q::from_euler(0.6, PI / 2.0, 0.0);
        let b = Q::from_euler(0.0, PI / 2.0, -0.6);
        let v = V3::new(1.0, 2.0, 3.0);
        assert!(
            close_v(a.rotate(v), b.rotate(v)),
            "yaw and roll should be interchangeable at the lock"
        );
        assert!(close(Q::gimbal_lock_severity(PI / 2.0), 1.0));

        // away from the lock they are properly independent again
        let c = Q::from_euler(0.6, 0.3, 0.0);
        let d = Q::from_euler(0.0, 0.3, -0.6);
        assert!(!close_v(c.rotate(v), d.rotate(v)));
        assert!(Q::gimbal_lock_severity(0.3) < 0.3);
    }

    /// Quaternions have no such failure mode: no orientation is special, and
    /// the axis-angle round trip works everywhere including at the pole.
    #[test]
    fn quaternions_have_no_lock_anywhere() {
        for k in 0..64 {
            let t = k as f64 / 64.0 * 2.0 * PI;
            for axis in [V3::X, V3::Y, V3::Z, V3::new(1.0, 1.0, 1.0)] {
                let q = Q::from_axis_angle(axis, t);
                assert!(close(q.norm(), 1.0), "lost normalisation at {t}");
                let v = V3::new(0.3, -0.7, 1.1);
                assert!(close(q.rotate(v).norm(), v.norm()));
            }
        }
    }

    /// Slerp hits both ends exactly and takes the halfway orientation at
    /// halfway.
    #[test]
    fn slerp_reaches_its_endpoints_and_the_true_midpoint() {
        let a = Q::from_axis_angle(V3::Z, 0.0);
        let b = Q::from_axis_angle(V3::Z, PI / 2.0);
        assert!(close_q(a.slerp(b, 0.0), a));
        assert!(close_q(a.slerp(b, 1.0), b));
        let mid = a.slerp(b, 0.5);
        assert!(close_q(mid, Q::from_axis_angle(V3::Z, PI / 4.0)));
    }

    /// The point of slerp: **constant angular speed**. Naive component-wise
    /// interpolation reaches the same endpoints but surges in the middle.
    #[test]
    fn slerp_turns_at_a_constant_rate_and_lerp_does_not() {
        let a = Q::ONE;
        let b = Q::from_axis_angle(V3::Z, 2.5); // a wide arc, where it shows
        let angle_at = |q: Q| {
            let d = a.dot(q.unit()).abs().clamp(-1.0, 1.0);
            2.0 * d.acos()
        };
        let lerp = |t: f64| a.scale(1.0 - t).add(b.scale(t)).unit();

        let n = 24;
        let (mut s_min, mut s_max) = (f64::MAX, 0.0f64);
        let (mut l_min, mut l_max) = (f64::MAX, 0.0f64);
        let mut prev_s = angle_at(a.slerp(b, 0.0));
        let mut prev_l = angle_at(lerp(0.0));
        for i in 1..=n {
            let t = i as f64 / n as f64;
            let s = angle_at(a.slerp(b, t));
            let l = angle_at(lerp(t));
            let (ds, dl) = (s - prev_s, l - prev_l);
            s_min = s_min.min(ds);
            s_max = s_max.max(ds);
            l_min = l_min.min(dl);
            l_max = l_max.max(dl);
            prev_s = s;
            prev_l = l;
        }
        assert!((s_max - s_min).abs() < 1e-6, "slerp was not constant-rate");
        assert!(l_max / l_min > 1.15, "lerp should visibly surge: {l_min}..{l_max}");
    }

    /// Slerp must take the short way round even when the inputs are on
    /// opposite sides of the double cover.
    #[test]
    fn slerp_takes_the_short_arc_across_the_double_cover() {
        let a = Q::from_axis_angle(V3::Z, 0.2);
        let b = Q::from_axis_angle(V3::Z, 0.6).scale(-1.0); // same rotation, flipped sign
        let mid = a.slerp(b, 0.5);
        let (_, angle) = mid.to_axis_angle();
        assert!(
            (angle - 0.4).abs() < 1e-6,
            "took the long way: midpoint angle {angle}, wanted 0.4"
        );
    }

    /// Chaining thousands of rotations must not degrade the quaternion - one
    /// renormalisation per step is all it takes. A rotation matrix would drift
    /// out of orthogonality and need Gram-Schmidt.
    #[test]
    fn long_chains_stay_normalised() {
        let step = Q::from_axis_angle(V3::new(1.0, 2.0, 3.0), 0.01);
        let mut q = Q::ONE;
        for _ in 0..100_000 {
            q = (q * step).unit();
        }
        assert!(close(q.norm(), 1.0));
        let v = V3::new(1.0, 0.0, 0.0);
        assert!(close(q.rotate(v).norm(), 1.0));
    }

    /// A quaternion restricted to the k axis IS a complex number, doing what
    /// `complex.rs` does. The plane case was never a different subject.
    #[test]
    fn restricted_to_one_axis_it_is_just_a_complex_number() {
        use crate::complex::Cx;
        let theta = 0.7;
        let q = Q::from_axis_angle(V3::Z, theta);
        let z = Cx::expi(theta);
        // the quaternion holds the HALF angle...
        assert!(close(q.w, (theta / 2.0).cos()));
        assert!(close(q.z, (theta / 2.0).sin()));
        // ...but rotates a vector by the full one, matching the complex form
        let v = V3::new(2.0, 1.0, 0.0);
        let r = q.rotate(v);
        let c = Cx::new(v.x, v.y) * z;
        assert!(close(r.x, c.re) && close(r.y, c.im) && close(r.z, 0.0));
    }
}
