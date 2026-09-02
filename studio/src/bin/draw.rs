//! # draw — the studio
//!
//! ```text
//!     cargo run -p studio --release --bin draw
//!     cargo run -p studio --release --bin draw -- mine.easel
//! ```
//!
//! Draw shapes with the pen, give them something to do, press **play**, and
//! save the lot to a file that opens again and runs.
//!
//! ## The pen
//!
//! ```text
//!     drag                 draw, or pick up, or rub out
//!     Shift + drag         move the paper
//!     wheel                zoom about the pointer
//! ```
//!
//! Nothing is bound to the right button and nothing ever will be: on a pen and
//! a trackpad it does not arrive at all. Shift is the modifier, and it already
//! means *"talk to the graph rather than to the drawing"* everywhere else in
//! this repository.
//!
//! ## Choosing things
//!
//! ```text
//!     drag        draw / move / rub out, according to the tool
//!     tap         choose that shape -- whatever the tool is
//!     tap again   let go of it
//!     tap paper   choose nothing
//! ```
//!
//! No trip to the pick tool and back. A tap already made no mark, so the
//! gesture was going spare, and tapping **toggles** so several things can be
//! chosen without a modifier key — which matters with a pen, where there is
//! no second button.
//!
//! ## Making something move
//!
//! ```text
//!     1.  tap the strokes of a figure, then group    -- one tap takes it after that
//!     2.  walk / run / jump / spin / bob             -- each press adds a step
//!     3.  play
//! ```
//!
//! Presses **stack**: walk, then jump, then spin is a sequence, and each step
//! starts from wherever the last one left off — which is the only part of
//! animating that has any real content in it, and it lives in
//! [`easel::action`], tested without a window.
//!
//! Every step is two seconds and the whole act loops, so the buttons' rates
//! are chosen to come round exactly in that time — otherwise a spin gets four
//! fifths of the way and snaps back, for ever.
//!
//! ## The keys, for everything the bar can do
//!
//! ```text
//!     1 2 3        nib: quill, round, broad        D E P   draw, rub, pick
//!     [ ]          thinner / thicker               SPACE   play or stop
//!     , .          the broad nib's angle           B       back to the start
//!     - =          more / less spring              G H     group, split up
//!                                                  N       do nothing (clear the act)
//!     T            taper on and off                U R     undo, redo
//!     C            the next colour                 F       even out the shakes
//!                                                  S O     save, open
//! ```
//!
//! ## Where the work happens
//!
//! Not here. This file is a window: it paints what [`easel`] says and reports
//! where the pen went. The toolbar is data — [`easel::Bar`] is a list of
//! rectangles and what each one means — so the buttons are tested rather than
//! clicked at, and the same arithmetic paints them and decides what was hit.
//!
//! ```text
//!     easel/       what a drawing is, what editing it means, what the buttons are
//!     studio/      this: a window, a pointer and some keys
//! ```

use easel::bar::{Bar, Cmd, INKS, STEP, WIDTH};
use easel::{Board, Tool};
use plotkit::{Anchor, Cx, Frame, Shape};
use shapes::Nib;
use studio::Graph;

/// The window's own concerns: the bar, the file, and what to say.
struct Studio {
    board: Board,
    bar: Bar,
    /// How far the evening-out dial has been turned, in presses.
    cut: usize,
    file: String,
    say: String,
    /// The clock last frame, so the animation runs on real seconds.
    was: f64,
    /// Where the pointer is in pixels, for the toolbar.
    px: (f64, f64),
    /// Down last frame, so a press on the bar happens once.
    was_down: bool,
}

const EDGE: u32 = 0x22303C;

impl Studio {
    fn new(file: String) -> Studio {
        let mut s = Studio {
            board: Board::new(),
            bar: Bar::new(),
            cut: 24,
            file,
            say: "draw with the pen. shift-drag moves the paper.".into(),
            was: 0.0,
            px: (-1.0, -1.0),
            was_down: false,
        };
        if std::path::Path::new(&s.file).exists() {
            s.open();
        }
        s
    }

    fn open(&mut self) {
        self.say = match self.board.load(&self.file) {
            Ok(0) => format!("opened {} -- {} marks", self.file, self.board.sheet.len()),
            Ok(bad) => format!("opened {} -- {} marks, {bad} lines lost", self.file, self.board.sheet.len()),
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

    fn width(&self) -> f64 {
        match self.board.nib {
            Nib::Round(w) => w,
            Nib::Quill { slow, .. } => slow,
            Nib::Broad { width, .. } => width,
        }
    }

    fn set_nib(&mut self, which: usize) {
        let w = self.width();
        self.board.nib = match which {
            1 => Nib::Round(w),
            2 => Nib::Broad { width: w, angle: std::f64::consts::PI / 4.0 },
            _ => Nib::Quill { slow: w, fast: w * 0.15, pace: 0.16 },
        };
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
            Nib::Quill { slow, .. } => format!("quill {slow:.2}"),
            Nib::Broad { width, angle } => format!("broad {width:.2} at {:.0} deg", angle.to_degrees()),
        }
    }

    fn even_out(&mut self) {
        self.cut = self.cut.saturating_sub(3).max(1);
        self.board.smooth_all(self.cut);
        self.say = format!("evened out: keeping waves up to pitch {}", self.cut);
    }

    /// Do what a button says.
    fn obey(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Colour(c) => self.board.colour = c,
            Cmd::Nib(k) => self.set_nib(k),
            Cmd::Use(t) => self.board.tool = t,
            Cmd::Do(action) => {
                // Every step is the same length, and the bar's rates are
                // chosen so a whole number of cycles fits in it -- an act
                // loops, and a cycle that does not close jerks every time
                // round.
                self.say = if self.board.give(action, Some(STEP)) {
                    let steps = self.board.chosen().map_or(0, |m| m.act.steps.len());
                    format!("{} -- step {steps}. press play.", action.name())
                } else {
                    "choose a shape first: press pick, then tap one".into()
                };
            }
            Cmd::Group => {
                self.say = if self.board.group() {
                    "one figure now -- tap any part to take it all".into()
                } else {
                    "tap two or more shapes first".into()
                };
            }
            Cmd::Ungroup => {
                self.say = if self.board.ungroup() { "split up".into() } else { "nothing grouped".into() };
            }
            Cmd::Stop => {
                self.say = if self.board.stop_doing() { "it does nothing now".into() } else { "nothing chosen".into() };
            }
            Cmd::Play => {
                self.board.play(true);
                self.say = if self.board.has_animation() {
                    "playing".into()
                } else {
                    "nothing has been given anything to do yet".into()
                };
            }
            Cmd::Pause => {
                self.board.play(false);
                self.say = "stopped".into();
            }
            Cmd::Rewind => {
                self.board.rewind();
                self.say = "back to the start".into();
            }
            Cmd::Undo => self.say = if self.board.undo() { "undone".into() } else { "nothing to undo".into() },
            Cmd::Redo => self.say = if self.board.redo() { "redone".into() } else { "nothing to redo".into() },
            Cmd::Smooth => self.even_out(),
            Cmd::Clear => {
                self.board.clear();
                self.say = "cleared -- undo puts it back".into();
            }
            Cmd::Save => self.save(),
            Cmd::Open => self.open(),
        }
    }

    /// The pointer, every frame. **The bar gets first refusal.**
    ///
    /// A press that lands on the toolbar must not also reach the paper, or
    /// every button press leaves a speck of ink under the button — invisible,
    /// unreachable, and slowly filling the file.
    fn pointer(&mut self, at: Cx, px: (f64, f64), down: bool) {
        self.px = px;
        let on_bar = self.bar.covers(self.px.0, self.px.1);
        if on_bar {
            if down && !self.was_down {
                if let Some(cmd) = self.bar.at(self.px.0, self.px.1) {
                    self.obey(cmd);
                }
            }
            // Tell the board the pen is up, so a stroke in progress is
            // finished properly rather than left hanging when the hand
            // wanders over the toolbar.
            self.board.pointer(at, false);
        } else {
            self.board.pointer(at, down);
        }
        self.was_down = down;
    }
}

fn main() {
    let file = std::env::args().nth(1).unwrap_or_else(|| "drawing.easel".to_string());

    Graph::new("studio")
        .scale(70.0)
        .with(Studio::new(file))
        .each_frame(|s, t| {
            let dt = (t - s.was).clamp(0.0, 1.0 / 15.0);
            s.was = t;
            s.board.tick(dt);
        })
        .on_pointer_px(|s, at, px, down| s.pointer(at, px, down))
        // --- the nib ---------------------------------------------------------
        .on('1', |s| s.set_nib(0))
        .on('2', |s| s.set_nib(1))
        .on('3', |s| s.set_nib(2))
        .on_hold('[', |s| s.resize(0.97))
        .on_hold(']', |s| s.resize(1.03))
        .on_hold(',', |s| {
            if let Nib::Broad { width, angle } = s.board.nib {
                s.board.nib = Nib::Broad { width, angle: angle - 0.03 };
            }
        })
        .on_hold('.', |s| {
            if let Nib::Broad { width, angle } = s.board.nib {
                s.board.nib = Nib::Broad { width, angle: angle + 0.03 };
            }
        })
        .on_hold('-', |s| s.board.pull = (s.board.pull * 1.03).min(1.0))
        .on_hold('=', |s| s.board.pull = (s.board.pull * 0.97).max(0.03))
        .on('t', |s| s.board.taper = if s.board.taper > 0.0 { 0.0 } else { 0.15 })
        .on('c', |s| {
            let next = INKS.iter().position(|c| *c == s.board.colour).map_or(0, |k| (k + 1) % INKS.len());
            s.board.colour = INKS[next];
        })
        // --- what a drag does -------------------------------------------------
        .on('d', |s| s.obey(Cmd::Use(Tool::Draw)))
        .on('e', |s| s.obey(Cmd::Use(Tool::Erase)))
        .on('p', |s| s.obey(Cmd::Use(Tool::Pick)))
        // --- the clock --------------------------------------------------------
        .on(' ', |s| s.obey(if s.board.playing { Cmd::Pause } else { Cmd::Play }))
        .on('b', |s| s.obey(Cmd::Rewind))
        .on('n', |s| s.obey(Cmd::Stop))
        .on('g', |s| s.obey(Cmd::Group))
        .on('h', |s| s.obey(Cmd::Ungroup))
        // --- the page ---------------------------------------------------------
        .on('u', |s| s.obey(Cmd::Undo))
        .on('r', |s| s.obey(Cmd::Redo))
        .on('f', |s| s.obey(Cmd::Smooth))
        .on('x', |s| s.obey(Cmd::Clear))
        .on('s', |s| s.obey(Cmd::Save))
        .on('o', |s| s.obey(Cmd::Open))
        .run(page);
}

fn page(s: &Studio) -> Frame {
    let mut f = s.board.frame();

    // What is chosen, so the action buttons have something visible to act on.
    for ring in s.board.selection() {
        f.add(ring).color(0x6FCF97).width(1);
    }

    // A cross at the origin, so there is somewhere to be on a blank page.
    if s.board.sheet.is_empty() {
        f.add(Shape::path(vec![Cx::new(-0.2, 0.0), Cx::new(0.2, 0.0)])).color(EDGE).width(1);
        f.add(Shape::path(vec![Cx::new(0.0, -0.2), Cx::new(0.0, 0.2)])).color(EDGE).width(1);
    }

    // --- the toolbar ---------------------------------------------------------
    // One call. The rectangles are `easel`'s, and the same ones decide what a
    // tap hit, so painting and hitting cannot drift apart.
    s.bar.paint(&mut f, &s.board, 620);

    // --- what is going on ----------------------------------------------------
    let doing = match s.board.chosen() {
        None => "tap a shape to choose it".to_string(),
        Some(_) if s.board.chosen().is_some_and(|m| m.act.steps.is_empty()) => {
            format!("{} chosen -- give it something to do", s.board.selected.len())
        }
        Some(m) => {
            let now = m.act.step_at(s.board.clock).and_then(|k| m.act.steps.get(k));
            format!(
                "{} chosen, {} step(s){}",
                s.board.selected.len(),
                m.act.steps.len(),
                now.map_or(String::new(), |st| format!(", now: {}", st.action.name()))
            )
        }
    };
    f.pin(
        Anchor::TopLeft,
        (WIDTH + 14) as f64,
        14.0,
        format!("{}   {}   {}", tool_name(s), s.nib_name(), doing),
        s.board.colour,
        2,
    );
    f.pin(
        Anchor::TopLeft,
        (WIDTH + 14) as f64,
        32.0,
        format!(
            "{}  t {:.1}s   spring {:.2}   taper {}   {} marks   {}{}",
            if s.board.playing { "PLAYING" } else { "stopped" },
            s.board.clock,
            s.board.pull,
            if s.board.taper > 0.0 { "on" } else { "off" },
            s.board.sheet.len(),
            if s.board.can_undo() { "U" } else { "-" },
            if s.board.can_redo() { "R" } else { "-" },
        ),
        0x94A1AE,
        2,
    );
    f.pin(Anchor::BottomLeft, (WIDTH + 14) as f64, -16.0, &s.say, 0x6FCF97, 2);
    f.pin(
        Anchor::BottomRight,
        -14.0,
        -16.0,
        "pick a shape, then walk / jump / spin, then play   --   shift-drag moves the paper",
        0x46525E,
        2,
    );
    f
}

fn tool_name(s: &Studio) -> &'static str {
    match s.board.tool {
        Tool::Draw => "draw",
        Tool::Erase => "rub out",
        Tool::Pick => "pick up",
    }
}

