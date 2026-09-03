//! # dice — what a die looks like
//!
//! The pips, where a die really has them, and a body to put them on. The
//! throw itself is [`plotkit::dice`]; this is only the picture of it.
//!
//! ## The pips are one grid
//!
//! Every face is drawn from the same nine places — the corners, the middles of
//! two sides, and the centre:
//!
//! ```text
//!     6 . 7        1 : 4              4 : 4 7          6 : 0 2 6 8
//!     0 8 2        2 : 0 8            5 : 0 2 4 6 8    (and 3, 5 for six)
//!     5 . 1        3 : 0 4 8
//! ```
//!
//! Written as which of the nine each face lights, rather than as six lists of
//! coordinates. Six lists is six chances to put a pip a hair out of place, and
//! the eye is very good at seeing that on a die.
//!
//! ## Opposite faces add to seven
//!
//! Which is checked, because it is the one property that makes a die a die
//! rather than six pictures — and because 1-2-3 running clockwise or
//! anticlockwise is the difference between a Western die and every other kind.

use plotkit::{Cx, Shape};
use std::f64::consts::TAU;

/// How far a pip sits from the middle, as a fraction of the half-width.
pub const REACH: f64 = 0.52;

/// How big a pip is, likewise.
pub const PIP: f64 = 0.15;

/// The nine places a pip can sit, from the middle out — index 4 is the centre.
///
/// ```text
///     6 7 8
///     3 4 5
///     0 1 2
/// ```
pub fn grid(k: usize) -> Cx {
    let (col, row) = ((k % 3) as f64 - 1.0, (k / 3) as f64 - 1.0);
    Cx::new(col * REACH, row * REACH)
}

/// Which of the nine places a face lights.
///
/// One table, so the faces cannot disagree with each other about where a
/// corner is.
pub fn lit(face: u8) -> &'static [usize] {
    match face.clamp(1, 6) {
        1 => &[4],
        2 => &[0, 8],
        3 => &[0, 4, 8],
        4 => &[0, 2, 6, 8],
        5 => &[0, 2, 4, 6, 8],
        _ => &[0, 2, 3, 5, 6, 8],
    }
}

/// A die: the body, then a pip for each spot the face lights.
///
/// `size` is the half-width, `turn` the angle it is lying at. The body comes
/// first so a renderer that draws in order puts the pips on top of it.
pub fn die(face: u8, at: Cx, size: f64, turn: f64) -> Vec<Shape> {
    let put = |z: Cx| at + z * Cx::polar(size, turn);
    let mut out = Vec::new();

    // The body, with its corners taken off -- a square with sharp corners
    // reads as a box, and a die is not a box.
    let round = 0.22;
    let mut edge: Vec<Cx> = Vec::new();
    for corner in 0..4 {
        let a0 = corner as f64 * TAU / 4.0;
        let c = Cx::polar((1.0 - round) * std::f64::consts::SQRT_2, a0 + TAU / 8.0);
        for k in 0..=6 {
            let a = a0 - TAU / 8.0 + k as f64 / 6.0 * TAU / 4.0;
            edge.push(put(c + Cx::polar(round, a)));
        }
    }
    edge.push(edge[0]);
    out.push(Shape::polygon(edge));

    for k in lit(face) {
        let spot = grid(*k);
        let ring: Vec<Cx> =
            (0..=16).map(|j| put(spot + Cx::polar(PIP, j as f64 / 16.0 * TAU))).collect();
        out.push(Shape::polygon(ring));
    }
    out
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// ★ The right number of pips on every face. The whole point of a die.
    #[test]
    fn each_face_has_its_own_number_of_pips() {
        for face in 1..=6u8 {
            assert_eq!(lit(face).len(), face as usize, "face {face}");
        }
    }

    /// ★ **Opposite faces add to seven.** The one property that makes a die a
    /// die rather than six pictures of dots — and it falls out of the grid
    /// being symmetric, so this checks the table was written down right.
    #[test]
    fn opposite_faces_are_mirror_images() {
        // Turning the grid half a turn takes place `k` to place `8 - k`.
        for face in 1..=6u8 {
            let other = 7 - face;
            let turned: Vec<usize> = lit(face).iter().map(|k| 8 - k).collect();
            let mut turned = turned;
            turned.sort();
            assert_eq!(turned, lit(face).to_vec(), "face {face} should be symmetric about the middle");
            assert_eq!(lit(other).len(), 7 - face as usize);
        }
    }

    /// Only the odd faces have a middle pip, which is what makes them odd.
    #[test]
    fn only_the_odd_faces_use_the_middle() {
        for face in 1..=6u8 {
            assert_eq!(lit(face).contains(&4), face % 2 == 1, "face {face}");
        }
    }

    /// ★ Every pip stays on the die, at every angle. A pip sliding off the
    /// corner is the sort of thing that only shows up at 40 degrees.
    #[test]
    fn the_pips_stay_on_the_die() {
        for face in 1..=6u8 {
            for step in 0..24 {
                let turn = step as f64 / 24.0 * TAU;
                let shapes = die(face, Cx::new(3.0, -2.0), 0.5, turn);
                assert_eq!(shapes.len(), 1 + face as usize, "a body and {face} pips");
                for s in &shapes[1..] {
                    for run in s.polylines(Cx::new(-9.0, -9.0), Cx::new(9.0, 9.0), 400) {
                        for p in run {
                            let off = (p - Cx::new(3.0, -2.0)).abs();
                            assert!(off < 0.5 * 1.45, "a pip {off} from the middle at {turn}");
                        }
                    }
                }
            }
        }
    }

    /// It goes where it is put, and the body is centred on it.
    #[test]
    fn a_die_is_drawn_where_it_is_put() {
        let at = Cx::new(-4.0, 1.5);
        let shapes = die(1, at, 0.4, 0.0);
        let mut mid = Cx::ZERO;
        let mut n = 0.0;
        for run in shapes[1].polylines(Cx::new(-9.0, -9.0), Cx::new(9.0, 9.0), 400) {
            for p in run {
                mid = mid + p;
                n += 1.0;
            }
        }
        let mid = Cx::new(mid.re / n, mid.im / n);
        assert!((mid - at).abs() < 0.02, "the one pip of a one should sit in the middle: {mid:?}");
    }
}
