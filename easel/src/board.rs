//! # board — the editor itself, with no window round it
//!
//! Everything the drawing program does, expressed as *"the pointer is here and
//! it is down"* plus a few commands. No window, no key codes, no pixels — so
//! all of it is tested in the dark, and a different front end could be put
//! round it without touching any of this.
//!
//! ```text
//!     window                      board                       page
//!     ------                      -----                       ----
//!     pen at (x, y), down   -->   pointer(at, down)     -->   Sheet
//!     key U                 -->   undo()
//!     key S                 -->   save(path)
//!     every frame           <--   frame()               <--   marks + the
//!                                                             stroke in progress
//! ```
//!
//! ## Decide on the press
//!
//! What a drag *means* is settled the instant the pointer goes down and is not
//! reconsidered until it comes up. Start on a mark with the pick tool and you
//! are moving that mark — for the whole drag, even if the pointer wanders off
//! it, which it will, because moving something means moving it away from where
//! it was.
//!
//! The alternative — asking "what is under the pointer?" every frame — feels
//! broken in a way people cannot describe: a mark you are dragging escapes
//! from under the pointer and the drag transfers to whatever it was covering.
//! Same rule as [`shapes::Disc`], for the same reason.
//!
//! ## One rule for undo
//!
//! [`History::remember`] is called **before** each change, and that is the
//! only thing the editor has to get right. There are no inverse operations to
//! write, so a new tool needs no undo code at all.
//!
//! ## What is deliberately absent
//!
//! No key codes and no pixel coordinates. The window converts a pointer
//! position into world coordinates before this sees it, because a board that
//! knew about pixels would have to know about zoom, and then about panning,
//! and then it would be the window.

use plotkit::{Cx, Frame};
use shapes::Nib;

use crate::history::History;
use crate::ink::Ink;
use crate::mark::Mark;
use crate::sheet::Sheet;

/// What a drag means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    /// Lay down ink.
    Draw,
    /// Pick a mark up and move it.
    Pick,
    /// Rub marks out.
    Erase,
}

/// How near a mark the pointer has to be to catch it, in world units.
///
/// The window sets this from the zoom, because "near" is a thing the eye
/// judges in pixels and the drawing is measured in world units. Six pixels at
/// the current scale is about right: less and fine lines cannot be caught,
/// more and everything grabs everything.
pub const TOUCH: f64 = 0.12;

/// The drawing program.
pub struct Board {
    pub sheet: Sheet,
    pub tool: Tool,

    // --- the tool in hand ---------------------------------------------------
    pub nib: Nib,
    pub colour: u32,
    pub taper: f64,
    /// How hard the ink is pulled towards the pen. See [`crate::ink`].
    pub pull: f64,
    /// How near counts as touching. Set from the zoom by whoever owns the view.
    pub touch: f64,

    // --- what is going on right now -----------------------------------------
    ink: Option<Ink>,
    /// Which mark is being moved, and where the pointer was last frame.
    holding: Option<(usize, Cx)>,
    /// Whether the pointer was down last frame, so a press can be told from a
    /// drag without the window having to say.
    was_down: bool,
    past: History,
}

impl Default for Board {
    fn default() -> Board {
        Board::new()
    }
}

impl Board {
    pub fn new() -> Board {
        Board {
            sheet: Sheet::new(),
            tool: Tool::Draw,
            nib: Nib::Quill { slow: 0.13, fast: 0.02, pace: 0.16 },
            colour: 0xE3E9EF,
            taper: 0.12,
            pull: crate::ink::PULL,
            touch: TOUCH,
            ink: None,
            holding: None,
            was_down: false,
            past: History::new(),
        }
    }

    // ---- the pointer -------------------------------------------------------

    /// Where the pointer is, and whether it is down. Called every frame.
    ///
    /// Press, drag and release are worked out here rather than being asked
    /// for, so the window has one thing to report and cannot report a release
    /// it never noticed.
    pub fn pointer(&mut self, at: Cx, down: bool) {
        match (self.was_down, down) {
            (false, true) => self.press(at),
            (true, true) => self.drag(at),
            (true, false) => self.release(at),
            (false, false) => {}
        }
        self.was_down = down;
    }

    fn press(&mut self, at: Cx) {
        match self.tool {
            Tool::Draw => {
                let mut ink = Ink::new(self.nib, self.colour).with_pull(self.pull).with_taper(self.taper);
                ink.sample(at);
                self.ink = Some(ink);
            }
            Tool::Pick => {
                if let Some(k) = self.sheet.at(at, self.touch) {
                    // Remembered on the press, so the whole drag is one step
                    // back rather than sixty.
                    self.past.remember(&self.sheet);
                    self.holding = Some((k, at));
                }
            }
            Tool::Erase => {
                self.past.remember(&self.sheet);
                self.rub(at);
            }
        }
    }

    fn drag(&mut self, at: Cx) {
        match self.tool {
            Tool::Draw => {
                if let Some(ink) = self.ink.as_mut() {
                    ink.sample(at);
                }
            }
            Tool::Pick => {
                if let Some((k, was)) = self.holding {
                    if let Some(m) = self.sheet.marks.get_mut(k) {
                        *m = m.shifted(at - was);
                    }
                    self.holding = Some((k, at));
                }
            }
            // Rubbing out is a continuous thing, like a real eraser — one
            // press-and-sweep takes out everything it crosses, and all of it
            // comes back with one undo.
            Tool::Erase => self.rub(at),
        }
    }

    fn release(&mut self, at: Cx) {
        if let Some(ink) = self.ink.take() {
            if let Some(mark) = ink.lift(at) {
                self.past.remember(&self.sheet);
                self.sheet.add(mark);
            }
        }
        self.holding = None;
    }

    /// Take out whatever is under the pointer.
    fn rub(&mut self, at: Cx) {
        if let Some(k) = self.sheet.at(at, self.touch) {
            self.sheet.marks.remove(k);
        }
    }

    /// The stroke being drawn right now, if there is one.
    pub fn drawing(&self) -> Option<Mark> {
        let ink = self.ink.as_ref()?;
        (ink.len() >= 2).then(|| {
            Mark {
                pts: ink.points().to_vec(),
                nib: self.nib,
                taper: self.taper,
                colour: self.colour,
                filled: true,
                // Not closed while it is still being drawn: whether a stroke
                // loops is decided when the pen lifts, and guessing early
                // makes the mark flicker between open and closed under your
                // hand.
                closed: false,
            }
        })
    }

    // ---- commands ----------------------------------------------------------

    pub fn undo(&mut self) -> bool {
        match self.past.undo(&self.sheet) {
            Some(was) => {
                self.sheet = was;
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        match self.past.redo(&self.sheet) {
            Some(next) => {
                self.sheet = next;
                true
            }
            None => false,
        }
    }

    pub fn can_undo(&self) -> bool {
        self.past.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.past.can_redo()
    }

    /// Throw the whole drawing away — undoably, because people press this by
    /// accident and a drawing program that cannot take it back is a cruel one.
    pub fn clear(&mut self) {
        self.past.remember(&self.sheet);
        self.sheet = Sheet::new();
    }

    /// Run every closed mark through the low-pass dial.
    ///
    /// The whole page at once, because that is how it is used: draw roughly,
    /// then decide how much of your hand to keep.
    pub fn smooth_all(&mut self, cut: usize) {
        self.past.remember(&self.sheet);
        for m in &mut self.sheet.marks {
            *m = m.smooth(cut);
        }
    }

    pub fn save(&self, path: &str) -> std::io::Result<()> {
        self.sheet.save(path)
    }

    /// Open a drawing, replacing this one. Says how many lines confused it.
    ///
    /// The history is forgotten, or undo would walk back into the drawing you
    /// had before — which looks exactly like data loss.
    pub fn load(&mut self, path: &str) -> std::io::Result<usize> {
        let (sheet, confused) = Sheet::load(path)?;
        self.sheet = sheet;
        self.past.forget();
        Ok(confused)
    }

    // ---- drawing -----------------------------------------------------------

    /// The page as it should look, including the stroke in progress.
    pub fn frame(&self) -> Frame {
        let mut f = Frame::new();
        for m in &self.sheet.marks {
            let item = f.add(m.shape()).color(m.colour);
            if m.filled {
                item.fill();
            }
        }
        if let Some(wet) = self.drawing() {
            f.add(wet.shape()).color(wet.colour).fill();
        }
        f
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    /// Draw a path with the pen, press to lift, as a window would.
    fn draw(b: &mut Board, path: &[Cx]) {
        for z in path {
            b.pointer(*z, true);
        }
        b.pointer(*path.last().expect("a path"), false);
    }

    fn line(from: Cx, to: Cx, n: usize) -> Vec<Cx> {
        (0..n).map(|k| from + (to - from).scale(k as f64 / (n - 1) as f64)).collect()
    }

    fn ring(r: f64, at: Cx, n: usize) -> Vec<Cx> {
        (0..=n).map(|k| at + Cx::polar(r, k as f64 / n as f64 * TAU)).collect()
    }

    /// ★ Press, drag, release — worked out from a position and a flag, so the
    /// window has one thing to report and cannot report a release it never
    /// noticed.
    #[test]
    fn drawing_a_stroke_leaves_one_mark() {
        let mut b = Board::new();
        assert!(b.sheet.is_empty());
        draw(&mut b, &line(Cx::new(-2.0, 0.0), Cx::new(2.0, 1.0), 40));
        assert_eq!(b.sheet.len(), 1);
        assert!(b.sheet.marks[0].pts.len() > 5, "the stroke should have kept its shape");
    }

    /// Nothing is added until the pen lifts — but it is visible while it is
    /// being made, or you are drawing blind.
    #[test]
    fn the_stroke_is_visible_before_it_is_finished() {
        let mut b = Board::new();
        for z in line(Cx::ZERO, Cx::new(3.0, 0.0), 20) {
            b.pointer(z, true);
        }
        assert!(b.sheet.is_empty(), "not committed yet");
        assert!(b.drawing().is_some(), "but you can see it");
        assert!(!b.frame().is_empty(), "and it is in the frame");

        b.pointer(Cx::new(3.0, 0.0), false);
        assert_eq!(b.sheet.len(), 1);
        assert!(b.drawing().is_none(), "and now it is a mark rather than a stroke");
    }

    /// ★ The whole point of the crate: what was drawn survives being written
    /// and read back.
    #[test]
    fn a_drawing_survives_a_round_trip_through_a_file() {
        let mut b = Board::new();
        draw(&mut b, &line(Cx::new(-2.0, 0.0), Cx::new(2.0, 1.0), 40));
        draw(&mut b, &ring(1.5, Cx::new(3.0, 3.0), 60));

        let path = std::env::temp_dir().join("easel-round-trip.easel");
        let path = path.to_str().expect("a path");
        b.save(path).expect("saved");

        let mut opened = Board::new();
        assert_eq!(opened.load(path).expect("loaded"), 0, "nothing should confuse it");
        assert_eq!(opened.sheet.len(), b.sheet.len());
        assert_eq!(opened.sheet.marks[1].closed, true, "the ring should still be a ring");
        let _ = std::fs::remove_file(path);
    }

    /// ★ Picking a mark up is decided on the **press**. Ask again every frame
    /// and the mark escapes from under the pointer as soon as it moves, and
    /// the drag transfers to whatever it was covering.
    #[test]
    fn a_mark_being_moved_does_not_escape_from_under_the_pointer() {
        let mut b = Board::new();
        draw(&mut b, &ring(1.0, Cx::ZERO, 60));
        let was = b.sheet.marks[0].bounds().expect("bounds");

        b.tool = Tool::Pick;
        // Grab the ring itself, then drag a long way — well past anywhere the
        // mark still is.
        b.pointer(Cx::new(1.0, 0.0), true);
        for k in 1..=40 {
            b.pointer(Cx::new(1.0 + k as f64 * 0.25, 0.0), true);
        }
        b.pointer(Cx::new(11.0, 0.0), false);

        let now = b.sheet.marks[0].bounds().expect("bounds");
        assert!((now.0.re - was.0.re - 10.0).abs() < 1e-6, "it should have come all the way: {:?}", now.0);
    }

    /// Grabbing empty paper moves nothing, rather than moving the last thing
    /// drawn.
    #[test]
    fn grabbing_nothing_moves_nothing() {
        let mut b = Board::new();
        draw(&mut b, &ring(1.0, Cx::ZERO, 60));
        let was = b.sheet.marks[0].clone();

        b.tool = Tool::Pick;
        b.pointer(Cx::new(40.0, 40.0), true);
        b.pointer(Cx::new(45.0, 45.0), true);
        b.pointer(Cx::new(45.0, 45.0), false);
        assert_eq!(b.sheet.marks[0], was);
    }

    /// ★ A whole drag is **one** step back, not sixty. Undo that unwound a
    /// move one frame at a time would take a minute of pressing to get past a
    /// single gesture.
    #[test]
    fn moving_something_is_one_step_back() {
        let mut b = Board::new();
        draw(&mut b, &ring(1.0, Cx::ZERO, 60));
        let before = b.sheet.clone();

        b.tool = Tool::Pick;
        b.pointer(Cx::new(1.0, 0.0), true);
        for k in 1..=30 {
            b.pointer(Cx::new(1.0 + k as f64 * 0.1, 0.0), true);
        }
        b.pointer(Cx::new(4.0, 0.0), false);

        assert!(b.undo(), "one press should be enough");
        assert_eq!(b.sheet, before);
    }

    /// ★ And so is one sweep of the eraser, however many marks it crossed.
    #[test]
    fn one_sweep_of_the_eraser_is_one_step_back() {
        let mut b = Board::new();
        for k in 0..5 {
            draw(&mut b, &ring(0.4, Cx::new(k as f64, 0.0), 40));
        }
        let before = b.sheet.clone();
        assert_eq!(b.sheet.len(), 5);

        b.tool = Tool::Erase;
        b.pointer(Cx::new(-0.4, 0.0), true);
        for k in 0..=80 {
            b.pointer(Cx::new(-0.4 + k as f64 * 0.06, 0.0), true);
        }
        b.pointer(Cx::new(4.4, 0.0), false);

        assert!(b.sheet.len() < 5, "it should have rubbed something out");
        assert!(b.undo(), "and it should all come back at once");
        assert_eq!(b.sheet, before);
    }

    /// Undo walks all the way back to an empty page, and redo comes forward
    /// again.
    #[test]
    fn undo_walks_back_through_every_stroke() {
        let mut b = Board::new();
        for k in 0..4 {
            draw(&mut b, &line(Cx::new(k as f64, 0.0), Cx::new(k as f64, 2.0), 20));
        }
        assert_eq!(b.sheet.len(), 4);

        while b.undo() {}
        assert!(b.sheet.is_empty(), "it should get back to a blank page");

        while b.redo() {}
        assert_eq!(b.sheet.len(), 4, "and forward again");
    }

    /// ★ Clearing is undoable. People press it by accident, and a drawing
    /// program that cannot take that back is a cruel one.
    #[test]
    fn clearing_the_page_can_be_taken_back() {
        let mut b = Board::new();
        draw(&mut b, &ring(1.0, Cx::ZERO, 60));
        b.clear();
        assert!(b.sheet.is_empty());
        assert!(b.undo());
        assert_eq!(b.sheet.len(), 1, "it should still be there");
    }

    /// ★ Opening a drawing forgets the old history, or undo walks back into
    /// the drawing you had before opening — which looks exactly like data loss
    /// even though nothing was lost.
    #[test]
    fn opening_a_drawing_does_not_leave_the_old_one_one_press_away() {
        let mut b = Board::new();
        draw(&mut b, &ring(1.0, Cx::ZERO, 60));

        let path = std::env::temp_dir().join("easel-other.easel");
        let path = path.to_str().expect("a path");
        let mut other = Board::new();
        draw(&mut other, &line(Cx::new(-1.0, -1.0), Cx::new(1.0, 1.0), 20));
        other.save(path).expect("saved");

        b.load(path).expect("loaded");
        assert!(!b.can_undo(), "the drawing you had before must not be one press away");
        let _ = std::fs::remove_file(path);
    }

    /// The dial applies to the page and is itself one step back.
    #[test]
    fn smoothing_the_page_is_undoable() {
        let mut b = Board::new();
        let shaky: Vec<Cx> =
            (0..=200).map(|k| { let th = k as f64 / 200.0 * TAU; Cx::polar(2.0 + 0.1 * (th * 9.0).sin(), th) }).collect();
        draw(&mut b, &shaky);
        assert!(b.sheet.marks[0].closed, "it should have closed");

        let before = b.sheet.clone();
        b.smooth_all(3);
        assert_ne!(b.sheet, before, "it should have changed something");
        assert!(b.undo());
        assert_eq!(b.sheet, before);
    }

    /// A tap leaves nothing — no invisible speck that can still be clicked on.
    #[test]
    fn a_tap_on_the_page_leaves_nothing_behind() {
        let mut b = Board::new();
        b.pointer(Cx::new(1.0, 1.0), true);
        b.pointer(Cx::new(1.0, 1.0), false);
        assert!(b.sheet.is_empty());
        assert!(!b.can_undo(), "and nothing to undo either");
    }

    /// Changing the tool does not reach back and change what is already down.
    #[test]
    fn changing_the_nib_does_not_change_finished_marks() {
        let mut b = Board::new();
        b.nib = Nib::Round(0.5);
        b.colour = 0x00FF00;
        draw(&mut b, &line(Cx::ZERO, Cx::new(2.0, 0.0), 20));

        b.nib = Nib::Broad { width: 0.1, angle: 1.0 };
        b.colour = 0xFF0000;
        assert_eq!(b.sheet.marks[0].nib, Nib::Round(0.5));
        assert_eq!(b.sheet.marks[0].colour, 0x00FF00);
    }
}
