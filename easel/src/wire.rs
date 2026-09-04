//! # wire — the drawing as something a browser can read
//!
//! ## Shapes, not pixels
//!
//! The obvious way to put this in a browser is to render to a buffer and blit
//! it. It works, and it throws away everything the browser is good at: a
//! rasterised curve arrives as a fixed grid of dots, so it cannot be
//! antialiased, cannot be scaled without going soft, and cannot be scrolled or
//! zoomed without asking for another one.
//!
//! So what goes over the wire is **runs of points**, and the browser strokes
//! them itself. That is not extra work: [`Shape::polylines`] already reduces
//! every kind of shape — curves, implicit equations, Fourier series, hand-drawn
//! strokes — to exactly that, because it is what the rasteriser needed too.
//! One list, two renderers, and no chance of them disagreeing.
//!
//! ```text
//!     Board  ->  Frame  ->  parts  ->  polylines  ->  JSON  ->  canvas 2d
//! ```
//!
//! ## World coordinates, not screen
//!
//! The points are sent in the numbers the drawing is written in, and the
//! browser applies the view. That means **panning and zooming never reach the
//! server**: they are a matrix on the client, at whatever rate the hand moves,
//! with no round trip at all. Sending screen coordinates would put a network
//! hop inside a drag, which is the one place it would be felt.
//!
//! What the client must send back is where it is *looking*, because a `graph`
//! or an `implicit` is sampled against the visible window and genuinely cannot
//! be drawn without knowing it.
//!
//! ## Why the JSON is written by hand
//!
//! Forty lines, against a dependency that would sit under every crate here.
//! This one is small enough to read, produces exactly what the client parses,
//! and keeps [`easel`](crate) resting on nothing but arithmetic — which is the
//! same reason the whole workspace links no C library.

use std::fmt::Write as _;

use plotkit::Cx;

use crate::board::Board;
use crate::tree::{Half, Node, Tree};

/// What the client is looking at, so shapes sampled against the window can be
/// sampled against the right one.
#[derive(Clone, Copy, Debug)]
pub struct Look {
    pub lo: Cx,
    pub hi: Cx,
    /// How many pixels wide the canvas is — the sampling budget for a curve.
    pub px: usize,
}

impl Look {
    pub fn new(lo: Cx, hi: Cx, px: usize) -> Look {
        Look { lo, hi, px: px.clamp(64, 4096) }
    }
}

impl Default for Look {
    fn default() -> Look {
        Look::new(Cx::new(-10.0, -10.0), Cx::new(10.0, 10.0), 900)
    }
}

/// Everything the client needs to draw one frame.
pub fn scene(board: &Board, look: Look) -> String {
    with_word(board, look, "")
}

/// The same, with something for the page to say.
pub fn with_word(board: &Board, look: Look, word: &str) -> String {
    since(board, look, word, 0)
}

/// A number naming the still half of this drawing, at this zoom.
///
/// It changes when the drawing changes or when the view does — the still shapes
/// are sampled to fit the window, so panning is a new set of points even though
/// it is the same board. When the page already holds this number, the still
/// half is left out of the answer entirely.
///
/// Any hash would do; this is FNV over the rows, the marks and the view, which
/// is a few thousand bytes and costs nothing beside the frame it saves.
pub fn still_mark(board: &Board, look: Look) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100_0000_01b3);
        }
    };
    for r in &board.sheet.script.rows {
        eat(r.text.as_bytes());
        eat(&[r.on as u8]);
    }
    for m in &board.sheet.marks {
        eat(&m.colour.to_le_bytes());
        eat(&(m.pts.len() as u64).to_le_bytes());
        eat(&[m.filled as u8, m.closed as u8, m.moves() as u8]);
    }
    for v in [look.lo.re, look.lo.im, look.hi.re, look.hi.im, look.px as f64] {
        eat(&v.to_bits().to_le_bytes());
    }
    h
}

/// The scene, leaving out the still half if the page says it already has
/// `have`.
pub fn since(board: &Board, look: Look, word: &str, have: u64) -> String {
    let (still, moving) = board.frames();
    let mark = still_mark(board, look);
    let mut out = String::new();
    let _ = write!(out, "{{\"stillv\":{mark}");
    if have != mark {
        out.push_str(",\"still\":[");
        let mut first = true;
        for (shape, style) in still.parts() {
            piece(&mut out, shape, style, look, &mut first);
        }
        out.push(']');
    }
    out.push_str(",\"pieces\":[");
    let mut first = true;
    for (shape, style) in moving.parts() {
        piece(&mut out, shape, style, look, &mut first);
    }
    out.push_str("],\"rings\":[");
    for (k, ring) in board.selection().into_iter().enumerate() {
        for run in ring.polylines(look.lo, look.hi, look.px) {
            if k > 0 || !out.ends_with('[') {
                out.push(',');
            }
            points(&mut out, &run);
        }
    }
    out.push(']');

    let _ = write!(out, ",\"clock\":{:.3},\"playing\":{}", board.clock, board.playing);
    let _ = write!(out, ",\"game\":{},\"watching\":{}", board.playing_game, board.watching);
    // The house rules this drawing declares, for the setup screen. Sent every
    // frame because they are a handful of numbers, and because a page that had
    // to ask for them separately could show a stale one.
    // The box the drawing says it lives in, if it says. A page that gets one
    // fits to it and stops offering the wheel.
    match board.sheet.script.bounds(board.clock) {
        Some((lo, hi)) => {
            let _ = write!(
                out,
                ",\"bounds\":[{:.3},{:.3},{:.3},{:.3}]",
                lo.re, lo.im, hi.re, hi.im
            );
        }
        None => out.push_str(",\"bounds\":null"),
    }
    out.push_str(",\"rules\":[");
    for (n, (id, name, label, value)) in board.sheet.script.house(board.clock).into_iter().enumerate() {
        if n > 0 {
            out.push(',');
        }
        let _ = write!(out, "{{\"id\":{id},\"name\":");
        text(&mut out, &name);
        out.push_str(",\"label\":");
        text(&mut out, &label);
        let _ = write!(out, ",\"value\":{value}}}");
    }
    out.push(']');
    out.push_str(",\"say\":");
    text(&mut out, word);
    out.push_str(",\"tree\":");
    tree(&mut out, board);
    out.push('}');
    out
}

/// The list, as the browser will show it — real rows with real inputs.
fn tree(out: &mut String, board: &Board) {
    let laid = Tree::new(board);
    let made = board.written();
    out.push('[');
    let mut first = true;
    for line in &laid.lines {
        if !first {
            out.push(',');
        }
        first = false;
        match line.node {
            Node::Title(h) => {
                let _ = write!(
                    out,
                    "{{\"kind\":\"title\",\"name\":\"{}\"}}",
                    if h == Half::Shapes { "shapes" } else { "functions" }
                );
            }
            Node::Group(id, n) => {
                let _ = write!(
                    out,
                    "{{\"kind\":\"group\",\"id\":{id},\"count\":{n},\"folded\":{},\"chosen\":{}}}",
                    board.folded.contains(&id),
                    board.chosen_group() == Some(id)
                );
            }
            Node::Mark(k) => {
                let m = &board.sheet.marks[k];
                let _ = write!(
                    out,
                    "{{\"kind\":\"mark\",\"id\":{k},\"colour\":\"#{:06X}\",\"moves\":{},\"chosen\":{},\"depth\":{}}}",
                    m.colour & 0xFF_FFFF,
                    m.moves(),
                    board.selected.contains(&k),
                    line.depth
                );
            }
            Node::Row(k) => {
                let r = &board.sheet.script.rows[k];
                let wrong = made.errors.iter().find(|(l, _)| *l == k + 1).map(|(_, m)| m.clone());
                let _ = write!(out, "{{\"kind\":\"row\",\"id\":{k},\"on\":{},\"text\":", r.on);
                text(out, &r.text);
                if let Some(msg) = wrong {
                    out.push_str(",\"wrong\":");
                    text(out, &msg);
                }
                if let Some((name, value, _)) = &line.dial {
                    out.push_str(",\"dial\":");
                    text(out, name);
                    let _ = write!(out, ",\"value\":{value}");
                }
                out.push('}');
            }
        }
    }
    out.push(']');
}

/// Coordinates go over the wire as **whole numbers of hundredths**, and the
/// page multiplies by [`GRAIN`] again.
///
/// This is not a micro-optimisation, it was the difference between a studio
/// that answered and one that did not. A scene of this board is about thirty
/// thousand numbers, and `write!("{:.4}")` costs the best part of three
/// microseconds each — nearly all of it the formatting machinery rather than
/// the arithmetic. That put a single scene at **84 ms**, so the clock could not
/// tick faster than twelve times a second and every tap queued behind one.
///
/// Whole numbers can be written a digit at a time, which is perhaps twenty
/// times quicker and smaller besides. And a hundredth of a world unit is a
/// fortieth of a pixel at the zoom anybody draws at, so nothing is lost.
pub const GRAIN: f64 = 0.01;

/// One shape, as however many runs of points it draws in.
fn piece(
    out: &mut String,
    shape: &plotkit::Shape,
    style: &plotkit::frame::Style,
    look: Look,
    first: &mut bool,
) {
    // How many pixels a world unit is worth, for throwing away what cannot be
    // seen.
    let scale = look.px as f64 / (look.hi.re - look.lo.re).abs().max(1e-9);
    for run in shape.polylines(look.lo, look.hi, look.px) {
        if run.len() < 2 && !style.filled {
            continue;
        }
        // **A shape smaller than a pixel is not sent.** This is how things are
        // hidden here -- `param(if(can, 0.52, 0)*exp(i*t) + ...)` draws a ring
        // at no size when you may not move that token -- and a ring at no size
        // was still three hundred and twenty-one identical points on the wire,
        // every frame, for every token. Sixteen of those is most of a scene
        // spent drawing nothing.
        let (mut lo, mut hi) = (run[0], run[0]);
        for p in &run {
            lo = Cx::new(lo.re.min(p.re), lo.im.min(p.im));
            hi = Cx::new(hi.re.max(p.re), hi.im.max(p.im));
        }
        if (hi.re - lo.re).max(hi.im - lo.im) * scale < 1.0 {
            continue;
        }
        if !*first {
            out.push(',');
        }
        *first = false;
        let _ = write!(
            out,
            "{{\"c\":\"#{:06X}\",\"w\":{},\"fill\":{},\"p\":",
            style.colour & 0xFF_FFFF,
            style.width.max(1),
            style.filled
        );
        points(out, &run);
        out.push('}');
    }
}

fn points(out: &mut String, run: &[Cx]) {
    out.push('[');
    for (k, z) in run.iter().enumerate() {
        if k > 0 {
            out.push(',');
        }
        whole(out, z.re);
        out.push(',');
        whole(out, z.im);
    }
    out.push(']');
}

/// One coordinate, in hundredths, written a digit at a time.
fn whole(out: &mut String, v: f64) {
    // A coordinate that is not a number at all would make the page stop dead
    // on a parse error rather than draw the rest, so it becomes a nought.
    let n = if v.is_finite() { (v / GRAIN).round() } else { 0.0 };
    let mut n = n.clamp(-2e9, 2e9) as i64;
    if n < 0 {
        out.push('-');
        n = -n;
    }
    if n == 0 {
        out.push('0');
        return;
    }
    let mut digits = [0u8; 20];
    let mut k = 0;
    while n > 0 {
        digits[k] = b'0' + (n % 10) as u8;
        n /= 10;
        k += 1;
    }
    while k > 0 {
        k -= 1;
        out.push(digits[k] as char);
    }
}

/// A string, with the six things JSON insists on escaping.
///
/// Somebody's formula will one day contain a quote or a backslash, and an
/// unescaped one does not make a wrong drawing — it makes a page that stops
/// dead, which is much harder to trace back to the row that caused it.
fn text(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use shapes::Nib;
    use std::f64::consts::TAU;

    fn a_board() -> Board {
        let mut b = Board::new();
        b.nib = Nib::Round(0.2);
        let pts: Vec<Cx> = (0..=40).map(|k| Cx::polar(2.0, k as f64 / 40.0 * TAU)).collect();
        for z in &pts {
            b.pointer(*z, true);
        }
        b.pointer(pts[pts.len() - 1], false);
        b.sheet.script.add("r = 3");
        b.sheet.script.add("circle(0, r)");
        b
    }

    /// Very rough JSON well-formedness: brackets balance and quotes pair.
    fn looks_like_json(s: &str) -> bool {
        let (mut curly, mut square, mut quoted, mut escaped) = (0i32, 0i32, false, false);
        for c in s.chars() {
            if escaped {
                escaped = false;
                continue;
            }
            match c {
                '\\' if quoted => escaped = true,
                '"' => quoted = !quoted,
                '{' if !quoted => curly += 1,
                '}' if !quoted => curly -= 1,
                '[' if !quoted => square += 1,
                ']' if !quoted => square -= 1,
                _ => {}
            }
            if curly < 0 || square < 0 {
                return false;
            }
        }
        curly == 0 && square == 0 && !quoted
    }

    /// ★ **Shapes, not pixels.** Every kind of shape reduces to runs of points
    /// already — it is what the rasteriser needed too — so one list serves both
    /// renderers and they cannot disagree.
    #[test]
    fn a_drawing_goes_over_as_runs_of_points() {
        let s = scene(&a_board(), Look::default());
        assert!(looks_like_json(&s), "{s}");
        assert!(s.contains("\"pieces\":["));
        assert!(s.contains("\"p\":["), "there should be points in it");
        // The drawn ring and the written circle.
        assert!(s.matches("\"c\":\"#").count() >= 2, "both halves should be there");
    }

    /// ★ A `graph` or an `implicit` is sampled against the window, so what the
    /// client is looking at has to come with the question. Looking somewhere
    /// else must give a different answer, or the sampling is being ignored.
    #[test]
    fn what_is_sent_depends_on_where_the_client_is_looking() {
        let mut b = Board::new();
        b.sheet.script.add("plot(sin(x))");
        let near = scene(&b, Look::new(Cx::new(-2.0, -2.0), Cx::new(2.0, 2.0), 400));
        let far = scene(&b, Look::new(Cx::new(-40.0, -40.0), Cx::new(40.0, 40.0), 400));
        assert_ne!(near, far, "a curve sampled across the window must follow the window");
    }

    /// ★ A quote or a backslash in somebody's formula must not stop the page
    /// dead — which is much harder to trace back to the row that caused it
    /// than a wrong drawing would be.
    #[test]
    fn an_awkward_character_in_a_row_does_not_break_the_page() {
        let mut b = Board::new();
        b.sheet.script.add("# a \"quoted\" thing, a back\\slash, and a\ttab");
        let s = scene(&b, Look::default());
        assert!(looks_like_json(&s), "{s}");
        assert!(s.contains("\\\"quoted\\\""));
        assert!(s.contains("back\\\\slash"));
    }

    /// The list goes over as rows rather than as a picture of rows, so the
    /// browser can put a real input in each one — which is the whole reason
    /// for doing this.
    #[test]
    fn the_list_goes_over_as_rows_and_not_as_a_picture() {
        let s = scene(&a_board(), Look::default());
        assert!(s.contains("\"kind\":\"title\""));
        assert!(s.contains("\"kind\":\"mark\""));
        assert!(s.contains("\"kind\":\"row\""));
        assert!(s.contains("\"dial\":\"r\""), "and a row that binds a number says so");
    }

    /// A row that will not parse says why, next to itself.
    #[test]
    fn a_bad_row_carries_its_own_complaint() {
        let mut b = Board::new();
        b.sheet.script.add("circle(0, 1)");
        b.sheet.script.add("this is not a thing");
        let s = scene(&b, Look::default());
        assert!(s.contains("\"wrong\":"), "{s}");
    }

    /// An empty drawing is an empty scene, not a broken one.
    #[test]
    fn an_empty_drawing_still_makes_a_scene() {
        let s = scene(&Board::new(), Look::default());
        assert!(looks_like_json(&s), "{s}");
        assert!(s.contains("\"pieces\":[]"));
    }

    /// Whatever is chosen goes over separately, because a selection is
    /// furniture — it is drawn over the picture and must never be saved or
    /// exported as part of it.
    #[test]
    fn what_is_chosen_is_sent_apart_from_the_drawing() {
        let mut b = a_board();
        b.selected = vec![0];
        let s = scene(&b, Look::default());
        assert!(s.contains("\"rings\":[["), "{s}");
        assert!(looks_like_json(&s));
    }
}
