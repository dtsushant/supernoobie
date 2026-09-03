//! # ludo — the board, drawn
//!
//! The geometry is [`plotkit::ludo`]: where a token stands is arithmetic, and
//! `plotkit::expr` needs it, so it lives next to the arithmetic. This is what
//! that board looks like.

use plotkit::ludo::{cell, home, start, track, yard};
use plotkit::{Cx, Shape};

pub use plotkit::ludo::{place, waiting, APART, FINISH, SIDE};
pub use plotkit::ludo::{home as home_of, start as start_of, track as track_of, yard as yard_of};
pub use plotkit::ludo::{HOME as HOME_LEN, TRACK as TRACK_LEN};

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

    /// The board draws every square once: fifty-two of track, four columns of
    /// six, four yards of four, and the middle.
    #[test]
    fn the_board_draws_everything_once() {
        assert_eq!(board().len(), plotkit::ludo::TRACK + 4 * plotkit::ludo::HOME + 4 * 4 + 1);
    }

    /// A seat's own start square is painted its colour, and nothing else on
    /// the track is.
    #[test]
    fn a_start_square_is_painted_in_its_seats_colour() {
        let drawn = board();
        let coloured = (0..plotkit::ludo::TRACK).filter(|k| SEATS.contains(&drawn[*k].1)).count();
        assert_eq!(coloured, 4, "four starts, four colours");
        for seat in 0..4 {
            assert_eq!(drawn[start(seat)].1, SEATS[seat]);
        }
    }
}
