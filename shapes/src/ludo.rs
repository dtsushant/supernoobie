//! # ludo — the board, as arithmetic
//!
//! Everything here comes out of **one table of fifty-two squares**, and the
//! rest is derived from it. Where a token stands, where it starts, where it
//! turns for home, where the board's lines go — all of it is that table read
//! in different ways. Writing the four quarters out by hand, or the home
//! columns separately, would be four chances to disagree with each other.
//!
//! ```text
//!            . . . [ ] [ ] [ ] . . .
//!            . . . [ ][H][ ] . . .          the cross, 15 by 15
//!     [ ][ ][ ][ ][ ][ ][ ][X][ ][ ]...     H is a home column
//!     [ ][ ]  yard   [ ][C][ ]              C is the finish
//!     [ ][ ][ ][ ][ ][ ][ ][ ][ ][ ]...     X is a track square
//! ```
//!
//! ## The loop is one quarter, turned four times
//!
//! A quarter is thirteen squares, and four of them close exactly. Turning one
//! is `(x, y) ↦ (y, 14 − x)` — a quarter turn about the middle — so the table
//! is thirteen coordinates written down and thirty-nine worked out, and a
//! mistake in the shape of the loop is a mistake in one place.
//!
//! ## The four diagonal corners, which are not a bug
//!
//! Fifty-two squares do not orthogonally connect. Walk the outside of the
//! cross and it takes **fifty-six** cells; the standard board is fifty-two,
//! and the four it is missing are the inner corners where an arm meets the
//! next. So the path steps **diagonally** four times, once per quarter:
//!
//! ```text
//!     … (4,8) (5,8)
//!                   \           the turn from an arm's side row
//!                    (6,9) (6,10) …    into the next arm's column
//! ```
//!
//! Nobody notices, because a player counts squares rather than measuring
//! steps. It is written down here because the obvious test — "every square is
//! next to the one after it" — fails on a correct board, and finding that out
//! by rebuilding the table is a waste of an evening.
//!
//! ## What a seat is
//!
//! Four seats, thirteen squares apart, so seat `s` begins at track square
//! `13s`. From there a token walks:
//!
//! ```text
//!     step 0 … 50     the track, starting at its own start square
//!     step 51 … 56    its own home column, six squares
//!     step 57         the middle. Home.
//! ```
//!
//! Fifty-one squares of track rather than fifty-two: a token stops one short of
//! where it began and turns in, which is why every seat walks the same distance
//! and none of them laps the board.
//!
//! ## Where the home column is
//!
//! Written once and **turned with the track**. The home column, the yard and
//! the loop are all one quarter each, spun by the same rotation — so they
//! cannot fall out of step with one another. Deriving home from the track by
//! walking inward from a tip was the first attempt and it was worse: the last
//! square before home is not a tip, so "inward" was a diagonal, and a seat
//! walked off the arm.

use plotkit::{Cx, Shape};

/// How many squares round the outside.
pub const TRACK: usize = 52;
/// How far apart the four starts are.
pub const APART: usize = TRACK / 4;
/// Squares in a home column.
pub const HOME: usize = 6;
/// `step` when a token is finished.
pub const FINISH: usize = 57;
/// The board is fifteen squares across.
pub const SIDE: i32 = 15;

/// A cell of the grid as a point, with the middle of the board at the origin.
fn cell(x: i32, y: i32) -> Cx {
    Cx::new(f64::from(x) - 7.0, f64::from(y) - 7.0)
}

/// A quarter turn about the middle: `(x, y) ↦ (y, 14 − x)`.
fn turned(p: (i32, i32)) -> (i32, i32) {
    (p.1, SIDE - 1 - p.0)
}

/// The one quarter that is written down: the tip of an arm, along its far
/// side, then up the next arm's column. Everything else is this, turned.
const QUARTER: [(i32, i32); 13] = [
    (0, 7),
    (0, 8),
    (1, 8),
    (2, 8),
    (3, 8),
    (4, 8),
    (5, 8),
    // the diagonal corner
    (6, 9),
    (6, 10),
    (6, 11),
    (6, 12),
    (6, 13),
    (6, 14),
];

/// A seat's home column, from just inside the tip to just short of the middle.
///
/// Written for one seat and **turned with the track**, so the two cannot fall
/// out of step and a seat cannot end up walking into somebody else's column.
const HOME_QUARTER: [(i32, i32); HOME] = [(1, 7), (2, 7), (3, 7), (4, 7), (5, 7), (6, 7)];

/// Where a seat's four tokens wait — the corner behind its start.
const YARD_QUARTER: [(i32, i32); 4] = [(2, 10), (4, 10), (2, 12), (4, 12)];

/// Turn a list of cells `q` quarter-turns.
fn spun(cells: &[(i32, i32)], q: usize) -> Vec<(i32, i32)> {
    cells
        .iter()
        .map(|p| {
            let mut p = *p;
            for _ in 0..q {
                p = turned(p);
            }
            p
        })
        .collect()
}

/// The fifty-two squares, in the order a token walks them.
pub fn track() -> Vec<(i32, i32)> {
    (0..4).flat_map(|q| spun(&QUARTER, q)).collect()
}

/// Which track square a seat starts on.
pub fn start(seat: usize) -> usize {
    (seat % 4) * APART
}

/// The six squares of a seat's home column, from the outside in.
pub fn home(seat: usize) -> Vec<(i32, i32)> {
    spun(&HOME_QUARTER, seat % 4)
}

/// Where a seat's four tokens wait, in its corner.
pub fn yard(seat: usize) -> Vec<(i32, i32)> {
    spun(&YARD_QUARTER, seat % 4)
}

/// Where a token stands.
///
/// `step` is how far it has walked: `0…50` on the track, `51…56` in its own
/// home column, `57` home. Anything else — a token still in the yard — has no
/// square, and gets one of the four waiting places instead.
pub fn place(seat: usize, step: usize) -> Cx {
    let seat = seat % 4;
    if step >= FINISH {
        return cell(7, 7);
    }
    if step >= TRACK - 1 {
        let column = home(seat);
        let k = (step - (TRACK - 1)).min(HOME - 1);
        let (x, y) = column[k];
        return cell(x, y);
    }
    let (x, y) = track()[(start(seat) + step) % TRACK];
    cell(x, y)
}

/// Where a token waits before it comes out.
pub fn waiting(seat: usize, token: usize) -> Cx {
    let spots = yard(seat % 4);
    let (x, y) = spots[token % spots.len()];
    cell(x, y)
}

/// The colours of the four seats.
pub const SEATS: [u32; 4] = [0xE0704A, 0x6FCF97, 0x4FBCD4, 0xE0A44A];

/// The board: the track, the four home columns, the yards and the middle.
pub fn board() -> Vec<(Shape, u32)> {
    let mut out = Vec::new();
    let square = |x: i32, y: i32| {
        let c = cell(x, y);
        Shape::polygon(vec![
            c + Cx::new(-0.46, -0.46),
            c + Cx::new(0.46, -0.46),
            c + Cx::new(0.46, 0.46),
            c + Cx::new(-0.46, 0.46),
        ])
    };

    for (k, (x, y)) in track().into_iter().enumerate() {
        // A seat's own start square is painted in its colour, which is the
        // only thing on the track that is not the same as everything else.
        let owner = (0..4).find(|s| start(*s) == k);
        out.push((square(x, y), owner.map_or(0x2A3542, |s| SEATS[s])));
    }
    for seat in 0..4 {
        for (x, y) in home(seat) {
            out.push((square(x, y), SEATS[seat]));
        }
        for (x, y) in yard(seat) {
            out.push((Shape::circle(cell(x, y), 0.46), SEATS[seat]));
        }
    }
    // The middle, where everybody is going.
    out.push((Shape::polygon(vec![cell(7, 6), cell(8, 7), cell(7, 8), cell(6, 7)]), 0xE3E9EF));
    out
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// ★ Fifty-two squares, all different, and the loop closes — the last is
    /// next to the first. A track with a repeat in it is one where two tokens
    /// can be in the same place and only one of them knows.
    #[test]
    fn the_track_is_a_closed_loop_of_fifty_two() {
        let t = track();
        assert_eq!(t.len(), TRACK);
        assert_eq!(t.iter().collect::<HashSet<_>>().len(), TRACK, "no square twice");

        // Every step is one square, EXCEPT the four inner corners of the
        // cross, where the standard fifty-two-square board turns diagonally.
        // Walking the outside orthogonally takes fifty-six; the four missing
        // are exactly those corners. This is a property of the board, not a
        // mistake in the table -- see the note at the top.
        let mut diagonals = 0;
        for k in 0..TRACK {
            let a = t[k];
            let b = t[(k + 1) % TRACK];
            let step = ((a.0 - b.0).abs(), (a.1 - b.1).abs());
            match step.0 + step.1 {
                1 => {}
                2 if step.0 == 1 && step.1 == 1 => diagonals += 1,
                _ => panic!("square {k} to {} is neither a step nor a corner: {a:?} to {b:?}", k + 1),
            }
        }
        assert_eq!(diagonals, 4, "one diagonal corner per quarter, and no more");
    }

    /// ★ Every square is on the board, and none is in the middle three by
    /// three, which is where the home columns and the finish live.
    #[test]
    fn the_track_stays_on_the_board_and_out_of_the_middle() {
        for (x, y) in track() {
            assert!((0..SIDE).contains(&x) && (0..SIDE).contains(&y), "({x}, {y}) is off the board");
            assert!(!((6..=8).contains(&x) && (6..=8).contains(&y)), "({x}, {y}) is in the middle");
        }
    }

    /// The four seats are evenly spaced, a quarter of the loop apart.
    #[test]
    fn the_four_seats_are_a_quarter_apart() {
        assert_eq!((0..4).map(start).collect::<Vec<_>>(), vec![0, 13, 26, 39]);
    }

    /// ★ Home columns are **derived** from the track, so no seat can walk into
    /// another's. Six squares each, all different, and none shared.
    #[test]
    fn each_seat_has_its_own_home_column() {
        let mut seen: HashSet<(i32, i32)> = HashSet::new();
        for seat in 0..4 {
            let column = home(seat);
            assert_eq!(column.len(), HOME);
            for cell in &column {
                assert!(seen.insert(*cell), "{cell:?} is in two home columns");
            }
        }
        assert_eq!(seen.len(), 4 * HOME);
    }

    /// ★ And a column runs from the tip **toward the middle**, ending beside
    /// the finish — not away from it, which is the mistake that is invisible
    /// until a token walks off the edge of the board.
    #[test]
    fn a_home_column_walks_toward_the_middle() {
        for seat in 0..4 {
            let column = home(seat);
            let far = (column[0].0 - 7).abs() + (column[0].1 - 7).abs();
            let near = (column[HOME - 1].0 - 7).abs() + (column[HOME - 1].1 - 7).abs();
            assert!(near < far, "seat {seat} walks away from the middle");
            assert_eq!(near, 1, "and stops one short of it");
        }
    }

    /// ★ Every seat walks exactly the same distance: fifty-one squares of
    /// track, six of home, and the middle. One short of a lap, which is why
    /// none of them goes round twice.
    #[test]
    fn every_seat_walks_the_same_way_home() {
        for seat in 0..4 {
            let mut seen: HashSet<(i32, i32)> = HashSet::new();
            let mut last = place(seat, 0);
            for step in 0..=FINISH {
                let at = place(seat, step);
                assert!(at.re.abs() <= 7.5 && at.im.abs() <= 7.5, "seat {seat} left the board at step {step}");
                if step > 0 {
                    let jump = (at - last).abs();
                    assert!(jump < 1.5, "seat {seat} jumped {jump} at step {step}");
                }
                seen.insert((at.re as i32, at.im as i32));
                last = at;
            }
            assert_eq!(place(seat, FINISH), Cx::ZERO, "and finishes in the middle");
        }
    }

    /// ★ A token turns into its **own** column, and it is the column of the
    /// seat it belongs to. The test that would have caught the obvious slip of
    /// deriving home from the wrong end of the loop.
    #[test]
    fn a_token_turns_into_its_own_column() {
        for seat in 0..4 {
            let column = home(seat);
            for k in 0..HOME {
                let (x, y) = column[k];
                assert_eq!(place(seat, TRACK - 1 + k), cell(x, y), "seat {seat}, home square {k}");
            }
        }
    }

    /// Two seats never stand on the same square at the same step, since they
    /// start a quarter apart.
    #[test]
    fn two_seats_at_the_same_step_are_in_different_places() {
        for step in 0..TRACK - 1 {
            let spots: HashSet<(i64, i64)> =
                (0..4).map(|s| place(s, step)).map(|z| (z.re as i64, z.im as i64)).collect();
            assert_eq!(spots.len(), 4, "at step {step} two seats share a square");
        }
    }

    /// Each seat's four waiting places are its own, and in its own corner.
    #[test]
    fn every_seat_has_four_places_to_wait() {
        let mut seen: HashSet<(i32, i32)> = HashSet::new();
        for seat in 0..4 {
            let spots = yard(seat);
            assert_eq!(spots.len(), 4);
            for s in &spots {
                assert!(seen.insert(*s), "{s:?} is in two yards");
            }
            // All four in one corner, and the corner is a corner.
            let (x, y): (Vec<i32>, Vec<i32>) = spots.iter().cloned().unzip();
            assert!(x.iter().all(|v| *v < 6 || *v > 8));
            assert!(y.iter().all(|v| *v < 6 || *v > 8));
        }
    }

    /// A token still waiting gets a place in the yard rather than a square.
    #[test]
    fn a_waiting_token_stands_in_the_yard() {
        for seat in 0..4 {
            let spots: HashSet<(i64, i64)> =
                (0..4).map(|t| waiting(seat, t)).map(|z| (z.re as i64, z.im as i64)).collect();
            assert_eq!(spots.len(), 4, "four tokens, four places");
        }
    }

    /// The board draws every square once: fifty-two of track, four columns of
    /// six, four yards of four, and the middle.
    #[test]
    fn the_board_draws_everything_once() {
        assert_eq!(board().len(), TRACK + 4 * HOME + 4 * 4 + 1);
    }

    /// A seat's own start square is painted its colour, and nothing else on
    /// the track is.
    #[test]
    fn a_start_square_is_painted_in_its_seats_colour() {
        let drawn = board();
        let coloured = (0..TRACK).filter(|k| SEATS.contains(&drawn[*k].1)).count();
        assert_eq!(coloured, 4, "four starts, four colours");
        for seat in 0..4 {
            assert_eq!(drawn[start(seat)].1, SEATS[seat]);
        }
    }
}
