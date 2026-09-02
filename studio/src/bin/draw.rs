//! # draw — the drawing program
//!
//! ```text
//!     cargo run -p studio --release --bin draw
//!     cargo run -p studio --release --bin draw -- mine.easel
//! ```
//!
//! ## The pen
//!
//! ```text
//!     drag                 draw
//!     Shift + drag         move the paper
//!     wheel                zoom about the pointer
//! ```
//!
//! Nothing is bound to the right button, and nothing ever will be: on a pen
//! and a trackpad it does not arrive at all. Shift is the modifier, and it
//! already means *"talk to the graph rather than to the drawing"* everywhere
//! else in this repository.
//!
//! ## The keys
//!
//! ```text
//!     1 2 3        the nib: quill, round, broad
//!     [ ]          thinner / thicker
//!     , .          the broad nib's angle
//!     - =          less / more spring -- how hard the ink follows your hand
//!     T            taper on and off
//!     C            the next colour
//!
//!     D E P        draw, erase, pick things up
//!     U R          undo, redo
//!     F            smooth every closed mark -- press again to go further
//!     X            clear the page (undoable)
//!     S O          save, open
//! ```
//!
//! ## Where the work happens
//!
//! Not here. This file is a window and a set of key bindings; the drawing
//! program is [`easel`], which has no window in it and is therefore tested in
//! the dark — the press-drag-release logic, the spring on the ink, undo, the
//! file format and the Fourier dial all have tests that never open anything.
//!
//! ```text
//!     easel/       what a drawing is and what editing it means
//!     studio/      this: a window, a pointer and some keys
//! ```
//!
//! Same arrangement as `world` and `live`, and for the same reason: the moment
//! logic needs a window to run, it stops being checkable.
//!
//! ## What the pen can and cannot tell us
//!
//! No pressure and no tilt reach a program through this window, and none ever
//! will. Weight comes from the stroke instead — the **quill** nib is thin when
//! your hand is quick, because the points arrive one per frame and so the gap
//! between them is the speed. The **broad** nib is a segment held at a fixed
//! angle, which is calligraphy and needs no pressure at all.

use easel::{Board, Tool};
use plotkit::{Anchor, Cx};
use shapes::Nib;
use studio::Graph;

/// Somewhere to keep the things the window cares about but the board does not.
struct Pad {
    board: Board,
    /// How far the smoothing dial has been turned, in presses of `F`.
    ///
    /// It counts **down** from a lot of harmonics to a few, so each press
    /// takes a little more of your hand out and you can stop where you like.
    cut: usize,
    file: String,
    say: String,
}

const INKS: [u32; 7] = [0xE3E9EF, 0xE0A44A, 0x4FBCD4, 0xE585AC, 0x6FCF97, 0x9B7BD4, 0x46525E];

impl Pad {
    fn new(file: String) -> Pad {
        let mut pad = Pad { board: Board::new(), cut: 24, file, say: String::new() };
        if std::path::Path::new(&pad.file).exists() {
            pad.open();
        }
        pad
    }

    fn open(&mut self) {
        self.say = match self.board.load(&self.file) {
            Ok(0) => format!("opened {} -- {} marks", self.file, self.board.sheet.len()),
            Ok(bad) => format!("opened {} -- {} marks, {bad} lines made no sense", self.file, self.board.sheet.len()),
            Err(e) => format!("could not open {}: {e}", self.file),
        };
        self.cut = 24;
    }

    fn save(&mut self) {
        self.say = match self.board.save(&self.file) {
            Ok(()) => format!("saved {} -- {} marks", self.file, self.board.sheet.len()),
            Err(e) => format!("could not save {}: {e}", self.file),
        };
    }

    /// Change the nib, keeping the width it already had.
    fn width(&self) -> f64 {
        match self.board.nib {
            Nib::Round(w) => w,
            Nib::Quill { slow, .. } => slow,
            Nib::Broad { width, .. } => width,
        }
    }

    fn resize(&mut self, by: f64) {
        let w = (self.width() * by).clamp(0.01, 3.0);
        self.board.nib = match self.board.nib {
            Nib::Round(_) => Nib::Round(w),
            Nib::Quill { pace, .. } => Nib::Quill { slow: w, fast: w * 0.15, pace },
            Nib::Broad { angle, .. } => Nib::Broad { width: w, angle },
        };
    }

    fn nib_name(&self) -> String {
        match self.board.nib {
            Nib::Round(w) => format!("round {w:.2}"),
            Nib::Quill { slow, .. } => format!("quill {slow:.2} -- thin when quick"),
            Nib::Broad { width, angle } => format!("broad {width:.2} at {:.0} deg", angle.to_degrees()),
        }
    }
}

fn main() {
    let file = std::env::args().nth(1).unwrap_or_else(|| "drawing.easel".to_string());

    Graph::new("draw")
        .scale(70.0)
        .with(Pad::new(file))
        // The pen. `down` is already false while the graph is panning, so
        // shift-dragging the paper does not leave a stroke across it.
        .on_pointer(|p, at, down| p.board.pointer(at, down))
        // --- the nib ---------------------------------------------------------
        .on('1', |p| {
            let w = p.width();
            p.board.nib = Nib::Quill { slow: w, fast: w * 0.15, pace: 0.16 };
        })
        .on('2', |p| p.board.nib = Nib::Round(p.width()))
        .on('3', |p| {
            let w = p.width();
            p.board.nib = Nib::Broad { width: w, angle: std::f64::consts::PI / 4.0 };
        })
        .on_hold('[', |p| p.resize(0.97))
        .on_hold(']', |p| p.resize(1.03))
        .on_hold(',', |p| {
            if let Nib::Broad { width, angle } = p.board.nib {
                p.board.nib = Nib::Broad { width, angle: angle - 0.03 };
            }
        })
        .on_hold('.', |p| {
            if let Nib::Broad { width, angle } = p.board.nib {
                p.board.nib = Nib::Broad { width, angle: angle + 0.03 };
            }
        })
        .on_hold('-', |p| p.board.pull = (p.board.pull * 1.03).min(1.0))
        .on_hold('=', |p| p.board.pull = (p.board.pull * 0.97).max(0.03))
        .on('t', |p| p.board.taper = if p.board.taper > 0.0 { 0.0 } else { 0.15 })
        .on('c', |p| {
            let next = INKS.iter().position(|c| *c == p.board.colour).map_or(0, |k| (k + 1) % INKS.len());
            p.board.colour = INKS[next];
        })
        // --- what a drag means ----------------------------------------------
        .on('d', |p| p.board.tool = Tool::Draw)
        .on('e', |p| p.board.tool = Tool::Erase)
        .on('p', |p| p.board.tool = Tool::Pick)
        // --- the page --------------------------------------------------------
        .on('u', |p| {
            p.say = if p.board.undo() { "undone".into() } else { "nothing left to undo".into() };
        })
        .on('r', |p| {
            p.say = if p.board.redo() { "redone".into() } else { "nothing to redo".into() };
        })
        .on('f', |p| {
            // Each press takes a little more of your hand out. It stops at 1,
            // which is a single harmonic -- a perfect circle, and as far as
            // this dial goes.
            p.cut = p.cut.saturating_sub(3).max(1);
            p.board.smooth_all(p.cut);
            p.say = format!("smoothed: keeping waves up to pitch {}", p.cut);
        })
        .on('x', |p| {
            p.board.clear();
            p.say = "cleared -- U puts it back".into();
        })
        .on('s', Pad::save)
        .on('o', Pad::open)
        .run(page);
}

fn page(p: &Pad) -> plotkit::Frame {
    let mut f = p.board.frame();

    let tool = match p.board.tool {
        Tool::Draw => "draw",
        Tool::Erase => "erase",
        Tool::Pick => "pick up",
    };
    f.pin(Anchor::TopLeft, 14.0, 14.0, format!("{tool}   {}", p.nib_name()), p.board.colour, 2);
    f.pin(
        Anchor::TopLeft,
        14.0,
        32.0,
        format!(
            "spring {:.2}   taper {}   {} marks   {}{}",
            p.board.pull,
            if p.board.taper > 0.0 { "on" } else { "off" },
            p.board.sheet.len(),
            if p.board.can_undo() { "U" } else { "-" },
            if p.board.can_redo() { "R" } else { "-" },
        ),
        0x94A1AE,
        2,
    );
    f.pin(Anchor::BottomLeft, 14.0, -34.0, "1 2 3 nib   [ ] size   , . angle   - = spring   T taper   C colour", 0x6B7987, 2);
    f.pin(Anchor::BottomLeft, 14.0, -16.0, "D draw  E erase  P pick   U undo  R redo  F smooth  X clear  S save  O open   shift-drag moves the paper", 0x6B7987, 2);
    if !p.say.is_empty() {
        f.pin(Anchor::BottomRight, -14.0, -16.0, &p.say, 0x6FCF97, 2);
    }

    // A cross at the origin, so there is somewhere to be when the page is
    // blank and you have panned away from everything.
    if p.board.sheet.is_empty() {
        f.add(plotkit::Shape::path(vec![Cx::new(-0.2, 0.0), Cx::new(0.2, 0.0)])).color(0x22303C).width(1);
        f.add(plotkit::Shape::path(vec![Cx::new(0.0, -0.2), Cx::new(0.0, 0.2)])).color(0x22303C).width(1);
    }
    f
}
