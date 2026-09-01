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
    /// Where it stands **on the ground**: `x` across, `y` into the distance.
    ///
    /// Not a screen offset. A storm crossing the ground away from you climbs
    /// the page by only `sin(tilt)` of the distance it covers, and that
    /// difference is what makes it read as tracking over a plane rather than
    /// sliding about on the glass.
    pub at: Cx,
    /// How tall, from its foot to the top.
    pub height: f64,
    /// How far its foot is off the ground.
    ///
    /// Zero while it is on the ground. A dying vortex ropes out and lifts, and
    /// raising this is what that looks like.
    pub lift: f64,
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
            at: Cx::ZERO,
            height: 5.0,
            lift: 0.0,
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
    ///
    /// Absolute: this does **not** add [`Cyclone::at`], so it can be used to
    /// place anything on the same ground — a trail, a marker, a fence post.
    pub fn project(&self, x: f64, y: f64, z: f64) -> Cx {
        Cx::new(x, y * self.tilt.sin() + z * self.tilt.cos())
    }

    /// A point measured from the storm's own axis, on the ground it is
    /// standing on.
    fn mine(&self, x: f64, y: f64, z: f64) -> Cx {
        self.project(self.at.re + x, self.at.im + y, z)
    }

    /// Where its foot is, on the page.
    pub fn foot(&self) -> Cx {
        self.mine(0.0, 0.0, 0.0)
    }

    /// The path it has taken over the ground, between two times.
    ///
    /// Projected like everything else, so the track recedes with the ground
    /// instead of lying flat on the screen — which is most of what sells the
    /// plane.
    pub fn trail(&self, path: impl Fn(f64) -> Cx + Send + Sync + 'static, from: f64, to: f64) -> Shape {
        let me = *self;
        Shape::param(
            move |t| {
                let g = path(t);
                me.project(g.re, g.im, 0.0)
            },
            from,
            to,
            400,
        )
    }

    /// How much a circle in the ground plane is squashed on the page — the
    /// ratio of the ellipse's axes.
    pub fn foreshortening(&self) -> f64 {
        self.tilt.sin()
    }

    // ---- the funnel ------------------------------------------------------

    /// The height of ring `u`, where `u` runs 0 (the foot) to 1 (the top).
    pub fn height_at(&self, u: f64) -> f64 {
        self.lift + (self.height - self.lift) * u
    }

    /// The wind speed at a point on the ground.
    ///
    /// `v = Γ / 2πr` — the free-vortex speed, flattening off inside the core
    /// exactly as [`Cyclone::spin_at`] does, because they are the same flow
    /// asked about two different ways.
    ///
    /// This is what [`crate::terrain::Field::blow`] wants, and tying damage to
    /// it means a storm that loses its circulation stops being able to flatten
    /// anything without a single line written to arrange that.
    pub fn wind_at(&self, ground: Cx) -> f64 {
        let r = (ground - self.at).abs().max(self.core).max(1e-6);
        self.circulation / (TAU * r)
    }

    /// The most wind it has anywhere — which is at the edge of the core,
    /// since inside it the flow turns as a solid body and stops speeding up.
    pub fn strongest_wind(&self) -> f64 {
        self.circulation / (TAU * self.core.max(1e-6))
    }

    /// How far out it can still knock over something of this strength.
    ///
    /// Solve `v = Γ/2πr` for the `r` where the wind drops to what the thing
    /// can take. Not a number anybody chose.
    ///
    /// **Zero** when that radius lies inside the core: the wind stops rising
    /// in there, so a thing that strong is never blown over at all rather than
    /// being blown over very close in. Forgetting that is easy — the formula
    /// keeps returning a smaller and smaller radius, and it stops meaning
    /// anything once it passes the core.
    pub fn reach(&self, strength: f64) -> f64 {
        if strength <= 1e-9 {
            return f64::INFINITY;
        }
        let r = self.circulation / (TAU * strength);
        if r < self.core {
            0.0
        } else {
            r
        }
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

    /// Part of a ring, between two angles **measured on the ring itself** —
    /// so the piece drawn turns with the ring.
    fn arc(&self, u: f64, t: f64, a0: f64, a1: f64) -> Shape {
        let me = *self;
        let (r, z, phase) = (self.radius_at(u), self.height_at(u), self.phase_at(u, t));
        Shape::param(move |a| me.mine(r * (a + phase).cos(), r * (a + phase).sin(), z), a0, a1, 96)
    }

    /// Part of a ring, between two angles **measured in the world** — so the
    /// piece drawn stays where it is while the ring turns through it.
    ///
    /// The difference between this and [`Cyclone::arc`] is the whole of
    /// near-and-far. Depth belongs to the camera, not to the material: a patch
    /// of air is in front of the axis or behind it depending on where it *is*,
    /// not on how far the ring it belongs to has turned.
    fn arc_world(&self, u: f64, t: f64, w0: f64, w1: f64) -> Shape {
        let phase = self.phase_at(u, t);
        self.arc(u, t, w0 - phase, w1 - phase)
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
        // World angles, not ring angles. Split on the ring's own angle and the
        // lit half whirls round with the ring — fastest at the bottom, where
        // omega is largest — and the whole funnel appears to lurch about.
        let far = (0..self.rings).map(|k| self.arc_world(self.u(k), t, 0.0, PI)).collect::<Vec<_>>();
        let near = (0..self.rings).map(|k| self.arc_world(self.u(k), t, PI, TAU)).collect::<Vec<_>>();
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
                me.mine(r * a.cos(), r * a.sin(), z)
            },
            0.0,
            1.0,
            420,
        )
    }

    /// `n` strands, evenly spaced round the funnel — the `n`th roots of unity
    /// again, put to work.
    ///
    /// These wind forever. That is true of a strand of *dye*, and it turns the
    /// funnel into a smear after a while; see [`Cyclone::shed`] for strands of
    /// air, which do not.
    pub fn streamers(&self, n: usize, t: f64) -> Shape {
        Shape::group((0..n).map(|k| self.streamer(TAU * k as f64 / n.max(1) as f64, t)).collect::<Vec<_>>())
    }

    /// `n` strands, each only `age` seconds old, shed one after another.
    ///
    /// The air in a funnel is not the same air for a minute — it is drawn in
    /// at the bottom and thrown out at the top in seconds. So a strand that
    /// kept winding forever would be dye, not air, and after half a minute it
    /// is an unreadable smear.
    ///
    /// Each strand is staggered by `age/n`, so as one reaches its age and is
    /// let go, the next is already halfway. What you see is a steady spiral
    /// being continuously renewed, which is what a funnel looks like.
    pub fn shed(&self, n: usize, age: f64, t: f64) -> Shape {
        let n = n.max(1);
        let age = age.max(1e-3);
        Shape::group(
            (0..n)
                .map(|k| {
                    let born = (t + age * k as f64 / n as f64).rem_euclid(age);
                    self.streamer(TAU * k as f64 / n as f64, born)
                })
                .collect::<Vec<_>>(),
        )
    }

    /// The eye: the core, drawn on the ground.
    pub fn eye(&self) -> Shape {
        let me = *self;
        Shape::param(move |a| me.mine(me.core * a.cos(), me.core * a.sin(), 0.0), 0.0, TAU, 96)
    }

    /// The axis it turns about.
    pub fn spine(&self) -> Shape {
        Shape::path(vec![self.mine(0.0, 0.0, 0.0), self.mine(0.0, 0.0, self.height)])
    }

    /// Everything, as one shape.
    pub fn shape(&self, t: f64) -> Shape {
        Shape::group(vec![self.funnel(t), self.shed(5, 2.5, t), self.eye()])
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
    }

    /// ★ Depth belongs to the camera, not to the material.
    ///
    /// The split has to be at fixed **world** angles. Split on the ring's own
    /// angle instead and the lit half turns with the ring — fastest at the
    /// bottom, where omega is largest — so the funnel appears to lurch about
    /// the plane at random. Which is exactly what it did.
    ///
    /// Checked per ring and at many times, because averaging over the whole
    /// funnel hides it: at any instant some rings have turned far enough to
    /// flip and others have not, and the mean comes out plausible anyway.
    #[test]
    fn the_far_half_stays_far_however_much_it_has_turned() {
        let c = Cyclone::default();
        for step in 0..40 {
            let t = step as f64 * 0.37; // well past a full turn at the bottom
            for k in 0..8 {
                let u = k as f64 / 7.0;
                let far = c.arc_world(u, t, 0.0, PI);
                let near = c.arc_world(u, t, PI, TAU);
                assert!(
                    mid_y(&far) > mid_y(&near),
                    "at t = {t:.2}, ring {u:.2} has its far half below its near half —                      the split is turning with the ring"
                );
            }
        }
    }

    /// And the rings really are turning under that fixed split — otherwise the
    /// test above would pass for the boring reason that nothing ever moves.
    #[test]
    fn the_rings_do_turn_under_the_fixed_split() {
        let c = Cyclone::default();
        // Several turns over the span the depth test walks, which is what makes
        // that test able to catch a split that turns with the material.
        let bottom = c.phase_at(0.0, 40.0 * 0.37);
        assert!(bottom > 4.0 * TAU, "the bottom barely moved: {bottom} rad");
        assert!(c.phase_at(0.0, 1.0) > 8.0 * c.phase_at(1.0, 1.0), "and the bottom should far outrun the top");
    }

    /// A cyclone drawn with one ring, or none, must not divide by zero on its
    /// way to looking silly.
    /// ★ Shed strands never wind past their age, so the funnel stays readable
    /// however long it runs. Strands that kept winding are dye, not air.
    #[test]
    fn shed_strands_never_wind_past_their_age() {
        let c = Cyclone::default();
        let reach = |t: f64| {
            pts(&c.shed(5, 2.0, t)).iter().fold(0.0f64, |m, p| m.max(p.abs()))
        };
        let early = reach(1.0);
        for k in 0..200 {
            let late = reach(60.0 + k as f64 * 0.31);
            assert!(late <= early * 1.5 + 1e-9, "at a minute in it had grown to {late} from {early}");
        }
    }

    #[test]
    fn a_degenerate_cyclone_does_not_panic() {
        for rings in [0usize, 1, 2] {
            let c = Cyclone { rings, ..Cyclone::default() };
            let n = pts(&c.shape(1.0)).len();
            assert!(n > 0 || rings == 0, "{rings} rings produced nothing");
        }
    }

    /// ★ Crossing the ground is not a translation on screen.
    ///
    /// Sideways is one-to-one, but moving away from you climbs the page by
    /// only `sin(tilt)` of the distance covered. Add the offset in 2D instead
    /// and the storm floats about on the glass rather than tracking over a
    /// plane — it would look wrong without ever looking obviously wrong.
    #[test]
    fn crossing_the_ground_is_foreshortened_not_translated() {
        let c = Cyclone { tilt: 0.9, ..Cyclone::default() };
        let home = c.foot();

        let sideways = Cyclone { at: Cx::new(2.0, 0.0), ..c }.foot() - home;
        assert!((sideways - Cx::new(2.0, 0.0)).abs() < 1e-12, "across the view is one for one");

        let away = Cyclone { at: Cx::new(0.0, 2.0), ..c }.foot() - home;
        assert!(away.re.abs() < 1e-12, "going away must not slide sideways");
        assert!((away.im - 2.0 * 0.9f64.sin()).abs() < 1e-12, "and must climb only sin(tilt) of it");
        assert!(away.im < sideways.re, "the same distance away covers less page than across");
    }

    /// The whole storm goes with it — funnel, strands, eye and spine — rather
    /// than the foot wandering off and leaving the rest behind.
    #[test]
    fn the_whole_storm_travels_together() {
        let a = Cyclone::default();
        let b = Cyclone { at: Cx::new(3.0, 1.0), ..a };
        let shift = b.foot() - a.foot();

        for (name, x, y) in [("funnel", a.ring(0.6, 0.4), b.ring(0.6, 0.4)), ("eye", a.eye(), b.eye()), ("spine", a.spine(), b.spine())] {
            let (px, py) = (pts(&x), pts(&y));
            assert_eq!(px.len(), py.len(), "{name}");
            for (p, q) in px.iter().zip(&py) {
                assert!((*q - *p - shift).abs() < 1e-9, "{name} did not travel with the foot");
            }
        }
    }

    /// A trail is drawn on the ground, so it recedes with it. A track that lay
    /// flat on the screen would undo the plane the rest of the drawing builds.
    #[test]
    fn a_trail_lies_on_the_ground() {
        let c = Cyclone { tilt: 0.8, ..Cyclone::default() };
        // A path straight away from the viewer.
        let line = c.trail(|t| Cx::new(0.0, t), 0.0, 4.0);
        let p = pts(&line);
        let (lo, hi) = (p[0], p[p.len() - 1]);
        assert!((hi.re - lo.re).abs() < 1e-9, "it should not drift sideways");
        assert!((hi.im - lo.im - 4.0 * 0.8f64.sin()).abs() < 1e-6, "and should be foreshortened");
    }

    #[test]
    fn it_stands_on_the_ground_and_reaches_its_height() {
        let c = Cyclone::default();
        assert!((c.height_at(0.0)).abs() < 1e-12);
        assert!((c.height_at(1.0) - c.height).abs() < 1e-12);

        // Lifted off, its foot leaves the ground and the top stays put.
        let up = Cyclone { lift: 2.0, ..c };
        assert!((up.height_at(0.0) - 2.0).abs() < 1e-12);
        assert!((up.height_at(1.0) - c.height).abs() < 1e-12);
    }

    /// ★ The wind and the spin are the same flow, asked about two different
    /// ways: `v = Γ/2πr` and `ω = v/r`. If they disagreed, the storm would
    /// flatten trees at a distance that had nothing to do with how fast it
    /// looked like it was turning.
    #[test]
    fn the_wind_and_the_spin_are_the_same_flow() {
        let c = Cyclone::default();
        for r in [0.5, 1.0, 2.0, 4.0] {
            let v = c.wind_at(Cx::new(r, 0.0));
            assert!((v / r - c.spin_at(r)).abs() < 1e-12, "omega should be v/r at r = {r}");
        }
    }

    /// The reach is where the wind falls to what a thing can take — solved,
    /// not chosen. Halve the circulation and the reach halves with it.
    #[test]
    fn the_reach_is_solved_from_the_wind_not_picked() {
        // Strong enough that all these strengths sit outside the core, where
        // the formula applies.
        let c = Cyclone { circulation: 400.0, ..Cyclone::default() };
        for strength in [2.0, 6.0, 20.0] {
            let r = c.reach(strength);
            assert!((c.wind_at(Cx::new(r, 0.0)) - strength).abs() < 1e-9, "at the reach the wind should be exactly {strength}");
        }
        let half = Cyclone { circulation: c.circulation / 2.0, ..c };
        assert!((half.reach(5.0) - c.reach(5.0) / 2.0).abs() < 1e-12);
    }

    /// ★ Inside the core the wind stops rising, so something stronger than the
    /// fiercest wind the storm has is never knocked over — not knocked over
    /// very close in. The formula alone keeps handing back a smaller radius
    /// and quietly stops meaning anything.
    #[test]
    fn nothing_stronger_than_the_fiercest_wind_ever_falls() {
        let c = Cyclone { circulation: 40.0, ..Cyclone::default() };
        let most = c.strongest_wind();

        assert!(c.reach(most * 0.9) > 0.0, "it can still fell what it can out-blow");
        assert_eq!(c.reach(most * 1.1), 0.0, "and nothing it cannot");

        // And that agrees with the wind itself: nowhere on the ground is the
        // wind above `most`.
        for k in 0..200 {
            let p = Cx::polar(0.001 + 0.05 * k as f64, 0.7 * k as f64);
            assert!(c.wind_at(p) <= most + 1e-9, "found a wind above the maximum at {p:?}");
        }
    }

    /// A spent storm reaches nowhere.
    #[test]
    fn a_spent_storm_reaches_nowhere() {
        let dead = Cyclone { circulation: 0.0, ..Cyclone::default() };
        assert!(dead.reach(5.0).abs() < 1e-12);
        assert!(dead.wind_at(Cx::new(0.1, 0.0)).abs() < 1e-12);
    }
}
