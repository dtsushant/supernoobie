//! # tree — what the drawing is made of, down the left
//!
//! One list, two halves, and everything in it can be folded, dragged and added
//! to.
//!
//! ```text
//!     SHAPES                   [+]
//!     v  figure 1                     <- a group. tap the arrow to fold it
//!        - head
//!        - body
//!        - left arm
//!     >  figure 2                     <- folded: four strokes, out of the way
//!        - a loose stroke
//!
//!     FUNCTIONS                 [+]
//!     [x] r = 2
//!         --------o--------           <- because the row binds a number
//!     [x] circle(0, r)
//!     [ ] ngon(0, r, 6)               <- kept, not running
//! ```
//!
//! ## The options belong to the shape
//!
//! Choose a line and the things you can do **to that thing** open under it:
//! its colour, and what it does when the clock runs.
//!
//! ```text
//!     v  figure 1
//!        [][][][][][][][]                 its colour
//!        walk  run  jump  spin  bob       what it does
//!        pulse  none
//!        - head
//! ```
//!
//! Not on a toolbar. A global "current colour" can only ever mean *the colour
//! of the next stroke* — but what you usually want is to change the colour of
//! one already drawn, and a toolbar cannot say which one. Put the swatches
//! beside the thing and the question does not arise.
//!
//! Choosing a **group** and pressing `walk` tells the whole figure, which is
//! the point of having grouped it.
//!
//! ## Why one list and not two panels
//!
//! Because a drawing does not have two halves in any way that matters to the
//! person making it. A circle typed as `circle(0, r)` and a circle drawn by
//! hand are both *a circle in the picture*, and both want to be reordered,
//! hidden and grouped. Two panels would mean learning two sets of habits for
//! the same job.
//!
//! ## Size, and being able to read it
//!
//! The rows are set at **twice** the size of the toolbar's labels, and the
//! list is wide enough for them. A function is the one thing here you have to
//! read character by character — `max(0, 1.1 - 1.4*(time - cheer))` is not
//! something you recognise by its shape — so it is the one thing that must not
//! be small. Everything else on screen is a word you know by sight.
//!
//! ## Collapsing
//!
//! The whole list folds away to a strip, because the drawing is the point and
//! the list is how you got there. What is left is wide enough to press and
//! nothing else.
//!
//! ## Folding
//!
//! A figure is six strokes and you almost never want to see all six. Folded,
//! it is one line saying how many are inside. **The fold is not saved**: it is
//! about looking, not about the drawing, and a file that remembered which
//! folders you had open would make two people's copies differ for no reason
//! anybody cares about.
//!
//! ## Dragging
//!
//! Order is **paint order** — later is on top — so dragging a line up and down
//! is how one shape is put in front of another. Dropping a shape onto a group
//! puts it in that group.
//!
//! Where a drop will land is decided by the **gap** the pointer is nearest,
//! not by the line it is over. Over-a-line means "which half of it?", which is
//! a rule people have to be taught; nearest-gap is the rule they already
//! expect, because it is where the line is drawn.
//!
//! ## What a `+` does
//!
//! Under `SHAPES` it makes an empty group to drag things into. Under
//! `FUNCTIONS` it makes a row and starts typing in it. Neither asks a question
//! first — a dialog before you have done anything is a dialog answered wrongly.

use plotkit::{Anchor, Canvas, Cx, Frame};

use crate::action::Action;

use crate::board::Board;

/// How wide the tree is when it is open.
pub const WIDTH: i32 = 380;
/// How wide it is when it is collapsed: enough to press, and nothing more.
pub const SHUT: i32 = 26;
/// How far a slider reaches either side of zero.
pub const RANGE: f64 = 10.0;

/// The colours on offer, for whatever is chosen.
pub const INKS: [u32; 8] =
    [0xE3E9EF, 0xE0A44A, 0x4FBCD4, 0xE585AC, 0x6FCF97, 0x9B7BD4, 0xE0704A, 0x46525E];

/// How long one press of an action lasts.
///
/// The rates below are chosen so a whole number of cycles fits in it — an act
/// loops, and a cycle that does not close jerks every time round.
pub const STEP: f64 = 2.0;

/// What a chosen thing can be told to do, with a default worth watching.
///
/// One press should do something; an animation needing six decisions before it
/// moves is one nobody makes.
pub fn verbs_list() -> [(&'static str, Option<Action>); 7] {
    let right = Cx::new(1.6, 0.0);
    [
        ("walk", Some(Action::Walk(right))),
        ("run", Some(Action::Run(right))),
        ("jump", Some(Action::Jump { height: 1.2, rate: 1.5 })),
        ("spin", Some(Action::Spin(0.5))),
        ("bob", Some(Action::Bob { height: 0.4, rate: 0.5 })),
        ("pulse", Some(Action::Pulse { amount: 0.25, rate: 0.5 })),
        ("none", None),
    ]
}

const PAD: i32 = 8;
/// Tall enough for text at twice the size, with room round it.
const LINE: i32 = 36;
const HEAD: i32 = 28;
const SLIDER: i32 = 22;
const INDENT: i32 = 16;
/// How big the row text is. Two, not one: a function has to be read character
/// by character, and everything else on screen is a word you know by sight.
const TEXT: i32 = 2;
/// The fold arrow, and the tick, are this wide.
const KNOB: i32 = 26;
/// A swatch, and a verb button.
const SWATCH: i32 = 26;
const VERB: i32 = 46;

/// Which half of the tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Half {
    Shapes,
    Functions,
}

/// One line in the tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Node {
    Title(Half),
    /// A group, and how many marks are in it.
    Group(u32, usize),
    /// A mark, by its index in the sheet.
    Mark(usize),
    /// A script row, by its index.
    Row(usize),
}

impl Node {
    /// Can this line be picked up and moved?
    ///
    /// Titles cannot: they are what the halves *are*, and a list whose
    /// headings can be dragged about is a list that can be put into a state
    /// with no meaning.
    pub fn movable(self) -> bool {
        !matches!(self, Node::Title(_))
    }

    pub fn half(self) -> Half {
        match self {
            Node::Title(h) => h,
            Node::Row(_) => Half::Functions,
            _ => Half::Shapes,
        }
    }
}

/// A line, laid out.
#[derive(Clone, Debug)]
pub struct Line {
    pub node: Node,
    pub y: i32,
    pub h: i32,
    pub depth: i32,
    /// The dial under a formula row, if it binds a plain number.
    pub dial: Option<(String, f64, i32)>,
}

/// What was pressed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Poke {
    /// Open the list, or shut it away.
    Collapse,
    /// Fold or unfold a group.
    Fold(u32),
    /// Switch a formula row on or off.
    Tick(usize),
    /// Choose this — a shape, or a whole group.
    Choose(Node),
    /// Type in this row.
    Edit(usize),
    /// Move this row's dial.
    Dial(usize, f64),
    /// Add something to this half.
    Add(Half),
    /// Let go here: before the line at this index in the laid-out list.
    Drop(usize),
    /// Paint whatever is chosen this colour.
    Paint(u32),
    /// Tell whatever is chosen to do this. `None` means stop doing anything.
    Verb(Option<Action>),
}

/// The list, laid out.
#[derive(Clone, Debug, Default)]
pub struct Tree {
    pub lines: Vec<Line>,
    pub adds: Vec<(Half, i32)>,
    /// The options for whatever is chosen: where the swatches are, and where
    /// the verbs are.
    pub inspector: Option<Inspector>,
    pub height: i32,
}

/// The options belonging to whatever is chosen, laid out.
#[derive(Clone, Debug)]
pub struct Inspector {
    /// The line it opens under.
    pub under: usize,
    pub x: i32,
    pub swatches: i32,
    pub verbs: Vec<(&'static str, Option<Action>, i32, i32)>,
    pub h: i32,
}

impl Tree {
    /// How wide the tree is for this board — a strip when it is shut away.
    pub fn width(board: &Board) -> i32 {
        if board.tree_shut {
            SHUT
        } else {
            WIDTH
        }
    }

    /// Lay the tree out for this board.
    ///
    /// Everything is placed from the top and then shifted by however far the
    /// list has been scrolled, so there is one layout and scrolling is a
    /// single subtraction. Laying out from the scrolled position instead would
    /// mean every rectangle carried the scroll in it, and the day one of them
    /// forgot, that row would be unclickable at some scroll positions only.
    pub fn new(board: &Board) -> Tree {
        let mut t = Tree::default();
        if board.tree_shut {
            // Nothing but the handle to open it again. Laying the rest out
            // and then not painting it would leave every line hittable
            // underneath, which is the sort of thing that only shows up as
            // "the drawing sometimes ignores me near the left edge".
            return t;
        }
        let mut y = PAD;

        // --- shapes ---------------------------------------------------------
        t.lines.push(Line { node: Node::Title(Half::Shapes), y, h: HEAD, depth: 0, dial: None });
        t.adds.push((Half::Shapes, y));
        y += HEAD + 2;

        // Groups keep the order of their first member, so dragging a stroke
        // about moves its figure with it rather than shuffling the list.
        let mut done: Vec<u32> = Vec::new();
        for (k, m) in board.sheet.marks.iter().enumerate() {
            if m.group == 0 {
                t.lines.push(Line { node: Node::Mark(k), y, h: LINE, depth: 1, dial: None });
                y += LINE + 2;
                if board.selected == vec![k] {
                    y = t.open_options(y, 2);
                }
                continue;
            }
            if done.contains(&m.group) {
                continue;
            }
            done.push(m.group);
            let members: Vec<usize> =
                (0..board.sheet.len()).filter(|j| board.sheet.marks[*j].group == m.group).collect();
            let folded = board.folded.contains(&m.group);
            t.lines.push(Line { node: Node::Group(m.group, members.len()), y, h: LINE, depth: 1, dial: None });
            y += LINE + 2;
            if board.chosen_group() == Some(m.group) {
                y = t.open_options(y, 2);
            }
            if !folded {
                for j in members {
                    t.lines.push(Line { node: Node::Mark(j), y, h: LINE, depth: 2, dial: None });
                    y += LINE + 2;
                }
            }
        }

        // --- functions -------------------------------------------------------
        y += PAD;
        t.lines.push(Line { node: Node::Title(Half::Functions), y, h: HEAD, depth: 0, dial: None });
        t.adds.push((Half::Functions, y));
        y += HEAD + 2;

        // From what is actually in effect, not from the rows alone. A game's
        // score lives in the tally, and `Script::dials` runs the rows with an
        // empty one -- so a slider took its value from the starting position
        // and sat there while the game moved on. The board already works this
        // out once, for the drawing; asking it again is the only way the two
        // cannot disagree.
        let dials: Vec<(String, f64)> = board
            .written()
            .vars
            .into_iter()
            .filter(|(name, v)| name != "time" && v.im.abs() < 1e-12)
            .map(|(name, v)| (name, v.re))
            .collect();
        for (k, r) in board.sheet.script.rows.iter().enumerate() {
            let dial = r
                .binds()
                .and_then(|name| dials.iter().find(|(n, _)| n == name))
                .map(|(n, v)| (n.clone(), *v, y + LINE));
            let h = if dial.is_some() { LINE + SLIDER } else { LINE };
            t.lines.push(Line { node: Node::Row(k), y, h, depth: 1, dial });
            y += h + 2;
        }

        t.height = y + PAD;

        // The shift, applied once, at the end.
        let by = board.scrolled.round() as i32;
        if by != 0 {
            for l in t.lines.iter_mut() {
                l.y -= by;
                if let Some((_, _, sy)) = l.dial.as_mut() {
                    *sy -= by;
                }
            }
            for (_, ay) in t.adds.iter_mut() {
                *ay -= by;
            }
            if let Some(ins) = t.inspector.as_mut() {
                ins.swatches -= by;
                for (_, _, _, vy) in ins.verbs.iter_mut() {
                    *vy -= by;
                }
            }
        }
        t
    }

    /// How far the list could be scrolled before its end is on screen.
    ///
    /// Never negative: a list shorter than the window does not scroll at all,
    /// and one that could be dragged up off its own top is a list that feels
    /// broken.
    pub fn most(&self, window_h: i32) -> f64 {
        // `height` is worked out before the shift, so it is the full length of
        // the list however far it has been scrolled.
        f64::from((self.height - window_h + PAD).max(0))
    }

    /// Lay the options out at `y`, indented to `depth`. Returns the new `y`.
    fn open_options(&mut self, y: i32, depth: i32) -> i32 {
        let x = PAD + depth * INDENT;
        let mut verbs = Vec::new();
        let (mut vx, mut vy) = (x, y + KNOB + 4);
        for (name, action) in verbs_list() {
            if vx + VERB > WIDTH - PAD {
                vx = x;
                vy += LINE - 2;
            }
            verbs.push((name, action, vx, vy));
            vx += VERB + 3;
        }
        let bottom = vy + LINE - 2;
        self.inspector = Some(Inspector { under: self.lines.len(), x, swatches: y, verbs, h: bottom - y });
        bottom + 4
    }

    /// Is this pixel over the tree at all?
    pub fn covers_at(px: f64, width: i32) -> bool {
        px < f64::from(width)
    }

    /// Is this pixel over an **open** tree?
    pub fn covers(px: f64) -> bool {
        Tree::covers_at(px, WIDTH)
    }

    /// The handle that opens and shuts it, down the right-hand edge.
    pub fn handle(width: i32) -> (i32, i32) {
        (width - SHUT, SHUT)
    }

    /// What was pressed here.
    pub fn at(&self, px: f64, py: f64, width: i32) -> Option<Poke> {
        if !Tree::covers_at(px, width) {
            return None;
        }
        let (hx, hw) = Tree::handle(width);
        if px >= f64::from(hx) && px < f64::from(hx + hw) && py < f64::from(SHUT + PAD) {
            return Some(Poke::Collapse);
        }
        if self.lines.is_empty() {
            return None;
        }
        for (half, y) in &self.adds {
            let x = WIDTH - PAD - KNOB - SHUT;
            if py >= *y as f64 && py < (y + HEAD) as f64 && px >= x as f64 && px < (x + KNOB) as f64 {
                return Some(Poke::Add(*half));
            }
        }
        if let Some(ins) = &self.inspector {
            if py >= ins.swatches as f64 && py < (ins.swatches + KNOB) as f64 {
                let k = ((px - ins.x as f64) / SWATCH as f64).floor();
                if k >= 0.0 && (k as usize) < INKS.len() {
                    return Some(Poke::Paint(INKS[k as usize]));
                }
            }
            for (_, action, vx, vy) in &ins.verbs {
                if px >= *vx as f64 && px < (vx + VERB) as f64 && py >= *vy as f64 && py < (vy + LINE - 6) as f64 {
                    return Some(Poke::Verb(*action));
                }
            }
        }
        for line in &self.lines {
            if let Some((_, _, sy)) = &line.dial {
                let (sy, x0, x1) = (*sy, PAD + INDENT, WIDTH - PAD);
                if py >= sy as f64 && py < (sy + SLIDER) as f64 {
                    let s = ((px - x0 as f64) / (x1 - x0).max(1) as f64).clamp(0.0, 1.0);
                    if let Node::Row(k) = line.node {
                        return Some(Poke::Dial(k, (s * 2.0 - 1.0) * RANGE));
                    }
                }
            }
            if py < line.y as f64 || py >= (line.y + LINE.min(line.h)) as f64 {
                continue;
            }
            let knob = PAD + line.depth * INDENT;
            let on_knob = px < (knob + KNOB) as f64;
            return Some(match line.node {
                Node::Title(_) => continue,
                Node::Group(id, _) if on_knob => Poke::Fold(id),
                Node::Row(k) if on_knob => Poke::Tick(k),
                Node::Row(k) => Poke::Edit(k),
                other => Poke::Choose(other),
            });
        }
        None
    }

    /// Where a drop at this height would land: **before** this line.
    ///
    /// The nearest gap, not the line the pointer is over. Over-a-line means
    /// "which half of it?", a rule people have to be taught; nearest-gap is
    /// the one they already expect, because it is where the line is drawn.
    pub fn gap_at(&self, py: f64) -> usize {
        let mut best = (f64::MAX, 0usize);
        for (k, line) in self.lines.iter().enumerate() {
            let d = (py - line.y as f64).abs();
            if d < best.0 {
                best = (d, k);
            }
            let below = (py - (line.y + line.h) as f64).abs();
            if below < best.0 {
                best = (below, k + 1);
            }
        }
        best.1
    }

    /// Where a value sits along its slider, 0 to 1.
    fn along(value: f64) -> f64 {
        ((value / RANGE) * 0.5 + 0.5).clamp(0.0, 1.0)
    }

    /// Paint it, from the same rectangles [`Tree::at`] reads.
    pub fn paint(&self, f: &mut Frame, board: &Board, window_h: i32) {
        let width = Tree::width(board);
        let deep = window_h.max(self.height);
        f.chip(0, 0, width, deep, PANEL);
        f.chip(width - 1, 0, 1, deep, EDGE);

        // The handle, always. It is how the list comes back.
        let (hx, hw) = Tree::handle(width);
        f.chip(hx, PAD / 2, hw - 2, SHUT, EDGE);
        f.pin(
            Anchor::TopLeft,
            f64::from(hx + 8),
            f64::from(PAD / 2 + 9),
            if board.tree_shut { ">" } else { "<" },
            0xC3CDD7,
            1,
        );
        if board.tree_shut {
            return;
        }
        let made = board.written();

        for line in &self.lines {
            let x = PAD + line.depth * INDENT;
            match line.node {
                Node::Title(half) => {
                    let name = if half == Half::Shapes { "SHAPES" } else { "FUNCTIONS" };
                    f.pin(Anchor::TopLeft, x as f64, (line.y + 10) as f64, name, 0x6B7987, 1);
                    let ax = WIDTH - PAD - KNOB - SHUT;
                    f.chip(ax, line.y, KNOB, HEAD, EDGE);
                    f.pin(Anchor::TopLeft, (ax + 9) as f64, (line.y + 10) as f64, "+", 0xC3CDD7, TEXT);
                }
                Node::Group(id, n) => {
                    let chosen = board.chosen_group() == Some(id);
                    f.chip(x, line.y, WIDTH - PAD - x, LINE, if chosen { LIT } else { EDGE });
                    let arrow = if board.folded.contains(&id) { ">" } else { "v" };
                    f.pin(Anchor::TopLeft, (x + 7) as f64, (line.y + (LINE - 7 * TEXT) / 2) as f64, arrow, 0xE0A44A, TEXT);
                    f.pin(
                        Anchor::TopLeft,
                        (x + KNOB) as f64,
                        (line.y + (LINE - 7 * TEXT) / 2) as f64,
                        format!("figure {id}  ({n})"),
                        if chosen { 0xFFFFFF } else { INK },
                        TEXT,
                    );
                }
                Node::Mark(k) => {
                    let chosen = board.selected.contains(&k);
                    let colour = board.sheet.marks.get(k).map_or(INK, |m| m.colour);
                    if chosen {
                        f.chip(x, line.y, WIDTH - PAD - x, LINE, LIT);
                    }
                    // A swatch of the mark's own colour, so a list of six
                    // strokes is not six identical lines.
                    f.chip(x + 6, line.y + 8, 14, LINE - 16, colour);
                    let moves = board.sheet.marks.get(k).is_some_and(|m| m.moves());
                    f.pin(
                        Anchor::TopLeft,
                        (x + KNOB) as f64,
                        (line.y + (LINE - 7 * TEXT) / 2) as f64,
                        if moves { format!("stroke {k}  *") } else { format!("stroke {k}") },
                        if chosen { 0xFFFFFF } else { INK },
                        TEXT,
                    );
                }
                Node::Row(k) => {
                    let Some(r) = board.sheet.script.rows.get(k) else { continue };
                    let wrong = made.errors.iter().any(|(l, _)| *l == k + 1);
                    let typing = board.editing == Some(k);
                    f.chip(x, line.y, WIDTH - PAD - x, LINE, if typing { LIT } else { EDGE });
                    f.chip(x + 5, line.y + 6, KNOB - 12, LINE - 12, if r.on { 0x6FCF97 } else { 0x33404E });

                    let ink = if wrong {
                        0xE0704A
                    } else if r.on {
                        INK
                    } else {
                        0x6B7987
                    };
                    // The caret goes **where the caret is**, not always at the
                    // end. A bar that always sat at the end while typing went
                    // in somewhere else would be worse than none at all.
                    let text = if typing {
                        let at = r.text.char_indices().nth(board.caret).map_or(r.text.len(), |(b, _)| b);
                        format!("{}|{}", &r.text[..at], &r.text[at..])
                    } else {
                        r.text.clone()
                    };
                    let room = ((WIDTH - PAD - x - KNOB - 8) / Canvas::text_w("n", TEXT).max(1)) as usize;
                    // The END of a long row, not the beginning: what you are
                    // typing is at the end, and a box that scrolled away from
                    // the cursor would be useless to type into.
                    let n = text.chars().count();
                    let shown: String = if n > room {
                        // Scrolled to keep the **caret** in view, not always to
                        // the end. Editing the middle of a long row and being
                        // shown its end instead is typing blind.
                        let want = if typing { board.caret + 1 } else { n };
                        let from = want.saturating_sub(room).min(n - room);
                        text.chars().skip(from).take(room).collect()
                    } else {
                        text
                    };
                    f.pin(
                        Anchor::TopLeft,
                        (x + KNOB) as f64,
                        (line.y + (LINE - 7 * TEXT) / 2) as f64,
                        shown,
                        ink,
                        TEXT,
                    );

                    if let Some((name, value, sy)) = &line.dial {
                        let (x0, x1) = (PAD + INDENT, WIDTH - PAD);
                        let mid = *sy + SLIDER / 2;
                        f.chip(x0, mid - 1, x1 - x0, 2, 0x33404E);
                        let knob = x0 + ((x1 - x0) as f64 * Tree::along(*value)) as i32;
                        f.chip(knob - 4, mid - 7, 8, 14, 0xE0A44A);
                        f.pin(
                            Anchor::TopLeft,
                            x0 as f64,
                            (*sy + SLIDER - 8) as f64,
                            format!("{name} = {value:.2}"),
                            0x94A1AE,
                            1,
                        );
                    }
                }
            }
        }

        // The options for whatever is chosen, under its line.
        if let Some(ins) = &self.inspector {
            for (k, ink) in INKS.iter().enumerate() {
                let x = ins.x + k as i32 * SWATCH;
                if board.chosen_colour() == Some(*ink) {
                    f.chip(x - 1, ins.swatches - 1, SWATCH - 2, KNOB + 2, INK);
                }
                f.chip(x + 1, ins.swatches + 1, SWATCH - 6, KNOB - 4, *ink);
            }
            for (name, action, vx, vy) in &ins.verbs {
                let doing = board.chosen().is_some_and(|m| match action {
                    None => m.act.steps.is_empty() && m.track.is_empty(),
                    Some(a) => m.act.steps.iter().any(|s| s.action == *a),
                });
                f.chip(*vx, *vy, VERB, LINE - 6, if doing { LIT } else { EDGE });
                let indent = ((VERB - Canvas::text_w(name, 1)) / 2).max(2);
                f.pin(
                    Anchor::TopLeft,
                    (vx + indent) as f64,
                    (vy + (LINE - 6 - 7) / 2) as f64,
                    *name,
                    if doing { 0xFFFFFF } else { INK },
                    1,
                );
            }
        }

        // Where a drop would land, while something is being dragged.
        if let Some(before) = board.dropping {
            let y = self.lines.get(before).map_or(self.height, |l| l.y) - 1;
            f.chip(PAD, y, WIDTH - 2 * PAD, 2, 0xE0A44A);
        }

        if let Some((line, msg)) = made.errors.first() {
            f.pin(Anchor::TopLeft, PAD as f64, (self.height + 4) as f64, format!("row {line}: {msg}"), 0xE0704A, 1);
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
    use plotkit::Cx;
    use std::f64::consts::TAU;

    fn ring(r: f64, at: Cx) -> Vec<Cx> {
        (0..=40).map(|k| at + Cx::polar(r, k as f64 / 40.0 * TAU)).collect()
    }

    fn draw(b: &mut Board, path: &[Cx]) {
        for z in path {
            b.pointer(*z, true);
        }
        b.pointer(*path.last().expect("a path"), false);
    }

    fn a_board() -> Board {
        let mut b = Board::new();
        for k in 0..4 {
            draw(&mut b, &ring(0.4, Cx::new(k as f64 * 2.0, 0.0)));
        }
        b.selected = vec![0, 1];
        b.group();
        b.selected.clear();
        b.sheet.script.add("r = 2");
        b.sheet.script.add("circle(0, r)");
        b
    }

    /// ★ One list holds both halves. A circle typed and a circle drawn are both
    /// *a circle in the picture*, and two panels would mean two sets of habits
    /// for the same job.
    #[test]
    fn one_list_holds_the_drawn_and_the_written() {
        let t = Tree::new(&a_board());
        let kinds: Vec<Node> = t.lines.iter().map(|l| l.node).collect();
        assert!(kinds.contains(&Node::Title(Half::Shapes)));
        assert!(kinds.contains(&Node::Title(Half::Functions)));
        assert!(kinds.iter().any(|n| matches!(n, Node::Group(..))));
        assert!(kinds.iter().any(|n| matches!(n, Node::Mark(_))));
        assert!(kinds.iter().any(|n| matches!(n, Node::Row(_))));
    }

    /// ★ Folding a figure hides its strokes and leaves one line saying how many
    /// there are. Six strokes you almost never want to see all of.
    #[test]
    fn folding_a_figure_puts_its_strokes_away() {
        let mut b = a_board();
        let open = Tree::new(&b).lines.len();
        let group = b.sheet.marks[0].group;

        b.folded.push(group);
        let shut = Tree::new(&b);
        assert_eq!(shut.lines.len(), open - 2, "the two strokes inside should be gone");
        assert!(shut.lines.iter().any(|l| l.node == Node::Group(group, 2)), "and it should say how many");
    }

    /// ★ Painting and hit testing read the same rectangles. The fold arrow and
    /// the tick are on the left of a line, the rest of it chooses or edits.
    #[test]
    fn each_part_of_a_line_does_its_own_thing() {
        let b = a_board();
        let t = Tree::new(&b);
        let find = |want: fn(&Node) -> bool| t.lines.iter().find(|l| want(&l.node)).expect("a line").clone();

        let g = find(|n| matches!(n, Node::Group(..)));
        let knob = (PAD + g.depth * INDENT + 4) as f64;
        let far = (WIDTH - PAD - 20) as f64;
        assert!(matches!(t.at(knob, (g.y + 5) as f64, WIDTH), Some(Poke::Fold(_))), "the arrow folds");
        assert!(matches!(t.at(far, (g.y + 5) as f64, WIDTH), Some(Poke::Choose(Node::Group(..)))), "the rest chooses");

        let r = find(|n| matches!(n, Node::Row(_)));
        let rknob = (PAD + r.depth * INDENT + 4) as f64;
        assert!(matches!(t.at(rknob, (r.y + 5) as f64, WIDTH), Some(Poke::Tick(_))), "the tick switches it off");
        assert!(matches!(t.at(far, (r.y + 5) as f64, WIDTH), Some(Poke::Edit(_))), "the rest is typed into");
    }

    /// A title is not a button, except for its `+`.
    #[test]
    fn a_title_does_nothing_but_its_plus() {
        let b = a_board();
        let t = Tree::new(&b);
        let title = t.lines.iter().find(|l| matches!(l.node, Node::Title(Half::Shapes))).expect("a title").clone();
        assert_eq!(t.at(20.0, (title.y + 5) as f64, WIDTH), None, "the heading itself is not a button");
        assert_eq!(
            t.at((WIDTH - PAD - SHUT - 10) as f64, (title.y + 5) as f64, WIDTH),
            Some(Poke::Add(Half::Shapes)),
            "but the plus is"
        );
    }

    /// ★ Headings cannot be dragged. A list whose headings move is a list that
    /// can be put into a state with no meaning.
    #[test]
    fn headings_cannot_be_picked_up() {
        assert!(!Node::Title(Half::Shapes).movable());
        assert!(Node::Mark(0).movable());
        assert!(Node::Group(1, 2).movable());
        assert!(Node::Row(0).movable());
    }

    /// ★ A drop lands at the nearest **gap**, not on the line the pointer is
    /// over. "Which half of the line is it in" is a rule people have to be
    /// taught; the nearest gap is the one they already expect, because it is
    /// where the line gets drawn.
    #[test]
    fn a_drop_lands_at_the_nearest_gap() {
        let b = a_board();
        let t = Tree::new(&b);
        let first = &t.lines[0];
        assert_eq!(t.gap_at(first.y as f64), 0, "at the very top, before everything");

        let second = &t.lines[1];
        let between = (first.y + first.h + second.y) as f64 / 2.0;
        assert_eq!(t.gap_at(between), 1, "between the first and second");

        let last = t.lines.last().expect("lines");
        assert_eq!(t.gap_at((last.y + last.h + 40) as f64), t.lines.len(), "past the end, after everything");
    }

    /// Lines never overlap, so a near miss lands on nothing rather than on the
    /// neighbour — the worse of the two failures, because it does something.
    #[test]
    fn lines_do_not_overlap() {
        let t = Tree::new(&a_board());
        for (k, a) in t.lines.iter().enumerate() {
            for c in &t.lines[k + 1..] {
                assert!(a.y + a.h <= c.y || c.y + c.h <= a.y, "{:?} and {:?} overlap", a.node, c.node);
            }
        }
    }

    /// ★ A slider shows what is **in effect**, not what the row says. A game's
    /// score lives in the tally, and a dial computed from the rows alone sat
    /// at the starting value while the game moved on -- which reads as the
    /// game not working.
    #[test]
    fn a_slider_follows_the_game_and_not_just_the_row() {
        let mut b = Board::new();
        b.sheet.script.add("score = 0");
        b.sheet.script.add("circle(0, 1 + score)");

        let shown = |b: &Board| {
            Tree::new(b)
                .lines
                .iter()
                .find_map(|l| l.dial.clone())
                .map(|(_, v, _)| v)
                .expect("a dial")
        };
        assert_eq!(shown(&b), 0.0);

        b.tally.values.insert("score".into(), 5.0);
        assert_eq!(shown(&b), 5.0, "it should follow the game");
    }

    /// A row that binds a plain number gets a slider, and the ends of the
    /// slider are the ends of its range.
    #[test]
    fn a_row_that_binds_a_number_gets_a_slider() {
        let b = a_board();
        let t = Tree::new(&b);
        let with = t.lines.iter().find(|l| l.dial.is_some()).expect("a dial");
        let (_, _, sy) = with.dial.clone().expect("a dial");
        let y = (sy + SLIDER / 2) as f64;

        let value = |px: f64| match t.at(px, y, WIDTH) {
            Some(Poke::Dial(_, v)) => v,
            other => panic!("expected a dial, got {other:?}"),
        };
        assert!((value((PAD + INDENT) as f64) + RANGE).abs() < 1e-9);
        assert!((value((WIDTH - PAD) as f64) - RANGE).abs() < 0.2);
        assert!((Tree::along(0.0) - 0.5).abs() < 1e-9);
    }

    /// An empty drawing still offers both `+` buttons, so there is somewhere
    /// to begin.
    #[test]
    fn an_empty_drawing_still_has_somewhere_to_begin() {
        let b = Board::new();
        let t = Tree::new(&b);
        assert_eq!(t.adds.len(), 2);
        for (half, y) in &t.adds {
            assert_eq!(t.at((WIDTH - PAD - SHUT - 10) as f64, (y + 5) as f64, WIDTH), Some(Poke::Add(*half)));
        }
    }

    /// ★ A long list scrolls, and every line moves together — including the
    /// sliders and the options, which are laid out separately and are exactly
    /// the things that get forgotten.
    #[test]
    fn scrolling_moves_the_whole_list_together() {
        let mut b = a_board();
        for k in 0..20 {
            b.sheet.script.add(format!("r{k} = {k}"));
        }
        b.selected = vec![2];

        let before = Tree::new(&b);
        b.scrolled = 120.0;
        let after = Tree::new(&b);

        assert_eq!(before.lines.len(), after.lines.len());
        for (a, c) in before.lines.iter().zip(&after.lines) {
            assert_eq!(a.y - 120, c.y, "{:?} did not move with the rest", a.node);
            if let (Some((_, _, x)), Some((_, _, y))) = (&a.dial, &c.dial) {
                assert_eq!(x - 120, *y, "a slider was left behind");
            }
        }
        for (a, c) in before.adds.iter().zip(&after.adds) {
            assert_eq!(a.1 - 120, c.1, "a + button was left behind");
        }
        let (x, y) = (before.inspector.expect("options"), after.inspector.expect("options"));
        assert_eq!(x.swatches - 120, y.swatches, "the swatches were left behind");
        assert_eq!(x.verbs[0].3 - 120, y.verbs[0].3, "and the verbs");
    }

    /// ★ And the hit test follows, because it reads the same rectangles. A row
    /// you can see but cannot press is the worst kind of scrolling bug: it
    /// looks like the program ignoring you.
    #[test]
    fn a_scrolled_row_is_pressed_where_it_now_is() {
        let mut b = a_board();
        for k in 0..20 {
            b.sheet.script.add(format!("r{k} = {k}"));
        }
        b.scrolled = 200.0;
        let t = Tree::new(&b);
        let line = t.lines.iter().find(|l| l.y > 40 && matches!(l.node, Node::Row(_))).expect("a row on screen");
        let far = (WIDTH - PAD - 20) as f64;
        assert!(matches!(t.at(far, (line.y + 5) as f64, WIDTH), Some(Poke::Edit(_))), "it should be where it looks");
    }

    /// A list shorter than the window does not scroll at all — one that could
    /// be dragged up off its own top feels broken.
    #[test]
    fn a_short_list_does_not_scroll() {
        let b = Board::new();
        assert_eq!(Tree::new(&b).most(800), 0.0);

        let mut long = Board::new();
        for k in 0..40 {
            long.sheet.script.add(format!("r{k} = {k}"));
        }
        assert!(Tree::new(&long).most(800) > 0.0, "but a long one does");
    }

    /// ★ Collapsed, the list lays out **nothing**. Laying it out and then not
    /// painting it would leave every line hittable underneath — which shows up
    /// only as "the drawing sometimes ignores me near the left edge".
    #[test]
    fn a_collapsed_tree_has_nothing_to_press_but_its_handle() {
        let mut b = a_board();
        assert!(!Tree::new(&b).lines.is_empty());

        b.tree_shut = true;
        let t = Tree::new(&b);
        assert!(t.lines.is_empty());
        assert_eq!(Tree::width(&b), SHUT);

        // Only the handle answers.
        assert_eq!(t.at(4.0, 4.0, SHUT), Some(Poke::Collapse));
        assert_eq!(t.at(4.0, 300.0, SHUT), None, "and the rest of the strip is nothing");
        assert_eq!(t.at(200.0, 40.0, SHUT), None, "and the drawing beside it is the drawing");
    }

    /// The handle is there when it is open too, or there would be no way to
    /// shut it.
    #[test]
    fn the_handle_is_there_either_way() {
        let b = a_board();
        let t = Tree::new(&b);
        let (hx, _) = Tree::handle(WIDTH);
        assert_eq!(t.at(f64::from(hx + 4), 4.0, WIDTH), Some(Poke::Collapse));
    }

    /// ★ A function has to be read character by character, so the rows are set
    /// at twice the size of everything else — and the list is wide enough for
    /// them. Everything else on screen is a word you know by sight.
    #[test]
    fn a_function_row_is_big_enough_to_read() {
        assert!(TEXT >= 2, "the rows are the one thing that must not be small");
        assert!(LINE >= 7 * TEXT + 8, "and the line has to fit the text with room round it");
        // Enough width for a formula worth writing.
        let room = (WIDTH - PAD - INDENT - KNOB - 8) / Canvas::text_w("n", TEXT).max(1);
        assert!(room >= 24, "only {room} characters fit, which is not a formula");
    }

    /// The drawing beside the tree is the drawing.
    #[test]
    fn the_canvas_is_not_the_tree() {
        assert!(!Tree::covers(WIDTH as f64));
        assert!(Tree::covers((WIDTH - 1) as f64));
        assert_eq!(Tree::new(&a_board()).at(WIDTH as f64 + 5.0, 40.0, WIDTH), None);
    }
}
