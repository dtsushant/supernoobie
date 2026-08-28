//! # A bicycle over hills — the mathematics
//!
//! Maths only, as always. And the pleasing part: a bicycle **is** the machine
//! from `pulley.rs`. Two gears at a fixed separation, joined by a loop that
//! runs over their common tangents. The chain is the rope; the chainring and
//! the rear sprocket are gears A and B; the tangent geometry is identical.
//!
//! ---
//!
//! ## 1. The drivetrain
//!
//! The chain does not stretch, so the two rims must move at the same speed:
//!
//! ```text
//! chain speed  =  w_crank * r_chainring  =  w_wheel * r_sprocket
//!
//!      ->  w_wheel = w_crank * (r_chainring / r_sprocket)
//! ```
//!
//! and the ground rolls under the wheel at `r_wheel * w_wheel`, which is
//! `r * theta` again — arc length becoming distance, the same identity the
//! crane uses to turn winch rotation into rope.
//!
//! ## 2. Why gears exist
//!
//! Follow the *force* rather than the speed. Push the pedals with torque
//! `tau`; the chain tension is `T = tau / r_chainring`. That tension acts on
//! the sprocket at radius `r_sprocket`, giving wheel torque `T * r_sprocket`,
//! and the ground sees
//!
//! ```text
//! F = tau * r_sprocket / (r_chainring * r_wheel)
//! ```
//!
//! Compare the two results and the trade is exact and unavoidable:
//!
//! | | speed | force |
//! |---|---|---|
//! | **big chainring** (high gear) | `r_c/r_s` large — fast | `r_s/r_c` small — weak |
//! | **small chainring** (low gear) | slow | strong |
//!
//! Multiply them and the chainring cancels: `speed x force` is fixed. A gear
//! cannot give you power, only choose how to spend it. That is the whole idea,
//! and it is why you drop into low gear at the bottom of a hill.
//!
//! ## 3. The hill
//!
//! A sum of sines with unrelated frequencies — the "organic motion" trick:
//! deterministic, smooth, and never repeating, because the periods have no
//! common multiple.
//!
//! ```text
//! h(x) = sum a_i sin(f_i x + p_i)
//! ```
//!
//! Because it is a sum of sines its slope is known **exactly**, by
//! differentiating rather than by finite differences:
//!
//! ```text
//! h'(x) = sum a_i f_i cos(f_i x + p_i)
//! ```
//!
//! and the surface normal is that slope turned a quarter turn — a
//! multiplication by `i`, once more.
//!
//! ## 4. The rider
//!
//! The legs are **two-bone inverse kinematics**: given the hip and the foot,
//! and fixed thigh and shin lengths, where is the knee?
//!
//! That is the intersection of two circles — one of radius `thigh` about the
//! hip, one of radius `shin` about the foot — and the classical construction
//! applies. Along the hip-to-foot line, the intersection sits at
//!
//! ```text
//! x = (d^2 + thigh^2 - shin^2) / (2d)        y = sqrt(thigh^2 - x^2)
//! ```
//!
//! and the knee is `x` along that line, `y` off to one side. The two signs of
//! `y` are the two ways a knee can bend; picking one is picking which way the
//! joint faces.
//!
//! The feet are on the pedals, which are `crank_centre + r e^(i theta)` — so
//! the whole animation is driven by the drivetrain. The rider does not have a
//! walk cycle; the rider has a gear ratio.

use crate::complex::Cx;

// ---------------------------------------------------------------------------
// terrain
// ---------------------------------------------------------------------------

/// A hill: a sum of sines, so both the height and the exact slope are cheap.
#[derive(Clone, Copy, Debug)]
pub struct Terrain {
    /// `(amplitude, frequency, phase)` per component.
    pub waves: [(f64, f64, f64); 4],
    pub base: f64,
}

impl Default for Terrain {
    fn default() -> Self {
        // frequencies deliberately not multiples of one another, so the
        // landscape never repeats
        Terrain {
            waves: [
                (70.0, 0.0031, 0.0),
                (34.0, 0.0072, 1.7),
                (16.0, 0.0143, 3.1),
                (7.0, 0.0291, 0.6),
            ],
            base: 210.0,
        }
    }
}

impl Terrain {
    pub fn height(&self, x: f64) -> f64 {
        self.base
            + self
                .waves
                .iter()
                .map(|(a, f, p)| a * (f * x + p).sin())
                .sum::<f64>()
    }

    /// `dh/dx`, differentiated exactly rather than sampled.
    pub fn slope(&self, x: f64) -> f64 {
        self.waves
            .iter()
            .map(|(a, f, p)| a * f * (f * x + p).cos())
            .sum()
    }

    /// The point on the surface directly below `x`.
    pub fn at(&self, x: f64) -> Cx {
        Cx::new(x, self.height(x))
    }

    /// Unit tangent, pointing in +x.
    pub fn tangent(&self, x: f64) -> Cx {
        Cx::new(1.0, self.slope(x)).unit()
    }

    /// Outward (upward) unit normal — the tangent turned a quarter turn, which
    /// is a multiplication by `i`.
    pub fn normal(&self, x: f64) -> Cx {
        Cx::I * self.tangent(x)
    }

    /// Deepest point of the surface inside a circle of radius `r`, if any.
    fn deepest(&self, centre: Cx, r: f64) -> Option<(f64, Cx)> {
        let mut best: Option<(f64, Cx)> = None;
        let n = 19;
        for k in 0..=n {
            let x = centre.re - r + 2.0 * r * k as f64 / n as f64;
            let p = self.at(x);
            let d = (centre - p).abs();
            if best.map_or(true, |(bd, _)| d < bd) {
                best = Some((d, p));
            }
        }
        best.filter(|(d, _)| *d < r)
    }

    /// Push a circle of radius `r` out of the ground, if it is inside.
    /// Returns the corrected centre and the contact normal.
    ///
    /// **Iterated, and it has to be.** Pushing the circle away from the single
    /// nearest surface point does not necessarily clear it: the ground curves,
    /// so once you have moved, a *different* point of the surface can be the
    /// closest one and can still be inside. A valley floor is the obvious case
    /// — the wheel rests against two sides at once and no single push
    /// satisfies both.
    ///
    /// Repeating the push converges quickly for terrain this gentle. Sharper
    /// geometry would want the true closest point on the curve, found by
    /// solving `d/dx |centre - (x, h(x))|^2 = 0` rather than by sampling.
    pub fn resolve(&self, centre: Cx, r: f64) -> Option<(Cx, Cx)> {
        let (d0, p0) = self.deepest(centre, r)?;
        let mut c = centre;
        let mut n = if d0 < 1e-9 { self.normal(centre.re) } else { (centre - p0).unit() };
        for _ in 0..6 {
            match self.deepest(c, r) {
                Some((d, p)) => {
                    n = if d < 1e-9 { self.normal(c.re) } else { (c - p).unit() };
                    c = p + n.scale(r);
                }
                None => break,
            }
        }
        Some((c, n))
    }
}

// ---------------------------------------------------------------------------
// drivetrain
// ---------------------------------------------------------------------------

/// Chainring, sprocket and wheel — the gears from `pulley.rs`, pedalled.
#[derive(Clone, Copy, Debug)]
pub struct Drivetrain {
    pub chainring: f64,
    pub sprocket: f64,
    pub wheel: f64,
    /// Crank angle, radians. The pedals are at `+/-` this angle.
    pub crank: f64,
    /// Wheel angle, radians. Accumulated from the ratio.
    pub wheel_angle: f64,
    /// Highest cadence the rider can turn the pedals at, rad/s.
    pub max_cadence: f64,
    /// Torque the rider can apply at the crank.
    pub torque: f64,
}

impl Default for Drivetrain {
    fn default() -> Self {
        Drivetrain {
            chainring: 26.0,
            sprocket: 13.0,
            wheel: 34.0,
            crank: 0.0,
            wheel_angle: 0.0,
            max_cadence: 13.0,
            torque: 30_000.0,
        }
    }
}

impl Drivetrain {
    /// `r_chainring / r_sprocket` — wheel turns per crank turn.
    pub fn ratio(&self) -> f64 {
        self.chainring / self.sprocket
    }

    /// Ground speed at a given cadence: `w * ratio * r_wheel`.
    pub fn speed_at(&self, cadence: f64) -> f64 {
        cadence * self.ratio() * self.wheel
    }

    /// Fastest this gear can go, cadence-limited.
    pub fn top_speed(&self) -> f64 {
        self.speed_at(self.max_cadence)
    }

    /// Force at the contact patch: `tau * r_sprocket / (r_chainring * r_wheel)`.
    ///
    /// Note that `top_speed * drive_force` does not depend on the chainring at
    /// all — the gear chooses how to spend the power, never how much there is.
    pub fn drive_force(&self) -> f64 {
        self.torque * self.sprocket / (self.chainring * self.wheel)
    }

    /// Turn the cranks and let the ratio carry it to the wheel.
    pub fn pedal(&mut self, cadence: f64, dt: f64) {
        self.crank += cadence * dt;
        self.wheel_angle += cadence * self.ratio() * dt;
    }

    /// Drive the crank *backwards* from how far the bike actually rolled, so
    /// the pedals stay in step with the ground even when coasting or being
    /// pushed downhill.
    pub fn sync_to_distance(&mut self, delta_s: f64) {
        let dw = delta_s / self.wheel;
        self.wheel_angle += dw;
        self.crank += dw / self.ratio();
    }

    /// Pedal positions: `centre + r e^(i theta)`, half a turn apart.
    pub fn pedals(&self, centre: Cx, crank_len: f64) -> (Cx, Cx) {
        let a = centre + Cx::expi(self.crank).scale(crank_len);
        let b = centre + Cx::expi(self.crank + std::f64::consts::PI).scale(crank_len);
        (a, b)
    }
}

// ---------------------------------------------------------------------------
// inverse kinematics
// ---------------------------------------------------------------------------

/// Where the knee goes, given the hip, the foot, and two bone lengths.
///
/// Two circles intersect; `bend` picks which of the two solutions. If the foot
/// is further away than the leg can reach, the limb is straightened and aimed
/// at it rather than snapping.
pub fn two_bone_ik(hip: Cx, foot: Cx, thigh: f64, shin: f64, bend: f64) -> Cx {
    let d = foot - hip;
    let dist = d.abs().clamp(1e-6, thigh + shin - 1e-9);
    let dir = d.unit();
    // distance along the hip-foot line to the intersection
    let a = (dist * dist + thigh * thigh - shin * shin) / (2.0 * dist);
    let h = (thigh * thigh - a * a).max(0.0).sqrt();
    // off to one side: perpendicular is a multiplication by i
    hip + dir.scale(a) + (Cx::I * dir).scale(h * bend.signum())
}

// ---------------------------------------------------------------------------
// the bike
// ---------------------------------------------------------------------------

/// Two wheels and a seat, held apart by distance constraints — the same
/// position-based method as `soft.rs`, with three particles instead of a
/// thousand.
#[derive(Clone, Copy, Debug)]
pub struct Bike {
    pub rear: Cx,
    pub rear_prev: Cx,
    pub front: Cx,
    pub front_prev: Cx,
    pub seat: Cx,
    pub seat_prev: Cx,

    pub wheelbase: f64,
    pub seat_rear: f64,
    pub seat_front: f64,
    pub wheel_r: f64,

    pub drive: Drivetrain,
    pub gravity: f64,
    pub damping: f64,
    /// True when the rear wheel has grip - you cannot accelerate in mid-air.
    pub rear_grounded: bool,
    pub front_grounded: bool,
    pub distance: f64,
}

impl Bike {
    pub fn new(t: &Terrain, x: f64) -> Self {
        let wheel_r = 34.0;
        let wheelbase = 96.0;
        let rear = Cx::new(x, t.height(x) + wheel_r);
        let front = Cx::new(x + wheelbase, t.height(x + wheelbase) + wheel_r);
        let seat = (rear + front).scale(0.5) + Cx::new(0.0, 64.0);
        Bike {
            rear,
            rear_prev: rear,
            front,
            front_prev: front,
            seat,
            seat_prev: seat,
            wheelbase,
            seat_rear: (seat - rear).abs(),
            seat_front: (seat - front).abs(),
            wheel_r,
            drive: Drivetrain::default(),
            gravity: -900.0,
            damping: 0.6,
            rear_grounded: true,
            front_grounded: true,
            distance: 0.0,
        }
    }

    pub fn speed(&self) -> f64 {
        ((self.rear - self.rear_prev).abs() + (self.front - self.front_prev).abs()) * 0.5
    }

    /// The crank sits between the wheels, a little below the seat.
    pub fn crank_centre(&self) -> Cx {
        let along = (self.front - self.rear).unit();
        self.rear + along.scale(self.wheelbase * 0.46) + (Cx::I * along).scale(20.0)
    }

    /// Handlebar position, at the front of the frame.
    pub fn bars(&self) -> Cx {
        let along = (self.front - self.rear).unit();
        self.front + (Cx::I * along).scale(64.0) - along.scale(6.0)
    }

    /// One step. `pedal` in 0..=1 is how hard the rider is pushing;
    /// `brake` likewise.
    pub fn step(&mut self, t: &Terrain, dt: f64, pedal: f64, brake: f64) {
        let d = self.damping.powf(dt);
        let g = Cx::new(0.0, self.gravity);
        let start = self.rear;

        // --- Verlet on the three frame points ---------------------------
        for (p, prev) in [
            (&mut self.rear, &mut self.rear_prev),
            (&mut self.front, &mut self.front_prev),
            (&mut self.seat, &mut self.seat_prev),
        ] {
            let v = (*p - *prev).scale(d);
            let next = *p + v + g.scale(dt * dt);
            *prev = *p;
            *p = next;
        }

        // --- drive, only where there is grip ----------------------------
        if self.rear_grounded && pedal > 0.0 {
            let along = t.tangent(self.rear.re);
            let speed = self.speed() / dt.max(1e-9);
            // cadence-limited: past top speed the pedals cannot keep up
            let head = (1.0 - speed / self.drive.top_speed().max(1e-6)).clamp(0.0, 1.0);
            let f = self.drive.drive_force() * pedal * head;
            let a = along.scale(f * dt * dt);
            self.rear = self.rear + a;
            self.front = self.front + a;
            self.seat = self.seat + a;
        }
        if brake > 0.0 {
            for (p, prev) in [
                (&mut self.rear, &mut self.rear_prev),
                (&mut self.front, &mut self.front_prev),
                (&mut self.seat, &mut self.seat_prev),
            ] {
                let v = *p - *prev;
                *p = *prev + v.scale(1.0 - 0.85 * brake * (dt * 60.0).min(1.0));
            }
        }

        // --- the frame is rigid: relax the three distances --------------
        for _ in 0..12 {
            solve(&mut self.rear, &mut self.front, self.wheelbase);
            solve(&mut self.rear, &mut self.seat, self.seat_rear);
            solve(&mut self.front, &mut self.seat, self.seat_front);
        }

        // --- keep it the right way up  ★ ---------------------------------
        //
        // Three distance constraints fix the frame's SHAPE and nothing else.
        // They do not say which end is the front, or which side the rider sits
        // on — a bike rotated 180 degrees satisfies every one of them exactly.
        // Left alone it tumbles end over end and rides backwards, upside down,
        // with the solver reporting no error whatsoever.
        //
        // Every side-scrolling bike game solves this the same way: a restoring
        // torque toward the intended heading. Here that is a gentle rotation of
        // the whole frame about its centroid, by the signed angle between where
        // it points and where the hill runs.
        //
        // The signed angle comes out of the two plane products at once:
        //     angle = atan2(a x b, a . b)
        // which is `arg(conj(a) * b)` — one complex multiplication, again.
        {
            let cur = (self.front - self.rear).unit();
            let target = t.tangent(self.rear.re);
            let err = cur.cross(target).atan2(cur.dot(target));
            let gain = if self.rear_grounded || self.front_grounded { 7.0 } else { 2.2 };
            let da = err * (gain * dt).min(1.0);
            let rot = Cx::expi(da);
            let c = (self.rear + self.front + self.seat).scale(1.0 / 3.0);
            for (p, prev) in [
                (&mut self.rear, &mut self.rear_prev),
                (&mut self.front, &mut self.front_prev),
                (&mut self.seat, &mut self.seat_prev),
            ] {
                *p = c + (*p - c) * rot;
                *prev = c + (*prev - c) * rot; // carry the velocity round too
            }
        }

        // Backstop: the seat must sit on the anticlockwise side of the wheel
        // line. Reflect it back if a hard landing ever punches it through.
        let along = self.front - self.rear;
        if along.cross(self.seat - self.rear) < 0.0 {
            let n = Cx::I * along.unit();
            let d = n.dot(self.seat - self.rear);
            self.seat = self.seat - n.scale(2.0 * d);
            self.seat_prev = self.seat_prev - n.scale(2.0 * d);
        }

        // --- the ground -------------------------------------------------
        self.rear_grounded = false;
        self.front_grounded = false;
        if let Some((p, _)) = t.resolve(self.rear, self.wheel_r) {
            self.rear = p;
            self.rear_grounded = true;
        }
        if let Some((p, _)) = t.resolve(self.front, self.wheel_r) {
            self.front = p;
            self.front_grounded = true;
        }

        // --- distance travelled drives the wheels and the pedals --------
        let moved = self.rear.re - start.re;
        self.distance += moved;
        self.drive.sync_to_distance(moved);
    }
}

/// One distance constraint, split evenly between two free points.
fn solve(a: &mut Cx, b: &mut Cx, rest: f64) {
    let d = *b - *a;
    let dist = d.abs();
    if dist < 1e-9 {
        return;
    }
    let corr = d.scale((dist - rest) / dist * 0.5);
    *a = *a + corr;
    *b = *b - corr;
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    /// The slope is differentiated, not sampled, so it must agree with a
    /// finite difference of the height everywhere.
    #[test]
    fn the_slope_is_the_derivative_of_the_height() {
        let t = Terrain::default();
        for k in 0..400 {
            let x = k as f64 * 7.3;
            let e = 1e-5;
            let fd = (t.height(x + e) - t.height(x - e)) / (2.0 * e);
            assert!(close(t.slope(x), fd, 1e-5), "at x={x}: {} vs {fd}", t.slope(x));
        }
    }

    /// The normal is the tangent turned a quarter turn, so it must be a unit
    /// vector, perpendicular to the surface, and point upward.
    #[test]
    fn the_normal_is_perpendicular_and_points_up() {
        let t = Terrain::default();
        for k in 0..200 {
            let x = k as f64 * 11.0;
            let (tan, n) = (t.tangent(x), t.normal(x));
            assert!(close(n.abs(), 1.0, 1e-12));
            assert!(close(n.dot(tan), 0.0, 1e-12));
            assert!(n.im > 0.0, "normal points into the ground at x={x}");
        }
    }

    /// A wheel dropped into the hill must come out sitting exactly on it.
    #[test]
    fn a_wheel_is_pushed_clear_of_the_ground() {
        let t = Terrain::default();
        let r = 30.0;
        for k in 0..120 {
            let x = k as f64 * 19.0;
            let sunk = Cx::new(x, t.height(x) - 10.0);
            let (p, n) = t.resolve(sunk, r).expect("should have been inside");
            assert!(close(n.abs(), 1.0, 1e-9));
            // After iterating, any remaining overlap is a fraction of a pixel.
            // It is not exactly zero, and cannot be: a circle resting in a
            // curved valley touches at more than one point.
            if let Some((d, _)) = t.deepest(p, r) {
                assert!(r - d < 0.02 * r, "still {:.3} deep at x={x}", r - d);
            }
        }
        // well above the ground, nothing happens
        assert!(t.resolve(Cx::new(0.0, t.height(0.0) + 500.0), r).is_none());
    }

    /// ★ The chain does not stretch: one crank turn gives `ratio` wheel turns.
    #[test]
    fn the_gear_ratio_is_chainring_over_sprocket() {
        let mut d = Drivetrain { chainring: 30.0, sprocket: 10.0, ..Drivetrain::default() };
        assert!(close(d.ratio(), 3.0, 1e-12));
        let turn = 2.0 * std::f64::consts::PI;
        d.pedal(turn, 1.0); // exactly one crank revolution
        assert!(close(d.crank, turn, 1e-12));
        assert!(close(d.wheel_angle, 3.0 * turn, 1e-12), "three wheel turns per crank turn");
    }

    /// Distance is `r * theta` — arc length again, the same identity the crane
    /// uses to turn winch rotation into rope.
    #[test]
    fn distance_travelled_is_wheel_radius_times_wheel_angle() {
        let mut d = Drivetrain { chainring: 26.0, sprocket: 13.0, wheel: 34.0, ..Drivetrain::default() };
        let cadence = 5.0;
        let secs = 3.0;
        d.pedal(cadence, secs);
        let s = d.wheel * d.wheel_angle;
        assert!(close(s, d.speed_at(cadence) * secs, 1e-9));
    }

    /// Syncing from distance is the exact inverse of pedalling.
    #[test]
    fn pedalling_and_syncing_are_inverses() {
        let base = Drivetrain::default();
        let mut a = base;
        a.pedal(4.0, 2.5);
        let s = a.wheel * (a.wheel_angle - base.wheel_angle);
        let mut b = base;
        b.sync_to_distance(s);
        assert!(close(a.crank, b.crank, 1e-9));
        assert!(close(a.wheel_angle, b.wheel_angle, 1e-9));
    }

    /// ★★ The whole point of gears. A bigger chainring goes faster and pushes
    /// less hard, and the product of the two does not change — a gear chooses
    /// how to spend the power, it cannot create any.
    #[test]
    fn gears_trade_speed_against_force_and_nothing_else() {
        let low = Drivetrain { chainring: 16.0, ..Drivetrain::default() };
        let high = Drivetrain { chainring: 40.0, ..Drivetrain::default() };

        assert!(high.top_speed() > low.top_speed(), "high gear should be faster");
        assert!(high.drive_force() < low.drive_force(), "high gear should be weaker");

        let power = |d: &Drivetrain| d.top_speed() * d.drive_force();
        assert!(
            close(power(&low), power(&high), 1e-6),
            "speed x force must be independent of the gear: {} vs {}",
            power(&low),
            power(&high)
        );
    }

    /// The pedals are half a turn apart, on a circle about the crank.
    #[test]
    fn the_pedals_are_opposite_points_on_the_crank_circle() {
        let d = Drivetrain { crank: 0.9, ..Drivetrain::default() };
        let c = Cx::new(50.0, 20.0);
        let (a, b) = d.pedals(c, 18.0);
        assert!(close((a - c).abs(), 18.0, 1e-12));
        assert!(close((b - c).abs(), 18.0, 1e-12));
        assert!(close((a - b).abs(), 36.0, 1e-12), "should be diametrically opposite");
    }

    /// ★ Two-bone IK: the knee must be exactly one thigh from the hip and one
    /// shin from the foot. That is what makes it a solution.
    #[test]
    fn the_knee_lands_at_both_bone_lengths() {
        let hip = Cx::new(0.0, 100.0);
        let (thigh, shin) = (46.0, 44.0);
        for k in 0..60 {
            let a = k as f64 * 0.1;
            let foot = hip + Cx::expi(-1.0 + a * 0.05).scale(50.0 + 30.0 * a.sin());
            let knee = two_bone_ik(hip, foot, thigh, shin, 1.0);
            assert!(close((knee - hip).abs(), thigh, 1e-9), "thigh wrong at k={k}");
            assert!(close((knee - foot).abs(), shin, 1e-9), "shin wrong at k={k}");
        }
    }

    /// The two bend signs are mirror images across the hip-foot line.
    #[test]
    fn the_bend_sign_picks_which_way_the_knee_faces() {
        let hip = Cx::new(0.0, 0.0);
        let foot = Cx::new(60.0, -20.0);
        let a = two_bone_ik(hip, foot, 46.0, 44.0, 1.0);
        let b = two_bone_ik(hip, foot, 46.0, 44.0, -1.0);
        assert!((a - b).abs() > 1.0, "the two solutions should differ");
        // both are still valid solutions
        for k in [a, b] {
            assert!(close((k - hip).abs(), 46.0, 1e-9));
            assert!(close((k - foot).abs(), 44.0, 1e-9));
        }
    }

    /// Out of reach, the leg straightens instead of producing a NaN.
    #[test]
    fn an_unreachable_foot_straightens_the_leg() {
        let hip = Cx::ZERO;
        let foot = Cx::new(500.0, 0.0);
        let knee = two_bone_ik(hip, foot, 46.0, 44.0, 1.0);
        assert!(knee.re.is_finite() && knee.im.is_finite());
        assert!(close((knee - hip).abs(), 46.0, 1e-6));
        // and it points at the foot
        assert!(knee.re > 40.0);
    }

    /// The frame is rigid: however it is thrown about, the three distances
    /// hold.
    #[test]
    fn the_frame_keeps_its_shape() {
        let t = Terrain::default();
        let mut b = Bike::new(&t, 400.0);
        let (wb, sr, sf) = (b.wheelbase, b.seat_rear, b.seat_front);
        for k in 0..3000 {
            let pedal = if k % 400 < 250 { 1.0 } else { 0.0 };
            b.step(&t, 1.0 / 600.0, pedal, 0.0);
        }
        assert!(close((b.front - b.rear).abs(), wb, 1.0), "wheelbase drifted");
        assert!(close((b.seat - b.rear).abs(), sr, 1.0));
        assert!(close((b.seat - b.front).abs(), sf, 1.0));
    }

    /// Pedalling moves the bike forward, and it stays on the hill.
    #[test]
    fn pedalling_moves_the_bike_and_it_stays_on_the_ground() {
        let t = Terrain::default();
        let mut b = Bike::new(&t, 300.0);
        let x0 = b.rear.re;
        for _ in 0..6000 {
            b.step(&t, 1.0 / 600.0, 1.0, 0.0);
        }
        assert!(b.rear.re > x0 + 200.0, "only moved {}", b.rear.re - x0);
        // never sunk into the hill
        assert!(t.resolve(b.rear, b.wheel_r * 0.98).is_none(), "the wheel is buried");
        assert!(b.rear.im > t.height(b.rear.re) - 1.0);
    }

    /// Coasting must still turn the pedals - the drivetrain is driven by the
    /// ground as much as by the rider.
    #[test]
    fn the_pedals_keep_up_with_the_ground_while_coasting() {
        let t = Terrain::default();
        let mut b = Bike::new(&t, 300.0);
        for _ in 0..2000 {
            b.step(&t, 1.0 / 600.0, 1.0, 0.0);
        }
        let crank = b.drive.crank;
        for _ in 0..2000 {
            b.step(&t, 1.0 / 600.0, 0.0, 0.0); // no pedalling at all
        }
        assert!(
            (b.drive.crank - crank).abs() > 0.5,
            "the cranks stopped turning while the bike was still rolling"
        );
    }

    /// ★ The bike must stay upright AND facing forward. Distance constraints
    /// alone permit neither: a frame rotated 180 degrees satisfies every one
    /// of them exactly, so without a restoring torque the bike ends up riding
    /// backwards and upside down with the solver reporting no error at all.
    #[test]
    fn the_bike_stays_upright_and_facing_forward() {
        let t = Terrain::default();
        let mut b = Bike::new(&t, 300.0);
        for k in 0..12_000 {
            // pedal hard, brake hard, launch off crests - try to flip it
            let pedal = if k % 900 < 700 { 1.0 } else { 0.0 };
            let brake = if k % 1700 < 120 { 1.0 } else { 0.0 };
            b.step(&t, 1.0 / 600.0, pedal, brake);
            let along = b.front - b.rear;
            assert!(along.re > 0.0, "the bike turned round at step {k}: along = {along}");
            assert!(along.cross(b.seat - b.rear) > 0.0, "the frame inverted at step {k}");
            assert!(
                b.seat.im > (b.rear.im + b.front.im) * 0.5,
                "the seat sank below the wheels at step {k}"
            );
        }
    }

    /// Braking takes speed off.
    #[test]
    fn braking_slows_it_down() {
        let t = Terrain::default();
        let mut b = Bike::new(&t, 300.0);
        for _ in 0..4000 {
            b.step(&t, 1.0 / 600.0, 1.0, 0.0);
        }
        let fast = b.speed();
        for _ in 0..1200 {
            b.step(&t, 1.0 / 600.0, 0.0, 1.0);
        }
        assert!(b.speed() < fast * 0.5, "{} -> {}", fast, b.speed());
    }
}
