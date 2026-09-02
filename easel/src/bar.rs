//! # bar — the toolbar, as data
//!
//! A toolbar is buttons, and a button is a rectangle that means something. Put
//! that in the window and it becomes untestable — you would be reduced to
//! looking at it and clicking about. So it lives here as a list of rectangles
//! and what each one means, and the window only paints them and reports where
//! it was tapped.
//!
//! ```text
//!     bar.at(px, py)  ->  Some(Cmd::Colour(0xE0A44A))
//! ```
//!
//! ## What is here, and what is not
//!
//! Here: things that are **modes of the program** — which nib, what a drag
//! does, the clock, undo, saving.
//!
//! Not here: **colour**, and what a shape *does*. Those belong to a particular
//! shape, and a shape is chosen in the tree, so they appear there beside it —
//! see [`tree`](crate::tree). A global "current colour" can only ever mean
//! *the colour of the next stroke*, which is a strange thing to have on a
//! toolbar when what you usually want is to change the colour of one already
//! drawn.
//!
//! ## Across the top, not down the side
//!
//! The left belongs to the [`tree`](crate::tree) — the list of what the
//! drawing is made of, which is the thing you look at while working and the
//! thing that grows. Tools are a fixed set that never grows, so they go across
//! the top where a fixed set fits.
//!
//! The buttons **flow**: they are pushed in order and wrap when they run out
//! of width, with a gap between one kind and the next. So the bar rearranges
//! itself for a narrow window instead of running off the edge, and adding a
//! button later needs no arithmetic redone.
//!
//! ## Pixels, in a crate that otherwise refuses to know about them
//!
//! Everywhere else in [`easel`](crate) the world is measured in world units,
//! because the drawing is. A toolbar is the exception and honestly so: it does
//! not live *in* the drawing. It stays the same size when you zoom, it does
//! not move when you pan, and a button is a thing the eye judges in pixels. A
//! toolbar measured in world units would grow to fill the screen the moment
//! you zoomed in.
//!
//! So the rectangles are in pixels, from the **top left of the window**. Not
//! from an anchor: hit testing has to agree exactly with painting, and the
//! surest way to make two pieces of arithmetic agree is for there to be only
//! one of them.
//!
//! ## How big a button has to be
//!
//! Bigger than looks necessary. A pen tip covers a couple of pixels and a
//! fingertip covers forty, and neither lands where you think it did — a pen
//! held at an angle reports a point offset from where the nib appears to be,
//! and a finger reports the middle of a contact patch you cannot see. The
//! usual guidance for a touch target is about 9 mm square, which on an
//! ordinary screen is around **44 pixels**.
//!
//! The first version of this bar had 22-pixel rows, and the complaint was
//! immediate and correct: too many taps to hit the right thing. Everything
//! here is now at least [`TAP`] across, which costs width and is worth it —
//! a button you miss is worse than a button you have to scroll to.
//!
//! ## Why it is a flat list
//!
//! No rows, no groups, no layout engine. The buttons are built with their
//! positions already worked out, because a layout that is computed twice —
//! once to paint and once to hit — is a layout that will one day disagree with
//! itself, and the symptom is a button that does the wrong thing near its
//! edge, which nobody can reproduce on purpose.

use plotkit::{Anchor, Canvas, Frame};
use shapes::Nib;

use crate::board::{Board, Tool};

/// What pressing a button means.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cmd {
    Nib(usize),
    Use(Tool),
    /// Bind the chosen marks into one figure.
    Group,
    /// Break a figure up again.
    Ungroup,
    /// Leave a key at the clock.
    Key,
    /// Take the key at the clock away.
    Unkey,
    /// Step the clock to the next key, or the previous one.
    Step(bool),
    Play,
    Pause,
    Rewind,
    Undo,
    Redo,
    Smooth,
    Clear,
    Save,
    Open,
}

/// One rectangle that means something.
#[derive(Clone, Debug, PartialEq)]
pub struct Button {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub label: &'static str,
    /// The swatch colour, or `None` for a plain button.
    pub swatch: Option<u32>,
    pub cmd: Cmd,
}

impl Button {
    pub fn holds(&self, px: f64, py: f64) -> bool {
        // Half open, so two buttons that touch cannot both claim the pixel on
        // the join. The same rule as the scanline fill, for the same reason.
        px >= self.x as f64 && px < (self.x + self.w) as f64 && py >= self.y as f64 && py < (self.y + self.h) as f64
    }
}

/// The three nibs, by the index [`Cmd::Nib`] carries.
pub const NIBS: [&str; 3] = ["quill", "round", "broad"];

/// The smallest a button may be, in pixels.
///
/// About 9 mm on an ordinary screen, which is the usual guidance for
/// something meant to be hit with a finger. A pen is more precise but not as
/// much more as it feels: held at an angle it reports a point offset from
/// where the nib looks to be.
pub const TAP: i32 = 40;

/// How long one press of an action button lasts, in seconds.
///
/// The bar's rates are chosen so a whole number of cycles fits in it, because
/// an act loops and a cycle that does not close jerks every time round.
pub const STEP: f64 = 2.0;

const PAD: i32 = 8;
/// Gap between buttons: enough that a near miss lands on nothing rather than
/// on the neighbour, which is the worse of the two failures.
const GAP: i32 = 5;
/// The gap between one kind of button and the next.
const BREAK: i32 = 18;
const WIDE: i32 = 54;

/// The toolbar.
#[derive(Clone, Debug, Default)]
pub struct Bar {
    pub buttons: Vec<Button>,
    /// Where it starts and how far down it reaches.
    pub x: i32,
    pub depth: i32,
}

/// Lays buttons out left to right, wrapping.
struct Flow {
    x: i32,
    y: i32,
    left: i32,
    right: i32,
    out: Vec<Button>,
}

impl Flow {
    fn put(&mut self, w: i32, label: &'static str, swatch: Option<u32>, cmd: Cmd) {
        if self.x + w > self.right {
            self.x = self.left;
            self.y += TAP + GAP;
        }
        self.out.push(Button { x: self.x, y: self.y, w, h: TAP, label, swatch, cmd });
        self.x += w + GAP;
    }

    /// A gap between one kind of button and the next.
    fn gap(&mut self) {
        self.x += BREAK;
    }
}

impl Bar {
    /// Build the bar across the top, beside the tree. Positions are worked out
    /// **once**, here, and both the painting and the hit testing read them.
    pub fn new(window_w: i32) -> Bar {
        let left = crate::tree::WIDTH + PAD;
        let mut f = Flow { x: left, y: PAD, left, right: window_w - PAD, out: Vec::new() };

        for (k, name) in NIBS.iter().enumerate() {
            f.put(WIDE, name, None, Cmd::Nib(k));
        }
        f.gap();
        for (name, tool) in [("draw", Tool::Draw), ("pick", Tool::Pick), ("rub", Tool::Erase)] {
            f.put(WIDE, name, None, Cmd::Use(tool));
        }
        f.gap();
        // No colours and no actions here. Both belong to a **particular
        // shape**, not to the program as a whole, so they live in the tree
        // beside whatever is chosen -- see `crate::tree::inspector`.

        for (name, cmd) in [("|<", Cmd::Step(false)), ("key", Cmd::Key), (">|", Cmd::Step(true))] {
            f.put(TAP, name, None, cmd);
        }
        f.put(WIDE, "unkey", None, Cmd::Unkey);
        f.gap();
        for (name, cmd) in [("play", Cmd::Play), ("stop", Cmd::Pause), ("|<<", Cmd::Rewind)] {
            f.put(WIDE, name, None, cmd);
        }
        f.gap();
        for (name, cmd) in [("group", Cmd::Group), ("split", Cmd::Ungroup)] {
            f.put(WIDE, name, None, cmd);
        }
        f.gap();
        for (name, cmd) in [
            ("undo", Cmd::Undo),
            ("redo", Cmd::Redo),
            ("even", Cmd::Smooth),
            ("save", Cmd::Save),
            ("open", Cmd::Open),
            ("clear", Cmd::Clear),
        ] {
            f.put(WIDE, name, None, cmd);
        }

        let depth = f.out.iter().map(|b| b.y + b.h).max().unwrap_or(0) + PAD;
        Bar { buttons: f.out, x: left - PAD, depth }
    }

    /// What was pressed at this pixel, if anything.
    pub fn at(&self, px: f64, py: f64) -> Option<Cmd> {
        self.buttons.iter().find(|b| b.holds(px, py)).map(|b| b.cmd)
    }

    /// Is this pixel over the bar at all?
    ///
    /// The whole strip, not just the buttons. The gaps between them are still
    /// the bar, and a stroke started in a gap would be drawn *underneath* the
    /// toolbar where it can be neither seen nor picked up again.
    pub fn covers(&self, px: f64, py: f64) -> bool {
        px >= self.x as f64 && py < self.depth as f64
    }

    /// Which nib button is the one in hand.
    pub fn nib_index(nib: Nib) -> usize {
        match nib {
            Nib::Quill { .. } => 0,
            Nib::Round(_) => 1,
            Nib::Broad { .. } => 2,
        }
    }

    /// Is this button showing the state the board is actually in?
    pub fn lit(&self, button: &Button, board: &Board) -> bool {
        match button.cmd {
            Cmd::Use(t) => t == board.tool,
            Cmd::Nib(k) => k == Bar::nib_index(board.nib),
            Cmd::Play => board.playing,
            Cmd::Pause => !board.playing,
            Cmd::Group => board.selected.len() >= 2,
            Cmd::Ungroup => board.chosen_groups() > 0,
            Cmd::Key | Cmd::Unkey => board.on_a_key(),
            _ => false,
        }
    }

    /// Paint the bar.
    ///
    /// **Here rather than in the window**, and that is not tidiness. Painting a
    /// button and deciding whether a tap hit it are the same rectangle; put
    /// them in two crates and they are two pieces of arithmetic that have to
    /// agree, which one day they will not. The symptom is a button that
    /// misbehaves near its edge, and nobody can reproduce it on purpose.
    ///
    /// So the window's whole part in the toolbar is one call to this and one
    /// call to [`Bar::at`].
    pub fn paint(&self, f: &mut Frame, board: &Board, window_w: i32) {
        f.chip(self.x, 0, window_w - self.x, self.depth, PANEL);
        f.chip(self.x, self.depth - 1, window_w - self.x, 1, EDGE);

        for b in &self.buttons {
            let lit = self.lit(b, board);
            match b.swatch {
                // A swatch cannot show it is chosen by changing colour --
                // that is the one thing it must not do -- so it grows a
                // surround instead.
                Some(ink) => {
                    if lit {
                        f.chip(b.x - 2, b.y - 2, b.w + 4, b.h + 4, INK_TEXT);
                    }
                    f.chip(b.x, b.y, b.w, b.h, ink);
                }
                None => {
                    f.chip(b.x, b.y, b.w, b.h, if lit { LIT } else { EDGE });
                    let indent = ((b.w - Canvas::text_w(b.label, 1)) / 2).max(2);
                    f.pin(
                        Anchor::TopLeft,
                        (b.x + indent) as f64,
                        (b.y + (b.h - 7) / 2) as f64,
                        b.label,
                        if lit { 0xFFFFFF } else { INK_TEXT },
                        1,
                    );
                }
            }
        }
    }
}

const PANEL: u32 = 0x141C26;
const EDGE: u32 = 0x22303C;
const LIT: u32 = 0x2E4257;
const INK_TEXT: u32 = 0xC3CDD7;

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    const WIN: i32 = 1400;

    /// ★ Painting and hit testing must agree exactly, and the way to be sure
    /// is for there to be only one piece of arithmetic. Every pixel inside a
    /// button's own rectangle must find that button -- the alternative is an
    /// edge that does the wrong thing, which nobody can reproduce on purpose.
    #[test]
    fn every_pixel_of_a_button_finds_that_button() {
        let bar = Bar::new(WIN);
        for b in &bar.buttons {
            for (px, py) in [
                (b.x, b.y),
                (b.x + b.w - 1, b.y),
                (b.x, b.y + b.h - 1),
                (b.x + b.w - 1, b.y + b.h - 1),
                (b.x + b.w / 2, b.y + b.h / 2),
            ] {
                assert_eq!(bar.at(px as f64, py as f64), Some(b.cmd), "{:?} missed at ({px}, {py})", b.cmd);
            }
        }
    }

    /// ★ The buttons FLOW: pushed in order, wrapping when they run out of
    /// width. So a narrow window rearranges the bar instead of running it off
    /// the edge, and adding a button later needs no arithmetic redone.
    #[test]
    fn the_buttons_wrap_rather_than_running_off_the_edge() {
        for width in [900, 1200, 1400, 1920] {
            let bar = Bar::new(width);
            for b in &bar.buttons {
                assert!(b.x + b.w <= width, "at {width} wide, {:?} runs off the edge", b.cmd);
                assert!(b.x >= crate::tree::WIDTH, "{:?} is over the tree", b.cmd);
            }
        }
        // Narrower means more rows, not a wider bar.
        assert!(Bar::new(900).depth > Bar::new(1920).depth);
    }

    /// No two buttons may claim the same pixel -- overlapping ones make the
    /// second unreachable, and only sometimes.
    #[test]
    fn no_two_buttons_overlap() {
        let bar = Bar::new(WIN);
        for (k, a) in bar.buttons.iter().enumerate() {
            for b in &bar.buttons[k + 1..] {
                let apart = a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y;
                assert!(apart, "{:?} and {:?} overlap", a.cmd, b.cmd);
            }
        }
    }

    /// ★ Every button must be big enough to hit. A pen held at an angle
    /// reports a point offset from where its nib appears to be, and a finger
    /// reports the middle of a contact patch you cannot see.
    #[test]
    fn every_button_is_big_enough_to_hit() {
        for b in &Bar::new(WIN).buttons {
            assert!(b.h >= TAP, "{:?} is only {} tall", b.cmd, b.h);
            assert!(b.w >= TAP, "{:?} is only {} wide", b.cmd, b.w);
        }
    }

    /// Just outside a button is nothing, rather than the neighbour.
    #[test]
    fn just_outside_a_button_is_nothing_in_particular() {
        let bar = Bar::new(WIN);
        let first = bar.buttons[0].clone();
        assert_ne!(bar.at((first.x - 1) as f64, first.y as f64), Some(first.cmd));
    }

    /// ★ The whole strip is the bar, not just the buttons. A stroke begun in
    /// a gap would be drawn *underneath* the toolbar, where it can be neither
    /// seen nor picked up -- which looks like the pen having stopped working.
    #[test]
    fn the_gaps_between_buttons_are_still_the_toolbar() {
        let bar = Bar::new(WIN);
        assert!(bar.covers((crate::tree::WIDTH + 4) as f64, 4.0), "the margin above the first button");
        assert!(bar.covers((WIN - 4) as f64, 4.0), "and the empty end of a row");
        assert!(!bar.covers((WIN - 4) as f64, (bar.depth + 1) as f64), "but the paper below it is the paper");
        assert!(!bar.covers(10.0, 4.0), "and the tree beside it is the tree");
        for b in &bar.buttons {
            assert!(bar.covers((b.x + b.w - 1) as f64, (b.y + b.h - 1) as f64), "{:?} sticks out", b.cmd);
        }
    }

    /// ★ What is here is a mode of the PROGRAM. Colour and what a shape does
    /// belong to a particular shape, so they live in the tree beside it -- a
    /// global "current colour" can only mean the colour of the NEXT stroke,
    /// which is a strange thing to offer when you want to change one already
    /// drawn.
    #[test]
    fn the_bar_holds_modes_and_not_properties_of_a_shape() {
        let bar = Bar::new(WIN);
        let has = |c: Cmd| bar.buttons.iter().any(|b| b.cmd == c);
        for c in [
            Cmd::Play, Cmd::Pause, Cmd::Rewind, Cmd::Undo, Cmd::Redo, Cmd::Save, Cmd::Open, Cmd::Clear,
            Cmd::Smooth, Cmd::Group, Cmd::Ungroup, Cmd::Key, Cmd::Unkey, Cmd::Step(true), Cmd::Step(false),
        ] {
            assert!(has(c), "{c:?} is not on the bar");
        }
        for k in 0..NIBS.len() {
            assert!(has(Cmd::Nib(k)));
        }
        for t in [Tool::Draw, Tool::Pick, Tool::Erase] {
            assert!(has(Cmd::Use(t)));
        }
        assert!(bar.buttons.iter().all(|b| b.swatch.is_none()));
    }
}
