//! # Eigenvectors, and PCA — the mathematics
//!
//! Maths only, as always.
//!
//! ---
//!
//! ## 1. The question
//!
//! A matrix transforms space: it takes every vector somewhere else, usually
//! pointing a different way. But for almost any matrix there are a few special
//! directions that come out **pointing exactly as they went in**, merely
//! stretched:
//!
//! ```text
//! A v = lambda v
//! ```
//!
//! `v` is an **eigenvector** — a direction the transformation leaves alone —
//! and `lambda` its **eigenvalue**, the stretch factor. They are the
//! transformation's own natural axes, the ones it would have chosen for
//! itself.
//!
//! ## 2. You have met the eigenvalue rule three times already
//!
//! Apply `A` repeatedly to an eigenvector and the eigenvalue just compounds:
//!
//! ```text
//! A^n v = lambda^n v
//! ```
//!
//! So `|lambda|` decides everything about the long run: below 1 it decays,
//! above 1 it explodes, exactly 1 it persists. That is **the same rule** as:
//!
//! | where | the quantity |
//! |---|---|
//! | `complex.rs` | `\|z\|` — powers of a complex number spiral in or out |
//! | `dynamics.rs` | `lambda = -zeta*wn + i*wd` — the damped oscillator |
//! | `rigid.rs` / integrators | whether explicit Euler gains energy |
//!
//! And it is not an analogy. The damped oscillator's `lambda` **is** an
//! eigenvalue: write `theta_ddot + 2 zeta wn theta_dot + wn^2 theta = 0` as a
//! 2x2 system in `(theta, theta_dot)` and solve `det(A - lambda I) = 0`. The
//! spiral you dragged around in Desmos was an eigenvalue with a non-zero
//! imaginary part.
//!
//! **Complex eigenvalues mean rotation.** A real eigenvalue says "this
//! direction is preserved"; a complex pair says "no direction is preserved,
//! the plane turns."
//!
//! ## 3. Symmetric matrices are the good case
//!
//! If `A = A^T` then — the **spectral theorem** — all its eigenvalues are
//! real, and its eigenvectors are **mutually perpendicular**. No rotation, no
//! complex parts, just three clean axes with a stretch along each.
//!
//! That matters because the two symmetric matrices worth knowing are both
//! already in this crate:
//!
//! * the **inertia tensor** from `body3.rs` — its eigenvectors are the
//!   principal axes of rotation, the ones Euler's equations are written in;
//! * the **covariance matrix** of a point cloud — its eigenvectors are the
//!   directions the data actually spreads along.
//!
//! Same theorem. One tells a tumbling box which way to spin; the other tells a
//! dataset which of its features are really one feature.
//!
//! ## 4. PCA in three lines
//!
//! ```text
//! 1. centre the data          x_i <- x_i - mean
//! 2. covariance               C = (1/n) sum x_i x_i^T
//! 3. eigen-decompose C        axes = eigenvectors, spread = sqrt(eigenvalues)
//! ```
//!
//! The eigenvector with the largest eigenvalue is the direction of greatest
//! variance. Keep the top few and you have thrown away dimensions while losing
//! as little spread as possible — which is dimensionality reduction, and is
//! also how you fit a tight oriented box around a mesh.
//!
//! ## 5. Two ways to actually compute them
//!
//! **Power iteration** — multiply a random vector by `A` over and over,
//! renormalising. Every application multiplies each eigen-component by its own
//! eigenvalue, so the largest one wins by a factor of `(l1/l2)^n` and the
//! others are ground away. Five lines, and it is literally the spiral from C1
//! run in a space of directions.
//!
//! **Jacobi rotations** — for a symmetric matrix, repeatedly find the biggest
//! off-diagonal entry and apply the plane rotation that zeroes it. Each
//! rotation disturbs the others slightly, so you sweep until the off-diagonal
//! mass is negligible. It is Gauss-Seidel again — the same "fix one, disturb
//! the rest, repeat" as the contact solver in `rigid.rs` and the constraint
//! solver in `soft.rs`.

use crate::vec3::V3;

/// A 3x3 matrix, row-major.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct M3(pub [[f64; 3]; 3]);

impl M3 {
    pub const ZERO: M3 = M3([[0.0; 3]; 3]);

    pub fn identity() -> M3 {
        M3([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]])
    }

    pub fn diag(d: V3) -> M3 {
        M3([[d.x, 0.0, 0.0], [0.0, d.y, 0.0], [0.0, 0.0, d.z]])
    }

    /// Outer product `v v^T` — the building block of a covariance matrix.
    pub fn outer(v: V3) -> M3 {
        let a = [v.x, v.y, v.z];
        let mut m = [[0.0; 3]; 3];
        for (i, mi) in m.iter_mut().enumerate() {
            for (j, mij) in mi.iter_mut().enumerate() {
                *mij = a[i] * a[j];
            }
        }
        M3(m)
    }

    pub fn mul_v(&self, v: V3) -> V3 {
        let a = [v.x, v.y, v.z];
        let r = |i: usize| self.0[i][0] * a[0] + self.0[i][1] * a[1] + self.0[i][2] * a[2];
        V3::new(r(0), r(1), r(2))
    }

    pub fn mul(&self, o: &M3) -> M3 {
        let mut m = [[0.0; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                m[i][j] = (0..3).map(|k| self.0[i][k] * o.0[k][j]).sum();
            }
        }
        M3(m)
    }

    pub fn transpose(&self) -> M3 {
        let mut m = [[0.0; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                m[i][j] = self.0[j][i];
            }
        }
        M3(m)
    }

    pub fn add(&self, o: &M3) -> M3 {
        let mut m = [[0.0; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                m[i][j] = self.0[i][j] + o.0[i][j];
            }
        }
        M3(m)
    }

    pub fn scale(&self, s: f64) -> M3 {
        let mut m = self.0;
        for r in m.iter_mut() {
            for x in r.iter_mut() {
                *x *= s;
            }
        }
        M3(m)
    }

    pub fn trace(&self) -> f64 {
        self.0[0][0] + self.0[1][1] + self.0[2][2]
    }

    /// Total magnitude off the diagonal — Jacobi drives this to zero.
    pub fn off_diagonal_norm(&self) -> f64 {
        let mut s = 0.0;
        for i in 0..3 {
            for j in 0..3 {
                if i != j {
                    s += self.0[i][j] * self.0[i][j];
                }
            }
        }
        s.sqrt()
    }

    pub fn is_symmetric(&self, tol: f64) -> bool {
        (0..3).all(|i| (0..3).all(|j| (self.0[i][j] - self.0[j][i]).abs() < tol))
    }

    /// **Power iteration.** Multiply and renormalise until the direction stops
    /// moving. Returns `(eigenvalue, eigenvector)` for the largest `|lambda|`.
    ///
    /// Each multiplication scales every eigen-component by its own eigenvalue,
    /// so after `n` steps the dominant one leads the runner-up by
    /// `(l1/l2)^n`. It converges slowly when those are close, and not at all
    /// when they are equal — there is a test for both.
    pub fn power_iteration(&self, iters: usize) -> (f64, V3) {
        // a start vector that is unlikely to be exactly orthogonal to the
        // eigenvector we want
        let mut v = V3::new(0.5773, 0.5774, 0.5775).unit();
        for _ in 0..iters {
            let w = self.mul_v(v);
            let n = w.norm();
            if n < 1e-300 {
                return (0.0, v);
            }
            v = w.scale(1.0 / n);
        }
        // Rayleigh quotient: the best eigenvalue estimate for this direction
        let lambda = v.dot(self.mul_v(v)) / v.norm_sq();
        (lambda, v)
    }

    /// **Jacobi eigendecomposition** for a symmetric matrix.
    ///
    /// Returns eigenvalues sorted **descending** with their eigenvectors —
    /// which is the order PCA wants, since the first is then the direction of
    /// greatest spread.
    ///
    /// Each sweep rotates away the largest off-diagonal entry. That disturbs
    /// the ones already zeroed, so it iterates — the same relaxation shape as
    /// the solvers in `rigid.rs` and `soft.rs`.
    pub fn symmetric_eigen(&self) -> (V3, [V3; 3]) {
        let mut a = self.0;
        let mut v = M3::identity().0; // accumulates the rotations

        for _ in 0..64 {
            // largest off-diagonal entry
            let (mut p, mut q, mut best) = (0usize, 1usize, 0.0);
            for i in 0..3 {
                for j in (i + 1)..3 {
                    if a[i][j].abs() > best {
                        best = a[i][j].abs();
                        p = i;
                        q = j;
                    }
                }
            }
            if best < 1e-14 {
                break;
            }

            // the rotation angle that makes a[p][q] exactly zero
            let theta = 0.5 * (a[q][q] - a[p][p]) / a[p][q];
            let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
            let c = 1.0 / (t * t + 1.0).sqrt();
            let s = t * c;

            // A <- J^T A J
            for k in 0..3 {
                let (akp, akq) = (a[k][p], a[k][q]);
                a[k][p] = c * akp - s * akq;
                a[k][q] = s * akp + c * akq;
            }
            for k in 0..3 {
                let (apk, aqk) = (a[p][k], a[q][k]);
                a[p][k] = c * apk - s * aqk;
                a[q][k] = s * apk + c * aqk;
            }
            // V <- V J, so the columns end up as the eigenvectors
            for k in 0..3 {
                let (vkp, vkq) = (v[k][p], v[k][q]);
                v[k][p] = c * vkp - s * vkq;
                v[k][q] = s * vkp + c * vkq;
            }
        }

        let mut pairs = [
            (a[0][0], V3::new(v[0][0], v[1][0], v[2][0])),
            (a[1][1], V3::new(v[0][1], v[1][1], v[2][1])),
            (a[2][2], V3::new(v[0][2], v[1][2], v[2][2])),
        ];
        pairs.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap());
        (
            V3::new(pairs[0].0, pairs[1].0, pairs[2].0),
            [pairs[0].1.unit(), pairs[1].1.unit(), pairs[2].1.unit()],
        )
    }
}

/// Mean and covariance of a point cloud. The covariance is symmetric by
/// construction — it is a sum of outer products `x x^T`.
pub fn covariance(points: &[V3]) -> (V3, M3) {
    let n = points.len().max(1) as f64;
    let mean = points.iter().fold(V3::ZERO, |a, &p| a + p).scale(1.0 / n);
    let mut c = M3::ZERO;
    for &p in points {
        c = c.add(&M3::outer(p - mean));
    }
    (mean, c.scale(1.0 / n))
}

/// The result of a principal component analysis.
#[derive(Clone, Copy, Debug)]
pub struct Pca {
    pub mean: V3,
    /// Orthonormal, ordered by decreasing variance.
    pub axes: [V3; 3],
    /// Variance along each axis — the eigenvalues.
    pub variance: V3,
}

impl Pca {
    /// Fraction of the total spread captured by the first `k` axes. The number
    /// quoted as "variance explained".
    pub fn explained(&self, k: usize) -> f64 {
        let v = [self.variance.x, self.variance.y, self.variance.z];
        let total: f64 = v.iter().sum();
        if total <= 0.0 {
            return 1.0;
        }
        v.iter().take(k).sum::<f64>() / total
    }

    /// Express a point in the PCA frame — this is the projection that
    /// dimensionality reduction keeps the first components of.
    pub fn project(&self, p: V3) -> V3 {
        let d = p - self.mean;
        V3::new(d.dot(self.axes[0]), d.dot(self.axes[1]), d.dot(self.axes[2]))
    }

    pub fn unproject(&self, c: V3) -> V3 {
        self.mean + self.axes[0].scale(c.x) + self.axes[1].scale(c.y) + self.axes[2].scale(c.z)
    }
}

pub fn pca(points: &[V3]) -> Pca {
    let (mean, cov) = covariance(points);
    let (variance, axes) = cov.symmetric_eigen();
    Pca { mean, axes, variance }
}

/// Half-extents of the tightest box aligned to the PCA axes, and its centre.
///
/// The graphics use of exactly the same decomposition the data people use for
/// feature reduction: an oriented bounding box hugs an elongated shape far
/// more tightly than an axis-aligned one.
pub fn oriented_bounds(points: &[V3], p: &Pca) -> (V3, V3) {
    let (mut lo, mut hi) = (
        V3::new(f64::MAX, f64::MAX, f64::MAX),
        V3::new(f64::MIN, f64::MIN, f64::MIN),
    );
    for &q in points {
        let c = p.project(q);
        lo = V3::new(lo.x.min(c.x), lo.y.min(c.y), lo.z.min(c.z));
        hi = V3::new(hi.x.max(c.x), hi.y.max(c.y), hi.z.max(c.z));
    }
    let half = (hi - lo).scale(0.5);
    let centre = p.unproject((hi + lo).scale(0.5));
    (half, centre)
}

/// Axis-aligned half-extents and centre, for comparison.
pub fn axis_aligned_bounds(points: &[V3]) -> (V3, V3) {
    let (mut lo, mut hi) = (
        V3::new(f64::MAX, f64::MAX, f64::MAX),
        V3::new(f64::MIN, f64::MIN, f64::MIN),
    );
    for &q in points {
        lo = V3::new(lo.x.min(q.x), lo.y.min(q.y), lo.z.min(q.z));
        hi = V3::new(hi.x.max(q.x), hi.y.max(q.y), hi.z.max(q.z));
    }
    ((hi - lo).scale(0.5), (hi + lo).scale(0.5))
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }
    fn close_v(a: V3, b: V3, tol: f64) -> bool {
        (a - b).norm() < tol
    }
    /// Eigenvectors are only defined up to sign, so compare directions.
    fn parallel(a: V3, b: V3, tol: f64) -> bool {
        a.unit().cross(b.unit()).norm() < tol
    }

    /// The definition itself: `A v = lambda v`, for every pair returned.
    #[test]
    fn every_returned_pair_satisfies_the_definition() {
        let m = M3([[4.0, 1.0, 0.5], [1.0, 3.0, -0.2], [0.5, -0.2, 2.0]]);
        let (vals, vecs) = m.symmetric_eigen();
        let l = [vals.x, vals.y, vals.z];
        for k in 0..3 {
            let av = m.mul_v(vecs[k]);
            let lv = vecs[k].scale(l[k]);
            assert!(close_v(av, lv, 1e-9), "pair {k}: {av} != {lv}");
        }
    }

    /// The spectral theorem, in two assertions: real eigenvalues (we only
    /// return reals) and mutually perpendicular eigenvectors.
    #[test]
    fn a_symmetric_matrix_has_perpendicular_eigenvectors() {
        let m = M3([[6.0, 2.0, 1.0], [2.0, 5.0, -1.5], [1.0, -1.5, 4.0]]);
        assert!(m.is_symmetric(1e-12));
        let (_, v) = m.symmetric_eigen();
        for k in 0..3 {
            assert!(close(v[k].norm(), 1.0, 1e-9), "axis {k} is not a unit vector");
        }
        assert!(close(v[0].dot(v[1]), 0.0, 1e-9));
        assert!(close(v[1].dot(v[2]), 0.0, 1e-9));
        assert!(close(v[0].dot(v[2]), 0.0, 1e-9));
    }

    /// A diagonal matrix is already decomposed: its eigenvalues are the
    /// diagonal and its eigenvectors the coordinate axes.
    #[test]
    fn a_diagonal_matrix_decomposes_to_itself() {
        let (vals, vecs) = M3::diag(V3::new(2.0, 9.0, 5.0)).symmetric_eigen();
        assert!(close_v(vals, V3::new(9.0, 5.0, 2.0), 1e-12), "not sorted: {vals}");
        assert!(parallel(vecs[0], V3::Y, 1e-9));
        assert!(parallel(vecs[1], V3::Z, 1e-9));
        assert!(parallel(vecs[2], V3::X, 1e-9));
    }

    /// `A = V L V^T` — the decomposition really does rebuild the matrix.
    #[test]
    fn the_decomposition_reconstructs_the_matrix() {
        let m = M3([[3.0, -1.0, 0.7], [-1.0, 2.0, 0.4], [0.7, 0.4, 5.0]]);
        let (vals, vecs) = m.symmetric_eigen();
        let mut rebuilt = M3::ZERO;
        let l = [vals.x, vals.y, vals.z];
        for k in 0..3 {
            rebuilt = rebuilt.add(&M3::outer(vecs[k]).scale(l[k]));
        }
        for i in 0..3 {
            for j in 0..3 {
                assert!(close(rebuilt.0[i][j], m.0[i][j], 1e-9), "({i},{j})");
            }
        }
    }

    /// The trace is the sum of the eigenvalues - a cheap independent check.
    #[test]
    fn the_trace_equals_the_sum_of_eigenvalues() {
        let m = M3([[4.0, 1.0, 0.5], [1.0, 3.0, -0.2], [0.5, -0.2, 2.0]]);
        let (v, _) = m.symmetric_eigen();
        assert!(close(m.trace(), v.x + v.y + v.z, 1e-9));
    }

    /// Power iteration converges on the LARGEST eigenvalue, and Jacobi agrees.
    #[test]
    fn power_iteration_finds_the_dominant_pair() {
        let m = M3([[5.0, 1.0, 0.0], [1.0, 2.0, 0.5], [0.0, 0.5, 1.0]]);
        let (lambda, v) = m.power_iteration(400);
        let (vals, vecs) = m.symmetric_eigen();
        assert!(close(lambda, vals.x, 1e-8), "{lambda} vs {}", vals.x);
        assert!(parallel(v, vecs[0], 1e-6));
    }

    /// ★ `A^n v = lambda^n v`: the eigenvalue is the growth factor, which is
    /// the same `|z| < 1 / = 1 / > 1` rule as the complex spiral, the damped
    /// oscillator, and integrator stability.
    #[test]
    fn the_eigenvalue_is_the_growth_rate_of_repeated_application() {
        let m = M3::diag(V3::new(0.5, 1.0, 1.5));
        for (start, lambda) in [(V3::X, 0.5f64), (V3::Y, 1.0), (V3::Z, 1.5)] {
            let mut v = start;
            for _ in 0..20 {
                v = m.mul_v(v);
            }
            assert!(close(v.norm(), lambda.powi(20), 1e-6), "lambda = {lambda}");
        }
        // and the qualitative rule
        let decay = m.mul_v(V3::X).norm() < 1.0;
        let hold = close(m.mul_v(V3::Y).norm(), 1.0, 1e-12);
        let grow = m.mul_v(V3::Z).norm() > 1.0;
        assert!(decay && hold && grow);
    }

    /// Covariance of a known cloud: independent axes give a diagonal matrix
    /// whose entries are the per-axis variances.
    #[test]
    fn covariance_measures_spread_per_axis() {
        let mut pts = Vec::new();
        for k in -50..=50 {
            let t = k as f64 / 50.0;
            pts.push(V3::new(t * 4.0, t * 1.0, 0.0));
        }
        let (mean, c) = covariance(&pts);
        assert!(close_v(mean, V3::ZERO, 1e-12));
        // x spreads four times as far as y, so its variance is 16x
        assert!(close(c.0[0][0] / c.0[1][1], 16.0, 1e-9));
        assert!(close(c.0[2][2], 0.0, 1e-12), "no spread in z");
        assert!(c.is_symmetric(1e-12));
    }

    /// ★ PCA finds the true axis of an elongated, arbitrarily rotated cloud.
    #[test]
    fn pca_recovers_the_direction_a_cloud_is_stretched_along() {
        use crate::quat::Q;
        let q = Q::from_axis_angle(V3::new(0.4, 1.0, -0.3), 0.9);
        let long = q.rotate(V3::X);
        let mid = q.rotate(V3::Y);

        let mut pts = Vec::new();
        let mut seed = 12345u64;
        let mut rnd = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (seed >> 33) as f64 / (1u64 << 31) as f64 - 0.5
        };
        for _ in 0..3000 {
            // 10 long, 3 wide, 1 thin - then rotated
            let p = V3::new(rnd() * 10.0, rnd() * 3.0, rnd() * 1.0);
            pts.push(q.rotate(p) + V3::new(7.0, -2.0, 4.0));
        }

        let r = pca(&pts);
        assert!(close_v(r.mean, V3::new(7.0, -2.0, 4.0), 0.2), "mean {}", r.mean);
        assert!(parallel(r.axes[0], long, 0.05), "long axis wrong");
        assert!(parallel(r.axes[1], mid, 0.05), "middle axis wrong");
        // variances should be ordered like the squared extents: 100 : 9 : 1
        assert!(r.variance.x > r.variance.y && r.variance.y > r.variance.z);
        assert!(close(r.variance.x / r.variance.y, 100.0 / 9.0, 1.5));
        assert!(r.explained(1) > 0.85, "explained {}", r.explained(1));
        assert!(r.explained(2) > 0.98);
    }

    /// An isotropic cloud has no preferred direction, and PCA must say so
    /// rather than inventing one.
    #[test]
    fn an_isotropic_cloud_has_no_dominant_axis() {
        let mut pts = Vec::new();
        let mut seed = 999u64;
        let mut rnd = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (seed >> 33) as f64 / (1u64 << 31) as f64 - 0.5
        };
        for _ in 0..4000 {
            pts.push(V3::new(rnd(), rnd(), rnd()));
        }
        let r = pca(&pts);
        assert!(
            r.variance.x / r.variance.z < 1.25,
            "spurious anisotropy {:?}",
            r.variance
        );
        assert!(r.explained(1) < 0.45);
    }

    /// Projecting into the PCA frame and back must be lossless - it is a
    /// rotation, nothing more.
    #[test]
    fn projection_into_the_pca_frame_is_reversible() {
        let pts: Vec<V3> = (0..200)
            .map(|k| {
                let t = k as f64 * 0.1;
                V3::new(t.cos() * 3.0, t.sin(), t * 0.2)
            })
            .collect();
        let r = pca(&pts);
        for &p in &pts {
            assert!(close_v(r.unproject(r.project(p)), p, 1e-9));
        }
    }

    /// ★ The graphics payoff: an oriented box around a rotated, elongated
    /// cloud is far tighter than an axis-aligned one. Same decomposition, and
    /// the reason it is worth doing.
    #[test]
    fn an_oriented_box_beats_an_axis_aligned_one() {
        use crate::quat::Q;
        let q = Q::from_axis_angle(V3::new(1.0, 1.0, 0.0), 0.7);
        let pts: Vec<V3> = (-60..=60)
            .flat_map(|i| {
                (-6..=6).map(move |j| {
                    q.rotate(V3::new(i as f64 * 0.1, j as f64 * 0.02, 0.0))
                })
            })
            .collect();

        let r = pca(&pts);
        let (obb, _) = oriented_bounds(&pts, &r);
        let (aabb, _) = axis_aligned_bounds(&pts);
        let vol = |h: V3| 8.0 * h.x * h.y * h.z;
        assert!(
            vol(obb) < vol(aabb) * 0.5,
            "OBB {:.4} should be far under AABB {:.4}",
            vol(obb),
            vol(aabb)
        );
    }

    /// ★★ The fusion the curriculum promised: the inertia tensor from
    /// `body3.rs` is a symmetric matrix, so its eigenvectors ARE the principal
    /// axes Euler's equations are written in — and the eigenvalues are the
    /// moments of inertia. The same decomposition that reduces a dataset also
    /// tells a tumbling box which way it can spin cleanly.
    #[test]
    fn the_inertia_tensor_decomposes_into_the_principal_axes() {
        use crate::body3::Body3;
        use crate::quat::Q;

        let body = Body3::box_body(V3::new(1.0, 3.0, 5.0), 2.0);
        // express that diagonal tensor in some rotated frame, as it would
        // appear if the box were not aligned with the world
        let q = Q::from_axis_angle(V3::new(0.2, -1.0, 0.6), 1.1);
        let r = M3([
            [q.rotate(V3::X).x, q.rotate(V3::Y).x, q.rotate(V3::Z).x],
            [q.rotate(V3::X).y, q.rotate(V3::Y).y, q.rotate(V3::Z).y],
            [q.rotate(V3::X).z, q.rotate(V3::Y).z, q.rotate(V3::Z).z],
        ]);
        let world_i = r.mul(&M3::diag(body.inertia)).mul(&r.transpose());
        assert!(world_i.is_symmetric(1e-9), "an inertia tensor is symmetric");

        let (vals, vecs) = world_i.symmetric_eigen();
        // the eigenvalues are the moments of inertia, largest first
        let mut want = [body.inertia.x, body.inertia.y, body.inertia.z];
        want.sort_by(|a, b| b.partial_cmp(a).unwrap());
        assert!(close(vals.x, want[0], 1e-8));
        assert!(close(vals.y, want[1], 1e-8));
        assert!(close(vals.z, want[2], 1e-8));
        // and the eigenvectors are the body's own axes, back in world space
        assert!(parallel(vecs[0], q.rotate(V3::X), 1e-6), "largest moment axis");
        assert!(parallel(vecs[2], q.rotate(V3::Z), 1e-6), "smallest moment axis");
    }

    /// Power iteration is not magic: when two eigenvalues are equal there is
    /// no dominant direction to converge on, and it should not pretend.
    #[test]
    fn power_iteration_cannot_separate_equal_eigenvalues() {
        let m = M3::diag(V3::new(3.0, 3.0, 1.0));
        let (lambda, v) = m.power_iteration(500);
        assert!(close(lambda, 3.0, 1e-9), "the VALUE is still right");
        // ...but the vector is just some combination in the degenerate plane
        assert!(close(v.z, 0.0, 1e-6), "should lie in the x-y eigenplane");
    }
}
