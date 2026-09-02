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
//! ## Why it is a flat list
//!
//! No rows, no groups, no layout engine. The buttons are built with their
//! positions already worked out, because a layout that is computed twice —
//! once to paint and once to hit — is a layout that will one day disagree with
//! itself, and the symptom is a button that does the wrong thing near its
//! edge, which nobody can reproduce on purpose.

use plotkit::{Anchor, Canvas, Cx, Frame};
use shapes::Nib;

use crate::action::Action;
use crate::board::{Board, Tool};

/// What pressing a button means.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cmd {
    Colour(u32),
    Nib(usize),
    Use(Tool),
    /// Add this to what the selected mark does.
    Do(Action),
    /// Take away everything the selected mark does.
    Stop,
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

/// The colours on offer.
pub const INKS: [u32; 8] =
    [0xE3E9EF, 0xE0A44A, 0x4FBCD4, 0xE585AC, 0x6FCF97, 0x9B7BD4, 0xE0704A, 0x46525E];

/// The three nibs, by the index [`Cmd::Nib`] carries.
pub const NIBS: [&str; 3] = ["quill", "round", "broad"];

/// How wide the bar is, in pixels. The window keeps the drawing clear of it.
pub const WIDTH: i32 = 132;

/// How long one press of an action button lasts, in seconds.
///
/// The bar's rates are chosen so a whole number of cycles fits in it, because
/// an act loops and a cycle that does not close jerks every time round.
pub const STEP: f64 = 2.0;

const PAD: i32 = 8;
const ROW: i32 = 22;
const SWATCH: i32 = 26;

/// The toolbar.
#[derive(Clone, Debug, Default)]
pub struct Bar {
    pub buttons: Vec<Button>,
}

impl Bar {
    /// Build the bar. Positions are worked out **once**, here, and both the
    /// painting and the hit testing read them.
    pub fn new() -> Bar {
        let mut b = Bar { buttons: Vec::new() };
        let mut y = PAD;

        // --- colours, four to a row -----------------------------------------
        for (k, ink) in INKS.iter().enumerate() {
            let (col, row) = (k % 4, k / 4);
            b.buttons.push(Button {
                x: PAD + col as i32 * (SWATCH + 4),
                y: y + row as i32 * (SWATCH + 4),
                w: SWATCH,
                h: SWATCH,
                label: "",
                swatch: Some(*ink),
                cmd: Cmd::Colour(*ink),
            });
        }
        y += 2 * (SWATCH + 4) + PAD;

        // --- the nib ---------------------------------------------------------
        for (k, name) in NIBS.iter().enumerate() {
            b.buttons.push(Button {
                x: PAD + k as i32 * 39,
                y,
                w: 37,
                h: ROW,
                label: name,
                swatch: None,
                cmd: Cmd::Nib(k),
            });
        }
        y += ROW + PAD;

        // --- what a drag does -------------------------------------------------
        for (k, (name, tool)) in
            [("draw", Tool::Draw), ("pick", Tool::Pick), ("rub", Tool::Erase)].into_iter().enumerate()
        {
            b.buttons.push(Button {
                x: PAD + k as i32 * 39,
                y,
                w: 37,
                h: ROW,
                label: name,
                swatch: None,
                cmd: Cmd::Use(tool),
            });
        }
        y += ROW + PAD;

        // --- what the chosen mark does ---------------------------------------
        // A sensible default for each, so one press does something worth
        // watching. The numbers can be tuned afterwards; an empty animation
        // that needs six decisions before it moves is one nobody makes.
        // Rates chosen so that a whole number of cycles fits in [`STEP`]
        // seconds. An act loops by default, so a rate that does not close
        // leaves a visible jerk every time round — a spin that has got
        // four fifths of the way and snaps back.
        let right = Cx::new(1.6, 0.0);
        let acts: [(&'static str, Action); 6] = [
            ("walk", Action::Walk(right)),
            ("run", Action::Run(right)),
            ("jump", Action::Jump { height: 1.2, rate: 1.5 }),
            ("spin", Action::Spin(0.5)),
            ("bob", Action::Bob { height: 0.4, rate: 0.5 }),
            ("pulse", Action::Pulse { amount: 0.25, rate: 0.5 }),
        ];
        for (k, (name, action)) in acts.into_iter().enumerate() {
            let (col, row) = (k % 3, k / 3);
            b.buttons.push(Button {
                x: PAD + col as i32 * 39,
                y: y + row as i32 * (ROW + 4),
                w: 37,
                h: ROW,
                label: name,
                swatch: None,
                cmd: Cmd::Do(action),
            });
        }
        y += 2 * (ROW + 4) + PAD;

        // --- the clock --------------------------------------------------------
        for (k, (name, cmd)) in
            [("play", Cmd::Play), ("stop", Cmd::Pause), ("|<<", Cmd::Rewind)].into_iter().enumerate()
        {
            b.buttons.push(Button { x: PAD + k as i32 * 39, y, w: 37, h: ROW, label: name, swatch: None, cmd });
        }
        y += ROW + 4;
        b.buttons.push(Button {
            x: PAD,
            y,
            w: 37 + 39,
            h: ROW,
            label: "no act",
            swatch: None,
            cmd: Cmd::Stop,
        });
        y += ROW + PAD;

        // --- the page ---------------------------------------------------------
        for (k, (name, cmd)) in [
            ("undo", Cmd::Undo),
            ("redo", Cmd::Redo),
            ("even", Cmd::Smooth),
            ("save", Cmd::Save),
            ("open", Cmd::Open),
            ("clear", Cmd::Clear),
        ]
        .into_iter()
        .enumerate()
        {
            let (col, row) = (k % 3, k / 3);
            b.buttons.push(Button {
                x: PAD + col as i32 * 39,
                y: y + row as i32 * (ROW + 4),
                w: 37,
                h: ROW,
                label: name,
                swatch: None,
                cmd,
            });
        }

        b
    }

    /// What was pressed at this pixel, if anything.
    pub fn at(&self, px: f64, py: f64) -> Option<Cmd> {
        self.buttons.iter().find(|b| b.holds(px, py)).map(|b| b.cmd)
    }

    /// Is this pixel over the bar at all?
    ///
    /// Wider than the buttons on purpose. The gaps between them are still the
    /// bar, and a stroke started in a gap would be drawn *underneath* the
    /// toolbar where it cannot be seen or picked up again.
    pub fn covers(&self, px: f64, _py: f64) -> bool {
        px < WIDTH as f64
    }

    /// How far down the bar reaches, for drawing its backing.
    pub fn depth(&self) -> i32 {
        self.buttons.iter().map(|b| b.y + b.h).max().unwrap_or(0) + PAD
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
            Cmd::Colour(c) => c == board.colour,
            Cmd::Use(t) => t == board.tool,
            Cmd::Nib(k) => k == Bar::nib_index(board.nib),
            Cmd::Play => board.playing,
            Cmd::Pause => !board.playing,
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
    pub fn paint(&self, f: &mut Frame, board: &Board, tall: i32) {
        let deep = self.depth().max(tall);
        f.chip(0, 0, WIDTH, deep, PANEL);
        f.chip(WIDTH - 1, 0, 1, deep, EDGE);

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

    /// ★ Painting and hit testing must agree exactly, and the way to be sure
    /// is for there to be only one piece of arithmetic. Every pixel inside a
    /// button's own rectangle must find that button — the alternative is an
    /// edge that does the wrong thing, which nobody can reproduce on purpose.
    #[test]
    fn every_pixel_of_a_button_finds_that_button() {
        let bar = Bar::new();
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

    /// ★ And no two buttons may claim the same pixel — with a half-open test,
    /// buttons that touch cannot both own the join. Overlapping ones would
    /// make the second unreachable, and only sometimes.
    #[test]
    fn no_two_buttons_overlap() {
        let bar = Bar::new();
        for (k, a) in bar.buttons.iter().enumerate() {
            for b in &bar.buttons[k + 1..] {
                let apart = a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y;
                assert!(apart, "{:?} and {:?} overlap", a.cmd, b.cmd);
            }
        }
    }

    /// Just outside a button is nothing, rather than the neighbour.
    #[test]
    fn just_outside_a_button_is_nothing_in_particular() {
        let bar = Bar::new();
        let first = bar.buttons[0].clone();
        assert_ne!(bar.at((first.x - 1) as f64, first.y as f64), Some(first.cmd));
        assert_ne!(bar.at(first.x as f64, (first.y - 1) as f64), Some(first.cmd));
    }

    /// ★ The gaps between buttons are still the bar. A stroke begun in a gap
    /// would be drawn *underneath* the toolbar, where it can be neither seen
    /// nor picked up again — which looks like the pen having stopped working.
    #[test]
    fn the_gaps_between_buttons_are_still_the_toolbar() {
        let bar = Bar::new();
        assert!(bar.covers(4.0, 4.0), "the margin above the first button");
        assert!(bar.covers(WIDTH as f64 - 1.0, 300.0), "and the strip beside them");
        assert!(!bar.covers(WIDTH as f64 + 1.0, 300.0), "but the page beyond it is the page");
        // Every button is inside the covered strip, or it could be pressed
        // while the pen also drew on the paper behind it.
        for b in &bar.buttons {
            assert!(bar.covers((b.x + b.w - 1) as f64, b.y as f64), "{:?} sticks out of the bar", b.cmd);
        }
    }

    /// Every colour on offer has a swatch, and every swatch sets that colour —
    /// a swatch painted one colour and setting another is a special kind of
    /// maddening.
    #[test]
    fn a_swatch_sets_the_colour_it_is_painted() {
        let bar = Bar::new();
        let swatches: Vec<&Button> = bar.buttons.iter().filter(|b| b.swatch.is_some()).collect();
        assert_eq!(swatches.len(), INKS.len());
        for b in swatches {
            assert_eq!(b.cmd, Cmd::Colour(b.swatch.expect("a swatch")));
        }
    }

    /// The bar offers everything the keyboard does, so nothing is reachable
    /// only by knowing a secret.
    #[test]
    fn the_bar_can_reach_every_command() {
        let bar = Bar::new();
        let has = |c: Cmd| bar.buttons.iter().any(|b| b.cmd == c);
        for c in [Cmd::Play, Cmd::Pause, Cmd::Rewind, Cmd::Undo, Cmd::Redo, Cmd::Save, Cmd::Open, Cmd::Clear, Cmd::Smooth, Cmd::Stop] {
            assert!(has(c), "{c:?} is not on the bar");
        }
        for k in 0..NIBS.len() {
            assert!(has(Cmd::Nib(k)));
        }
        for t in [Tool::Draw, Tool::Pick, Tool::Erase] {
            assert!(has(Cmd::Use(t)));
        }
        assert!(bar.buttons.iter().filter(|b| matches!(b.cmd, Cmd::Do(_))).count() >= 6, "and the actions");
    }

    /// Every action button carries a default worth watching, rather than a
    /// zero that does nothing and looks broken.
    #[test]
    fn every_action_button_actually_moves_something() {
        let bar = Bar::new();
        for b in &bar.buttons {
            let Cmd::Do(action) = b.cmd else { continue };
            let after = action.at(0.37);
            let moved = (after.b).abs() > 1e-3 || (after.a - plotkit::Cx::ONE).abs() > 1e-3;
            assert!(moved, "{} does nothing", b.label);
        }
    }

    /// ★ The rates must close in [`STEP`] seconds. An act loops, so a cycle
    /// that has got four fifths of the way round when the step ends snaps back
    /// — a jerk every two seconds, for ever, which reads as the animation
    /// being broken rather than as a number being slightly wrong.
    #[test]
    fn a_cycling_action_comes_round_by_the_end_of_its_step() {
        for b in &Bar::new().buttons {
            let Cmd::Do(action) = b.cmd else { continue };
            // The ones that go somewhere are not expected to come back; the
            // ones that cycle in place are.
            if matches!(action, Action::Walk(_) | Action::Run(_) | Action::Drift(_)) {
                continue;
            }
            let start = action.at(0.0);
            let end = action.at(STEP);
            assert!((end.a - start.a).abs() < 1e-6, "{} does not come round: {:?}", b.label, end.a);
            assert!((end.b - start.b).abs() < 1e-6, "{} does not come back: {:?}", b.label, end.b);
        }
    }
}
