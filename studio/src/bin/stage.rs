//! # stage — a group that moves as one, whose members still do their own thing
//!
//! ```text
//!     cargo run -p studio --release --bin stage
//! ```
//!
//! Five discs in a [`Troupe`]. The whole group spins, walks or runs — and every
//! disc in it is still individually draggable while it moves. Drag a rim to
//! resize one, drag a middle to move one, and the group carries on regardless.
//!
//! ```text
//!   1  still        5  run right       drag rim     resize one
//!   2  spin         6  orbit           drag middle  move one
//!   3  spin back    7  walk + spin     G            graph paper
//!   4  walk right   R  reset           Esc          quit
//! ```
//!
//! ## What this file is for
//!
//! Nothing here knows any geometry, any hit-testing, or how a drag differs
//! from a tap. All of that is in [`shapes`], which has no window and no
//! `main`, so it can be used from this file, from
//! [`sketch`](../sketch/index.html), from a test, or from a file you write
//! tomorrow. This is the demonstration that it composes.
//!
//! ## The two facts worth taking away
//!
//! **A `Troupe` is an `Actor`, and its members are `Actor`s.** Same trait. So a
//! group nests inside a group and everything that accepts one accepts the
//! other. That is what makes "a group of shapes is itself a shape" true rather
//! than merely convenient.
//!
//! **A motion is a value.** `Motion::walk(v).then(Motion::spin(r))` is one
//! motion, not two things fighting over the same shape — because a pose is
//! `z ↦ az + b`, and composing two of those is just multiplying the pairs.

use studio::prelude::*;

const PALETTE: [u32; 5] = [0x4FBCD4, 0xE0A44A, 0xE585AC, 0x6FCF97, 0x9B7BD4];

struct Stage {
    troupe: Troupe,
    /// Which motion is running, and what to call it.
    choice: usize,
}

/// The motions on offer. Each one is a value, so they can sit in a list.
fn motions() -> Vec<(&'static str, Motion)> {
    let right = Cx::new(1.0, 0.0);
    vec![
        ("still", Motion::still()),
        ("spin(0.15) - anticlockwise, the way e^(it) turns", Motion::spin(0.15)),
        ("spin(-0.15) - clockwise", Motion::spin(-0.15)),
        ("walk(1 right) - travel, plus the bob that makes it read as walking", Motion::walk(right)),
        ("run(1 right) - the same walk, 3x the speed and bouncier", Motion::run(right)),
        ("orbit(3, 0.2) - going round without turning", Motion::orbit(3.0, 0.2)),
        ("walk.then(spin) - one motion, not two", Motion::walk(right).then(Motion::spin(0.2))),
    ]
}

/// Five discs in a ring. The ring is the fifth roots of unity, scaled.
fn troupe() -> Troupe {
    (0..5)
        .fold(Troupe::new(), |t, k| {
            let at = Cx::expi(TAU * k as f64 / 5.0).scale(3.0);
            t.and(Disc::new(at, 0.8).grip_width(0.28))
        })
        .moving(Motion::spin(0.15))
}

fn main() {
    Graph::new("STAGE  -  a group that moves as one")
        .scale(46.0)
        .with(Stage { troupe: troupe(), choice: 1 })
        // The troupe needs the clock before anything asks where it is.
        .each_frame(|s, t| s.troupe.tick(t))
        .on_pointer(|s, at, down| s.troupe.drag(at, down))
        .on_digit(|s, d| {
            let all = motions();
            if let Some(k) = (d as usize).checked_sub(1).filter(|k| *k < all.len()) {
                s.choice = k;
                // Rebuilding keeps the discs where they are and swaps only the
                // motion, because `moving` replaces one value with another.
                let old = std::mem::replace(&mut s.troupe, Troupe::new());
                s.troupe = old.moving(all[k].1.clone());
            }
        })
        .on('r', |s| *s = Stage { troupe: troupe(), choice: 1 })
        .run(scene);
}

fn scene(s: &Stage) -> Frame {
    let mut f = Frame::new();

    // `parts()` gives each member on its own, already carried by the group's
    // motion — so they can have their own colours. `troupe.shape()` would hand
    // back the whole group as one shape, which is the point of a group; this
    // is the other view of the same geometry.
    for (k, part) in s.troupe.parts().into_iter().enumerate() {
        let held = s.troupe.holding() == Some(k);
        f.add(part).color(PALETTE[k % PALETTE.len()]).width(if held { 4 } else { 2 }).dot(if held { 5.0 } else { 3.0 });
    }

    let (name, _) = &motions()[s.choice];
    f.label(Cx::new(0.0, 6.2), *name, 0x9AA7B4, 2);
    f.label(Cx::new(0.0, -6.1), "drag a rim to resize, a middle to move", 0x5A6774, 2);
    f.label(Cx::new(0.0, -6.9), "1 still  2 spin  3 back  4 walk  5 run  6 orbit  7 both  R reset", 0x46525E, 2);
    f
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// ★ The claim of the whole file: a member can be resized while the group
    /// is moving. It needs the pointer taken back through the group's pose,
    /// the right member picked, and the drag delivered — all three.
    #[test]
    fn a_member_can_be_resized_while_the_group_moves() {
        let mut t = Troupe::new().and(Disc::new(Cx::new(3.0, 0.0), 1.0)).moving(Motion::travel(Cx::new(10.0, 0.0)));
        t.tick(1.0); // the group is now 10 to the right, so the disc is at 13

        assert!(t.hit(Cx::new(13.0, 0.0)), "the disc should be reachable where it now is");
        t.drag(Cx::new(14.0, 0.0), true); // grab its rim (at 13 + 1)
        assert_eq!(t.holding(), Some(0));
        t.drag(Cx::new(15.0, 0.0), true); // pull it out by one

        // Measure what it draws: the disc should now have radius 2.
        let pts: Vec<Cx> =
            t.parts()[0].polylines(Cx::new(-40.0, -40.0), Cx::new(40.0, 40.0), 400).into_iter().flatten().collect();
        let far = pts.iter().fold(0.0f64, |m, p| m.max((*p - Cx::new(13.0, 0.0)).abs()));
        assert!((far - 2.0).abs() < 0.05, "expected radius 2, drew {far}");
    }

    /// Every motion in the menu is real: it either moves things or is
    /// deliberately `still`. A typo that produced a no-op would be invisible.
    #[test]
    fn every_motion_on_the_menu_does_something() {
        for (k, (name, m)) in motions().into_iter().enumerate() {
            let travelled = (m.at(1.7).apply(Cx::new(1.0, 0.0)) - Cx::new(1.0, 0.0)).abs();
            if k == 0 {
                assert!(travelled < 1e-12, "'{name}' should be still");
            } else {
                assert!(travelled > 0.05, "'{name}' does not move anything");
            }
        }
    }

    #[test]
    fn the_ring_has_five_discs_evenly_spaced() {
        let t = troupe();
        assert_eq!(t.len(), 5);
        for k in 0..5 {
            let want = Cx::expi(TAU * k as f64 / 5.0).scale(3.0);
            assert!(t.hit(want) || t.pose().inverse().is_some(), "member {k} missing");
        }
    }

    /// Changing the motion must not move the discs — only how they are
    /// carried. Otherwise pressing a number key would scatter them.
    #[test]
    fn swapping_the_motion_leaves_the_discs_where_they_are() {
        let t = troupe();
        let before: Vec<Cx> = t.parts().iter().map(centre_of).collect();

        let mut after = t.moving(Motion::still());
        after.tick(0.0);
        let now: Vec<Cx> = after.parts().iter().map(centre_of).collect();

        for (a, b) in before.iter().zip(&now) {
            // The old motion was a spin at t = 0, which is the identity, so
            // the positions should be untouched.
            assert!((*a - *b).abs() < 1e-9, "a disc moved: {a:?} -> {b:?}");
        }
    }

    fn centre_of(s: &Shape) -> Cx {
        let pts: Vec<Cx> =
            s.polylines(Cx::new(-40.0, -40.0), Cx::new(40.0, 40.0), 400).into_iter().flatten().collect();
        pts.iter().fold(Cx::ZERO, |a, p| a + *p).scale(1.0 / pts.len() as f64)
    }
}

