//! # history — undo, and why it keeps whole drawings
//!
//! ## Snapshots, not operations
//!
//! The clever way to do undo is to record each *operation* and write an
//! inverse for it: added-a-mark undoes by removing, moved undoes by moving
//! back. It is small in memory and it is where undo bugs come from, because
//! every new operation needs a matching inverse and the day somebody writes
//! one that is not quite the inverse, undo starts corrupting the drawing
//! instead of restoring it — and does so *silently*, several steps later.
//!
//! This keeps **whole drawings**. Before each change, the current one is
//! pushed. Undo is then not a computation at all:
//!
//! ```text
//!     undo   =   go back to a drawing that definitely existed
//! ```
//!
//! There is no inverse to get wrong, and a new kind of edit needs no undo code
//! written for it at all — which is the property that matters most while the
//! editor is still growing.
//!
//! ## What it costs
//!
//! A mark is a few hundred points; a drawing is a few hundred marks; a
//! snapshot is therefore some tens of thousands of numbers, and [`DEPTH`] of
//! them is a few megabytes at the very worst. That is a good trade for undo
//! that cannot be wrong. If a drawing ever gets big enough for it to hurt, the
//! fix is to store the marks behind a shared pointer so that unchanged ones
//! are shared between snapshots rather than copied — which changes the cost
//! without changing the idea.
//!
//! ## The redo rule
//!
//! Doing something new after undoing **throws the redo away**. That is not a
//! limitation, it is the only coherent answer: the future you had undone is no
//! longer reachable from where you now are, and offering to redo into it would
//! graft two different drawings together.

use crate::sheet::Sheet;

/// How many steps back it is possible to go.
pub const DEPTH: usize = 64;

/// The drawings there have been.
#[derive(Clone, Debug, Default)]
pub struct History {
    past: Vec<Sheet>,
    ahead: Vec<Sheet>,
}

impl History {
    pub fn new() -> History {
        History { past: Vec::new(), ahead: Vec::new() }
    }

    /// Record the drawing as it is **before** it is changed.
    ///
    /// Called at the start of every edit, which is the one rule the rest of
    /// the editor has to remember.
    pub fn remember(&mut self, sheet: &Sheet) {
        self.past.push(sheet.clone());
        if self.past.len() > DEPTH {
            self.past.remove(0);
        }
        // Anything that was undone is now unreachable: this is a different
        // future from the one that was put aside.
        self.ahead.clear();
    }

    /// Step back. Hands over the drawing to restore, and keeps the current one
    /// so it can be stepped forward into again.
    pub fn undo(&mut self, now: &Sheet) -> Option<Sheet> {
        let was = self.past.pop()?;
        self.ahead.push(now.clone());
        Some(was)
    }

    /// Step forward again.
    pub fn redo(&mut self, now: &Sheet) -> Option<Sheet> {
        let next = self.ahead.pop()?;
        self.past.push(now.clone());
        Some(next)
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.ahead.is_empty()
    }

    /// Forget everything, for when a different drawing is opened.
    ///
    /// Without this, undo after opening a file walks back into the *previous*
    /// drawing, which is alarming.
    pub fn forget(&mut self) {
        self.past.clear();
        self.ahead.clear();
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Mark;
    use plotkit::Cx;
    use shapes::Nib;

    fn mark(x: f64) -> Mark {
        Mark::new(vec![Cx::new(x, 0.0), Cx::new(x + 1.0, 1.0)], Nib::Round(0.2), 0xFFFFFF)
    }

    /// A drawing of `n` marks.
    fn sheet(n: usize) -> Sheet {
        let mut s = Sheet::new();
        for k in 0..n {
            s.add(mark(k as f64));
        }
        s
    }

    /// ★ Undo goes back to a drawing that definitely existed, rather than
    /// computing an inverse that might not be one. That is the whole design,
    /// and it is why a new kind of edit needs no undo code written for it.
    #[test]
    fn undo_restores_a_drawing_that_really_existed() {
        let mut h = History::new();
        let one = sheet(1);
        h.remember(&one);
        let two = sheet(2);

        assert_eq!(h.undo(&two), Some(one.clone()));
        assert_eq!(h.redo(&one), Some(two));
    }

    /// ★ Doing something new after undoing throws the redo away. The future
    /// that was put aside is no longer reachable from here, and offering to
    /// redo into it would graft two different drawings together.
    #[test]
    fn a_new_edit_after_undoing_abandons_the_future() {
        let mut h = History::new();
        h.remember(&sheet(1));
        let two = sheet(2);
        let back = h.undo(&two).expect("a step back");
        assert!(h.can_redo());

        h.remember(&back); // something new instead
        assert!(!h.can_redo(), "the abandoned future should be gone");
    }

    /// Undo and redo walk a chain in both directions and end up where they
    /// started — the property people actually rely on when they are unsure.
    #[test]
    fn walking_back_and_forward_returns_the_same_drawing() {
        let mut h = History::new();
        let steps: Vec<Sheet> = (0..5).map(sheet).collect();
        for s in &steps[..4] {
            h.remember(s);
        }

        let mut now = steps[4].clone();
        for want in steps[..4].iter().rev() {
            now = h.undo(&now).expect("a step back");
            assert_eq!(&now, want);
        }
        for want in &steps[1..] {
            now = h.redo(&now).expect("a step forward");
            assert_eq!(&now, want);
        }
        assert_eq!(now, steps[4]);
    }

    /// ★ Undoing at the beginning is nothing, not a panic or an empty page.
    /// People press it repeatedly on purpose, to get as far back as they can.
    #[test]
    fn undoing_past_the_beginning_is_simply_nothing() {
        let mut h = History::new();
        let now = sheet(3);
        assert_eq!(h.undo(&now), None);
        assert!(!h.can_undo());
        for _ in 0..20 {
            assert_eq!(h.undo(&now), None);
        }
        assert_eq!(h.redo(&now), None);
    }

    /// It forgets the oldest rather than growing forever, and the recent
    /// steps — the ones anybody actually uses — all survive.
    #[test]
    fn it_remembers_a_bounded_number_of_steps() {
        let mut h = History::new();
        for k in 0..DEPTH * 2 {
            h.remember(&sheet(k % 7 + 1));
        }
        let mut now = sheet(1);
        let mut steps = 0;
        while let Some(back) = h.undo(&now) {
            now = back;
            steps += 1;
        }
        assert_eq!(steps, DEPTH, "it should keep exactly the last {DEPTH}");
    }

    /// ★ Opening a different drawing forgets the old one. Without this, undo
    /// after opening a file walks back into the drawing you had before, which
    /// is alarming and looks like data loss even though it is not.
    #[test]
    fn opening_another_drawing_does_not_leave_the_old_one_reachable() {
        let mut h = History::new();
        h.remember(&sheet(3));
        h.forget();
        assert!(!h.can_undo() && !h.can_redo());
    }
}
