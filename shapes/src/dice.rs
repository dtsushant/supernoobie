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

/// A die: the face you are looking at, the face rolling into view beside it,
/// and a pip for each spot they light.
///
/// ## Why two faces and not one squashed one
///
/// One face, foreshortened, is a **plank**: full height, narrowing width, and
/// nothing anywhere says the thing has a third dimension. A cube going over its
/// edge shows two faces at once — the one you were looking at closing like a
/// door, and the next one opening — and their projected widths are `cos φ` and
/// `sin φ`, so together they run 1 → √2 → 1 as it goes over. The silhouette
/// **widens in the middle of a roll**, which is exactly what a cube does and
/// exactly what a flat plate does not.
///
/// That is the whole difference between something rolling and something being
/// squashed, and it costs one extra quad.
///
/// `squash` is the width of the near face, 1 flat on and 0 edge-on; `next` is
/// the face rolling in behind it.
pub fn die(face: u8, next: u8, at: Cx, size: f64, turn: f64, squash: f64) -> Vec<Shape> {
    let near = squash.clamp(0.0, 1.0);
    // The two widths are the cosine and sine of one angle, so they cannot drift
    // apart: whatever the near face gives up, the far one takes.
    let far = (1.0 - near * near).max(0.0).sqrt();
    // The pair runs from -(near+far) to +(near+far), so the far face is centred
    // on -near and the near face on +far. Symmetric, and it falls out in one
    // line each -- which is the sign the widths were the right two numbers.
    let lean = Cx::polar(size, turn);
    let put = |z: Cx| at + z * lean;

    let mut out = Vec::new();
    // The far face first, so the near one is drawn over the edge between them.
    if far > 0.01 {
        out.extend(panel(next, -near, far, &put));
    }
    if near > 0.01 {
        out.extend(panel(face, far, near, &put));
    }
    out
}

/// One face of the die: a rounded panel `w` wide, centred `x` from the middle,
/// with its pips squeezed into it.
fn panel(face: u8, x: f64, w: f64, put: &impl Fn(Cx) -> Cx) -> Vec<Shape> {
    let squeeze = |z: Cx| put(Cx::new(x + z.re * w, z.im));
    let mut out = Vec::new();

    let round = 0.22;
    let mut edge: Vec<Cx> = Vec::new();
    for corner in 0..4 {
        let a0 = corner as f64 * TAU / 4.0;
        let c = Cx::polar((1.0 - round) * std::f64::consts::SQRT_2, a0 + TAU / 8.0);
        for k in 0..=6 {
            let a = a0 - TAU / 8.0 + k as f64 / 6.0 * TAU / 4.0;
            edge.push(squeeze(c + Cx::polar(round, a)));
        }
    }
    edge.push(edge[0]);
    out.push(Shape::polygon(edge));

    for k in lit(face) {
        let spot = grid(*k);
        let ring: Vec<Cx> =
            (0..=16).map(|j| squeeze(spot + Cx::polar(PIP, j as f64 / 16.0 * TAU))).collect();
        out.push(Shape::polygon(ring));
    }
    out
}

/// How many shapes [`die`] draws for a given pair of faces, so a caller can
/// tell the body of one panel from its pips without counting by hand.
pub fn parts(face: u8, next: u8, squash: f64) -> (usize, usize) {
    let near = squash.clamp(0.0, 1.0);
    let far = (1.0 - near * near).max(0.0).sqrt();
    let a = if far > 0.01 { 1 + lit(next).len() } else { 0 };
    let b = if near > 0.01 { 1 + lit(face).len() } else { 0 };
    (a, b)
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
                let shapes = die(face, 1, Cx::new(3.0, -2.0), 0.5, turn, 1.0);
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

    /// ★ **The silhouette widens in the middle of a roll.** This is the whole
    /// difference between a cube going over its edge and a plate being
    /// squashed, and it is why there are two panels and not one.
    ///
    /// The two projected widths are `cos φ` and `sin φ`, so together they run
    /// 1 → √2 → 1. A single foreshortened face runs 1 → 0, which is a plank.
    #[test]
    fn a_rolling_die_is_widest_half_way_over() {
        let wide = |sq: f64| {
            let pts: Vec<Cx> = die(6, 3, Cx::ZERO, 1.0, 0.0, sq)
                .iter()
                .flat_map(|s| s.polylines(Cx::new(-9.0, -9.0), Cx::new(9.0, 9.0), 400))
                .flatten()
                .collect();
            pts.iter().map(|p| p.re).fold(f64::MIN, f64::max)
                - pts.iter().map(|p| p.re).fold(f64::MAX, f64::min)
        };
        let flat = wide(1.0);
        let half = wide(std::f64::consts::FRAC_1_SQRT_2);
        let ratio = half / flat;
        assert!(
            (ratio - std::f64::consts::SQRT_2).abs() < 0.12,
            "half way over should be about root two as wide, not {ratio:.2}"
        );
        // And it comes back: edge-on, one face has closed and the other is open.
        assert!((wide(0.02) - flat).abs() < flat * 0.12, "and square again at the end of the roll");
    }

    /// ★ Both faces are on show while it rolls, and only one when it is not.
    /// A die that showed two faces lying still would be two cards side by side.
    #[test]
    fn two_faces_while_rolling_and_one_at_rest() {
        assert_eq!(die(6, 3, Cx::ZERO, 1.0, 0.0, 1.0).len(), 1 + 6, "flat on: one face");
        let rolling = die(6, 3, Cx::ZERO, 1.0, 0.0, 0.7);
        assert_eq!(rolling.len(), (1 + 6) + (1 + 3), "mid-roll: both, with their pips");
        let (far, _) = parts(6, 3, 0.7);
        assert_eq!(far, 1 + 3, "the far one is drawn first, so it can be shaded");
    }

    /// The near face narrows as it goes over, even though the pair does not.
    #[test]
    fn the_near_face_closes_like_a_door() {
        let near = |sq: f64| {
            let all = die(6, 3, Cx::ZERO, 1.0, 0.0, sq);
            let (far, _) = parts(6, 3, sq);
            let pts: Vec<Cx> = all[far]
                .polylines(Cx::new(-9.0, -9.0), Cx::new(9.0, 9.0), 400)
                .into_iter()
                .flatten()
                .collect();
            pts.iter().map(|p| p.re).fold(f64::MIN, f64::max)
                - pts.iter().map(|p| p.re).fold(f64::MAX, f64::min)
        };
        assert!(near(0.5) < near(1.0) * 0.6, "half way over it is about half as wide");
    }

    /// It goes where it is put, and the body is centred on it.
    #[test]
    fn a_die_is_drawn_where_it_is_put() {
        let at = Cx::new(-4.0, 1.5);
        let shapes = die(1, 2, at, 0.4, 0.0, 1.0);
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
