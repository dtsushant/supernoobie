//! # The pulley system - iteration 1
//!
//! Two toothed gears at arbitrary positions, joined by a rope that runs over
//! their common external tangent, with a mass hanging from each end.
//!
//! ## What "iteration 1" means
//! This is **kinematics**, not dynamics. The crank angle `theta` is *imposed*;
//! nothing causes it. The masses are recorded and their imbalance is reported,
//! but they do not yet pull. Making them pull is iteration 2 (an ODE).
//!
//! ## The one conserved quantity
//! The rope does not stretch, so its total length is fixed:
//!
//! ```text
//! L  =  h1  +  arc_A  +  tangent  +  arc_B  +  h2
//!       \___/  \_______________________/  \___/
//!       hangs         fixed by geometry     hangs
//! ```
//!
//! The middle three terms depend only on where the gears ARE, not on how far
//! they have turned. So turning the crank trades `h1` against `h2` one for
//! one - that trade is the whole machine.

use crate::complex::Cx;
use std::f64::consts::PI;

/// Everything the operator can set.
#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub sep_x: f64,   // horizontal separation of the two gear centres
    pub sep_y: f64,   // vertical separation (positive = B is higher)
    pub r_a: f64,     // radius of gear A (the driven one)
    pub r_b: f64,     // radius of gear B
    pub teeth: usize, // teeth on gear A
    pub rope_len: f64,
    pub m1: f64, // mass hanging on A's side
    pub m2: f64, // mass hanging on B's side
}

impl Default for Config {
    fn default() -> Self {
        Config {
            sep_x: 300.0,
            sep_y: 90.0,
            r_a: 64.0,
            r_b: 46.0,
            teeth: 14,
            rope_len: 980.0,
            m1: 2.0,
            m2: 3.0,
        }
    }
}

/// The fully solved geometry at one crank angle.
#[derive(Clone, Copy, Debug)]
pub struct State {
    pub a: Cx, // centre of gear A
    pub b: Cx, // centre of gear B
    pub ta: Cx, // tangent point on A - where the straight run leaves
    pub tb: Cx, // tangent point on B
    pub pa: Cx, // departure point on A - where the rope drops vertically
    pub pb: Cx, // departure point on B
    pub w1: Cx, // position of mass 1
    pub w2: Cx, // position of mass 2
    pub tangent_dir: Cx, // unit complex number along the tangent offset
    pub tangent_angle: f64,
    pub seg_len: f64,  // length of the straight run
    pub wrap_a: f64,   // radians of rope wrapped on A
    pub wrap_b: f64,   // radians of rope wrapped on B
    pub fixed: f64,    // arc_A + tangent + arc_B  (constant in theta)
    pub h1: f64,       // hanging length on A's side
    pub h2: f64,       // hanging length on B's side
    pub theta: f64,    // the crank angle actually used (after clamping)
    pub theta_max: f64,
    pub clamped: bool,
    pub slope: f64,     // m in y = m x + c, for the straight run
    pub intercept: f64, // c
    pub gear_b_angle: f64,
}

impl Config {
    /// Range of crank angle for which both weights still hang clear.
    pub fn theta_max(&self) -> f64 {
        let fixed = self.fixed_path();
        let half = (self.rope_len - fixed) / 2.0;
        ((half - MIN_HANG) / self.r_a).max(0.0)
    }

    /// The part of the rope that never changes: the two wraps plus the run.
    pub fn fixed_path(&self) -> f64 {
        let g = self.tangent_geometry();
        self.r_a * g.wrap_a.abs() + g.seg_len + self.r_b * g.wrap_b.abs()
    }

    fn tangent_geometry(&self) -> TangentGeom {
        let a = Cx::new(-self.sep_x / 2.0, -self.sep_y / 2.0);
        let b = Cx::new(self.sep_x / 2.0, self.sep_y / 2.0);

        // Vector from one centre to the other, as a complex number.
        let d = b - a;
        let dist = d.abs();
        let phi = d.arg(); // direction of the centre line

        // ---- THE EXTERNAL TANGENT ----------------------------------------
        // We need the direction in which to step off each centre to land on
        // the tangent line. Call that offset angle `alpha`, measured from the
        // centre line. The standard result is
        //
        //     cos(alpha) = (r_a - r_b) / dist
        //
        // Read the special case first: if the radii are EQUAL, this is
        // acos(0) = pi/2, so the offset is exactly 90 degrees from the centre
        // line - i.e. the offset direction is `i * d_hat`. Multiplication by
        // i, doing real work.
        //
        // When the radii differ the tangent line has to tilt to touch both
        // circles, and `alpha` leans off 90 degrees by just the right amount.
        let ratio = ((self.r_a - self.r_b) / dist).clamp(-1.0, 1.0);
        let alpha = ratio.acos();
        let tangent_angle = phi + alpha; // upper tangent
        let u = Cx::expi(tangent_angle); // the offset direction

        let ta = a + u.scale(self.r_a);
        let tb = b + u.scale(self.r_b);

        // Length of the straight run, by Pythagoras on the centre distance
        // and the radius difference.
        let seg_len = (dist * dist - (self.r_a - self.r_b).powi(2))
            .max(0.0)
            .sqrt();

        // ---- WHERE THE ROPE DROPS ----------------------------------------
        // The hanging rope is vertical, so it must leave the circle where the
        // tangent is vertical: the leftmost point of A (angle pi) and the
        // rightmost point of B (angle 0).
        let pa = a + Cx::expi(PI).scale(self.r_a);
        let pb = b + Cx::expi(0.0).scale(self.r_b);

        // ---- HOW MUCH ROPE IS WRAPPED ------------------------------------
        // Arc length = r * angle. This is ONLY true in radians - the same
        // reason e^(i*theta) has period 2*pi and not 360.
        let wrap_a = PI - tangent_angle; // from the drop point, over the top
        let wrap_b = tangent_angle; // over the top, down to the drop point

        TangentGeom { a, b, ta, tb, pa, pb, u, tangent_angle, seg_len, wrap_a, wrap_b }
    }

    /// Solve the whole system at crank angle `theta` (radians, positive =
    /// gear A turns counter-clockwise, paying rope out on the left).
    pub fn solve(&self, theta: f64) -> State {
        let g = self.tangent_geometry();
        let fixed = self.r_a * g.wrap_a.abs() + g.seg_len + self.r_b * g.wrap_b.abs();

        // The rope is inextensible, so the two hanging lengths must sum to
        // whatever is left over. This single line is the constraint.
        let hang_total = self.rope_len - fixed;
        let half = hang_total / 2.0;

        let theta_max = ((half - MIN_HANG) / self.r_a).max(0.0);
        let th = theta.clamp(-theta_max, theta_max);
        let clamped = (th - theta).abs() > 1e-12;

        // Turning the crank by `th` pays out `r_a * th` of rope on the left
        // and takes exactly the same amount in on the right. Rope paid out is
        // arc length: r * theta.
        let h1 = half + self.r_a * th;
        let h2 = half - self.r_a * th;

        // The weights hang straight down from the departure points.
        let w1 = Cx::new(g.pa.re, g.pa.im - h1);
        let w2 = Cx::new(g.pb.re, g.pb.im - h2);

        // The straight run, as the schoolbook line y = m x + c.
        let dx = g.tb.re - g.ta.re;
        let slope = if dx.abs() < 1e-12 { f64::INFINITY } else { (g.tb.im - g.ta.im) / dx };
        let intercept = if slope.is_finite() { g.ta.im - slope * g.ta.re } else { f64::NAN };

        // Gear B is driven by the same rope. The rope moves at r_a * theta,
        // and B's rim must move with it, so B turns by (r_a / r_b) * theta.
        // Same sense as A, because the belt is not crossed.
        let gear_b_angle = th * self.r_a / self.r_b;

        State {
            a: g.a, b: g.b, ta: g.ta, tb: g.tb, pa: g.pa, pb: g.pb,
            w1, w2,
            tangent_dir: g.u,
            tangent_angle: g.tangent_angle,
            seg_len: g.seg_len,
            wrap_a: g.wrap_a, wrap_b: g.wrap_b,
            fixed, h1, h2,
            theta: th, theta_max, clamped,
            slope, intercept, gear_b_angle,
        }
    }

    /// Positions of the gear teeth: the Nth roots of unity, scaled to the
    /// radius, translated to the centre, and rotated by the crank angle.
    ///
    /// ```text
    /// tooth_k = centre + r * e^(i * (angle + 2*pi*k/N))
    /// ```
    ///
    /// Turning the gear is *adding to the exponent*. There is no rotation
    /// matrix anywhere in this crate.
    pub fn teeth_of(centre: Cx, r: f64, angle: f64, n: usize) -> Vec<Cx> {
        (0..n)
            .map(|k| centre + Cx::expi(angle + 2.0 * PI * k as f64 / n as f64).scale(r))
            .collect()
    }
}

/// Closest a weight may come to its gear before we stop cranking.
pub const MIN_HANG: f64 = 34.0;
pub const G: f64 = 9.81;

struct TangentGeom {
    a: Cx, b: Cx, ta: Cx, tb: Cx, pa: Cx, pb: Cx,
    u: Cx, tangent_angle: f64, seg_len: f64, wrap_a: f64, wrap_b: f64,
}

impl State {
    /// Total rope length implied by this state. Must equal `rope_len`.
    pub fn rope_total(&self) -> f64 {
        self.h1 + self.h2 + self.fixed
    }

    /// Acceleration the system WOULD have if the crank were released.
    /// Classic Atwood machine: `a = g * (m2 - m1) / (m1 + m2)`.
    /// Positive means mass 2 descends. Not used yet - this is iteration 2.
    pub fn atwood_accel(&self, m1: f64, m2: f64) -> f64 {
        G * (m2 - m1) / (m1 + m2)
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    /// THE invariant. If this ever fails the machine is making rope.
    #[test]
    fn rope_length_is_conserved_for_every_crank_angle() {
        let cfg = Config::default();
        let tm = cfg.theta_max();
        for k in -60..=60 {
            let th = tm * k as f64 / 60.0;
            let s = cfg.solve(th);
            assert!(
                close(s.rope_total(), cfg.rope_len, 1e-9),
                "theta={th}: total {} != {}",
                s.rope_total(),
                cfg.rope_len
            );
        }
    }

    /// What one weight gains, the other loses. Exactly.
    #[test]
    fn cranking_trades_one_hang_against_the_other() {
        let cfg = Config::default();
        let a = cfg.solve(0.0);
        let b = cfg.solve(0.5);
        assert!(close(b.h1 - a.h1, -(b.h2 - a.h2), 1e-9));
    }

    /// Rope paid out is arc length `r * theta`. True only in radians.
    #[test]
    fn rope_paid_out_is_r_times_theta() {
        let cfg = Config::default();
        let base = cfg.solve(0.0);
        let th = 0.73;
        let s = cfg.solve(th);
        assert!(close(s.h1 - base.h1, cfg.r_a * th, 1e-9));
    }

    /// With equal radii the tangent offset must be exactly `i * d_hat` -
    /// a 90 degree turn by multiplication.
    #[test]
    fn equal_radii_give_a_perpendicular_tangent() {
        let cfg = Config { r_a: 50.0, r_b: 50.0, ..Config::default() };
        let s = cfg.solve(0.0);
        let d_hat = (s.b - s.a).unit();
        let expected = Cx::I * d_hat;
        assert!(close(s.tangent_dir.re, expected.re, 1e-9));
        assert!(close(s.tangent_dir.im, expected.im, 1e-9));
    }

    /// The tangent points must actually sit ON their circles...
    #[test]
    fn tangent_points_lie_on_the_circles() {
        let cfg = Config::default();
        let s = cfg.solve(0.0);
        assert!(close((s.ta - s.a).abs(), cfg.r_a, 1e-9));
        assert!(close((s.tb - s.b).abs(), cfg.r_b, 1e-9));
    }

    /// ...and the run between them must be perpendicular to both radii.
    /// That is what "tangent" MEANS, so it is worth asserting rather than
    /// trusting the formula.
    #[test]
    fn the_run_is_perpendicular_to_both_radii() {
        let cfg = Config::default();
        let s = cfg.solve(0.0);
        let run = (s.tb - s.ta).unit();
        for (t, c) in [(s.ta, s.a), (s.tb, s.b)] {
            let radius = (t - c).unit();
            let dot = run.re * radius.re + run.im * radius.im;
            assert!(dot.abs() < 1e-9, "dot product {dot} should be 0");
        }
    }

    /// Gear ratio: the smaller wheel turns faster, in proportion.
    #[test]
    fn gear_ratio_is_inverse_to_radius() {
        let cfg = Config { r_a: 60.0, r_b: 20.0, ..Config::default() };
        let s = cfg.solve(1.0);
        assert!(close(s.gear_b_angle, 3.0, 1e-9));
    }

    /// The teeth are evenly spaced and all sit on the rim.
    #[test]
    fn teeth_are_roots_of_unity_on_the_rim() {
        let c = Cx::new(10.0, -4.0);
        let teeth = Config::teeth_of(c, 7.0, 0.3, 12);
        assert_eq!(teeth.len(), 12);
        for t in &teeth {
            assert!(close((*t - c).abs(), 7.0, 1e-9));
        }
        // consecutive teeth are 2*pi/12 apart
        let step = (teeth[1] - c).arg() - (teeth[0] - c).arg();
        assert!(close(step, 2.0 * PI / 12.0, 1e-9));
    }

    /// Cranking must never push a weight into its gear.
    #[test]
    fn crank_is_clamped_before_the_weight_hits_the_gear() {
        let cfg = Config::default();
        let s = cfg.solve(1000.0);
        assert!(s.clamped);
        assert!(s.h1 >= MIN_HANG - 1e-9 && s.h2 >= MIN_HANG - 1e-9);
    }

    /// Atwood: equal masses balance, unequal accelerate toward the heavier.
    #[test]
    fn atwood_acceleration() {
        let cfg = Config::default();
        let s = cfg.solve(0.0);
        assert!(close(s.atwood_accel(4.0, 4.0), 0.0, 1e-12));
        assert!(close(s.atwood_accel(1.0, 3.0), G * 2.0 / 4.0, 1e-12));
    }
}
