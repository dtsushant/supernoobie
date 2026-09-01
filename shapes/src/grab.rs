//! # grab — shapes you can take hold of
//!
//! A [`Disc`] is a circle with its own behaviour attached: drag the **rim** to
//! resize it, drag the **inside** to move it, and a click that does not move is
//! a tap rather than a drag.
//!
//! It needs no window and no view — only where the pointer is and whether the
//! button is down — so it lives here with the geometry rather than up in the
//! application, and can be tested without a screen.
//!
//! ```no_run
//! # use shapes::grab::Disc;
//! # use plotkit::Cx;
//! # let (pointer, button_down) = (Cx::ZERO, false);
//! let mut d = Disc::new(Cx::ZERO, 2.0);
//! d.drag(pointer, button_down);      // once a frame
//! if d.tapped() { /* it was a click, not a drag */ }
//! ```
//!
//! ## The one thing that matters
//!
//! **What you have hold of is decided once, when the button goes down.** Not
//! re-decided every frame. Drag the rim quickly through the middle and a
//! frame-by-frame test would hand you the *inside* halfway through, and the
//! circle would stop resizing and start following the mouse. Deciding on the
//! press and holding it until release is what makes a drag a drag.

use crate::recipe::Recipe;
use plotkit::{Cx, Shape};

/// Which part of the disc is being held.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Grip {
    /// The rim: dragging changes the radius.
    Rim,
    /// The inside: dragging moves it. Carries the offset from the centre to
    /// where it was picked up, so it does not jump to the cursor.
    Inside(Cx),
}

/// A circle you can take hold of.
#[derive(Clone, Debug)]
pub struct Disc {
    pub centre: Cx,
    pub radius: f64,
    /// How near the rim counts as grabbing the rim, in world units.
    pub grip_width: f64,
    /// The smallest it can be dragged to, so it can never vanish and become
    /// impossible to pick up again.
    pub min_radius: f64,

    grip: Option<Grip>,
    was_down: bool,
    /// Where the button went down, so a tap can be told from a drag.
    press_at: Cx,
    moved: bool,
    tapped: bool,
}

impl Disc {
    pub fn new(centre: Cx, radius: f64) -> Disc {
        Disc {
            centre,
            radius,
            grip_width: 0.3,
            min_radius: 0.25,
            grip: None,
            was_down: false,
            press_at: Cx::ZERO,
            moved: false,
            tapped: false,
        }
    }

    /// How wide the rim's grab zone is, in world units.
    ///
    /// A fixed world width, so it stays honest to the geometry. A sketch that
    /// zooms a long way should scale this, or the rim becomes impossible to
    /// hit at one end and covers the whole disc at the other.
    pub fn grip_width(mut self, w: f64) -> Disc {
        self.grip_width = w;
        self
    }

    pub fn min_radius(mut self, r: f64) -> Disc {
        self.min_radius = r;
        self
    }

    /// Feed it the pointer once a frame.
    ///
    /// `at` is in world coordinates — the same numbers the disc is written in.
    pub fn drag(&mut self, at: Cx, down: bool) {
        self.tapped = false;

        if down && !self.was_down {
            // The press. Decide now what is being held, and only now.
            self.grip = self.pick(at);
            self.press_at = at;
            self.moved = false;
        } else if !down {
            // The release. A grip let go without travelling was a tap.
            if self.grip.is_some() && !self.moved {
                self.tapped = true;
            }
            self.grip = None;
        }
        self.was_down = down;

        if self.grip.is_some() && (at - self.press_at).abs() > self.grip_width * 0.5 {
            self.moved = true;
        }

        match self.grip {
            // The radius IS the distance to the pointer. Nothing else to work out.
            Some(Grip::Rim) => self.radius = (at - self.centre).abs().max(self.min_radius),
            // Keep the offset, so the disc does not jump its centre to the cursor.
            Some(Grip::Inside(offset)) => self.centre = at - offset,
            None => {}
        }
    }

    /// What `at` would take hold of, if the button went down there.
    fn pick(&self, at: Cx) -> Option<Grip> {
        let d = (at - self.centre).abs();
        if (d - self.radius).abs() <= self.grip_width {
            Some(Grip::Rim)
        } else if d < self.radius {
            Some(Grip::Inside(at - self.centre))
        } else {
            None
        }
    }

    /// Released without having moved — a click rather than a drag.
    ///
    /// True for exactly one frame, so it can drive a one-off like changing
    /// colour without also firing all the way through a resize.
    pub fn tapped(&self) -> bool {
        self.tapped
    }

    /// Being held at all.
    pub fn held(&self) -> bool {
        self.grip.is_some()
    }

    /// Being resized, as opposed to moved.
    pub fn resizing(&self) -> bool {
        self.grip == Some(Grip::Rim)
    }

    /// Is `p` inside? `|p - c| <= r`, which is what a disc *is*.
    pub fn contains(&self, p: Cx) -> bool {
        (p - self.centre).abs() <= self.radius
    }

    /// The circle itself.
    pub fn shape(&self) -> Shape {
        Shape::circle(self.centre, self.radius)
    }

    /// The places you can take hold of: the middle, and the rim at the four
    /// quarter turns — which are `r`, `ir`, `-r` and `-ir` from the centre.
    pub fn handles(&self) -> Shape {
        let mut pts = vec![self.centre];
        pts.extend((0..4).map(|k| self.centre + Cx::expi(std::f64::consts::FRAC_PI_2 * k as f64).scale(self.radius)));
        Shape::points(pts)
    }

    pub fn recipe(&self) -> Recipe {
        Recipe::new("disc", "|z - c| = r to draw it, |z - c| <= r to know if you hit it")
            .step("the circle: every point at distance r from the middle", self.shape())
            .step("the grips: the middle, and the rim at 1, i, -1, -i times r", self.handles())
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn disc() -> Disc {
        Disc::new(Cx::ZERO, 2.0).grip_width(0.3)
    }

    /// Grabbing the rim resizes and does not move.
    #[test]
    fn dragging_the_rim_changes_the_radius() {
        let mut d = disc();
        d.drag(Cx::new(2.0, 0.0), true); // on the rim
        assert!(d.resizing());
        d.drag(Cx::new(3.5, 0.0), true);
        assert!((d.radius - 3.5).abs() < 1e-12);
        assert_eq!(d.centre, Cx::ZERO, "resizing must not move it");
    }

    /// Grabbing the inside moves and does not resize.
    #[test]
    fn dragging_the_inside_moves_it() {
        let mut d = disc();
        d.drag(Cx::new(0.5, 0.0), true); // well inside
        assert!(d.held() && !d.resizing());
        d.drag(Cx::new(4.5, 1.0), true);
        assert_eq!(d.centre, Cx::new(4.0, 1.0), "it should keep the offset it was picked up by");
        assert!((d.radius - 2.0).abs() < 1e-12, "moving must not resize");
    }

    /// ★ What you have hold of is decided on the press and kept until release.
    ///
    /// Drag the rim in through the middle and out the far side. A test that
    /// re-decided every frame would hand over the *inside* on the way past and
    /// the disc would stop resizing and start following the cursor.
    #[test]
    fn a_grip_survives_being_dragged_through_the_middle() {
        let mut d = disc();
        d.drag(Cx::new(2.0, 0.0), true);
        assert!(d.resizing());
        for x in [1.5, 1.0, 0.4, 0.0, -0.6, -1.5, -3.0] {
            d.drag(Cx::new(x, 0.0), true);
            assert!(d.resizing(), "lost the rim at x = {x}");
            assert_eq!(d.centre, Cx::ZERO, "it started following the cursor at x = {x}");
        }
        assert!((d.radius - 3.0).abs() < 1e-12);
    }

    /// A press that misses does not become a grab later just because the
    /// button is still down and the pointer wandered over the shape.
    #[test]
    fn a_press_that_misses_stays_missed() {
        let mut d = disc();
        d.drag(Cx::new(9.0, 9.0), true); // nowhere near
        assert!(!d.held());
        d.drag(Cx::ZERO, true); // now over the middle, still holding
        assert!(!d.held(), "the button never came up, so this is not a new press");
        assert_eq!(d.centre, Cx::ZERO);
        assert!((d.radius - 2.0).abs() < 1e-12);
    }

    #[test]
    fn releasing_lets_go() {
        let mut d = disc();
        d.drag(Cx::new(2.0, 0.0), true);
        assert!(d.held());
        d.drag(Cx::new(2.0, 0.0), false);
        assert!(!d.held());
        // Moving with the button up changes nothing.
        d.drag(Cx::new(8.0, 0.0), false);
        assert!((d.radius - 2.0).abs() < 1e-12);
    }

    /// ★ A tap is a press and release that did not travel. Without this, a
    /// click meant to change the colour would also be a zero-length resize,
    /// and a real resize would fire the colour change as well.
    #[test]
    fn a_tap_is_a_click_that_did_not_travel() {
        let mut d = disc();
        d.drag(Cx::ZERO, true);
        assert!(!d.tapped(), "not yet — the button is still down");
        d.drag(Cx::ZERO, false);
        assert!(d.tapped());

        // And it lasts exactly one frame.
        d.drag(Cx::ZERO, false);
        assert!(!d.tapped());
    }

    #[test]
    fn a_drag_is_not_a_tap() {
        let mut d = disc();
        d.drag(Cx::new(2.0, 0.0), true);
        d.drag(Cx::new(3.4, 0.0), true);
        d.drag(Cx::new(3.4, 0.0), false);
        assert!(!d.tapped(), "that travelled, so it was a drag");
    }

    #[test]
    fn a_miss_is_not_a_tap() {
        let mut d = disc();
        d.drag(Cx::new(9.0, 9.0), true);
        d.drag(Cx::new(9.0, 9.0), false);
        assert!(!d.tapped());
    }

    /// It cannot be shrunk away to nothing, because nothing cannot be grabbed
    /// to make it bigger again.
    #[test]
    fn it_cannot_be_shrunk_out_of_existence() {
        let mut d = disc().min_radius(0.25);
        d.drag(Cx::new(2.0, 0.0), true);
        d.drag(Cx::ZERO, true);
        assert!(d.radius >= 0.25);
        // and it is still grabbable
        d.drag(Cx::ZERO, false);
        d.drag(Cx::new(0.25, 0.0), true);
        assert!(d.held());
    }

    /// The rim wins over the inside where the two overlap, so a big disc can
    /// still be resized rather than only moved.
    #[test]
    fn the_rim_wins_where_it_overlaps_the_inside() {
        let mut d = disc(); // radius 2, grip 0.3
        d.drag(Cx::new(1.85, 0.0), true); // inside, but within the grip of the rim
        assert!(d.resizing());
    }

    #[test]
    fn contains_is_the_definition_of_a_disc() {
        let d = Disc::new(Cx::new(1.0, 1.0), 2.0);
        assert!(d.contains(Cx::new(1.0, 1.0)));
        assert!(d.contains(Cx::new(3.0, 1.0)), "the rim counts");
        assert!(!d.contains(Cx::new(3.2, 1.0)));
    }
}
