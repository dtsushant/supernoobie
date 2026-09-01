//! # cyclone — a 2D drawing that reads as 3D
//!
//! A funnel of stacked rings, twisting. Nothing here is 3D — every point ends
//! up as one [`Cx`] on the flat page — but it reads as a solid turning thing,
//! and it does so on the back of one fact:
//!
//! > **An ellipse is a circle seen at an angle.**
//!
//! Look straight down on a circle and you see a circle. Tilt away and the
//! width you see across the line of sight is unchanged while the depth
//! shortens by `sin(tilt)`. Keep tilting and it flattens to a line. That is
//! the whole of it — one multiplication:
//!
//! ```text
//!     screen_x = x
//!     screen_y = y · sin(tilt)  +  z · cos(tilt)
//!                    ^^^^^^^^^      ^^^^^^^^^
//!                    depth is       height stands
//!                    squashed       up instead
//! ```
//!
//! At `tilt = π/2` you are overhead: `sin = 1`, so circles stay circles and
//! every height lands in the same place. At `tilt = 0` you are edge on:
//! `sin = 0`, the circles collapse to lines, and height is all you see. Turn
//! the tilt in the sketch and watch the two trade off.
//!
//! ## Where the physics is
//!
//! The funnel's silhouette is just a shape. The part that is real is **how
//! fast each ring turns**, and it is not the same for all of them.
//!
//! A vortex conserves its **circulation** `Γ`. Away from the middle the speed
//! goes as `v = Γ / 2πr`, so the angular rate is
//!
//! ```text
//!     ω(r) = v/r = Γ / (2π r²)
//! ```
//!
//! — the narrow bottom whips round while the wide top barely moves. That is
//! why the streamers wind up tighter as time passes rather than turning as a
//! rigid body, and it is the same conservation law as a skater pulling their
//! arms in.
//!
//! Taken literally, `ω → ∞` as `r → 0`. Real vortices do not do that, and
//! neither does this one: inside a **core radius** the flow turns as a solid
//! body, at one rate. That is the *Rankine vortex*, and the core is the eye.
//!
//! **The winding never stops.** Nothing here limits it, and nothing should:
//! two neighbouring heights turning at different rates shear apart for as long
//! as they keep turning. A real vortex ends that by breaking down into
//! turbulence and mixing; this one just keeps coiling until the strands are a
//! smear. `R` in the sketch starts it again.
//!
//! ## What would make it real
//!
//! Deliberately absent, so it is clear what is a drawing and what is a model:
//! air moving *up* the funnel, pressure falling toward the middle, the
//! Coriolis force that decides which way a real cyclone turns, and any
//! coupling between the rings. Each is a thing to add, not a thing forgotten.

use crate::recipe::Recipe;
use plotkit::{Cx, Shape};
use std::f64::consts::{PI, TAU};

/// A funnel of turning rings, drawn on the flat page.
#[derive(Clone, Copy, Debug)]
pub struct Cyclone {
    /// How tall, from touchdown to the top.
    pub height: f64,
    /// The radius at the top, where it is widest.
    pub top: f64,
    /// The radius at the bottom, where it touches down.
    pub tip: f64,
    /// How the radius grows with height. `1` is a straight cone; more than 1
    /// pinches the bottom into a funnel.
    pub flare: f64,
    /// How many rings the funnel is drawn with.
    pub rings: usize,
    /// The camera's angle above the horizontal, in radians. `π/2` is directly
    /// overhead; `0` is edge on.
    pub tilt: f64,
    /// Circulation `Γ`. Sets how fast the whole thing turns.
    pub circulation: f64,
    /// Inside this radius the flow turns as a solid body — the eye. Without
    /// it, `ω = Γ/2πr²` runs away to infinity at the middle.
    pub core: f64,
}

impl Default for Cyclone {
    fn default() -> Cyclone {
        Cyclone {
            height: 5.0,
            top: 2.6,
            tip: 0.22,
            flare: 1.7,
            rings: 16,
            tilt: 1.05,
            circulation: 3.0,
            core: 0.35,
        }
    }
}

impl Cyclone {
    pub fn new() -> Cyclone {
        Cyclone::default()
    }

    // ---- the one place 3D becomes 2D -------------------------------------

    /// A point in space, flattened onto the page.
    ///
    /// `x` is across, `y` is depth, `z` is up. Depth is squashed by
    /// `sin(tilt)` and height stands up by `cos(tilt)`, and those two are the
    /// entire illusion.
    pub fn project(&self, x: f64, y: f64, z: f64) -> Cx {
        Cx::new(x, y * self.tilt.sin() + z * self.tilt.cos())
    }

    /// How much a circle in the ground plane is squashed on the page — the
    /// ratio of the ellipse's axes.
    pub fn foreshortening(&self) -> f64 {
        self.tilt.sin()
    }

    // ---- the funnel ------------------------------------------------------

    /// The height of ring `u`, where `u` runs 0 (touchdown) to 1 (top).
    pub fn height_at(&self, u: f64) -> f64 {
        self.height * u
    }

    /// The radius at height fraction `u`. Widening upward, pinched by `flare`.
    pub fn radius_at(&self, u: f64) -> f64 {
        self.tip + (self.top - self.tip) * u.clamp(0.0, 1.0).powf(self.flare)
    }

    /// How fast a ring of radius `r` turns, in radians per second.
    ///
    /// `Γ / 2πr²` outside the core, and one flat rate inside it. The `max` is
    /// doing the whole job of keeping the middle finite.
    pub fn spin_at(&self, r: f64) -> f64 {
        let r = r.max(self.core).max(1e-6);
        self.circulation / (TAU * r * r)
    }

    /// Where ring `u` has turned to at time `t`.
    pub fn phase_at(&self, u: f64, t: f64) -> f64 {
        self.spin_at(self.radius_at(u)) * t
    }

    /// One ring, as the ellipse it projects to.
    pub fn ring(&self, u: f64, t: f64) -> Shape {
        self.arc(u, t, 0.0, TAU)
    }

    /// Part of a ring, between two angles — used to draw the near and far
    /// halves in different colours.
    fn arc(&self, u: f64, t: f64, a0: f64, a1: f64) -> Shape {
        let me = *self;
        let (r, z, phase) = (self.radius_at(u), self.height_at(u), self.phase_at(u, t));
        Shape::param(move |a| me.project(r * (a + phase).cos(), r * (a + phase).sin(), z), a0, a1, 96)
    }

    /// Every ring.
    pub fn funnel(&self, t: f64) -> Shape {
        Shape::group((0..self.rings).map(|k| self.ring(self.u(k), t)).collect::<Vec<_>>())
    }

    /// The rings split into the half facing away and the half facing you.
    ///
    /// Depth is `y`, and larger `y` is further off, so the far half is where
    /// `sin > 0`. Drawing the far half dimmer is what stops the funnel reading
    /// as a stack of flat hoops.
    pub fn halves(&self, t: f64) -> (Shape, Shape) {
        let far = (0..self.rings).map(|k| self.arc(self.u(k), t, 0.0, PI)).collect::<Vec<_>>();
        let near = (0..self.rings).map(|k| self.arc(self.u(k), t, PI, TAU)).collect::<Vec<_>>();
        (Shape::group(far), Shape::group(near))
    }

    /// A strand of air, from the ground to the top.
    ///
    /// This is where the differential rotation becomes visible. Each height
    /// turns at its own rate, so a strand that started straight **winds up**
    /// as time goes on — tighter at the bottom, where the radius is small and
    /// `ω = Γ/2πr²` is large. A rigid body would never do that.
    pub fn streamer(&self, from: f64, t: f64) -> Shape {
        let me = *self;
        Shape::param(
            move |u| {
                let (r, z) = (me.radius_at(u), me.height_at(u));
                let a = from + me.phase_at(u, t);
                me.project(r * a.cos(), r * a.sin(), z)
            },
            0.0,
            1.0,
            420,
        )
    }

    /// `n` strands, evenly spaced round the funnel — the `n`th roots of unity
    /// again, put to work.
    pub fn streamers(&self, n: usize, t: f64) -> Shape {
        Shape::group((0..n).map(|k| self.streamer(TAU * k as f64 / n.max(1) as f64, t)).collect::<Vec<_>>())
    }

    /// The eye: the core, drawn on the ground.
    pub fn eye(&self) -> Shape {
        let me = *self;
        Shape::param(move |a| me.project(me.core * a.cos(), me.core * a.sin(), 0.0), 0.0, TAU, 96)
    }

    /// The axis it turns about.
    pub fn spine(&self) -> Shape {
        Shape::path(vec![self.project(0.0, 0.0, 0.0), self.project(0.0, 0.0, self.height)])
    }

    /// Everything, as one shape.
    pub fn shape(&self, t: f64) -> Shape {
        Shape::group(vec![self.funnel(t), self.streamers(5, t), self.eye()])
    }

    fn u(&self, k: usize) -> f64 {
        if self.rings <= 1 {
            1.0
        } else {
            k as f64 / (self.rings - 1) as f64
        }
    }

    pub fn recipe(&self, t: f64) -> Recipe {
        Recipe::new("cyclone", "screen = (x, y sin(tilt) + z cos(tilt)); omega(r) = Gamma / 2 pi r^2")
            .step("the axis it turns about", self.spine())
            .step("rings, each a circle seen at an angle — so, an ellipse", self.funnel(t))
            .step("strands of air, winding up because each height turns at its own rate", self.streamers(5, t))
            .step("the eye: inside the core it turns as one solid piece", self.eye())
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn pts(s: &Shape) -> Vec<Cx> {
        s.polylines(Cx::new(-99.0, -99.0), Cx::new(99.0, 99.0), 600).into_iter().flatten().collect()
    }

    /// The bounding box, as `(lo_x, hi_x, lo_y, hi_y)`.
    ///
    /// Not the mean of the samples: a closed `param` visits its start point
    /// twice, so the mean sits about a hundredth of a point off centre — plenty
    /// to fail a 1e-6 assertion and send you looking for a bug in the
    /// projection that is not there.
    fn box_of(s: &Shape) -> (f64, f64, f64, f64) {
        let p = pts(s);
        let (lo_x, hi_x) = p.iter().fold((f64::MAX, f64::MIN), |(a, b), q| (a.min(q.re), b.max(q.re)));
        let (lo_y, hi_y) = p.iter().fold((f64::MAX, f64::MIN), |(a, b), q| (a.min(q.im), b.max(q.im)));
        (lo_x, hi_x, lo_y, hi_y)
    }

    /// The two semi-axes of a ring as drawn.
    fn axes(s: &Shape) -> (f64, f64) {
        let (lo_x, hi_x, lo_y, hi_y) = box_of(s);
        ((hi_x - lo_x) / 2.0, (hi_y - lo_y) / 2.0)
    }

    /// Where a ring sits up the page.
    fn mid_y(s: &Shape) -> f64 {
        let (_, _, lo, hi) = box_of(s);
        (lo + hi) / 2.0
    }

    /// ★ The one fact the whole illusion rests on: a circle seen at an angle
    /// is an ellipse, squashed by exactly `sin(tilt)`. If this were off, every
    /// ring would still be an ellipse and nothing would look obviously wrong —
    /// it would just never read as solid.
    #[test]
    fn a_circle_seen_at_an_angle_is_an_ellipse_squashed_by_sin_tilt() {
        for tilt in [0.3, 0.7, 1.0, 1.3, PI / 2.0] {
            let c = Cyclone { tilt, ..Cyclone::default() };
            let (wide, tall) = axes(&c.ring(1.0, 0.0));
            assert!((wide - c.radius_at(1.0)).abs() < 1e-6, "the across-axis should be the true radius");
            assert!((tall / wide - tilt.sin()).abs() < 1e-6, "at tilt {tilt} the squash was {}", tall / wide);
        }
    }

    /// Overhead, nothing is squashed and every height lands in the same place:
    /// you are looking straight down the axis.
    #[test]
    fn from_overhead_it_is_all_circles_and_no_height() {
        let c = Cyclone { tilt: PI / 2.0, ..Cyclone::default() };
        let (wide, tall) = axes(&c.ring(0.5, 0.0));
        assert!((wide - tall).abs() < 1e-9, "a circle from directly above");

        assert!(
            (mid_y(&c.ring(0.0, 0.0)) - mid_y(&c.ring(1.0, 0.0))).abs() < 1e-9,
            "height should contribute nothing from overhead"
        );
    }

    /// Edge on, the rings collapse to lines and height is all there is.
    #[test]
    fn from_the_side_the_rings_are_flat_and_height_is_everything() {
        let c = Cyclone { tilt: 0.0, ..Cyclone::default() };
        let (_, tall) = axes(&c.ring(0.5, 0.0));
        assert!(tall < 1e-9, "a circle seen edge on is a line");

        assert!(mid_y(&c.ring(1.0, 0.0)) > mid_y(&c.ring(0.0, 0.0)) + 1.0, "the top should sit above the bottom");
    }

    /// ★ The physics. A vortex conserves circulation, so the inside turns
    /// faster than the outside — `ω = Γ/2πr²`. A cyclone that turned as a
    /// rigid body would be a drawing of a cyclone, not a model of one.
    #[test]
    fn the_narrow_bottom_turns_faster_than_the_wide_top() {
        let c = Cyclone::default();
        let bottom = c.spin_at(c.radius_at(0.0));
        let top = c.spin_at(c.radius_at(1.0));
        assert!(bottom > top * 4.0, "bottom {bottom}, top {top}");

        // And it really is the inverse-square law, out where the core is not
        // interfering: doubling the radius quarters the rate.
        let (a, b) = (c.spin_at(1.0), c.spin_at(2.0));
        assert!((a / b - 4.0).abs() < 1e-9, "expected a factor of four, got {}", a / b);
    }

    /// ★ Without a core, `Γ/2πr²` runs away to infinity at the middle. Real
    /// vortices turn as a solid body in there, and so does this one — the eye.
    #[test]
    fn the_eye_keeps_the_middle_finite() {
        let c = Cyclone { core: 0.4, ..Cyclone::default() };
        let at_core = c.spin_at(0.4);
        for r in [0.0, 1e-12, 0.1, 0.2, 0.399] {
            assert!(c.spin_at(r).is_finite(), "blew up at r = {r}");
            assert!((c.spin_at(r) - at_core).abs() < 1e-9, "inside the eye it should turn as one piece");
        }
        assert!(c.spin_at(0.8) < at_core, "and outside it should fall away again");
    }

    /// The funnel is a funnel: it never gets narrower going up.
    #[test]
    fn the_funnel_only_widens_upward() {
        let c = Cyclone::default();
        let mut last = c.radius_at(0.0);
        for k in 1..=50 {
            let r = c.radius_at(k as f64 / 50.0);
            assert!(r >= last - 1e-12, "narrowed at u = {}", k as f64 / 50.0);
            last = r;
        }
        assert!((c.radius_at(0.0) - c.tip).abs() < 1e-12);
        assert!((c.radius_at(1.0) - c.top).abs() < 1e-12);
    }

    /// ★ A strand winds up as time goes on, because each height turns at its
    /// own rate. This is the differential rotation made visible, and it is the
    /// thing that would be missing if every ring shared one phase.
    #[test]
    fn a_strand_winds_tighter_as_time_passes() {
        let c = Cyclone::default();
        let twist = |t: f64| (c.phase_at(0.0, t) - c.phase_at(1.0, t)).abs();
        assert!(twist(0.0) < 1e-12, "it starts straight");
        assert!(twist(2.0) > twist(1.0), "and keeps winding");
        assert!(twist(1.0) > 1.0, "by a visible amount");
    }

    /// Both halves together are the whole ring, and each is half of it — the
    /// split is for depth, not for leaving bits out.
    #[test]
    fn the_two_halves_make_a_whole_ring() {
        let c = Cyclone { rings: 4, ..Cyclone::default() };
        let (far, near) = c.halves(0.3);
        assert_eq!(pts(&far).len(), pts(&near).len());

        // The far half sits higher on the page, which is what makes it read as
        // further away.
        let mean = |s: &Shape| pts(s).iter().map(|p| p.im).sum::<f64>() / pts(s).len() as f64;
        assert!(mean(&far) > mean(&near), "the far half should be the upper one");
    }

    /// A cyclone drawn with one ring, or none, must not divide by zero on its
    /// way to looking silly.
    #[test]
    fn a_degenerate_cyclone_does_not_panic() {
        for rings in [0usize, 1, 2] {
            let c = Cyclone { rings, ..Cyclone::default() };
            let n = pts(&c.shape(1.0)).len();
            assert!(n > 0 || rings == 0, "{rings} rings produced nothing");
        }
    }

    #[test]
    fn it_stands_on_the_ground_and_reaches_its_height() {
        let c = Cyclone::default();
        assert!((c.height_at(0.0)).abs() < 1e-12);
        assert!((c.height_at(1.0) - c.height).abs() < 1e-12);
    }
}
