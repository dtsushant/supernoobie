//! # panel — the list of rows, Desmos-style
//!
//! A column of rows down one side. Each row is a line of script you can type
//! into, a tick to switch it off, and — if it binds a plain number — a slider
//! under it.
//!
//! ```text
//!     +---------------------------+
//!     | [x] r = 2                 |
//!     |     ---------o---------   |   <- the slider for r
//!     | [x] circle(0, r)          |
//!     | [ ] ngon(0, r, 6)         |   <- switched off, not deleted
//!     | [+] add a row             |
//!     +---------------------------+
//! ```
//!
//! ## The same arrangement as [`bar`](crate::bar), for the same reason
//!
//! Rectangles in pixels, worked out **once**, read by both the painting and
//! the hit testing. Two pieces of arithmetic that have to agree will one day
//! not, and the symptom is a row that does the wrong thing near its edge.
//!
//! The one difference is that the panel is built fresh each frame, because it
//! depends on how many rows there are and on the window's size. It is a few
//! dozen rectangles; working them out is nothing next to drawing a curve.
//!
//! ## Why the slider does not remember its own range
//!
//! It runs from `−RANGE` to `+RANGE` and that is all. A slider that widened
//! itself to fit whatever you typed would move under your hand the moment you
//! typed a bigger number — the pointer would stay still and the value would
//! jump, because the same pixel now means something else. Type the number you
//! want; the slider is for *exploring near* it, not for reaching everything.

use plotkit::{Anchor, Canvas, Frame};

use crate::board::Board;

/// How wide the panel is.
pub const WIDTH: i32 = 340;
/// How far a slider reaches either side of zero.
pub const RANGE: f64 = 10.0;

const PAD: i32 = 10;
/// A row is tall enough to hit — the same reasoning as [`crate::bar::TAP`].
const ROW: i32 = 34;
const SLIDER: i32 = 22;
const TICK: i32 = 26;

/// What was pressed in the panel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Poke {
    /// Switch this row on or off.
    Tick(usize),
    /// Start typing in this row.
    Edit(usize),
    /// Move this row's dial to this value.
    Drag(usize, f64),
    /// Add a row at the end.
    Add,
}

/// One row's rectangles.
#[derive(Clone, Debug)]
pub struct RowBox {
    pub row: usize,
    pub y: i32,
    pub h: i32,
    /// The dial this row binds, and where its slider is.
    pub dial: Option<(String, f64, i32)>,
}

/// The rows, laid out.
#[derive(Clone, Debug, Default)]
pub struct Panel {
    pub x: i32,
    pub rows: Vec<RowBox>,
    /// The "add a row" button.
    pub add: (i32, i32),
    pub height: i32,
}

impl Panel {
    /// Lay the panel out down the right-hand side of a window this wide.
    pub fn new(window_w: i32, board: &Board) -> Panel {
        let x = window_w - WIDTH;
        let dials = board.sheet.script.dials(board.clock);
        let mut p = Panel { x, rows: Vec::new(), add: (x + PAD, PAD), height: 0 };
        let mut y = PAD;

        for (k, r) in board.sheet.script.rows.iter().enumerate() {
            // A row that binds a plain number gets a slider under it, which is
            // the whole reason to type `r = 2` on a line of its own.
            let dial = r
                .binds()
                .and_then(|name| dials.iter().find(|(n, _)| n == name))
                .map(|(n, v)| (n.clone(), *v, y + ROW));
            let h = if dial.is_some() { ROW + SLIDER } else { ROW };
            p.rows.push(RowBox { row: k, y, h, dial });
            y += h + 4;
        }
        p.add = (x + PAD, y);
        p.height = y + ROW + PAD;
        p
    }

    /// Is this pixel over the panel at all?
    ///
    /// The gaps between rows are still the panel — a stroke begun in a gap
    /// would be drawn underneath it, where it can be neither seen nor picked
    /// up again.
    pub fn covers(&self, px: f64) -> bool {
        px >= self.x as f64
    }

    /// What was pressed here.
    pub fn at(&self, px: f64, py: f64) -> Option<Poke> {
        if !self.covers(px) {
            return None;
        }
        for b in &self.rows {
            if let Some((_, _, sy)) = &b.dial {
                let (sy, x0, x1) = (*sy, self.x + PAD, self.x + WIDTH - PAD);
                if py >= sy as f64 && py < (sy + SLIDER) as f64 {
                    let s = ((px - x0 as f64) / (x1 - x0).max(1) as f64).clamp(0.0, 1.0);
                    return Some(Poke::Drag(b.row, (s * 2.0 - 1.0) * RANGE));
                }
            }
            if py >= b.y as f64 && py < (b.y + ROW) as f64 {
                let tick = self.x + PAD;
                return Some(if px < (tick + TICK) as f64 { Poke::Tick(b.row) } else { Poke::Edit(b.row) });
            }
        }
        let (ax, ay) = self.add;
        if py >= ay as f64 && py < (ay + ROW) as f64 && px >= ax as f64 {
            return Some(Poke::Add);
        }
        None
    }

    /// Where a value sits along its slider, 0 to 1.
    fn along(value: f64) -> f64 {
        ((value / RANGE) * 0.5 + 0.5).clamp(0.0, 1.0)
    }

    /// Paint it, from the same rectangles [`Panel::at`] reads.
    pub fn paint(&self, f: &mut Frame, board: &Board, window_h: i32) {
        let made = board.written();
        f.chip(self.x, 0, WIDTH, window_h.max(self.height), PANEL);
        f.chip(self.x, 0, 1, window_h.max(self.height), EDGE);

        for b in &self.rows {
            let Some(r) = board.sheet.script.rows.get(b.row) else { continue };
            let wrong = made.errors.iter().any(|(line, _)| *line == b.row + 1);
            let being_typed = board.editing == Some(b.row);

            f.chip(self.x + PAD, b.y, WIDTH - 2 * PAD, ROW, if being_typed { LIT } else { EDGE });
            // The tick. Filled when the row is on, hollow when it is not.
            f.chip(self.x + PAD + 4, b.y + 4, TICK - 8, ROW - 8, if r.on { 0x6FCF97 } else { 0x33404E });

            let ink = if wrong {
                0xE0704A
            } else if r.on {
                INK
            } else {
                0x6B7987
            };
            // A caret while typing, so there is something to aim at.
            let text = if being_typed { format!("{}_", r.text) } else { r.text.clone() };
            let room = ((WIDTH - 2 * PAD - TICK - 12) / Canvas::text_w("n", 1).max(1)) as usize;
            let shown: String = if text.chars().count() > room {
                // The end, not the beginning: what you are typing is at the
                // end, and a box that scrolled away from the cursor would be
                // useless for typing into.
                text.chars().skip(text.chars().count() - room).collect()
            } else {
                text
            };
            f.pin(Anchor::TopLeft, (self.x + PAD + TICK + 6) as f64, (b.y + (ROW - 7) / 2) as f64, shown, ink, 1);

            if let Some((name, value, sy)) = &b.dial {
                let (x0, x1) = (self.x + PAD, self.x + WIDTH - PAD);
                let mid = *sy + SLIDER / 2;
                f.chip(x0, mid - 1, x1 - x0, 2, 0x33404E);
                let knob = x0 + ((x1 - x0) as f64 * Panel::along(*value)) as i32;
                f.chip(knob - 4, mid - 8, 8, 16, 0xE0A44A);
                f.pin(
                    Anchor::TopLeft,
                    x0 as f64,
                    (*sy + SLIDER - 8) as f64,
                    format!("{name} = {value:.2}"),
                    0x6B7987,
                    1,
                );
            }
        }

        let (ax, ay) = self.add;
        f.chip(ax, ay, WIDTH - 2 * PAD, ROW, EDGE);
        f.pin(Anchor::TopLeft, (ax + 10) as f64, (ay + (ROW - 7) / 2) as f64, "+  a new row", 0x94A1AE, 1);

        // Whatever the script is complaining about, under the rows.
        if let Some((line, msg)) = made.errors.first() {
            f.pin(
                Anchor::TopLeft,
                (self.x + PAD) as f64,
                (ay + ROW + 8) as f64,
                format!("row {line}: {msg}"),
                0xE0704A,
                1,
            );
        }
    }
}

const PANEL: u32 = 0x141C26;
const EDGE: u32 = 0x22303C;
const LIT: u32 = 0x2E4257;
const INK: u32 = 0xC3CDD7;

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn board_with(rows: &[&str]) -> Board {
        let mut b = Board::new();
        for r in rows {
            b.sheet.script.add(*r);
        }
        b
    }

    /// ★ Painting and hit testing read the same rectangles, so every pixel of
    /// a row finds that row. The alternative is an edge that does the wrong
    /// thing, which nobody can reproduce on purpose.
    #[test]
    fn every_part_of_a_row_finds_that_row() {
        let b = board_with(&["r = 2", "circle(0, r)"]);
        let p = Panel::new(1200, &b);
        assert_eq!(p.rows.len(), 2);

        for rb in &p.rows {
            let left = (p.x + PAD + 2) as f64;
            let right = (p.x + WIDTH - PAD - 2) as f64;
            assert_eq!(p.at(left, rb.y as f64), Some(Poke::Tick(rb.row)), "the tick");
            assert_eq!(p.at(right, (rb.y + ROW / 2) as f64), Some(Poke::Edit(rb.row)), "the text");
        }
    }

    /// ★ A row that binds a plain number gets a slider — which is the whole
    /// reason to write `r = 2` on a line of its own.
    #[test]
    fn a_row_that_binds_a_number_gets_a_slider() {
        let b = board_with(&["r = 2", "circle(0, r)"]);
        let p = Panel::new(1200, &b);
        assert!(p.rows[0].dial.is_some(), "r should have one");
        assert!(p.rows[1].dial.is_none(), "a shape should not");

        let (_, _, sy) = p.rows[0].dial.clone().expect("a slider");
        let middle = (p.x + WIDTH / 2) as f64;
        assert!(matches!(p.at(middle, (sy + SLIDER / 2) as f64), Some(Poke::Drag(0, _))));
    }

    /// ★ Dragging a slider gives the value at that pixel, and the two ends are
    /// the two ends. A slider whose knob does not land where you put it is
    /// worse than no slider.
    #[test]
    fn the_slider_reads_the_value_at_the_pixel() {
        let b = board_with(&["r = 2"]);
        let p = Panel::new(1200, &b);
        let (_, _, sy) = p.rows[0].dial.clone().expect("a slider");
        let y = (sy + SLIDER / 2) as f64;

        let value = |px: f64| match p.at(px, y) {
            Some(Poke::Drag(_, v)) => v,
            other => panic!("expected a drag, got {other:?}"),
        };
        assert!((value((p.x + PAD) as f64) + RANGE).abs() < 1e-9, "the left end is -RANGE");
        assert!((value((p.x + WIDTH - PAD) as f64) - RANGE).abs() < 0.2, "the right end is +RANGE");
        assert!(value((p.x + WIDTH / 2) as f64).abs() < 0.5, "and the middle is nought");

        // And painting agrees: the knob for a value sits where that value is
        // read back.
        assert!((Panel::along(0.0) - 0.5).abs() < 1e-9);
        assert!(Panel::along(RANGE) > 0.99 && Panel::along(-RANGE) < 0.01);
    }

    /// A value past the end of the slider is clamped rather than drawn off the
    /// edge of the panel and over the drawing.
    #[test]
    fn a_value_past_the_end_stays_on_the_slider() {
        assert!((Panel::along(1e6) - 1.0).abs() < 1e-12);
        assert!(Panel::along(-1e6).abs() < 1e-12);
    }

    /// ★ The gaps between rows are still the panel. A stroke begun in a gap
    /// would be drawn *underneath* it, where it can be neither seen nor picked
    /// up — which looks like the pen having stopped working.
    #[test]
    fn the_gaps_in_the_panel_are_still_the_panel() {
        let b = board_with(&["r = 2", "circle(0, r)"]);
        let p = Panel::new(1200, &b);
        assert!(p.covers(p.x as f64));
        assert!(p.covers(1199.0));
        assert!(!p.covers((p.x - 1) as f64), "and the drawing beside it is the drawing");
    }

    /// Rows never overlap, so a near miss lands on nothing rather than on the
    /// neighbour.
    #[test]
    fn rows_do_not_overlap() {
        let b = board_with(&["a = 1", "b = 2", "circle(0, a)", "ngon(0, b, 5)"]);
        let p = Panel::new(1200, &b);
        for (k, a) in p.rows.iter().enumerate() {
            for c in &p.rows[k + 1..] {
                assert!(a.y + a.h <= c.y || c.y + c.h <= a.y, "rows {} and {} overlap", a.row, c.row);
            }
        }
        let (_, ay) = p.add;
        let last = p.rows.last().expect("rows");
        assert!(last.y + last.h <= ay, "and the add button is below them all");
    }

    /// An empty panel still offers a way to start.
    #[test]
    fn an_empty_panel_still_has_somewhere_to_begin() {
        let b = Board::new();
        let p = Panel::new(1200, &b);
        assert!(p.rows.is_empty());
        let (ax, ay) = p.add;
        assert_eq!(p.at((ax + 20) as f64, (ay + 5) as f64), Some(Poke::Add));
    }

    /// Pressing the drawing, not the panel, is nothing to do with the panel.
    #[test]
    fn the_drawing_is_not_the_panel() {
        let b = board_with(&["r = 2"]);
        let p = Panel::new(1200, &b);
        assert_eq!(p.at(10.0, 10.0), None);
    }
}
