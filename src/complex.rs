//! # Complex numbers, from the definition
//!
//! One rule: `i^2 = -1`. Everything in this file is a consequence of it.
//!
//! Geometric reading: a complex number is a **length and an angle**.
//! Multiplication multiplies the lengths and ADDS the angles. That single fact
//! is why this type can drive an entire animation with no rotation matrices.

// This is a general-purpose number type: a complete, symmetric API is the
// point, even where iteration 1 does not yet call every method.
#![allow(dead_code)]

use std::ops::{Add, Div, Mul, Neg, Sub};

/// A complex number `re + im * i`, equivalently the point `(re, im)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cx {
    pub re: f64,
    pub im: f64,
}

impl Cx {
    /// The imaginary unit. Multiplying by this is a **90 degree turn**.
    pub const I: Cx = Cx { re: 0.0, im: 1.0 };
    pub const ONE: Cx = Cx { re: 1.0, im: 0.0 };
    pub const ZERO: Cx = Cx { re: 0.0, im: 0.0 };

    pub const fn new(re: f64, im: f64) -> Self {
        Cx { re, im }
    }

    /// **Euler's formula.**  `e^(i*theta) = cos(theta) + i*sin(theta)`
    ///
    /// Read it as an instruction, not a multiplication: *start at 1, rotate by
    /// `theta` radians.* The result always has modulus 1, so it is pure
    /// direction. This is the single most-used function in the crate.
    ///
    /// Radians are not a convention here. `e^(i*theta)` traverses the unit
    /// circle at unit speed, so `theta` IS the arc length covered - which is
    /// exactly the definition of a radian. In degrees the formula is false.
    pub fn expi(theta: f64) -> Self {
        Cx {
            re: theta.cos(),
            im: theta.sin(),
        }
    }

    /// Polar construction: length `r` pointing at angle `theta`.
    pub fn polar(r: f64, theta: f64) -> Self {
        Cx::expi(theta).scale(r)
    }

    /// Modulus `|z| = sqrt(re^2 + im^2)` - distance from the origin.
    /// `hypot` is used rather than `sqrt(a*a + b*b)` because it avoids
    /// overflow when the components are large.
    pub fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }

    /// Squared modulus. Note `|z|^2 = z * conj(z)`, always a positive real -
    /// this is the identity that makes division possible.
    pub fn abs_sq(self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    /// Argument `arg(z)` - the angle from the positive real axis, in
    /// `(-pi, pi]`.
    ///
    /// This MUST be `atan2(im, re)` and never `atan(im/re)`. The ratio
    /// `im/re` is identical for `z` and `-z`, so plain `atan` cannot tell a
    /// direction from its opposite and is wrong in two of the four quadrants.
    pub fn arg(self) -> f64 {
        self.im.atan2(self.re)
    }

    /// Conjugate - the mirror image across the real axis.
    pub fn conj(self) -> Self {
        Cx::new(self.re, -self.im)
    }

    /// Multiply by a real number: stretch, do not turn.
    pub fn scale(self, s: f64) -> Self {
        Cx::new(self.re * s, self.im * s)
    }

    /// Unit vector in the same direction (`z / |z|`). Returns `ZERO` for zero
    /// input, which has no defined direction.
    pub fn unit(self) -> Self {
        let m = self.abs();
        if m == 0.0 {
            Cx::ZERO
        } else {
            self.scale(1.0 / m)
        }
    }

    /// Rotate by `theta` radians about the origin.
    /// This is just `self * e^(i*theta)` - one multiplication, no matrix.
    pub fn rotate(self, theta: f64) -> Self {
        self * Cx::expi(theta)
    }
}

impl Add for Cx {
    type Output = Cx;
    /// Componentwise, exactly like vectors. Nothing interesting happens here -
    /// all the character of complex numbers is in multiplication.
    fn add(self, o: Cx) -> Cx {
        Cx::new(self.re + o.re, self.im + o.im)
    }
}

impl Sub for Cx {
    type Output = Cx;
    fn sub(self, o: Cx) -> Cx {
        Cx::new(self.re - o.re, self.im - o.im)
    }
}

impl Neg for Cx {
    type Output = Cx;
    fn neg(self) -> Cx {
        Cx::new(-self.re, -self.im)
    }
}

impl Mul for Cx {
    type Output = Cx;
    /// `(a + bi)(c + di) = (ac - bd) + (ad + bc)i`
    ///
    /// Expand normally, then substitute `i^2 = -1`. That substitution is the
    /// entire origin of the minus sign in `ac - bd`.
    ///
    /// In polar terms this same operation reads
    /// `r1*e^(i*t1) * r2*e^(i*t2) = (r1*r2) * e^(i*(t1+t2))`:
    /// **multiply the lengths, add the angles.**
    fn mul(self, o: Cx) -> Cx {
        Cx::new(
            self.re * o.re - self.im * o.im,
            self.re * o.im + self.im * o.re,
        )
    }
}

impl Div for Cx {
    type Output = Cx;
    /// `z / w = z * conj(w) / |w|^2`
    ///
    /// Multiply top and bottom by the denominator's conjugate. The point is
    /// that `w * conj(w) = |w|^2` is a positive REAL, so what remains is just
    /// a scalar division. If your denominator is ever negative or still
    /// complex, you have made a mistake.
    fn div(self, o: Cx) -> Cx {
        let d = o.abs_sq();
        (self * o.conj()).scale(1.0 / d)
    }
}

impl std::fmt::Display for Cx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.im >= 0.0 {
            write!(f, "{:.4} + {:.4}i", self.re, self.im)
        } else {
            write!(f, "{:.4} - {:.4}i", self.re, -self.im)
        }
    }
}

// ===========================================================================
// The tests are the point: each one pins a fact you derived by hand.
// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }
    fn close_c(a: Cx, b: Cx) -> bool {
        close(a.re, b.re) && close(a.im, b.im)
    }

    /// The definition itself.
    #[test]
    fn i_squared_is_minus_one() {
        assert!(close_c(Cx::I * Cx::I, -Cx::ONE));
    }

    /// The 4-cycle: each multiplication by i is a 90 degree turn, so four of
    /// them is a full revolution and you are back where you started.
    #[test]
    fn powers_of_i_cycle_every_four() {
        let mut z = Cx::ONE;
        let expect = [Cx::I, -Cx::ONE, -Cx::I, Cx::ONE];
        for e in expect {
            z = z * Cx::I;
            assert!(close_c(z, e));
        }
    }

    /// `e^(i*pi) = -1`. Go out one unit, turn a half circle.
    #[test]
    fn euler_identity() {
        assert!(close_c(Cx::expi(PI), -Cx::ONE));
    }

    /// A full turn returns home - the periodicity that makes complex `ln`
    /// multi-valued.
    #[test]
    fn full_turn_is_identity() {
        assert!(close_c(Cx::expi(2.0 * PI), Cx::ONE));
        for k in 0..8 {
            let t = k as f64 * 0.7;
            assert!(close_c(Cx::expi(t), Cx::expi(t + 2.0 * PI)));
        }
    }

    /// `e^(i*theta)` is always ON the unit circle - pure direction, no size.
    #[test]
    fn expi_has_unit_modulus() {
        for k in 0..24 {
            let t = k as f64 * 0.31;
            assert!(close(Cx::expi(t).abs(), 1.0));
        }
    }

    /// The headline property: multiplying ADDS the angles and MULTIPLIES the
    /// lengths. Everything the pulley does rests on this.
    #[test]
    fn multiplication_adds_angles_and_multiplies_lengths() {
        let z = Cx::polar(3.0, 0.4);
        let w = Cx::polar(2.0, 1.1);
        let p = z * w;
        assert!(close(p.abs(), 6.0));
        assert!(close(p.arg(), 1.5));
    }

    /// Multiplying by i turns exactly 90 degrees, without changing length.
    #[test]
    fn i_is_a_quarter_turn() {
        let z = Cx::new(3.0, 1.0);
        let t = z * Cx::I;
        assert!(close(t.abs(), z.abs()));
        assert!(close(t.arg() - z.arg(), std::f64::consts::FRAC_PI_2));
    }

    /// `z * conj(z) = |z|^2`, a positive real. The division trick in one line.
    #[test]
    fn conjugate_product_is_real_and_positive() {
        let z = Cx::new(1.0, -2.0);
        let p = z * z.conj();
        assert!(close(p.im, 0.0));
        assert!(close(p.re, 5.0)); // 1^2 + 2^2
    }

    /// Your homework #2, pinned as a test.
    #[test]
    fn division_matches_hand_working() {
        let q = Cx::new(2.0, 3.0) / Cx::new(1.0, -2.0);
        assert!(close(q.re, -4.0 / 5.0));
        assert!(close(q.im, 7.0 / 5.0));
    }

    /// Your homework #4: `(1+i)^8 = 16`, reached by squaring three times.
    #[test]
    fn one_plus_i_to_the_eighth_is_sixteen() {
        let z = Cx::new(1.0, 1.0);
        let z2 = z * z; //  2i
        let z4 = z2 * z2; // -4   <- the step that is easy to mislabel
        let z8 = z4 * z4; // 16
        assert!(close_c(z2, Cx::new(0.0, 2.0)));
        assert!(close_c(z4, Cx::new(-4.0, 0.0)));
        assert!(close_c(z8, Cx::new(16.0, 0.0)));
    }

    /// arg must handle every quadrant - the atan2 lesson.
    #[test]
    fn arg_is_correct_in_all_four_quadrants() {
        let q = std::f64::consts::FRAC_PI_4;
        assert!(close(Cx::new(1.0, 1.0).arg(), q));
        assert!(close(Cx::new(-1.0, 1.0).arg(), 3.0 * q));
        assert!(close(Cx::new(-1.0, -1.0).arg(), -3.0 * q));
        assert!(close(Cx::new(1.0, -1.0).arg(), -q));
    }
}
