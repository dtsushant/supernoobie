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
//!     A            add a shape                     ~       just the drawing
//!     , .          step between keys                K L     leave / remove a key
//!     - =          more / less spring              G H     group, split up
//!                                                  N       do nothing (clear the act)
//!     T            taper on and off                U R     undo, redo
//!     C            the next colour                 F       even out the shakes
//!                                                  S O     save, open
//! ```
//!
//! ## The drawing gets its own space
//!
//! The toolbar and the tree are not painted *over* the drawing; the drawing is
//! **kept out of them**. A curve heading for the toolbar stops at its edge
//! rather than being drawn underneath and painted over — which looks the same
//! until you wonder why a shape you can half see cannot be clicked.
//!
//! One rectangle says where the drawing lives, and three things read it: the
//! clip, the pointer, and the wheel. `<` at the top of the tree folds it away
//! to a strip and the rectangle grows to suit.
//!
//! ## Just the game
//!
//! `~` puts the furniture away — no toolbar, no tree, only the drawing, filling
//! the window. Press it again to get them back.
//!
//! It is the same drawing and the same clock; nothing is running in a different
//! mode. What changes is only what is painted and what swallows the pointer, so
//! there is no second code path to keep in step with the first — which is the
//! usual way a "presentation mode" ends up subtly different from the editor.
//!
//! ## The tree, down the left
//!
//! ```text
//!     SHAPES                   [+]
//!     v  figure 1                     tap the arrow to fold it away
//!        [][][][][][][][]             its colour -- open because it is chosen
//!        walk run jump spin           what it does
//!        - head
//!        - body
//!     >  figure 2                     folded
//!
//!     FUNCTIONS                [+]
//!     [x] r = 2
//!         --------o--------
//!     [x] circle(0, r)
//! ```
//!
//! Everything the drawing is made of, drawn and written alike, in the order it
//! is painted — so dragging a line up and down is how one shape is put in
//! front of another.
//!
//! **Colour and what a shape does are not on the toolbar.** They belong to a
//! particular shape, so they open under whatever is chosen. A global "current
//! colour" can only ever mean the colour of the *next* stroke, which is a
//! strange thing to offer when what you want is to change one already drawn.
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

use easel::bar::{Bar, Cmd};
use easel::tree::{self, Half, Node, Poke, Tree, STEP};
use easel::{Board, Tool};
use plotkit::{Anchor, Cx, Frame, Shape};
use shapes::Nib;
use std::cell::Cell;
use std::rc::Rc;

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
    /// The tree down the left, laid out afresh each frame because its lines
    /// and their heights change with what is in the drawing.
    tree: Tree,
    /// What is being dragged in the tree, if anything.
    lifting: Option<Node>,
    /// The furniture put away, so there is only the drawing.
    full: bool,
    /// The arrows last frame, so a press moves the caret once rather than
    /// sixty times a second.
    was_arrows: Cx,
    /// Where the drawing lives: `(left, top)` in pixels.
    ///
    /// Shared with the window, because the graph has to know before the sketch
    /// runs whether the pointer is over the drawing — it pans and zooms at the
    /// top of the frame. Written here, read there, one frame later. The only
    /// moment that shows is the frame you fold the tree away on.
    stage: Rc<Cell<(i32, i32)>>,
    /// How wide and tall the window is, for laying the panel out.
    size: (i32, i32),
}

const EDGE: u32 = 0x22303C;
/// How far down the toolbar reaches, at the widths this window is opened at.
/// Generous: the strip is furniture and a few spare pixels of it are harmless,
/// where a few too few would let a stroke start underneath a button.
const BAR_DEEP: i32 = 100;

impl Studio {
    fn new(file: String) -> Studio {
        let mut s = Studio {
            board: Board::new(),
            bar: Bar::new(1400),
            cut: 24,
            file,
            say: "draw with the pen. shift-drag moves the paper.".into(),
            was: 0.0,
            px: (-1.0, -1.0),
            was_down: false,
            tree: Tree::default(),
            lifting: None,
            full: false,
            was_arrows: Cx::ZERO,
            stage: Rc::new(Cell::new((tree::WIDTH, BAR_DEEP))),
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
            Cmd::Nib(k) => self.set_nib(k),
            Cmd::Use(t) => self.board.tool = t,
            Cmd::Key => {
                self.say = if self.board.key() {
                    format!("key at {:.2}s -- now drag it where it should be then", self.board.clock)
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
            Cmd::Play => {
                self.board.play(true);
                // A drawing with rules in it is a game, so playing it means
                // tapping things rather than choosing them. Deciding by what
                // is written rather than with a separate switch: a drawing
                // with no rules has nothing to tap, and one with rules has
                // nothing else you would want play to mean.
                let game = !self.board.sheet.script.rules().is_empty();
                self.board.playing_game = game;
                self.say = if game {
                    "playing -- tap the shapes".into()
                } else if self.board.has_animation() {
                    "playing".into()
                } else {
                    "nothing has been given anything to do yet".into()
                };
            }
            Cmd::Pause => {
                self.board.play(false);
                self.board.playing_game = false;
                self.say = "stopped -- tapping chooses again".into();
            }
            Cmd::Rewind => {
                self.board.rewind();
                self.board.restart();
                self.board.playing_game = false;
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

    /// Where the drawing lives: everything the furniture is not using.
    fn free(&self) -> (i32, i32, i32, i32) {
        let (left, top) = if self.full { (0, 0) } else { (Tree::width(&self.board), BAR_DEEP) };
        (left, top, self.size.0 - left, self.size.1 - top)
    }

    /// Paint what is chosen, or set the pen if nothing is.
    fn paint_now(&mut self, colour: u32) {
        self.board.paint(colour);
    }

    /// Do what a line in the tree says.
    fn heed(&mut self, poke: Poke) {
        match poke {
            Poke::Collapse => {
                self.board.tree_shut = !self.board.tree_shut;
                self.board.edit(None);
            }
            Poke::Fold(g) => self.board.fold(g),
            Poke::Tick(k) => {
                self.board.toggle_row(k);
            }
            Poke::Edit(k) => self.board.edit(Some(k)),
            Poke::Dial(k, v) => {
                self.board.set_dial(k, v);
            }
            Poke::Choose(Node::Group(g, _)) => {
                self.board.edit(None);
                self.board.choose_group(g);
            }
            Poke::Choose(Node::Mark(k)) => {
                self.board.edit(None);
                self.board.choose_only(k);
            }
            Poke::Choose(_) => {}
            Poke::Add(Half::Functions) => {
                self.board.add_row();
                // Scrolled to, or a new row on a long list appears somewhere
                // below the window and looks like nothing having happened.
                let tree = Tree::new(&self.board);
                self.board.scrolled = tree.most(self.size.1);
                self.say = "type the function, then press enter for another".into();
            }
            Poke::Add(Half::Shapes) => {
                // With two or more chosen, `+` means "make these one figure".
                // With anything else, it means what `+` means everywhere:
                // there is now one more of them.
                self.say = if self.board.selected.len() >= 2 && self.board.new_group() {
                    "one figure now -- tap any part to take it all".into()
                } else {
                    let k = self.board.add_shape();
                    format!("shape {k} added, and chosen. drag it, colour it, tell it to walk.")
                };
            }
            Poke::Paint(c) => {
                self.board.paint(c);
                self.say = if self.board.any_chosen() { "painted".into() } else { "the pen is that colour now".into() };
            }
            Poke::Verb(None) => {
                self.say = if self.board.stop_doing() { "it does nothing now".into() } else { "nothing chosen".into() };
            }
            Poke::Verb(Some(action)) => {
                self.say = if self.board.give(action, Some(STEP)) {
                    let steps = self.board.chosen().map_or(0, |m| m.act.steps.len());
                    format!("{} -- step {steps}. press play.", action.name())
                } else {
                    "tap a shape first".into()
                };
            }
            Poke::Drop(_) => {}
        }
    }

    /// The pointer, every frame. **The bar gets first refusal.**
    ///
    /// A press that lands on the toolbar must not also reach the paper, or
    /// every button press leaves a speck of ink under the button — invisible,
    /// unreachable, and slowly filling the file.
    fn pointer(&mut self, at: Cx, px: (f64, f64), down: bool) {
        self.px = px;
        self.tree = Tree::new(&self.board);
        self.bar = Bar::new(self.size.0);
        let (left, top, _, _) = self.free();
        self.stage.set((left, top));
        let pressed = down && !self.was_down;
        let released = !down && self.was_down;
        // With the furniture away there is nothing but drawing, so nothing
        // swallows the pointer.
        let on_tree = !self.full && Tree::covers_at(px.0, left);
        let on_bar = !self.full && self.bar.covers(px.0, px.1);

        if pressed && on_bar {
            if let Some(cmd) = self.bar.at(px.0, px.1) {
                self.obey(cmd);
            }
        }

        if on_tree || on_bar {
            match (pressed, self.tree.at(px.0, px.1, left)) {
                // A slider answers to being held, not only to the press.
                (_, Some(Poke::Dial(k, v))) if down => {
                    if pressed {
                        self.board.edit(None);
                    }
                    self.board.set_dial(k, v);
                }
                (true, Some(poke)) => {
                    // A press on a movable line might be the start of a drag.
                    // Which it is cannot be known until the pointer moves, so
                    // the line is remembered and the poke is obeyed anyway --
                    // choosing something and then dragging it is one gesture,
                    // not two.
                    if let Poke::Choose(node) = poke {
                        self.lifting = node.movable().then_some(node);
                    }
                    self.heed(poke);
                }
                _ => {}
            }
            // Where a drop would land, while something is being carried.
            self.board.dropping = (down && self.lifting.is_some() && on_tree).then(|| self.tree.gap_at(px.1));
            self.board.pointer(at, false);
        } else {
            if pressed {
                self.board.edit(None);
            }
            self.board.dropping = None;
            self.board.pointer(at, down);
        }

        if released {
            self.land();
        }
        self.was_down = down;
    }

    /// Let go of whatever was being dragged in the tree.
    fn land(&mut self) {
        let (Some(node), Some(before)) = (self.lifting.take(), self.board.dropping.take()) else {
            self.board.dropping = None;
            return;
        };
        // What is at the gap decides what the drop MEANS. Dropped among a
        // figure's strokes, a mark joins that figure; dropped anywhere else it
        // leaves whatever it was in. Asking "which figure is this gap inside?"
        // is the whole rule, and it needs no separate drop targets.
        let landing = self.tree.lines.get(before).or_else(|| self.tree.lines.last()).map(|l| l.node);
        let into = match landing {
            Some(Node::Mark(k)) => self.board.sheet.marks.get(k).map(|m| m.group).unwrap_or(0),
            Some(Node::Group(g, _)) => g,
            _ => 0,
        };
        match node {
            Node::Mark(from) => {
                let to = self
                    .tree
                    .lines
                    .get(before)
                    .and_then(|l| match l.node {
                        Node::Mark(k) => Some(k),
                        _ => None,
                    })
                    .unwrap_or(self.board.sheet.len());
                if self.board.put_in_group(from, into) {
                    self.board.move_mark(from, to);
                    self.say = if into == 0 { "moved".into() } else { format!("moved into figure {into}") };
                }
            }
            // A whole figure keeps its own order; only where it sits changes,
            // and that is not something a flat list of marks can express
            // without renumbering every group. Left for when it is asked for.
            Node::Group(..) => self.say = "a whole figure cannot be reordered yet".into(),
            _ => {}
        }
    }
}

fn main() {
    let file = std::env::args().nth(1).unwrap_or_else(|| "drawing.easel".to_string());

    // Shared with the sketch: the window has to know where the drawing lives
    // before the sketch runs, since it pans and zooms at the top of the frame.
    let where_it_lives = Rc::new(Cell::new((tree::WIDTH, BAR_DEEP)));

    Graph::new("studio")
        .scale(70.0)
        .with({
            let mut studio = Studio::new(file);
            studio.stage = Rc::clone(&where_it_lives);
            studio
        })
        // While a row is being typed the keyboard belongs to it -- otherwise
        // typing `p` in a formula would switch to the pick tool. One place
        // decides, rather than thirty shortcuts each testing the same thing.
        .gate(|s: &Studio| s.board.editing.is_none())
        // The tree and the toolbar are furniture: the graph keeps its hands
        // off them, so scrolling a long list does not quietly zoom the drawing
        // behind it.
        .reserve({
            let stage = Rc::clone(&where_it_lives);
            move |px, py| {
                let (left, top) = stage.get();
                px < f64::from(left) || py < f64::from(top)
            }
        })
        .each_frame(|s, t| {
            let dt = (t - s.was).clamp(0.0, 1.0 / 15.0);
            s.was = t;
            s.board.tick(dt);
        })
        // Typing, and the two keys that are not characters. Registered before
        // the shortcuts so the row has the keyboard first -- though what
        // really settles it is that every shortcut below asks `not typing`.
        .on_keys(|s, keys| {
            // The arrows move the caret while a row is being typed. Edge
            // triggered against last frame: held down at sixty a second the
            // caret would cross the row before you let go.
            if s.board.editing.is_some() {
                let now = keys.arrows();
                if now.re != 0.0 && s.was_arrows.re == 0.0 {
                    s.board.nudge_caret(now.re.signum() as i32);
                }
                // Up and down go to the ends, which is what there is room for
                // on one line.
                if now.im != 0.0 && s.was_arrows.im == 0.0 {
                    s.board.caret_to_end(now.im < 0.0);
                }
                s.was_arrows = now;
            } else {
                s.was_arrows = Cx::ZERO;
            }
            // The wheel over the tree scrolls the tree.
            let wheel = keys.scroll();
            if wheel.abs() > 1e-6 && Tree::covers_at(keys.at_px().0, Tree::width(&s.board)) {
                let most = s.tree.most(s.size.1);
                s.board.scroll(-wheel * 46.0, most);
            }
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
            let next = tree::INKS.iter().position(|c| *c == s.board.colour).map_or(0, |k| (k + 1) % tree::INKS.len());
            s.paint_now(tree::INKS[next]);
        })
        // --- what a drag does -------------------------------------------------
        .on('d', |s| s.obey(Cmd::Use(Tool::Draw)))
        .on('e', |s| s.obey(Cmd::Use(Tool::Erase)))
        .on('p', |s| s.obey(Cmd::Use(Tool::Pick)))
        // --- the clock --------------------------------------------------------
        .on(' ', |s| s.obey(if s.board.playing { Cmd::Pause } else { Cmd::Play }))
        .on('b', |s| s.obey(Cmd::Rewind))
        .on('~', |s| {
            s.full = !s.full;
            s.say = if s.full { "just the drawing -- ~ brings the tools back".into() } else { String::new() };
        })
        .on('n', |s| s.heed(Poke::Verb(None)))
        .on('g', |s| s.obey(Cmd::Group))
        .on('a', |s| s.heed(Poke::Add(Half::Shapes)))
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
    {
        // Set first, so it applies to everything in the frame -- including
        // whatever `Board::frame` has already put there.
        let (left, top, w, h) = s.free();
        f.stage(left, top, w, h);
    }

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
    // The drawing is kept out of the furniture rather than painted under it.
    let (left, top, w, h) = s.free();
    f.stage(left, top, w, h);

    if !s.full {
        s.bar.paint(&mut f, &s.board, s.size.0);
        s.tree.paint(&mut f, &s.board, s.size.1);
    }

    // --- what is going on ----------------------------------------------------
    if s.full {
        // Only what the game itself has to say. A score and a question are the
        // drawing's own business; the studio's chatter is not.
        if !s.say.is_empty() {
            f.pin(Anchor::BottomLeft, 14.0, -16.0, &s.say, 0x46525E, 2);
        }
        return f;
    }

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
        (Tree::width(&s.board) + 14) as f64,
        14.0,
        format!("{}   {}   {}", tool_name(s), s.nib_name(), doing),
        s.board.colour,
        2,
    );
    let keys = s.board.keys_here();
    f.pin(
        Anchor::TopLeft,
        (Tree::width(&s.board) + 14) as f64,
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
        (Tree::width(&s.board) + 14) as f64,
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
    f.pin(Anchor::BottomLeft, (Tree::width(&s.board) + 14) as f64, -16.0, &s.say, 0x6FCF97, 2);
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

