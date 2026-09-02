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
//! ## A tap chooses, a drag does
//!
//! The one rule that took the most taps out of the studio.
//!
//! ```text
//!     drag        draw / move / rub out, according to the tool
//!     tap         choose that mark -- whatever the tool is
//!     tap again   let go of it
//!     tap paper   choose nothing
//! ```
//!
//! Choosing used to mean *switch to the pick tool, tap, switch back*, which is
//! three deliberate acts to say one thing. And a tap while drawing was already
//! being thrown away — [`Ink::lift`] refuses anything under two points,
//! because a one-point mark is an invisible speck that can still be clicked
//! on. So the gesture was free: nothing had to be given up to have it.
//!
//! Tapping **toggles**, so a selection can be several marks without a modifier
//! key. That matters here more than in most editors: this program is used with
//! a pen, where there is no second button and no comfortable way to hold shift
//! — and shift already means *"talk to the graph"* everywhere in this
//! repository.
//!
//! ## At zero you edit the shape; after zero you edit the animation
//!
//! One rule, and it removes a mode switch that every animation program has and
//! everybody forgets the state of.
//!
//! ```text
//!     clock at 0      dragging moves the shape itself
//!     clock past 0    dragging leaves a KEY at that moment
//! ```
//!
//! The clock is already on screen and already the thing you were thinking
//! about, so it says what a drag means without a separate switch to hunt for.
//! Wind to two seconds, drag the figure where it should be then, and you have
//! made an animation.
//!
//! The first key you make also plants one at zero, holding wherever the shape
//! already was. Without it a single key would apply at every moment — one key
//! is one pose for all time — and the shape would jump to its new place the
//! instant the drawing opened.
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

use plotkit::{Cx, Frame, Shape};
use shapes::Nib;

use crate::action::{Act, Action};
use crate::history::History;
use crate::ink::Ink;
use crate::mark::Mark;
use crate::sheet::Sheet;
use crate::rule::{self, Tally};
use crate::track::Ease;
use shapes::Pose;

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
    /// Where the pointer went down, and whether it has moved since — which is
    /// the whole of telling a tap from a drag.
    pressed_at: Option<Cx>,
    wandered: bool,
    past: History,

    // --- the clock ----------------------------------------------------------
    /// Which marks the commands apply to.
    ///
    /// A list, not one: a figure is several strokes, and telling six of them
    /// to walk one at a time is the thing that made this tiring.
    pub selected: Vec<usize>,
    /// Where the animation has got to, in seconds.
    ///
    /// Held here rather than taken from the wall clock, so that stopping means
    /// *going back to the beginning* and pausing means staying put — neither
    /// of which is possible if every mark works out its own pose from whatever
    /// time it happens to be.
    pub clock: f64,
    pub playing: bool,

    /// Which script row is being typed into, if any.
    ///
    /// While this is set the keyboard belongs to the row, and none of the
    /// studio's shortcuts fire. Typing `p` in a formula must not switch to the
    /// pick tool, and the way to be sure is that there is exactly one place
    /// that decides — here.
    pub editing: Option<usize>,

    // --- the tree -------------------------------------------------------------
    /// Which figures are folded shut in the tree.
    ///
    /// **Not saved.** Folding is about looking, not about the drawing, and a
    /// file that remembered which folders you had open would make two people's
    /// copies of the same picture differ for no reason anybody cares about.
    pub folded: Vec<u32>,
    /// The line a drop would land before, while something is being dragged.
    pub dropping: Option<usize>,
    /// How far down the tree has been scrolled, in pixels.
    ///
    /// Not saved, for the same reason folding is not: it is about looking, not
    /// about the drawing.
    pub scrolled: f64,
    /// Where the game has got to. Empty until a rule has fired.
    pub tally: Tally,
    /// Whether tapping a figure sets its rules off, rather than choosing it.
    ///
    /// The same tap cannot mean both — choosing something to edit it and
    /// playing with it are different intentions, and a program that guessed
    /// would guess wrong at the worst moment.
    pub playing_game: bool,
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
            pressed_at: None,
            wandered: false,
            past: History::new(),
            selected: Vec::new(),
            clock: 0.0,
            playing: false,
            editing: None,
            folded: Vec::new(),
            dropping: None,
            scrolled: 0.0,
            tally: Tally::new(),
            playing_game: false,
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
        self.pressed_at = Some(at);
        self.wandered = false;
        match self.tool {
            Tool::Draw => {
                let mut ink = Ink::new(self.nib, self.colour).with_pull(self.pull).with_taper(self.taper);
                ink.sample(at);
                self.ink = Some(ink);
            }
            Tool::Pick => {
                if let Some(k) = self.sheet.at(at, self.touch, self.clock) {
                    // Remembered on the press, so the whole drag is one step
                    // back rather than sixty. Choosing is left to the release,
                    // where a tap can be told from a drag.
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
        // Once it has wandered further than a fingertip it is a drag, and it
        // stays one. Deciding afresh every frame would let a slow, careful
        // drag flicker back into being a tap.
        if let Some(from) = self.pressed_at {
            if (at - from).abs() > self.touch {
                self.wandered = true;
            }
        }
        match self.tool {
            Tool::Draw => {
                if let Some(ink) = self.ink.as_mut() {
                    ink.sample(at);
                }
            }
            Tool::Pick => {
                if let Some((k, was)) = self.holding {
                    // Dragging one of the chosen marks moves all of them, so a
                    // figure is moved as a figure. Dragging something that is
                    // not chosen moves only it, which is what you meant if you
                    // reached past a selection to grab something else.
                    let moving: Vec<usize> =
                        if self.selected.contains(&k) { self.selected.clone() } else { vec![k] };
                    let by = at - was;
                    let when = self.clock;
                    for j in moving {
                        let Some(m) = self.sheet.marks.get_mut(j) else { continue };
                        if when <= 0.0 {
                            *m = m.shifted(by);
                        } else {
                            // A key at zero first, holding wherever it already
                            // was. Without it one key would mean one pose for
                            // all time, and the shape would jump to its new
                            // place the instant the drawing opened.
                            if m.track.is_empty() {
                                m.track.set(0.0, m.act.at(0.0), Ease::Smooth);
                            }
                            let now = m.pose_at(when);
                            m.track.set(when, Pose::new(now.a, now.b + by), Ease::Smooth);
                        }
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
        // The release carries a position like any other frame, and it has to
        // be acted on: a hand still moving when it lifts would otherwise lose
        // its last step, and the mark would stop a little short of where it
        // was let go. Rarely visible -- a window reports every frame, so the
        // lost step is one frame long -- but wrong, and wrong in the same
        // direction every time.
        self.drag(at);
        let tapped = !self.wandered;
        if let Some(ink) = self.ink.take() {
            // A tap makes no mark anyway -- `Ink::lift` refuses anything under
            // two points -- so the gesture was going spare.
            if let Some(mark) = ink.lift(at) {
                self.past.remember(&self.sheet);
                self.sheet.add(mark);
            }
        }
        // Not with the eraser. Rubbing out is destructive and single-minded,
        // and a tap that both removed a mark and then changed what was chosen
        // would be doing two things to one gesture.
        if tapped && self.tool != Tool::Erase {
            self.tap(at);
        }
        self.holding = None;
        self.pressed_at = None;
    }

    /// A tap: choose what is under it, or let go of it if it was already
    /// chosen. Nothing under it means choose nothing.
    fn tap(&mut self, at: Cx) {
        let Some(k) = self.sheet.at(at, self.touch, self.clock) else {
            if !self.playing_game {
                self.selected.clear();
            }
            return;
        };
        if self.playing_game {
            // While the game runs a tap is a move, not a choice. Nothing is
            // selected and nothing is edited -- which is the point of it being
            // a separate state rather than a guess.
            let group = self.sheet.marks[k].group;
            self.play_tap(group);
            return;
        }
        // Tapping any member of a figure takes the whole figure, which is the
        // point of having grouped it.
        let family = self.family_of(k);
        if family.iter().all(|j| self.selected.contains(j)) {
            self.selected.retain(|j| !family.contains(j));
        } else {
            for j in family {
                if !self.selected.contains(&j) {
                    self.selected.push(j);
                }
            }
        }
    }

    /// The mark, and everything grouped with it.
    fn family_of(&self, k: usize) -> Vec<usize> {
        match self.sheet.marks.get(k).map(|m| m.group) {
            Some(0) | None => vec![k],
            Some(g) => (0..self.sheet.len()).filter(|j| self.sheet.marks[*j].group == g).collect(),
        }
    }

    /// Take out whatever is under the pointer.
    fn rub(&mut self, at: Cx) {
        if let Some(k) = self.sheet.at(at, self.touch, self.clock) {
            self.sheet.marks.remove(k);
            // Everything above it has just shifted down one. A selection left
            // pointing at an index rather than at a mark would silently start
            // meaning a different shape — and then a walk gets given to the
            // wrong one.
            self.selected.retain(|s| *s != k);
            for s in self.selected.iter_mut() {
                if *s > k {
                    *s -= 1;
                }
            }
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
                act: crate::Act::still(),
                track: crate::Track::new(),
                group: 0,
            }
        })
    }

    // ---- commands ----------------------------------------------------------

    // ---- the clock ---------------------------------------------------------

    /// Move the animation on by `dt` seconds, if it is running.
    pub fn tick(&mut self, dt: f64) {
        if self.playing {
            self.clock += dt.max(0.0);
        }
    }

    /// Run, or stop running and stay where you are.
    pub fn play(&mut self, yes: bool) {
        self.playing = yes;
    }

    /// Back to the beginning, and stop.
    pub fn rewind(&mut self) {
        self.clock = 0.0;
        self.playing = false;
    }

    // ---- what things do ----------------------------------------------------

    /// The first chosen mark, for showing what is going on.
    pub fn chosen(&self) -> Option<&Mark> {
        self.sheet.marks.get(*self.selected.first()?)
    }

    pub fn any_chosen(&self) -> bool {
        !self.selected.is_empty()
    }

    /// Add a step to what **every** chosen mark does.
    ///
    /// All of them, in one press. A figure is several strokes, and giving them
    /// a walk one at a time was the thing that made this tiring.
    ///
    /// `None` for `seconds` means forever, which is what a single looping
    /// motion is — `spin`, and nothing after it.
    pub fn give(&mut self, action: Action, seconds: Option<f64>) -> bool {
        self.to_each(|m| m.act.steps.push(crate::action::Step { action, seconds: seconds.unwrap_or(f64::INFINITY) }))
    }

    /// Take away everything the chosen marks do — verbs and keys alike.
    pub fn stop_doing(&mut self) -> bool {
        self.to_each(|m| {
            m.act = Act::still();
            m.track = crate::Track::new();
        })
    }

    /// Whether the chosen marks start again at the end.
    pub fn set_looping(&mut self, yes: bool) -> bool {
        self.to_each(|m| m.act.looping = yes)
    }

    /// Do something to every chosen mark, as **one** step back.
    fn to_each(&mut self, mut f: impl FnMut(&mut Mark)) -> bool {
        let chosen: Vec<usize> = self.selected.iter().copied().filter(|k| *k < self.sheet.len()).collect();
        if chosen.is_empty() {
            return false;
        }
        self.past.remember(&self.sheet);
        for k in chosen {
            f(&mut self.sheet.marks[k]);
        }
        true
    }

    /// Bind the chosen marks into one figure, so a single tap takes them all
    /// and a single press tells them all to walk.
    pub fn group(&mut self) -> bool {
        if self.selected.len() < 2 {
            return false;
        }
        // The next number nobody is using. Reusing a freed number would
        // silently adopt any mark that still carried it.
        let next = self.sheet.marks.iter().map(|m| m.group).max().unwrap_or(0) + 1;
        self.to_each(|m| m.group = next)
    }

    /// Make a figure of whatever is chosen, even if it is only one thing.
    ///
    /// [`group`](Board::group) refuses one on its own, because binding a lone
    /// stroke into a figure is almost always a slip. A game is the exception:
    /// a rule names a figure, so a single shape that has to be tappable needs
    /// a number of its own.
    pub fn group_alone(&mut self) -> bool {
        if self.selected.is_empty() {
            return false;
        }
        let next = self.sheet.marks.iter().map(|m| m.group).max().unwrap_or(0) + 1;
        self.to_each(|m| m.group = next)
    }

    /// Break the figure up again.
    pub fn ungroup(&mut self) -> bool {
        if !self.selected.iter().any(|k| self.sheet.marks.get(*k).is_some_and(|m| m.group != 0)) {
            return false;
        }
        self.to_each(|m| m.group = 0)
    }

    /// How many separate figures the chosen marks belong to, for saying so.
    pub fn chosen_groups(&self) -> usize {
        let mut seen: Vec<u32> = Vec::new();
        for k in &self.selected {
            if let Some(m) = self.sheet.marks.get(*k) {
                if m.group != 0 && !seen.contains(&m.group) {
                    seen.push(m.group);
                }
            }
        }
        seen.len()
    }

    // ---- the written half ---------------------------------------------------

    /// Start typing in a row, or stop.
    ///
    /// Remembered as **one step back** for the whole edit: a row is retyped a
    /// character at a time, and an undo that walked back through every
    /// keystroke would be useless for getting out of a mess.
    pub fn edit(&mut self, row: Option<usize>) {
        if row.is_some() && row != self.editing {
            self.past.remember(&self.sheet);
        }
        self.editing = row.filter(|k| *k < self.sheet.script.len());
    }

    /// Add a row and start typing in it.
    pub fn add_row(&mut self) {
        self.past.remember(&self.sheet);
        self.sheet.script.add("");
        self.editing = Some(self.sheet.script.len() - 1);
    }

    /// Put characters into the row being typed.
    pub fn type_into(&mut self, text: &str) -> bool {
        let Some(k) = self.editing else { return false };
        let Some(r) = self.sheet.script.rows.get_mut(k) else { return false };
        r.text.push_str(text);
        true
    }

    /// Take the last character back.
    ///
    /// An empty row that is rubbed out again **goes away**, because that is
    /// what pressing backspace on nothing means, and the alternative is a
    /// drawing slowly filling with blank rows nobody can get rid of.
    pub fn rub_out(&mut self) -> bool {
        let Some(k) = self.editing else { return false };
        let Some(r) = self.sheet.script.rows.get_mut(k) else { return false };
        if r.text.pop().is_some() {
            return true;
        }
        self.sheet.script.rows.remove(k);
        self.editing = if k == 0 { None } else { Some(k - 1) };
        true
    }

    /// Switch a row on or off.
    pub fn toggle_row(&mut self, k: usize) -> bool {
        let Some(was) = self.sheet.script.rows.get(k).map(|r| r.on) else { return false };
        // Remembered before the change, and the row is reached again
        // afterwards -- the snapshot has to be of the state BEFORE, so the
        // borrow cannot be held across it.
        self.past.remember(&self.sheet);
        self.sheet.script.rows[k].on = !was;
        true
    }

    /// Move a slider.
    ///
    /// Not remembered per frame — a drag would otherwise leave a hundred steps
    /// to undo. It is remembered when the drag starts, by whoever starts it.
    pub fn set_dial(&mut self, row: usize, value: f64) -> bool {
        let Some(name) = self.sheet.script.rows.get(row).and_then(|r| r.binds()).map(str::to_string) else {
            return false;
        };
        self.sheet.script.set_dial(&name, value)
    }

    // ---- keyframes ---------------------------------------------------------

    /// Leave a key at the clock for every chosen mark, holding where it is now.
    ///
    /// For pausing a movement — two keys the same makes a shape wait — and for
    /// giving yourself a moment to drag from.
    pub fn key(&mut self) -> bool {
        let when = self.clock;
        self.to_each(|m| {
            if m.track.is_empty() {
                m.track.set(0.0, m.act.at(0.0), Ease::Smooth);
            }
            let now = m.pose_at(when);
            m.track.set(when, now, Ease::Smooth);
        })
    }

    /// Take away the key at the clock, if there is one.
    pub fn unkey(&mut self) -> bool {
        let when = self.clock;
        let any = self.selected.iter().any(|k| {
            self.sheet.marks.get(*k).is_some_and(|m| m.track.key_at(when).is_some())
        });
        if !any {
            return false;
        }
        self.to_each(|m| {
            m.track.clear_at(when);
        })
    }

    /// Step the clock to the next key on the chosen marks, so you can move
    /// between the moments you set rather than hunting for them.
    pub fn next_key(&mut self, forwards: bool) -> bool {
        let now = self.clock;
        let mut best: Option<f64> = None;
        for k in &self.selected {
            let Some(m) = self.sheet.marks.get(*k) else { continue };
            for at in m.track.moments() {
                let ahead = if forwards { at > now + crate::track::SAME } else { at < now - crate::track::SAME };
                if ahead && best.is_none_or(|b| if forwards { at < b } else { at > b }) {
                    best = Some(at);
                }
            }
        }
        match best {
            Some(at) => {
                self.clock = at;
                self.playing = false;
                true
            }
            None => false,
        }
    }

    /// Is there a key at the clock on anything chosen?
    pub fn on_a_key(&self) -> bool {
        self.selected
            .iter()
            .any(|k| self.sheet.marks.get(*k).is_some_and(|m| m.track.key_at(self.clock).is_some()))
    }

    /// How many keys the first chosen mark has.
    pub fn keys_here(&self) -> usize {
        self.chosen().map_or(0, |m| m.track.len())
    }

    /// **Onion skin**: the chosen marks at each of the moments they are keyed
    /// at, so you can see where a movement came from and where it is going.
    ///
    /// Nearly free, because a frame is only the same drawing at another time.
    pub fn ghosts(&self) -> Vec<Shape> {
        self.selected
            .iter()
            .filter_map(|k| self.sheet.marks.get(*k))
            .flat_map(|m| {
                m.track
                    .moments()
                    .into_iter()
                    .filter(|at| (at - self.clock).abs() > crate::track::SAME)
                    .map(|at| m.at(at))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// The figure everything chosen belongs to, if they all belong to one.
    ///
    /// `None` when nothing is chosen, when what is chosen is loose, or when it
    /// spans two figures — because in all three cases there is no single
    /// figure for an option to be *about*.
    pub fn chosen_group(&self) -> Option<u32> {
        let mut found: Option<u32> = None;
        for k in &self.selected {
            let g = self.sheet.marks.get(*k)?.group;
            if g == 0 || found.is_some_and(|f| f != g) {
                return None;
            }
            found = Some(g);
        }
        found
    }

    /// The colour everything chosen is, if they are all the same.
    pub fn chosen_colour(&self) -> Option<u32> {
        let mut found: Option<u32> = None;
        for k in &self.selected {
            let c = self.sheet.marks.get(*k)?.colour;
            if found.is_some_and(|f| f != c) {
                return None;
            }
            found = Some(c);
        }
        found
    }

    /// Paint whatever is chosen.
    ///
    /// With nothing chosen this sets the pen instead, so the swatches always
    /// do something — a control that is dead half the time gets treated as
    /// broken.
    pub fn paint(&mut self, colour: u32) {
        if self.selected.is_empty() {
            self.colour = colour;
            return;
        }
        self.colour = colour;
        self.to_each(|m| m.colour = colour);
    }

    /// Scroll the tree, never past either end.
    pub fn scroll(&mut self, by: f64, most: f64) {
        self.scrolled = (self.scrolled + by).clamp(0.0, most.max(0.0));
    }

    /// Fold a figure shut, or open it again.
    pub fn fold(&mut self, group: u32) {
        match self.folded.iter().position(|g| *g == group) {
            Some(k) => {
                self.folded.remove(k);
            }
            None => self.folded.push(group),
        }
    }

    /// Choose a whole figure.
    pub fn choose_group(&mut self, group: u32) {
        self.selected = (0..self.sheet.len()).filter(|k| self.sheet.marks[*k].group == group).collect();
    }

    /// Choose one mark, and nothing else.
    pub fn choose_only(&mut self, k: usize) {
        self.selected = if k < self.sheet.len() { vec![k] } else { Vec::new() };
    }

    /// Make an empty figure to drag things into.
    ///
    /// A group with no members has nowhere to live in the sheet, since a
    /// group is only a number written on marks. So this groups whatever is
    /// chosen if anything is, and otherwise says so rather than pretending.
    pub fn new_group(&mut self) -> bool {
        self.group()
    }

    /// Move a mark to sit before another, which is what dragging a line does.
    ///
    /// Order is **paint order** — later is on top — so this is how one shape
    /// is put in front of another.
    pub fn move_mark(&mut self, from: usize, before: usize) -> bool {
        if from >= self.sheet.len() || before > self.sheet.len() || from == before {
            return false;
        }
        self.past.remember(&self.sheet);
        let m = self.sheet.marks.remove(from);
        // Removing shifts everything above down one, so a target that was
        // above the thing moved is now one lower. Getting this wrong puts the
        // mark one place from where it was dropped, every time, in one
        // direction only -- which is the kind of bug people work around
        // without ever reporting.
        let at = if before > from { before - 1 } else { before };
        self.sheet.marks.insert(at.min(self.sheet.len()), m);
        self.selected = vec![at.min(self.sheet.len() - 1)];
        true
    }

    /// Put a mark into a figure, or take it out of one.
    pub fn put_in_group(&mut self, mark: usize, group: u32) -> bool {
        if mark >= self.sheet.len() {
            return false;
        }
        self.past.remember(&self.sheet);
        self.sheet.marks[mark].group = group;
        true
    }

    /// Does anything on the page move at all?
    pub fn has_animation(&self) -> bool {
        self.sheet.marks.iter().any(Mark::moves)
    }

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

    /// Forget every step back, for when a different drawing is opened.
    ///
    /// Without it, undo after opening walks back into the drawing you had
    /// before — which looks exactly like data loss even though nothing was
    /// lost.
    pub fn forget_history(&mut self) {
        self.past.forget();
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

    /// The page as it should look **now** — every mark posed at the clock,
    /// plus the stroke in progress.
    ///
    /// The clock is zero until something plays, and a still mark's pose at
    /// zero is the identity, so a drawing with no animation in it draws
    /// exactly as it was made. There is no separate "editing" path to keep in
    /// step with the "playing" one, which is the kind of pair that drifts.
    pub fn frame(&self) -> Frame {
        let mut f = Frame::new();
        // The written half first, so hand-drawn marks sit on top of the
        // scaffolding they were drawn over.
        for (shape, colour) in self.written().shapes {
            f.add(shape).color(colour).width(2);
        }
        for m in &self.sheet.marks {
            let item = f.add(m.at(self.clock)).color(m.colour);
            if m.filled {
                item.fill();
            }
        }
        if let Some(wet) = self.drawing() {
            f.add(wet.shape()).color(wet.colour).fill();
        }
        f
    }

    /// Set off whatever rules a tap on this figure has.
    ///
    /// Returns whether any fired, so the studio can say nothing happened
    /// rather than leaving somebody tapping a shape that has no rule.
    pub fn play_tap(&mut self, group: u32) -> bool {
        let rules = self.sheet.script.rules();
        let wanted: Vec<_> = rules.iter().filter(|r| r.on == rule::On::Tap(group)).collect();
        if wanted.is_empty() {
            return false;
        }
        // The environment is taken **once, before any rule runs**, so all the
        // rules of one tap see the same picture. Re-reading it between them
        // would make the order of two unrelated rules matter, which is a thing
        // nobody would ever think to check.
        // The same source the drawing is made from, so a deed saying
        // `cheer = time` means the moment the tap happened -- and so every
        // rule of one tap sees the same picture, since re-reading it between
        // them would make the order of two unrelated rules matter.
        let env = self.sheet.script.env(self.clock, &self.tally);
        for r in wanted {
            rule::carry_out(r, &mut self.tally, &env);
        }
        true
    }

    /// Start the game again from the beginning.
    pub fn restart(&mut self) {
        self.tally.clear();
        self.clock = 0.0;
    }

    /// Run the script at the current clock.
    ///
    /// Every frame, deliberately. A written shape may mention `time`, and
    /// caching it would mean deciding when the cache is stale — a question
    /// with no good answer, since a row can depend on the clock through three
    /// other rows. Parsing a few dozen short lines is far cheaper than being
    /// wrong about it.
    pub fn written(&self) -> crate::script::Made {
        self.sheet.script.play(self.clock, &self.tally)
    }

    /// A ring round whatever is selected, so you can see what a command will
    /// act on. Separate from [`frame`](Board::frame) because it is furniture
    /// rather than drawing, and must not be saved or exported.
    pub fn selection(&self) -> Vec<Shape> {
        self.selected
            .iter()
            .filter_map(|k| {
                let m = self.sheet.marks.get(*k)?;
                let pose = m.pose_at(self.clock);
                let here = m.anchor();
                let (lo, hi) = m.bounds()?;
                let pad = 0.08 + (hi - lo).abs() * 0.03;
                let (a, b) = (lo - Cx::new(pad, pad), hi + Cx::new(pad, pad));
                let corners = vec![a, Cx::new(b.re, a.im), b, Cx::new(a.re, b.im)];
                // Moved with the mark, or the ring stays behind while the
                // thing it is pointing at walks away.
                Some(Shape::polygon(corners).map(move |z| pose.apply(z - here) + here))
            })
            .collect()
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

    /// Tap a point: press and release without moving.
    fn tap(b: &mut Board, at: Cx) {
        b.pointer(at, true);
        b.pointer(at, false);
    }

    /// ★ Erasing a mark below a chosen one must move the choice with it. An
    /// index that quietly starts meaning a different shape is how a walk gets
    /// given to the wrong thing, and it would look like the action buttons
    /// being broken.
    #[test]
    fn erasing_does_not_leave_the_selection_pointing_at_the_wrong_mark() {
        let mut b = Board::new();
        for k in 0..3 {
            draw(&mut b, &ring(0.4, Cx::new(k as f64 * 2.0, 0.0), 40));
        }
        tap(&mut b, Cx::new(4.0 - 0.4, 0.0));
        assert_eq!(b.selected, vec![2], "the third one");

        // Rub out the first.
        b.tool = Tool::Erase;
        b.pointer(Cx::new(-0.4, 0.0), true);
        b.pointer(Cx::new(-0.35, 0.0), true);
        b.pointer(Cx::new(-0.35, 0.0), false);
        assert_eq!(b.selected, vec![1], "it should still be the same shape");

        // And rubbing out a chosen one drops it rather than pointing at
        // whatever slid into its place.
        b.pointer(Cx::new(4.0 - 0.4, 0.0), true);
        b.pointer(Cx::new(4.0 - 0.35, 0.0), true);
        b.pointer(Cx::new(4.0 - 0.35, 0.0), false);
        assert!(b.selected.is_empty());
    }

    /// ★ **A tap chooses, a drag draws.** The gesture was free: a tap already
    /// made no mark, because `Ink::lift` refuses anything under two points. So
    /// choosing no longer costs a trip to the pick tool and back — which was
    /// three deliberate acts to say one thing.
    #[test]
    fn a_tap_chooses_without_changing_tool() {
        let mut b = Board::new();
        draw(&mut b, &ring(1.0, Cx::ZERO, 60));
        assert_eq!(b.tool, Tool::Draw);

        tap(&mut b, Cx::new(1.0, 0.0));
        assert_eq!(b.selected, vec![0], "a tap should choose it");
        assert_eq!(b.sheet.len(), 1, "and must not leave a speck of ink");
    }

    /// ★ And tapping **toggles**, so a selection can be several marks with no
    /// modifier key — which matters here, because this is used with a pen,
    /// where there is no second button and shift already means "talk to the
    /// graph".
    #[test]
    fn tapping_toggles_so_several_things_can_be_chosen() {
        let mut b = Board::new();
        for k in 0..3 {
            draw(&mut b, &ring(0.4, Cx::new(k as f64 * 2.0, 0.0), 40));
        }
        tap(&mut b, Cx::new(-0.4, 0.0));
        tap(&mut b, Cx::new(2.0 - 0.4, 0.0));
        assert_eq!(b.selected, vec![0, 1], "both");

        tap(&mut b, Cx::new(-0.4, 0.0));
        assert_eq!(b.selected, vec![1], "tapping again lets go");

        tap(&mut b, Cx::new(40.0, 40.0));
        assert!(b.selected.is_empty(), "and tapping the paper chooses nothing");
    }

    /// A drag is not a tap, however slowly it is made — and once it has
    /// wandered it stays a drag, or a careful hand flickers between the two.
    #[test]
    fn a_slow_deliberate_drag_never_becomes_a_tap() {
        let mut b = Board::new();
        draw(&mut b, &ring(1.0, Cx::ZERO, 60));
        let before = b.sheet.len();

        // Out and back, ending exactly where it began.
        b.pointer(Cx::new(3.0, 3.0), true);
        for k in 1..=30 {
            b.pointer(Cx::new(3.0 + k as f64 * 0.05, 3.0), true);
        }
        for k in (0..=30).rev() {
            b.pointer(Cx::new(3.0 + k as f64 * 0.05, 3.0), true);
        }
        b.pointer(Cx::new(3.0, 3.0), false);

        assert_eq!(b.sheet.len(), before + 1, "it should have drawn");
        assert!(b.selected.is_empty(), "and not also chosen something");
    }

    /// ★ One press gives a whole figure a walk. Telling six strokes one at a
    /// time is the thing that made this tiring.
    #[test]
    fn a_grouped_figure_is_chosen_and_told_all_at_once() {
        let mut b = Board::new();
        for k in 0..4 {
            draw(&mut b, &ring(0.4, Cx::new(k as f64 * 2.0, 0.0), 40));
        }
        for k in 0..3 {
            tap(&mut b, Cx::new(k as f64 * 2.0 - 0.4, 0.0));
        }
        assert!(b.group(), "three chosen should make a figure");

        // One tap now takes all three.
        b.selected.clear();
        tap(&mut b, Cx::new(2.0 - 0.4, 0.0));
        assert_eq!(b.selected.len(), 3, "tapping one member takes the figure");

        assert!(b.give(Action::Walk(Cx::new(1.0, 0.0)), Some(2.0)));
        for k in 0..3 {
            assert_eq!(b.sheet.marks[k].act.steps.len(), 1, "mark {k} should have been told");
        }
        assert!(b.sheet.marks[3].act.steps.is_empty(), "and the one left out should not");
    }

    /// Giving a whole figure a walk is **one** step back, not one per stroke.
    #[test]
    fn telling_a_figure_to_walk_is_one_step_back() {
        let mut b = Board::new();
        for k in 0..3 {
            draw(&mut b, &ring(0.4, Cx::new(k as f64 * 2.0, 0.0), 40));
        }
        for k in 0..3 {
            tap(&mut b, Cx::new(k as f64 * 2.0 - 0.4, 0.0));
        }
        b.give(Action::Spin(0.5), Some(2.0));
        assert!(b.undo());
        assert!(b.sheet.marks.iter().all(|m| m.act.steps.is_empty()));
    }

    /// A group needs two. One mark on its own is already a figure of one, and
    /// giving it a number would only use one up.
    #[test]
    fn one_thing_is_not_a_group() {
        let mut b = Board::new();
        draw(&mut b, &ring(1.0, Cx::ZERO, 60));
        tap(&mut b, Cx::new(1.0, 0.0));
        assert!(!b.group());
        assert_eq!(b.sheet.marks[0].group, 0);
    }

    /// ★ Group numbers are never reused. A freed number handed out again would
    /// silently adopt any mark still carrying it — a stroke you ungrouped
    /// months ago rejoining a figure it has nothing to do with.
    #[test]
    fn a_group_number_is_never_handed_out_twice() {
        let mut b = Board::new();
        for k in 0..4 {
            draw(&mut b, &ring(0.4, Cx::new(k as f64 * 2.0, 0.0), 40));
        }
        tap(&mut b, Cx::new(-0.4, 0.0));
        tap(&mut b, Cx::new(2.0 - 0.4, 0.0));
        b.group();
        let first = b.sheet.marks[0].group;

        b.selected.clear();
        tap(&mut b, Cx::new(4.0 - 0.4, 0.0));
        tap(&mut b, Cx::new(6.0 - 0.4, 0.0));
        b.group();
        assert_ne!(b.sheet.marks[2].group, first, "the second figure must be its own");
    }

    /// Ungrouping breaks it up, and a tap then takes only what was tapped.
    #[test]
    fn ungrouping_lets_the_strokes_go_their_own_way() {
        let mut b = Board::new();
        for k in 0..2 {
            draw(&mut b, &ring(0.4, Cx::new(k as f64 * 2.0, 0.0), 40));
        }
        tap(&mut b, Cx::new(-0.4, 0.0));
        tap(&mut b, Cx::new(2.0 - 0.4, 0.0));
        b.group();
        assert!(b.ungroup());

        b.selected.clear();
        tap(&mut b, Cx::new(-0.4, 0.0));
        assert_eq!(b.selected, vec![0], "only the one tapped");
    }

    /// ★ Dragging one member of a figure moves the whole figure. Moving a
    /// head off its body is the single most annoying thing an editor can do.
    #[test]
    fn dragging_a_figure_moves_all_of_it() {
        let mut b = Board::new();
        for k in 0..3 {
            draw(&mut b, &ring(0.4, Cx::new(k as f64 * 2.0, 0.0), 40));
        }
        for k in 0..3 {
            tap(&mut b, Cx::new(k as f64 * 2.0 - 0.4, 0.0));
        }
        b.group();
        let was: Vec<Cx> = b.sheet.marks.iter().map(|m| m.anchor()).collect();

        b.tool = Tool::Pick;
        b.pointer(Cx::new(-0.4, 0.0), true);
        for k in 1..=20 {
            b.pointer(Cx::new(-0.4, k as f64 * 0.15), true);
        }
        b.pointer(Cx::new(-0.4, 3.0), false);

        for k in 0..3 {
            let moved = b.sheet.marks[k].anchor() - was[k];
            assert!((moved.im - 3.0).abs() < 0.01, "mark {k} moved {moved:?}, not 3 up");
        }
    }

    /// ★ A drawing with nothing animated in it draws exactly as it was made.
    /// There is one drawing path rather than an "editing" one and a "playing"
    /// one, because that is the kind of pair that drifts apart.
    #[test]
    fn a_still_drawing_is_unaffected_by_the_clock() {
        let mut b = Board::new();
        draw(&mut b, &ring(1.0, Cx::ZERO, 60));
        let at_rest = b.sheet.marks[0].at(0.0);
        let much_later = b.sheet.marks[0].at(500.0);
        let (lo, hi) = (Cx::new(-9.0, -9.0), Cx::new(9.0, 9.0));
        assert_eq!(at_rest.polylines(lo, hi, 400).len(), much_later.polylines(lo, hi, 400).len());
    }

    /// ★ An action goes to the mark that is selected, and to no mark at all if
    /// none is.
    #[test]
    fn an_action_goes_to_the_chosen_mark() {
        let mut b = Board::new();
        draw(&mut b, &ring(1.0, Cx::ZERO, 60));
        draw(&mut b, &ring(1.0, Cx::new(5.0, 0.0), 60));

        assert!(!b.give(Action::Spin(1.0), None), "nothing is selected yet");
        assert!(b.sheet.marks.iter().all(|m| m.act.steps.is_empty()));

        tap(&mut b, Cx::new(5.0 + 1.0, 0.0));
        assert!(b.give(Action::Spin(1.0), None));
        assert!(b.sheet.marks[0].act.steps.is_empty(), "the other one must be untouched");
        assert_eq!(b.sheet.marks[1].act.steps.len(), 1);
    }

    /// Actions build up in order, which is how "walk, then jump" is said.
    #[test]
    fn actions_stack_up_into_a_sequence() {
        let mut b = Board::new();
        draw(&mut b, &ring(1.0, Cx::ZERO, 60));
        tap(&mut b, Cx::new(1.0, 0.0));

        b.give(Action::Walk(Cx::new(1.0, 0.0)), Some(2.0));
        b.give(Action::Jump { height: 1.0, rate: 1.0 }, Some(1.0));
        let act = &b.sheet.marks[0].act;
        assert_eq!(act.steps.len(), 2);
        assert_eq!(act.steps[0].action, Action::Walk(Cx::new(1.0, 0.0)));
        assert_eq!(act.steps[1].action, Action::Jump { height: 1.0, rate: 1.0 });

        assert!(b.undo(), "and giving something an action is undoable");
        assert_eq!(b.sheet.marks[0].act.steps.len(), 1);
    }

    /// ★ The clock only moves while it is playing, and rewinding goes back to
    /// the beginning. Taking the time from the wall clock instead would make
    /// pause impossible — everything would jump forward by however long you
    /// paused for.
    #[test]
    fn the_clock_stands_still_unless_it_is_playing() {
        let mut b = Board::new();
        b.tick(1.0);
        assert_eq!(b.clock, 0.0, "it should not run before you press play");

        b.play(true);
        b.tick(0.5);
        b.tick(0.5);
        assert!((b.clock - 1.0).abs() < 1e-9);

        b.play(false);
        b.tick(5.0);
        assert!((b.clock - 1.0).abs() < 1e-9, "pausing should stay put, not skip ahead");

        b.rewind();
        assert_eq!(b.clock, 0.0);
        assert!(!b.playing);
    }

    /// ★ And the animation survives a file. A studio that reopened a drawing
    /// as a set of motionless shapes would have lost the part that took
    /// longest.
    #[test]
    fn an_animation_can_be_saved_and_run_again() {
        let mut b = Board::new();
        draw(&mut b, &ring(1.0, Cx::ZERO, 60));
        tap(&mut b, Cx::new(1.0, 0.0));
        b.give(Action::Walk(Cx::new(2.0, 0.0)), Some(2.0));
        b.give(Action::Jump { height: 1.5, rate: 2.0 }, Some(1.0));

        let path = std::env::temp_dir().join("easel-animation.easel");
        let path = path.to_str().expect("a path");
        b.save(path).expect("saved");

        let mut opened = Board::new();
        opened.load(path).expect("loaded");
        assert!(opened.has_animation());
        for t in [0.0, 0.5, 1.9, 2.1, 2.9] {
            let there = b.sheet.marks[0].act.at(t);
            let here = opened.sheet.marks[0].act.at(t);
            assert!((there.b - here.b).abs() < 1e-3, "at t={t} it moved differently");
        }
        let _ = std::fs::remove_file(path);
    }

    /// ★ **At zero you edit the shape; after zero you edit the animation.** One
    /// rule instead of a record button whose state everybody forgets.
    #[test]
    fn dragging_at_zero_moves_the_shape_and_dragging_later_makes_a_key() {
        let mut b = Board::new();
        draw(&mut b, &ring(1.0, Cx::ZERO, 60));
        let drawn = b.sheet.marks[0].pts.clone();
        tap(&mut b, Cx::new(1.0, 0.0));
        b.tool = Tool::Pick;

        // At zero: the geometry itself moves.
        b.pointer(Cx::new(1.0, 0.0), true);
        b.pointer(Cx::new(2.0, 0.0), true);
        b.pointer(Cx::new(3.0, 0.0), false);
        assert!(b.sheet.marks[0].track.is_empty(), "no keys should have been made");
        assert!((b.sheet.marks[0].pts[0] - drawn[0] - Cx::new(2.0, 0.0)).abs() < 1e-9);

        // Past zero: the geometry stays put and a key appears.
        let now = b.sheet.marks[0].pts.clone();
        b.clock = 2.0;
        b.pointer(Cx::new(3.0, 0.0), true);
        b.pointer(Cx::new(3.0, 1.0), true);
        b.pointer(Cx::new(3.0, 2.0), false);
        assert_eq!(b.sheet.marks[0].pts, now, "the shape itself must not have moved");
        assert_eq!(b.sheet.marks[0].track.len(), 2, "one key here, and one holding at zero");
    }

    /// ★ The first key you make plants one at zero. Without it a single key
    /// would apply at every moment — one key is one pose for all time — and
    /// the shape would jump to its new place the instant the drawing opened.
    #[test]
    fn the_first_key_leaves_the_shape_where_it_was_at_the_start() {
        let mut b = Board::new();
        draw(&mut b, &ring(1.0, Cx::ZERO, 60));
        tap(&mut b, Cx::new(1.0, 0.0));
        b.tool = Tool::Pick;

        b.clock = 1.5;
        b.pointer(Cx::new(1.0, 0.0), true);
        b.pointer(Cx::new(1.0, 2.0), true);
        b.pointer(Cx::new(1.0, 4.0), false);

        let m = &b.sheet.marks[0];
        assert!(m.pose_at(0.0).b.abs() < 1e-9, "at the start it should be where it was drawn");
        assert!((m.pose_at(1.5).b.im - 4.0).abs() < 0.01, "and at 1.5 where it was put: {:?}", m.pose_at(1.5).b);
    }

    /// Keys win over verbs, and they are not composed. A shape pushed by two
    /// hands at once cannot be edited, because undoing what you see means
    /// guessing which half caused it.
    #[test]
    fn keys_win_over_verbs_rather_than_adding_to_them() {
        let mut b = Board::new();
        draw(&mut b, &ring(1.0, Cx::ZERO, 60));
        tap(&mut b, Cx::new(1.0, 0.0));
        b.give(Action::Drift(Cx::new(5.0, 0.0)), Some(4.0));

        let m = &mut b.sheet.marks[0];
        m.track.set(0.0, Pose::STILL, Ease::Linear);
        m.track.set(2.0, Pose::new(Cx::ONE, Cx::new(0.0, 1.0)), Ease::Linear);
        // Halfway: the key says half a unit up and nothing sideways. The verb
        // would have said two and a half units along.
        let half = b.sheet.marks[0].pose_at(1.0).b;
        assert!(half.re.abs() < 1e-9, "the drift must not still be adding: {half:?}");
        assert!((half.im - 0.5).abs() < 1e-9);
    }

    /// A key can be dropped and taken away without dragging anything, for
    /// making a shape wait where it is.
    #[test]
    fn a_key_can_be_left_and_removed_where_it_stands() {
        let mut b = Board::new();
        draw(&mut b, &ring(1.0, Cx::ZERO, 60));
        tap(&mut b, Cx::new(1.0, 0.0));

        b.clock = 1.0;
        assert!(b.key());
        assert!(b.on_a_key());
        assert_eq!(b.keys_here(), 2, "and one at the start");

        assert!(b.unkey());
        assert!(!b.on_a_key());
        assert!(!b.unkey(), "twice is nothing");
    }

    /// ★ Stepping between keys, so the moments you set can be reached again
    /// rather than hunted for by dragging a clock.
    #[test]
    fn the_clock_can_step_between_the_moments_you_set() {
        let mut b = Board::new();
        draw(&mut b, &ring(1.0, Cx::ZERO, 60));
        tap(&mut b, Cx::new(1.0, 0.0));
        for at in [0.0, 1.0, 2.5] {
            b.clock = at;
            b.key();
        }

        b.clock = 0.0;
        assert!(b.next_key(true));
        assert!((b.clock - 1.0).abs() < 1e-9);
        assert!(b.next_key(true));
        assert!((b.clock - 2.5).abs() < 1e-9);
        assert!(!b.next_key(true), "there is nothing after the last");

        assert!(b.next_key(false));
        assert!((b.clock - 1.0).abs() < 1e-9);
    }

    /// Onion skinning: the chosen marks at each moment they are keyed at, so a
    /// movement can be seen rather than remembered.
    #[test]
    fn the_other_moments_can_be_seen_faintly() {
        let mut b = Board::new();
        draw(&mut b, &ring(1.0, Cx::ZERO, 60));
        tap(&mut b, Cx::new(1.0, 0.0));
        for at in [0.0, 1.0, 2.0] {
            b.clock = at;
            b.key();
        }
        b.clock = 1.0;
        // The one you are on is not a ghost of itself.
        assert_eq!(b.ghosts().len(), 2);

        b.selected.clear();
        assert!(b.ghosts().is_empty(), "nothing chosen, nothing to compare");
    }

    /// ★ And an animation made of keys survives the file and runs the same.
    #[test]
    fn an_animation_made_of_keys_reopens_and_runs_the_same() {
        let mut b = Board::new();
        draw(&mut b, &ring(1.0, Cx::ZERO, 60));
        tap(&mut b, Cx::new(1.0, 0.0));
        b.tool = Tool::Pick;
        for (when, to) in [(1.0, Cx::new(2.0, 1.0)), (2.0, Cx::new(4.0, -1.0))] {
            b.clock = when;
            b.pointer(Cx::new(1.0, 0.0), true);
            b.pointer(Cx::new(1.0, 0.0) + to.scale(0.5), true);
            b.pointer(Cx::new(1.0, 0.0) + to, false);
        }

        let path = std::env::temp_dir().join("easel-keys.easel");
        let path = path.to_str().expect("a path");
        b.save(path).expect("saved");
        let mut opened = Board::new();
        opened.load(path).expect("loaded");
        assert!(opened.has_animation());
        for t in [0.0, 0.4, 1.0, 1.7, 2.0, 3.0] {
            let there = b.sheet.marks[0].pose_at(t);
            let here = opened.sheet.marks[0].pose_at(t);
            assert!((there.b - here.b).abs() < 1e-3, "at t={t} it moved differently");
            assert!((there.a - here.a).abs() < 1e-3);
        }
        let _ = std::fs::remove_file(path);
    }

    /// ★ Written shapes and drawn ones live in the same picture, and one
    /// number moves everything that mentions it.
    #[test]
    fn written_shapes_are_drawn_alongside_the_hand_drawn_ones() {
        let mut b = Board::new();
        draw(&mut b, &ring(1.0, Cx::ZERO, 60));
        let drawn_only = b.frame().len();

        b.sheet.script.add("r = 2");
        b.sheet.script.add("circle(4, r)");
        b.sheet.script.add("ngon(-4, r, 6)");
        assert_eq!(b.frame().len(), drawn_only + 2, "both halves should be in the picture");

        b.sheet.script.set_dial("r", 3.0);
        assert_eq!(b.frame().len(), drawn_only + 2, "and still both after a dial moves");
    }

    /// ★ Typing goes into the row and nowhere else. While a row is being
    /// typed the keyboard belongs to it -- typing `p` in a formula must not
    /// also switch to the pick tool.
    #[test]
    fn typing_goes_into_the_row_being_edited() {
        let mut b = Board::new();
        b.add_row();
        assert_eq!(b.editing, Some(0));
        for c in ["c", "i", "r", "c", "l", "e", "(", "0", ",", " ", "2", ")"] {
            b.type_into(c);
        }
        assert_eq!(b.sheet.script.rows[0].text, "circle(0, 2)");
        assert_eq!(b.frame().len(), 1, "and it draws");

        b.edit(None);
        assert!(!b.type_into("x"), "nothing is being typed into now");
        assert_eq!(b.sheet.script.rows[0].text, "circle(0, 2)");
    }

    /// ★ Backspace on an empty row takes the row away. That is what pressing
    /// it on nothing means, and the alternative is a drawing slowly filling
    /// with blank rows nobody can get rid of.
    #[test]
    fn rubbing_out_an_empty_row_removes_it() {
        let mut b = Board::new();
        b.add_row();
        b.type_into("ab");
        assert!(b.rub_out());
        assert_eq!(b.sheet.script.rows[0].text, "a");
        assert!(b.rub_out());
        assert_eq!(b.sheet.script.rows[0].text, "");

        assert!(b.rub_out(), "and again takes the row itself");
        assert_eq!(b.sheet.script.len(), 0);
        assert_eq!(b.editing, None);
    }

    /// A whole edit is one step back, not one per keystroke -- an undo that
    /// walked back through every character would be useless for getting out of
    /// a mess.
    #[test]
    fn retyping_a_row_is_one_step_back() {
        let mut b = Board::new();
        b.sheet.script.add("circle(0, 1)");
        let before = b.sheet.clone();

        b.edit(Some(0));
        for _ in 0..6 {
            b.type_into("x");
        }
        assert!(b.undo());
        assert_eq!(b.sheet, before);
    }

    /// A row switched off is kept, and switching it is undoable.
    #[test]
    fn a_row_can_be_switched_off_and_back() {
        let mut b = Board::new();
        b.sheet.script.add("circle(0, 1)");
        assert_eq!(b.frame().len(), 1);

        assert!(b.toggle_row(0));
        assert_eq!(b.frame().len(), 0, "off means it does not draw");
        assert_eq!(b.sheet.script.rows[0].text, "circle(0, 1)", "but it is still there");

        assert!(b.undo());
        assert_eq!(b.frame().len(), 1);
    }

    /// ★ Moving a slider moves everything that mentions the variable.
    #[test]
    fn a_slider_moves_everything_that_mentions_it() {
        let mut b = Board::new();
        b.sheet.script.add("r = 2");
        b.sheet.script.add("circle(0, r)");
        b.sheet.script.add("ngon(5, r, 6)");

        let reach = |b: &Board| {
            let (lo, hi) = (Cx::new(-50.0, -50.0), Cx::new(50.0, 50.0));
            b.written()
                .shapes
                .iter()
                .flat_map(|(s, _)| s.polylines(lo, hi, 600))
                .flatten()
                .map(|z| z.im.abs())
                .fold(0.0, f64::max)
        };
        let small = reach(&b);
        assert!(b.set_dial(0, 5.0));
        assert!(reach(&b) > small * 2.0, "both shapes should have grown");
    }

    /// ★ Colour belongs to the shape, not to the program. With something
    /// chosen the swatches repaint it; with nothing chosen they set the pen,
    /// so they always do something -- a control that is dead half the time
    /// gets treated as broken.
    #[test]
    fn a_swatch_paints_what_is_chosen_or_sets_the_pen() {
        let mut b = Board::new();
        draw(&mut b, &ring(1.0, Cx::ZERO, 60));
        let was = b.sheet.marks[0].colour;

        b.paint(0xFF0000);
        assert_eq!(b.sheet.marks[0].colour, was, "nothing chosen, nothing repainted");
        assert_eq!(b.colour, 0xFF0000, "but the pen changed");

        tap(&mut b, Cx::new(1.0, 0.0));
        b.paint(0x00FF00);
        assert_eq!(b.sheet.marks[0].colour, 0x00FF00, "chosen, so repainted");
        assert_eq!(b.chosen_colour(), Some(0x00FF00));
    }

    /// And repainting a whole figure is one press and one step back.
    #[test]
    fn painting_a_figure_is_one_press() {
        let mut b = Board::new();
        for k in 0..3 {
            draw(&mut b, &ring(0.4, Cx::new(k as f64 * 2.0, 0.0), 40));
        }
        b.selected = vec![0, 1, 2];
        b.group();
        b.paint(0x123456);
        assert!(b.sheet.marks.iter().all(|m| m.colour == 0x123456));
        assert!(b.undo());
        assert!(b.sheet.marks.iter().all(|m| m.colour != 0x123456));
    }

    /// ★ Two things chosen with different colours have no one colour, and
    /// saying otherwise would show a swatch as lit that is not the truth.
    #[test]
    fn a_mixed_selection_has_no_one_colour() {
        let mut b = Board::new();
        for k in 0..2 {
            b.colour = if k == 0 { 0x111111 } else { 0x222222 };
            draw(&mut b, &ring(0.4, Cx::new(k as f64 * 2.0, 0.0), 40));
        }
        b.selected = vec![0, 1];
        assert_eq!(b.chosen_colour(), None);
        assert_eq!(b.chosen_group(), None, "and nor do they belong to one figure");
    }

    /// ★ Dragging a line reorders the drawing, and order is paint order.
    /// The index has to be adjusted after removing, or the mark lands one
    /// place from where it was dropped -- every time, in one direction, which
    /// is the kind of bug people work around without ever reporting.
    #[test]
    fn dragging_a_line_reorders_and_lands_where_it_was_dropped() {
        let mut b = Board::new();
        for k in 0..4 {
            b.colour = 0x100 * (k + 1) as u32;
            draw(&mut b, &ring(0.4, Cx::new(k as f64 * 2.0, 0.0), 40));
        }
        let colours = |b: &Board| b.sheet.marks.iter().map(|m| m.colour).collect::<Vec<_>>();
        assert_eq!(colours(&b), vec![0x100, 0x200, 0x300, 0x400]);

        // Move the first to the end.
        assert!(b.move_mark(0, 4));
        assert_eq!(colours(&b), vec![0x200, 0x300, 0x400, 0x100]);

        // And the last back to the front.
        assert!(b.move_mark(3, 0));
        assert_eq!(colours(&b), vec![0x100, 0x200, 0x300, 0x400]);
        assert!(b.undo());
    }

    /// Folding is a way of looking, so it toggles and holds nothing else.
    #[test]
    fn folding_a_figure_toggles() {
        let mut b = Board::new();
        b.fold(3);
        assert!(b.folded.contains(&3));
        b.fold(3);
        assert!(!b.folded.contains(&3));
    }

    /// A tap leaves nothing — no invisible speck that can still be clicked on.
    #[test]
    fn a_tap_on_the_page_leaves_nothing_behind() {
        let mut b = Board::new();
        tap(&mut b, Cx::new(1.0, 1.0));
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
