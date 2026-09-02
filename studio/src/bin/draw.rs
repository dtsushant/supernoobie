//! # draw — the studio
//!
//! ```text
//!     cargo run -p studio --release --bin draw                    drawing.easel
//!     cargo run -p studio --release --bin draw -- mine.easel      a drawing
//!     cargo run -p studio --release --bin draw -- some.rec        a plain script
//! ```
//!
//! The file named is the one `save` and `open` use — there is no "save as",
//! deliberately: one window, one file, and no dialog to get wrong. Start
//! another window for another drawing.
//!
//! A `.rec` is **imported**: its rows come in and saving goes to a `.easel`
//! beside it, so a script you were only borrowing from is never overwritten.
//! Samples to try are in `samples/` — see `cargo run -p studio --bin sample`.
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
//! ## Keyframes: at zero you edit the shape, after zero you edit the animation
//!
//! ```text
//!     clock at 0      dragging moves the shape itself
//!     clock past 0    dragging leaves a KEY at that moment
//! ```
//!
//! One rule instead of a record button whose state everybody forgets. Wind the
//! clock to two seconds with `>|`, drag the figure where it should be then,
//! and that is an animation. The in-between is worked out — turning the short
//! way, and taking size in ratios, which is why a half turn does not collapse
//! the shape through nothing on its way round.
//!
//! `|<` and `>|` step between the moments you set, `key` leaves one where the
//! shape stands, and the other moments are drawn faintly underneath so a
//! movement can be seen rather than remembered.
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
//!     ; '          the broad nib's angle           B       back to the start
//!     , .          step between keys                K L     leave / remove a key
//!     - =          more / less spring              G H     group, split up
//!                                                  N       do nothing (clear the act)
//!     T            taper on and off                U R     undo, redo
//!     C            the next colour                 F       even out the shakes
//!                                                  S O     save, open
//! ```
//!
//! ## The written half
//!
//! Down the right-hand side: a row of script each, Desmos-fashion.
//!
//! ```text
//!     [x] r = 2
//!         ------o------          a slider, because the row binds a number
//!     [x] circle(0, r)
//!     [ ] ngon(0, r, 6)          switched off, not deleted
//!     [+] a new row
//! ```
//!
//! Change `r` and everything that mentions it changes. `time` is bound to the
//! clock, so `param(exp(i*(t + time)), 0, tau)` animates without any
//! keyframes at all.
//!
//! **While a row is being typed the keyboard belongs to it** — otherwise
//! typing `p` in a formula would switch to the pick tool. Tapping the drawing
//! gives it back. (Not `Esc`: that closes the window, everywhere in this
//! repository, and a key that means two things is a key that will one day mean
//! the wrong one.)
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
use easel::panel::{Panel, Poke};
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
    /// The rows down the right, laid out afresh each frame because their
    /// number and heights change with what is in them.
    panel: Panel,
    /// How wide and tall the window is, for laying the panel out.
    size: (i32, i32),
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
            panel: Panel::default(),
            size: (1200, 780),
        };
        if std::path::Path::new(&s.file).exists() {
            s.open();
        }
        s
    }

    /// Open whatever was named on the command line.
    ///
    /// `.easel` is a whole drawing — marks, keys, script. `.rec` is a plain
    /// script, which is what the older live-reload playground has always used,
    /// and it is **imported** rather than opened: the rows come in and saving
    /// goes to a `.easel` beside it. Opening a file that quietly becomes the
    /// thing you then overwrite is how people lose work they were only
    /// borrowing from.
    fn open(&mut self) {
        if self.file.ends_with(".rec") {
            self.say = match std::fs::read_to_string(&self.file) {
                Ok(text) => {
                    self.board.sheet.script = easel::Script::from_rec(&text);
                    self.board.forget_history();
                    self.file = self.file.replace(".rec", ".easel");
                    format!("imported {} rows -- saving goes to {}", self.board.sheet.script.len(), self.file)
                }
                Err(e) => format!("could not read: {e}"),
            };
            return;
        }
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
            Cmd::Key => {
                self.say = if self.board.key() {
                    format!("key at {:.2}s -- drag it to where it should be then", self.board.clock)
                } else {
                    "tap a shape first".into()
                };
            }
            Cmd::Unkey => {
                self.say = if self.board.unkey() { "key removed".into() } else { "no key at this moment".into() };
            }
            Cmd::Step(forwards) => {
                self.say = if self.board.next_key(forwards) {
                    format!("at {:.2}s", self.board.clock)
                } else if forwards {
                    "no key after this one".into()
                } else {
                    "no key before this one".into()
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
        self.panel = Panel::new(self.size.0, &self.board);
        let pressed = down && !self.was_down;
        let furniture = self.bar.covers(px.0, px.1) || self.panel.covers(px.0);

        if furniture {
            if let Some(cmd) = pressed.then(|| self.bar.at(px.0, px.1)).flatten() {
                self.obey(cmd);
            }
            match self.panel.at(px.0, px.1) {
                // A slider is dragged, so it answers to being held, not only
                // to the press. Remembered once, at the press, or a drag
                // would leave a hundred steps to undo.
                Some(Poke::Drag(row, value)) if down => {
                    if pressed {
                        self.board.edit(None);
                    }
                    self.board.set_dial(row, value);
                }
                Some(poke) if pressed => match poke {
                    Poke::Tick(row) => {
                        self.board.toggle_row(row);
                    }
                    Poke::Edit(row) => self.board.edit(Some(row)),
                    Poke::Add => self.board.add_row(),
                    Poke::Drag(..) => {}
                },
                _ => {}
            }
            // Tell the board the pen is up, so a stroke in progress is
            // finished properly rather than left hanging when the hand
            // wanders over the furniture.
            self.board.pointer(at, false);
        } else {
            // Tapping the drawing puts the keyboard back where it belongs.
            if pressed {
                self.board.edit(None);
            }
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
        // While a row is being typed the keyboard belongs to it -- otherwise
        // typing `p` in a formula would switch to the pick tool. One place
        // decides, rather than thirty shortcuts each testing the same thing.
        .gate(|s: &Studio| s.board.editing.is_none())
        .each_frame(|s, t| {
            let dt = (t - s.was).clamp(0.0, 1.0 / 15.0);
            s.was = t;
            s.board.tick(dt);
        })
        // Typing, and the two keys that are not characters. Registered before
        // the shortcuts so the row has the keyboard first -- though what
        // really settles it is that every shortcut below asks `not typing`.
        .on_keys(|s, keys| {
            if s.board.editing.is_some() {
                if !keys.typed().is_empty() {
                    s.board.type_into(keys.typed());
                }
                if keys.backspace() {
                    s.board.rub_out();
                }
                if keys.enter() {
                    s.board.add_row();
                }
            }
        })
        .on_pointer_px(|s, at, px, down| s.pointer(at, px, down))
        // --- the nib ---------------------------------------------------------
        .on('1', |s| s.set_nib(0))
        .on('2', |s| s.set_nib(1))
        .on('3', |s| s.set_nib(2))
        .on_hold('[', |s| s.resize(0.97))
        .on_hold(']', |s| s.resize(1.03))
        .on_hold(';', |s| {
            if let Nib::Broad { width, angle } = s.board.nib {
                s.board.nib = Nib::Broad { width, angle: angle - 0.03 };
            }
        })
        .on_hold('\'', |s| {
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
        .on('k', |s| s.obey(Cmd::Key))
        .on('l', |s| s.obey(Cmd::Unkey))
        .on(',', |s| s.obey(Cmd::Step(false)))
        .on('.', |s| s.obey(Cmd::Step(true)))
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

    // Onion skin: the other moments this thing is keyed at, drawn faintly
    // underneath. Nearly free -- a frame is the same drawing at another time.
    for ghost in s.board.ghosts() {
        f.add(ghost).color(0x2A3542).width(1);
    }

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
    s.bar.paint(&mut f, &s.board, s.size.1);
    s.panel.paint(&mut f, &s.board, s.size.1);

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
    let keys = s.board.keys_here();
    f.pin(
        Anchor::TopLeft,
        (WIDTH + 14) as f64,
        50.0,
        if s.board.clock <= 0.0 {
            "at 0s: dragging moves the shape itself".to_string()
        } else {
            format!("at {:.2}s: dragging leaves a key here   ({keys} key(s))", s.board.clock)
        },
        if s.board.on_a_key() { 0xE0A44A } else { 0x6B7987 },
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

