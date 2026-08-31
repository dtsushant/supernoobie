//! # Shapes — geometry as values
//!
//! The difference between an instruction and a value:
//!
//! ```text
//! draw_rotated_square(pts, angle)     an instruction. Gone once it runs.
//! square.map(|z| r * z)               a value. Nameable, composable, testable.
//! ```
//!
//! On paper you write *"let R be rotation by theta; apply R to the square."*
//! You do not write *"draw a rotated square."* This module exists so the code
//! can be written the first way.
//!
//! ## Transformations are just functions
//!
//! Because a point, an offset and a scalar are all one [`Cx`], every plane
//! transformation you need is a one-line closure:
//!
//! ```text
//! |z| z + b                 translate
//! |z| r * z                 rotate AND scale, together     (r = polar(k, theta))
//! |z| a * z + b             any affine map
//! |z| z.conj()              reflect in the real axis
//! |z| z * z                 square the plane
//! ```
//!
//! and [`Shape::map`] applies one to a whole shape. They compose the way you
//! would expect, because function composition is what composing transforms
//! *is*: `s.map(a).map(b)` is `b` after `a`.
//!
//! ## Deferred, not eager
//!
//! `map` does not walk a point list. It wraps the shape and applies the
//! function at draw time. That matters because some shapes have no point list
//! to walk — `Graph` is sampled against whatever is on screen, and `Implicit`
//! is marched over a grid. Deferring means one `map` works on all of them.

use crate::complex::Cx;
use std::sync::Arc;

type Fx = Arc<dyn Fn(Cx) -> Cx + Send + Sync>;
type F1 = Arc<dyn Fn(f64) -> f64 + Send + Sync>;
type Fc = Arc<dyn Fn(f64) -> Cx + Send + Sync>;
type F2 = Arc<dyn Fn(f64, f64) -> f64 + Send + Sync>;

#[derive(Clone)]
pub enum Shape {
    /// Marks, not a path.
    Points(Vec<Cx>),
    Path { pts: Vec<Cx>, closed: bool },
    /// `t -> z(t)`. **The form to reach for** — one point per sample, exactly
    /// on the curve, evenly spaced.
    Param { f: Fc, t0: f64, t1: f64, n: usize },
    /// `y = f(x)`, sampled across whatever is visible.
    Graph { f: F1 },
    /// `F(x, y) = level`, marched over a grid.
    Implicit { f: F2, level: f64, res: usize },
    /// Several shapes treated as one — so a transform applies to all of them.
    Group(Vec<Shape>),
    /// A shape with a transformation waiting to be applied.
    Mapped(Box<Shape>, Fx),
}

impl Shape {
    // ---- constructors ----------------------------------------------------

    pub fn point(z: Cx) -> Shape {
        Shape::Points(vec![z])
    }
    pub fn points(v: impl Into<Vec<Cx>>) -> Shape {
        Shape::Points(v.into())
    }
    /// An open path.
    pub fn path(v: impl Into<Vec<Cx>>) -> Shape {
        Shape::Path { pts: v.into(), closed: false }
    }
    /// A closed path. **Two points make a straight line**, which is why
    /// `polygon(a, b)` does the obvious thing.
    pub fn polygon(v: impl Into<Vec<Cx>>) -> Shape {
        Shape::Path { pts: v.into(), closed: true }
    }
    pub fn param(f: impl Fn(f64) -> Cx + Send + Sync + 'static, t0: f64, t1: f64, n: usize) -> Shape {
        Shape::Param { f: Arc::new(f), t0, t1, n: n.max(2) }
    }
    pub fn graph(f: impl Fn(f64) -> f64 + Send + Sync + 'static) -> Shape {
        Shape::Graph { f: Arc::new(f) }
    }
    pub fn implicit(f: impl Fn(f64, f64) -> f64 + Send + Sync + 'static, level: f64) -> Shape {
        Shape::Implicit { f: Arc::new(f), level, res: 140 }
    }
    pub fn group(v: impl Into<Vec<Shape>>) -> Shape {
        Shape::Group(v.into())
    }

    /// The unit circle, parameterised — `e^(i t)`.
    pub fn circle(centre: Cx, r: f64) -> Shape {
        Shape::param(move |t| centre + Cx::expi(t).scale(r), 0.0, std::f64::consts::TAU, 192)
    }

    /// A regular n-gon: the nth roots of unity, scaled and moved.
    pub fn ngon(centre: Cx, r: f64, n: usize) -> Shape {
        let n = n.max(3);
        Shape::polygon(
            (0..n)
                .map(|k| centre + Cx::expi(std::f64::consts::TAU * k as f64 / n as f64).scale(r))
                .collect::<Vec<_>>(),
        )
    }

    /// The unit square, corners `0, 1, 1+i, i` — handy for showing what a
    /// transformation does, because you can see every corner move.
    pub fn unit_square() -> Shape {
        Shape::polygon(vec![Cx::ZERO, Cx::new(1.0, 0.0), Cx::new(1.0, 1.0), Cx::new(0.0, 1.0)])
    }

    /// An axis-aligned rectangle from two opposite corners.
    pub fn rect(a: Cx, b: Cx) -> Shape {
        Shape::polygon(vec![a, Cx::new(b.re, a.im), b, Cx::new(a.re, b.im)])
    }

    // ---- transformation --------------------------------------------------

    /// Apply a plane transformation. **The point of the whole module.**
    pub fn map(self, f: impl Fn(Cx) -> Cx + Send + Sync + 'static) -> Shape {
        Shape::Mapped(Box::new(self), Arc::new(f))
    }

    /// `z + by`
    pub fn shift(self, by: Cx) -> Shape {
        self.map(move |z| z + by)
    }
    /// `k z`
    pub fn scaled(self, k: f64) -> Shape {
        self.map(move |z| z.scale(k))
    }
    /// `z e^(i theta)` — about the origin.
    pub fn rotated(self, theta: f64) -> Shape {
        let r = Cx::expi(theta);
        self.map(move |z| z * r)
    }
    /// Rotate about a point: move it to the origin, turn, move it back.
    pub fn rotated_about(self, centre: Cx, theta: f64) -> Shape {
        let r = Cx::expi(theta);
        self.map(move |z| (z - centre) * r + centre)
    }
    /// `a z + b` — every similarity of the plane in one call.
    pub fn affine(self, a: Cx, b: Cx) -> Shape {
        self.map(move |z| a * z + b)
    }

    // ---- sampling --------------------------------------------------------

    /// The point list this shape resolves to, given the visible range — with
    /// every pending transformation applied.
    ///
    /// Returns *polylines*: `Points` yields one-point runs, a closed path
    /// repeats its first point at the end, and a `Group` concatenates.
    pub fn polylines(&self, lo: Cx, hi: Cx, width_px: usize) -> Vec<Vec<Cx>> {
        self.resolve(lo, hi, width_px, &|z| z)
    }

    fn resolve(&self, lo: Cx, hi: Cx, w: usize, xf: &dyn Fn(Cx) -> Cx) -> Vec<Vec<Cx>> {
        match self {
            Shape::Points(v) => v.iter().map(|z| vec![xf(*z)]).collect(),
            Shape::Path { pts, closed } => {
                if pts.is_empty() {
                    return Vec::new();
                }
                let mut out: Vec<Cx> = pts.iter().map(|z| xf(*z)).collect();
                if *closed && pts.len() > 2 {
                    out.push(xf(pts[0]));
                }
                vec![out]
            }
            Shape::Param { f, t0, t1, n } => {
                let mut run = Vec::with_capacity(n + 1);
                for k in 0..=*n {
                    let t = t0 + (t1 - t0) * k as f64 / *n as f64;
                    let p = xf(f(t));
                    if p.re.is_finite() && p.im.is_finite() {
                        run.push(p);
                    }
                }
                vec![run]
            }
            Shape::Graph { f } => {
                let steps = w.max(2);
                let mut runs = Vec::new();
                let mut run: Vec<Cx> = Vec::new();
                for k in 0..=steps {
                    let x = lo.re + (hi.re - lo.re) * k as f64 / steps as f64;
                    let y = f(x);
                    if y.is_finite() {
                        run.push(xf(Cx::new(x, y)));
                    } else if !run.is_empty() {
                        // a pole: break the run rather than draw across it
                        runs.push(std::mem::take(&mut run));
                    }
                }
                if !run.is_empty() {
                    runs.push(run);
                }
                runs
            }
            Shape::Implicit { f, level, res } => {
                // March in the ORIGINAL space, then transform the segments.
                // Transforming the equation would need the inverse map, which
                // may not exist; transforming the answer always works.
                crate::plot::contour(|x, y| f(x, y), *level, lo, hi, *res)
                    .into_iter()
                    .map(|(a, b)| vec![xf(a), xf(b)])
                    .collect()
            }
            Shape::Group(v) => v.iter().flat_map(|s| s.resolve(lo, hi, w, xf)).collect(),
            Shape::Mapped(inner, g) => {
                // compose: the outer transform runs after the inner one
                let composed = move |z: Cx| xf(g(z));
                inner.resolve(lo, hi, w, &composed)
            }
        }
    }

    /// Whether this shape draws marks rather than lines.
    pub fn is_points(&self) -> bool {
        match self {
            Shape::Points(_) => true,
            Shape::Mapped(inner, _) => inner.is_points(),
            _ => false,
        }
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{PI, TAU};

    const LO: Cx = Cx { re: -10.0, im: -10.0 };
    const HI: Cx = Cx { re: 10.0, im: 10.0 };

    fn pts(s: &Shape) -> Vec<Cx> {
        s.polylines(LO, HI, 64).into_iter().flatten().collect()
    }
    fn close(a: Cx, b: Cx) -> bool {
        (a - b).abs() < 1e-9
    }

    /// A closed path returns to its start; an open one does not.
    #[test]
    fn closed_paths_close_and_open_ones_do_not() {
        let v = vec![Cx::ZERO, Cx::new(1.0, 0.0), Cx::new(0.0, 1.0)];
        let c = pts(&Shape::polygon(v.clone()));
        assert_eq!(c.len(), 4);
        assert!(close(c[0], c[3]));
        assert_eq!(pts(&Shape::path(v)).len(), 3);
    }

    /// ★ `map` is what makes this worth having: a transformation is a value.
    #[test]
    fn map_applies_a_transformation_to_the_whole_shape() {
        let sq = Shape::unit_square();
        let turned = sq.clone().rotated(PI / 2.0);
        // the corner at 1 goes to i
        assert!(close(pts(&turned)[1], Cx::new(0.0, 1.0)));
        // and the original is untouched - shapes are values, not mutated state
        assert!(close(pts(&sq)[1], Cx::new(1.0, 0.0)));
    }

    /// Composition is function composition: `.map(a).map(b)` is b after a.
    #[test]
    fn maps_compose_in_the_order_written() {
        let s = Shape::point(Cx::new(1.0, 0.0));
        let a = s.clone().rotated(PI / 2.0).shift(Cx::new(10.0, 0.0));
        let b = s.shift(Cx::new(10.0, 0.0)).rotated(PI / 2.0);
        assert!(close(pts(&a)[0], Cx::new(10.0, 1.0)), "rotate then shift");
        assert!(close(pts(&b)[0], Cx::new(0.0, 11.0)), "shift then rotate");
    }

    /// Rotating about a point leaves that point alone. That is what "about"
    /// means, and it is the classic off-by-a-translation bug.
    #[test]
    fn rotating_about_a_point_fixes_that_point() {
        let c = Cx::new(3.0, -2.0);
        let s = Shape::points(vec![c, c + Cx::new(1.0, 0.0)]).rotated_about(c, PI);
        let p = pts(&s);
        assert!(close(p[0], c), "the centre moved");
        assert!(close(p[1], c - Cx::new(1.0, 0.0)));
    }

    /// An affine map `a z + b` covers translate, rotate and scale at once.
    #[test]
    fn one_affine_map_does_everything() {
        let a = Cx::expi(0.4).scale(1.3);
        let b = Cx::new(2.0, -1.0);
        let s = Shape::point(Cx::new(1.0, 0.0)).affine(a, b);
        assert!(close(pts(&s)[0], a * Cx::new(1.0, 0.0) + b));
    }

    /// Transforms reach inside a group, so a whole assembly moves together.
    #[test]
    fn a_transform_reaches_every_member_of_a_group() {
        let g = Shape::group(vec![
            Shape::point(Cx::new(1.0, 0.0)),
            Shape::point(Cx::new(0.0, 1.0)),
        ])
        .shift(Cx::new(5.0, 5.0));
        let p = pts(&g);
        assert!(close(p[0], Cx::new(6.0, 5.0)));
        assert!(close(p[1], Cx::new(5.0, 6.0)));
    }

    /// Deferred mapping is the reason `Graph` and `Implicit` can be
    /// transformed at all — there is no point list to walk until draw time.
    #[test]
    fn view_dependent_shapes_can_still_be_transformed() {
        let g = Shape::graph(|x| x * x).shift(Cx::new(0.0, 3.0));
        for run in g.polylines(Cx::new(-2.0, -2.0), Cx::new(2.0, 2.0), 32) {
            for p in run {
                assert!((p.im - (p.re * p.re + 3.0)).abs() < 1e-9, "at {p}");
            }
        }
        // an implicit circle, shifted: still a circle, in the new place
        let c = Shape::implicit(|x, y| x * x + y * y, 1.0).shift(Cx::new(4.0, 0.0));
        for run in c.polylines(Cx::new(-3.0, -3.0), Cx::new(3.0, 3.0), 64) {
            for p in run {
                assert!(((p - Cx::new(4.0, 0.0)).abs() - 1.0).abs() < 0.03, "at {p}");
            }
        }
    }

    /// A gap in the *domain* breaks the run: `sqrt` is undefined below zero,
    /// so the curve must begin at the origin rather than stretch back across.
    ///
    /// Worth knowing what does **not** break it. `1/x` sampled at 101 points
    /// across [-2, 2] never lands exactly on zero, so every value it is
    /// actually asked for is finite — merely enormous — and the run stays
    /// joined. Poles are handled by clipping against the view, not by the
    /// finiteness check. Only a genuine NaN or infinity lifts the pen.
    #[test]
    fn a_gap_in_the_domain_breaks_the_run() {
        let runs = Shape::graph(|x| x.sqrt())
            .polylines(Cx::new(-2.0, -2.0), Cx::new(2.0, 2.0), 100);
        let all: Vec<Cx> = runs.iter().flatten().copied().collect();
        assert!(!all.is_empty());
        for p in &all {
            assert!(p.re >= -1e-12, "sampled sqrt at x = {}", p.re);
        }

        // ...and the finite-but-huge case really does stay in one piece
        let joined = Shape::graph(|x| 1.0 / x)
            .polylines(Cx::new(-2.0, -50.0), Cx::new(2.0, 50.0), 101);
        assert_eq!(joined.len(), 1, "1/x is finite at every point sampled here");
    }

    /// A parametric circle really is a circle, and `map` keeps it one.
    #[test]
    fn a_parametric_circle_stays_a_circle_under_rotation() {
        let s = Shape::circle(Cx::ZERO, 2.0).rotated(0.7);
        for p in pts(&s) {
            assert!((p.abs() - 2.0).abs() < 1e-9);
        }
    }

    /// An n-gon is the roots of unity: same radius, even spacing.
    #[test]
    fn ngon_is_the_roots_of_unity() {
        let p = pts(&Shape::ngon(Cx::ZERO, 1.0, 6));
        assert_eq!(p.len(), 7, "six corners plus the closing repeat");
        for q in &p {
            assert!((q.abs() - 1.0).abs() < 1e-12);
        }
        let step = (p[1]).arg() - (p[0]).arg();
        assert!((step - TAU / 6.0).abs() < 1e-12);
    }

    /// Degenerate shapes must resolve to nothing rather than panic.
    #[test]
    fn degenerate_shapes_are_harmless() {
        assert!(pts(&Shape::polygon(Vec::<Cx>::new())).is_empty());
        assert_eq!(pts(&Shape::polygon(vec![Cx::ZERO])).len(), 1);
        assert_eq!(pts(&Shape::group(Vec::<Shape>::new())).len(), 0);
        let _ = pts(&Shape::param(|_| Cx::ZERO, 0.0, 0.0, 0));
        let _ = pts(&Shape::graph(|_| f64::NAN));
    }
}
