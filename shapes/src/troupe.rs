//! # troupe — a group that is itself one of the things it groups
//!
//! An [`Actor`] is anything that can be drawn, hit, and taken hold of. A
//! [`Troupe`] is a bag of actors — **and is an `Actor` itself**, so a group can
//! go inside a group, and anything that accepts one accepts the other.
//!
//! That single fact is what "treat a group as one shape while its members keep
//! their own behaviour" means. Drag a member and only that member moves. Drag
//! the group's own body and the whole thing moves, members and all. Give the
//! group a [`Motion`] and everything inside it travels together while each
//! member is still individually grabbable.
//!
//! ```no_run
//! # use shapes::{troupe::Troupe, grab::Disc, motion::Motion};
//! # use plotkit::Cx;
//! let mut t = Troupe::new()
//!     .and(Disc::new(Cx::new(-2.0, 0.0), 1.0))
//!     .and(Disc::new(Cx::new(2.0, 0.0), 0.7))
//!     .moving(Motion::spin(0.1));
//! ```
//!
//! ## The two rules that make it work
//!
//! **1. What you have hold of is decided on the press.** The same rule as a
//! single shape, now applied to choosing *which member*. Re-deciding every
//! frame would hand you a different actor the moment your drag crossed one.
//!
//! **2. A moving group hit-tests through the inverse of its pose.** The
//! members do not know they are being carried, so the pointer is taken back
//! through the motion before they are asked about it. Without this, everything
//! inside a spinning group would be grabbable only where it *used* to be.

use crate::grab::Disc;
use crate::motion::{Motion, Pose};
use plotkit::{Cx, Shape};

/// Something that can be drawn, hit, and dragged.
pub trait Actor {
    /// What to draw.
    fn shape(&self) -> Shape;

    /// Is `p` on it? In the actor's own coordinates.
    fn hit(&self, p: Cx) -> bool;

    /// The pointer, once a frame, in the actor's own coordinates.
    fn drag(&mut self, at: Cx, down: bool);

    /// Move it by `by`, without any dragging involved.
    fn nudge(&mut self, by: Cx);

    /// Released without travelling — a click rather than a drag.
    fn tapped(&self) -> bool {
        false
    }
}

impl Actor for Disc {
    fn shape(&self) -> Shape {
        Shape::group(vec![Disc::shape(self), self.handles()])
    }
    fn hit(&self, p: Cx) -> bool {
        // A little past the rim, so the resize grip is reachable from outside.
        (p - self.centre).abs() <= self.radius + self.grip_width
    }
    fn drag(&mut self, at: Cx, down: bool) {
        Disc::drag(self, at, down);
    }
    fn nudge(&mut self, by: Cx) {
        self.centre = self.centre + by;
    }
    fn tapped(&self) -> bool {
        Disc::tapped(self)
    }
}

/// A group of actors, which is itself an actor.
pub struct Troupe {
    members: Vec<Box<dyn Actor>>,
    /// How the whole group moves. Members are carried by it.
    motion: Motion,
    /// The clock the motion is read at. [`Troupe::tick`] sets it.
    t: f64,

    /// Which member is being dragged — chosen on the press, held until
    /// release.
    holding: Option<usize>,
    was_down: bool,
}

impl Default for Troupe {
    fn default() -> Troupe {
        Troupe::new()
    }
}

impl Troupe {
    pub fn new() -> Troupe {
        Troupe {
            members: Vec::new(),
            motion: Motion::still(),
            t: 0.0,
            holding: None,
            was_down: false,
        }
    }

    /// Add a member. Chains, so a troupe reads as a list of who is in it.
    pub fn and(mut self, a: impl Actor + 'static) -> Troupe {
        self.members.push(Box::new(a));
        self
    }

    /// How the whole group moves.
    pub fn moving(mut self, m: Motion) -> Troupe {
        self.motion = m;
        self
    }

    /// Set the clock. Call once a frame, before drawing or dragging.
    pub fn tick(&mut self, t: f64) {
        self.t = t;
    }

    /// Where the group is right now.
    pub fn pose(&self) -> Pose {
        self.motion.at(self.t)
    }

    pub fn len(&self) -> usize {
        self.members.len()
    }
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// Which member, if any, is being held.
    pub fn holding(&self) -> Option<usize> {
        self.holding
    }

    pub fn member(&self, k: usize) -> Option<&dyn Actor> {
        self.members.get(k).map(|m| m.as_ref())
    }

    /// Each member's shape on its own, already carried by the group's motion.
    ///
    /// [`Actor::shape`] hands back the group as one shape, which is the point
    /// of a group. This is for when the application wants to give members
    /// their own colours — the geometry is identical either way.
    pub fn parts(&self) -> Vec<Shape> {
        let pose = self.pose();
        self.members.iter().map(|m| pose.shape(m.shape())).collect()
    }

    /// Take the pointer back through the group's motion, so members can be
    /// asked about it in the coordinates they think in.
    fn local(&self, at: Cx) -> Cx {
        self.pose().inverse().map_or(at, |inv| inv.apply(at))
    }

    /// The member under `p`, latest-added first — so something drawn on top
    /// is grabbed first, which is what "on top" means.
    fn topmost(&self, p: Cx) -> Option<usize> {
        (0..self.members.len()).rev().find(|&k| self.members[k].hit(p))
    }
}

impl Actor for Troupe {
    fn shape(&self) -> Shape {
        let inner = Shape::group(self.members.iter().map(|m| m.shape()).collect::<Vec<_>>());
        self.pose().shape(inner)
    }

    fn hit(&self, p: Cx) -> bool {
        let local = self.local(p);
        self.members.iter().any(|m| m.hit(local))
    }

    fn drag(&mut self, at: Cx, down: bool) {
        let local = self.local(at);

        if down && !self.was_down {
            // The press. Decide once who is being held.
            self.holding = self.topmost(local);
        } else if !down {
            self.holding = None;
        }
        self.was_down = down;

        // Every member is told about the pointer every frame, because a member
        // needs to see the release to know its own drag has ended — even the
        // ones that were never picked up.
        for (k, m) in self.members.iter_mut().enumerate() {
            let mine = self.holding == Some(k);
            m.drag(local, down && mine);
        }
    }

    fn nudge(&mut self, by: Cx) {
        for m in &mut self.members {
            m.nudge(by);
        }
    }

    fn tapped(&self) -> bool {
        self.members.iter().any(|m| m.tapped())
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn two() -> Troupe {
        Troupe::new()
            .and(Disc::new(Cx::new(-3.0, 0.0), 1.0).grip_width(0.2))
            .and(Disc::new(Cx::new(3.0, 0.0), 1.0).grip_width(0.2))
    }

    fn radius(t: &Troupe, k: usize) -> f64 {
        // Read a member's radius back out through the trait by measuring what
        // it draws, so the test goes through the same path the screen does.
        let pts: Vec<Cx> = t.member(k).expect("member").shape().polylines(Cx::new(-40.0, -40.0), Cx::new(40.0, 40.0), 400).into_iter().flatten().collect();
        let mid = pts.iter().fold(Cx::ZERO, |a, p| a + *p).scale(1.0 / pts.len() as f64);
        pts.iter().fold(0.0f64, |m, p| m.max((*p - mid).abs()))
    }

    /// ★ Dragging one member moves only that member. If the group forwarded
    /// the drag to everybody, a troupe would be a rigid block and there would
    /// be no point having members at all.
    #[test]
    fn dragging_one_member_leaves_the_others_alone() {
        let mut t = two();
        let before = radius(&t, 1);

        t.drag(Cx::new(-3.0, 0.0), true); // the middle of member 0
        t.drag(Cx::new(-1.0, 2.0), true);
        assert_eq!(t.holding(), Some(0));
        assert!((radius(&t, 1) - before).abs() < 1e-9, "member 1 should not have changed");
    }

    /// The topmost member wins where two overlap, because that is what being
    /// drawn on top means.
    #[test]
    fn the_topmost_member_is_the_one_you_grab() {
        let mut t = Troupe::new()
            .and(Disc::new(Cx::ZERO, 2.0))
            .and(Disc::new(Cx::ZERO, 2.0)); // exactly on top
        t.drag(Cx::ZERO, true);
        assert_eq!(t.holding(), Some(1), "the later one is on top");
    }

    /// ★ A press that lands on nobody holds nobody, and stays that way while
    /// the button is down — the same decide-once rule as a single shape.
    #[test]
    fn a_press_on_empty_space_grabs_nothing_and_stays_that_way() {
        let mut t = two();
        t.drag(Cx::new(0.0, 9.0), true);
        assert_eq!(t.holding(), None);
        t.drag(Cx::new(-3.0, 0.0), true); // now over a member, still down
        assert_eq!(t.holding(), None, "the button never came up, so this is not a new press");
    }

    /// ★ A moving group still hit-tests correctly, because the pointer is
    /// taken back through the pose first. Without the inverse, everything in a
    /// moving group would be grabbable only where it started.
    #[test]
    fn a_moving_group_is_grabbable_where_it_actually_is() {
        let mut t = two().moving(Motion::travel(Cx::new(10.0, 0.0)));
        t.tick(1.0); // the whole group is now 10 to the right

        // Where member 0 used to be: no longer anything there.
        assert!(!t.hit(Cx::new(-3.0, 0.0)));
        // Where it is now: yes.
        assert!(t.hit(Cx::new(7.0, 0.0)));

        t.drag(Cx::new(7.0, 0.0), true);
        assert_eq!(t.holding(), Some(0), "should have grabbed the member under the pointer");
    }

    /// The same, for a group that is turning rather than travelling — the
    /// inverse has to undo a rotation, not just a shift.
    #[test]
    fn a_spinning_group_is_grabbable_where_it_actually_is() {
        let mut t = two().moving(Motion::spin(1.0));
        t.tick(0.25); // a quarter turn: (-3, 0) has gone to (0, -3)

        assert!(!t.hit(Cx::new(-3.0, 0.0)));
        assert!(t.hit(Cx::new(0.0, -3.0)), "member 0 should be at the bottom now");
        t.drag(Cx::new(0.0, -3.0), true);
        assert_eq!(t.holding(), Some(0));
    }

    /// ★ A troupe is an actor, so a troupe goes inside a troupe. This is the
    /// whole claim of the module, and if it failed the type would not compose.
    #[test]
    fn a_group_can_hold_a_group() {
        let inner = two();
        let mut outer = Troupe::new().and(inner).and(Disc::new(Cx::new(0.0, 8.0), 1.0));
        assert_eq!(outer.len(), 2);

        // Reaching a member of the inner group, through both levels.
        assert!(outer.hit(Cx::new(-3.0, 0.0)));
        outer.drag(Cx::new(-3.0, 0.0), true);
        assert_eq!(outer.holding(), Some(0), "the inner troupe is what was grabbed");
    }

    /// A nested group carries its own motion as well as its parent's, and the
    /// two compose rather than one overriding the other.
    #[test]
    fn motions_stack_through_nesting() {
        let inner = Troupe::new().and(Disc::new(Cx::ZERO, 1.0)).moving(Motion::travel(Cx::new(0.0, 5.0)));
        let mut outer = Troupe::new().and(inner).moving(Motion::travel(Cx::new(5.0, 0.0)));
        outer.tick(1.0);
        // The inner troupe's own tick is never called by the parent, so it
        // stays at t = 0 and contributes nothing — which is worth knowing:
        // a nested troupe needs ticking too if it is to move.
        assert!(outer.hit(Cx::new(5.0, 0.0)), "the outer motion carried it right");
        assert!(!outer.hit(Cx::new(5.0, 5.0)), "the inner one has not been ticked");
    }

    /// Nudging moves everybody, which is what makes the group usable as one
    /// thing.
    #[test]
    fn nudging_moves_the_whole_group() {
        let mut t = two();
        t.nudge(Cx::new(0.0, 4.0));
        assert!(t.hit(Cx::new(-3.0, 4.0)));
        assert!(t.hit(Cx::new(3.0, 4.0)));
        assert!(!t.hit(Cx::new(-3.0, 0.0)));
    }

    /// A tap on a member is a tap on the group, so a group can be wired to a
    /// one-off without unpacking it.
    #[test]
    fn a_tap_on_a_member_reaches_the_group() {
        let mut t = two();
        t.drag(Cx::new(-3.0, 0.0), true);
        assert!(!t.tapped());
        t.drag(Cx::new(-3.0, 0.0), false);
        assert!(t.tapped(), "pressed and released without travelling");
    }

    #[test]
    fn an_empty_troupe_is_harmless() {
        let mut t = Troupe::new();
        assert!(t.is_empty() && !t.hit(Cx::ZERO) && !t.tapped());
        t.drag(Cx::ZERO, true);
        t.tick(3.0);
        assert_eq!(t.holding(), None);
    }
}
