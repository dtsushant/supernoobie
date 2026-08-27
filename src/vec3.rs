//! # Three-dimensional vectors
//!
//! Small and unglamorous, but note what changes on the way up from the plane:
//!
//! * The **dot** product survives unchanged — it is still `sum a_i b_i`, still
//!   `|a||b|cos(theta)`, and still measures agreement.
//! * The **cross** product changes character completely. In 2-D it was a
//!   *number* (`Im(conj(a) b)`, the signed area). In 3-D it is a *vector*,
//!   perpendicular to both inputs, whose length is that same area.
//!
//! That difference is exactly why rotation gets hard. In the plane there is
//! only one axis to turn about — out of the page — so an angle is enough, and
//! a single complex multiplication does the job. In space there are infinitely
//! many axes, and turning about one changes which way the others point.

use std::ops::{Add, Mul, Neg, Sub};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct V3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl V3 {
    pub const ZERO: V3 = V3 { x: 0.0, y: 0.0, z: 0.0 };
    pub const X: V3 = V3 { x: 1.0, y: 0.0, z: 0.0 };
    pub const Y: V3 = V3 { x: 0.0, y: 1.0, z: 0.0 };
    pub const Z: V3 = V3 { x: 0.0, y: 0.0, z: 1.0 };

    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        V3 { x, y, z }
    }

    /// Unchanged from two dimensions.
    pub fn dot(self, o: V3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    /// A **vector** now, not a number: perpendicular to both, right-handed,
    /// with length `|a||b|sin(theta)` — the area of the parallelogram they
    /// span. Anti-commutative: `a x b = -(b x a)`.
    pub fn cross(self, o: V3) -> V3 {
        V3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    pub fn norm_sq(self) -> f64 {
        self.dot(self)
    }
    pub fn norm(self) -> f64 {
        self.norm_sq().sqrt()
    }
    pub fn scale(self, s: f64) -> V3 {
        V3::new(self.x * s, self.y * s, self.z * s)
    }
    pub fn unit(self) -> V3 {
        let n = self.norm();
        if n < 1e-15 { V3::ZERO } else { self.scale(1.0 / n) }
    }
    /// Componentwise product — for applying a diagonal inertia tensor.
    pub fn mul_each(self, o: V3) -> V3 {
        V3::new(self.x * o.x, self.y * o.y, self.z * o.z)
    }
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

impl Add for V3 {
    type Output = V3;
    fn add(self, o: V3) -> V3 {
        V3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl Sub for V3 {
    type Output = V3;
    fn sub(self, o: V3) -> V3 {
        V3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl Neg for V3 {
    type Output = V3;
    fn neg(self) -> V3 {
        V3::new(-self.x, -self.y, -self.z)
    }
}
impl Mul<f64> for V3 {
    type Output = V3;
    fn mul(self, s: f64) -> V3 {
        self.scale(s)
    }
}

impl std::fmt::Display for V3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({:.4}, {:.4}, {:.4})", self.x, self.y, self.z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    /// The cross product is perpendicular to both of its inputs. That is what
    /// it is *for*, and it is worth asserting rather than trusting.
    #[test]
    fn the_cross_product_is_perpendicular_to_both() {
        let a = V3::new(1.0, 2.0, 3.0);
        let b = V3::new(-4.0, 5.0, 6.0);
        let c = a.cross(b);
        assert!(close(c.dot(a), 0.0));
        assert!(close(c.dot(b), 0.0));
    }

    /// Order matters, and reversing it flips the direction.
    #[test]
    fn the_cross_product_anticommutes() {
        let a = V3::new(1.0, 0.5, -2.0);
        let b = V3::new(0.0, 3.0, 1.0);
        assert_eq!(a.cross(b), -(b.cross(a)));
        assert_eq!(a.cross(a), V3::ZERO);
    }

    /// Right-handed convention: x cross y is z, not -z. Get this backwards and
    /// every rotation in the crate turns the wrong way.
    #[test]
    fn the_basis_is_right_handed() {
        assert_eq!(V3::X.cross(V3::Y), V3::Z);
        assert_eq!(V3::Y.cross(V3::Z), V3::X);
        assert_eq!(V3::Z.cross(V3::X), V3::Y);
    }

    /// `|a x b|` is the area of the parallelogram, so it peaks at 90 degrees
    /// and vanishes when the vectors are parallel.
    #[test]
    fn cross_length_is_the_parallelogram_area() {
        let a = V3::new(3.0, 0.0, 0.0);
        let b = V3::new(0.0, 4.0, 0.0);
        assert!(close(a.cross(b).norm(), 12.0));
        assert!(close(a.cross(a.scale(9.0)).norm(), 0.0));
    }
}
