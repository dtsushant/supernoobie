//! # sheet — the drawing, and the file it lives in
//!
//! ## The format is text, on purpose
//!
//! ```text
//!     easel 1
//!     mark round 0.35 taper 0.25 colour E3E9EF fill open
//!     p -4.500 3.310 -4.400 3.352 -4.300 3.401
//!     p -4.200 3.455 -4.100 3.512
//!     mark broad 0.42 0.785 taper 0.000 colour E585AC fill closed
//!     p 1.000 0.000 0.951 0.309
//! ```
//!
//! Not a binary blob and not a database. A drawing you can open in a text
//! editor is a drawing you can diff, grep, fix by hand when something goes
//! wrong, and read in six months without this program. The same reasoning as
//! [`studio::tape`](../studio/tape/index.html), and for the same reason: the
//! file outlives the code that wrote it.
//!
//! It is also why the version number is the first word. A format with no
//! version is a format that can never change.
//!
//! ## Why points wrap across several `p` lines
//!
//! A stroke can be five hundred points and one enormous line is unreadable
//! and awkward to diff — a single changed point rewrites the whole line. Eight
//! points to a line keeps a change local.
//!
//! ## What is refused, and why nothing is refused loudly
//!
//! A line that cannot be understood is **skipped**, not fatal. A drawing is
//! somebody's work: recovering nine marks out of ten beats refusing to open
//! the file because the tenth has a bad number in it. [`Sheet::load`] reports
//! how many lines it could not make sense of, so a caller can say so without
//! having lost anything.

use plotkit::{Cx, Shape};
use shapes::Nib;
use std::fmt::Write as _;

use crate::action::{Action, Step};
use crate::mark::Mark;
use crate::script::{Row, Script};
use crate::track::Ease;
use shapes::Pose;

/// The format's version, written as the first line.
pub const VERSION: u32 = 1;

/// How many points go on one `p` line.
const PER_LINE: usize = 8;

/// A drawing: marks in the order they were made, which is the order they are
/// painted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sheet {
    pub marks: Vec<Mark>,
    /// The written half of the drawing. Saved in the same file, because it is
    /// the same drawing — a picture that opened with its hand-drawn half and
    /// not its written half would be half a picture.
    pub script: Script,
}

impl Sheet {
    pub fn new() -> Sheet {
        Sheet { marks: Vec::new(), script: Script::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.marks.is_empty()
    }

    pub fn len(&self) -> usize {
        self.marks.len()
    }

    pub fn add(&mut self, m: Mark) {
        self.marks.push(m);
    }

    /// Everything the drawing covers, for framing the view on it.
    pub fn bounds(&self) -> Option<(Cx, Cx)> {
        self.marks.iter().filter_map(Mark::bounds).reduce(|(alo, ahi), (blo, bhi)| {
            (
                Cx::new(alo.re.min(blo.re), alo.im.min(blo.im)),
                Cx::new(ahi.re.max(bhi.re), ahi.im.max(bhi.im)),
            )
        })
    }

    /// Which mark is at this point, if any, **at time `t`**.
    ///
    /// The time matters and its absence was a real bug. An animated mark is
    /// drawn wherever its pose puts it, while its points stay where they were
    /// made — so hit testing the points means you cannot grab a moving thing
    /// where you can see it, only where it was originally drawn. Which is
    /// nowhere in particular, and invisible.
    ///
    /// **Searched from the top down**, because the mark you can see is the one
    /// drawn last, and that is the one a person means when they point at an
    /// overlap.
    pub fn at(&self, p: Cx, tolerance: f64, t: f64) -> Option<usize> {
        self.at_in(p, tolerance, t, &std::collections::HashMap::new())
    }

    /// The same, following whatever the script says. **A placed mark has to be
    /// caught where it is drawn**, and where it is drawn depends on the
    /// numbers — so hit testing needs them too, or a token you can see cannot
    /// be tapped.
    pub fn at_in(&self, p: Cx, tolerance: f64, t: f64, env: &std::collections::HashMap<String, Cx>) -> Option<usize> {
        let lo = Cx::new(p.re - 1e4, p.im - 1e4);
        let hi = Cx::new(p.re + 1e4, p.im + 1e4);
        (0..self.marks.len()).rev().find(|k| {
            let m = &self.marks[*k];
            let (pose, here) = (m.pose_in(t, env), m.anchor());
            let shape = m.at_in(t, env);
            if shape.touches(p, tolerance, lo, hi, 800) {
                return true;
            }
            // A **closed** mark is hit anywhere inside it, and the test is
            // against its centreline rather than against the region the nib
            // swept.
            //
            // That distinction is the whole of this and it cost an evening.
            // Draw a box: the nib sweeps a thin rectangular *ring*, and the
            // middle of the box is the hole in that ring — genuinely outside
            // it by the even-odd rule, and correctly so. But nobody tapping a
            // box means "the two pixels of its edge". They mean the box. So
            // the question to ask is whether the point is inside the line the
            // hand actually drew.
            m.closed && Shape::polygon(m.pts.clone()).map(move |z| pose.apply(z - here) + here).contains(p, lo, hi, 800)
        })
    }

    /// The whole drawing as text.
    pub fn to_text(&self) -> String {
        let mut out = format!("easel {VERSION}\n");
        // The script first, so a person opening the file sees what the drawing
        // is *made of* before several hundred lines of points.
        for r in &self.script.rows {
            let _ = writeln!(out, "row {} {}", if r.on { "on" } else { "off" }, r.text);
        }
        for m in &self.marks {
            let nib = match m.nib {
                Nib::Round(w) => format!("round {w:.4}"),
                Nib::Quill { slow, fast, pace } => format!("quill {slow:.4} {fast:.4} {pace:.4}"),
                Nib::Broad { width, angle } => format!("broad {width:.4} {angle:.4}"),
            };
            let _ = writeln!(
                out,
                "mark {nib} taper {:.4} colour {:06X} {} {}",
                m.taper,
                m.colour & 0xFF_FFFF,
                if m.filled { "fill" } else { "line" },
                if m.closed { "closed" } else { "open" }
            );
            if m.group != 0 {
                let _ = writeln!(out, "group {}", m.group);
            }
            if let Some((x, y)) = &m.place {
                // One line each, so an expression may hold anything at all --
                // including whatever character would have been the separator.
                let _ = writeln!(out, "placex {x}");
                let _ = writeln!(out, "placey {y}");
            }
            if let Some(a) = &m.spin {
                let _ = writeln!(out, "placea {a}");
            }
            for chunk in m.pts.chunks(PER_LINE) {
                out.push('p');
                for z in chunk {
                    let _ = write!(out, " {:.4} {:.4}", z.re, z.im);
                }
                out.push('\n');
            }
            if !m.track.is_empty() {
                let _ = writeln!(out, "track {}", if m.track.looping { "loop" } else { "once" });
                for k in &m.track.keys {
                    let _ = writeln!(
                        out,
                        "key {:.4} {:.5} {:.5} {:.4} {:.4} {}",
                        k.at,
                        k.pose.a.re,
                        k.pose.a.im,
                        k.pose.b.re,
                        k.pose.b.im,
                        k.ease.name()
                    );
                }
            }
            if !m.act.steps.is_empty() {
                let _ = writeln!(out, "act {}", if m.act.looping { "loop" } else { "once" });
                for step in &m.act.steps {
                    let (word, n) = step.action.spelt();
                    // `inf` for a step that never ends, which is what
                    // `Act::just` makes and is the common case.
                    let secs =
                        if step.seconds.is_finite() { format!("{:.4}", step.seconds) } else { "inf".to_string() };
                    let _ = writeln!(out, "do {word} {:.4} {:.4} {secs}", n[0], n[1]);
                }
            }
        }
        out
    }

    /// Read a drawing back, and say how many lines made no sense.
    ///
    /// Never fails on content. A drawing is somebody's work, and recovering
    /// nine marks out of ten beats refusing to open the file because the tenth
    /// has a bad number in it.
    pub fn from_text(text: &str) -> (Sheet, usize) {
        let mut sheet = Sheet::new();
        let mut confused = 0;

        for line in text.lines() {
            let mut word = line.split_whitespace();
            match word.next() {
                None | Some("easel") | Some("#") => {}
                Some("mark") => match read_mark(&mut word) {
                    Some(m) => sheet.marks.push(m),
                    None => confused += 1,
                },
                Some("row") => {
                    let on = word.next() != Some("off");
                    // The rest of the line verbatim. A script row has spaces in it,
                    // so it cannot be split into words and put back together --
                    // `circle(0,  1)` would come back differently spaced, which is
                    // somebody's formatting quietly rewritten under them.
                    let text = line.trim_start().splitn(3, char::is_whitespace).nth(2).unwrap_or("").to_string();
                    sheet.script.rows.push(Row { text, on });
                }
                Some("track") => match sheet.marks.last_mut() {
                    Some(m) => m.track.looping = word.next() != Some("once"),
                    None => confused += 1,
                },
                Some("key") => match (sheet.marks.last_mut(), read_key(&mut word)) {
                    (Some(m), Some((at, pose, ease))) => m.track.set(at, pose, ease),
                    _ => confused += 1,
                },
                Some("placea") => {
                    let rest = line.trim_start().splitn(2, char::is_whitespace).nth(1).unwrap_or("").to_string();
                    match sheet.marks.last_mut() {
                        Some(m) => m.spin = Some(rest),
                        None => confused += 1,
                    }
                }
                Some(w @ ("placex" | "placey")) => {
                    let rest = line.trim_start().splitn(2, char::is_whitespace).nth(1).unwrap_or("").to_string();
                    match sheet.marks.last_mut() {
                        Some(m) => {
                            let (x, y) = m.place.clone().unwrap_or_default();
                            m.place = Some(if w == "placex" { (rest, y) } else { (x, rest) });
                        }
                        None => confused += 1,
                    }
                }
                Some("group") => match (sheet.marks.last_mut(), word.next().and_then(|w| w.parse().ok())) {
                    (Some(m), Some(g)) => m.group = g,
                    _ => confused += 1,
                },
                Some("act") => match sheet.marks.last_mut() {
                    Some(m) => m.act.looping = word.next() != Some("once"),
                    None => confused += 1,
                },
                Some("do") => match (sheet.marks.last_mut(), read_step(&mut word)) {
                    (Some(m), Some(step)) => m.act.steps.push(step),
                    _ => confused += 1,
                },
                Some("p") => {
                    let Some(m) = sheet.marks.last_mut() else {
                        // Points before any mark to hang them on.
                        confused += 1;
                        continue;
                    };
                    let numbers: Vec<f64> = word.filter_map(|w| w.parse().ok()).collect();
                    if numbers.len() % 2 != 0 {
                        confused += 1;
                    }
                    for pair in numbers.chunks_exact(2) {
                        m.pts.push(Cx::new(pair[0], pair[1]));
                    }
                }
                Some(_) => confused += 1,
            }
        }
        (sheet, confused)
    }

    pub fn save(&self, path: &str) -> std::io::Result<()> {
        std::fs::write(path, self.to_text())
    }

    /// Load a drawing. A missing file is an error; a confusing one is not.
    pub fn load(path: &str) -> std::io::Result<(Sheet, usize)> {
        Ok(Sheet::from_text(&std::fs::read_to_string(path)?))
    }
}

/// The rest of a `key` line, after the word `key`.
fn read_key<'a>(word: &mut impl Iterator<Item = &'a str>) -> Option<(f64, Pose, Ease)> {
    let mut number = || word.next()?.parse::<f64>().ok();
    let at = number()?;
    let (are, aim, bre, bim) = (number()?, number()?, number()?, number()?);
    // An unnamed ease is the default rather than a lost key -- an earlier
    // version of the format might not have written one.
    let ease = word.next().and_then(Ease::spell).unwrap_or(Ease::Smooth);
    Some((at, Pose::new(Cx::new(are, aim), Cx::new(bre, bim)), ease))
}

/// The rest of a `do` line, after the word `do`.
fn read_step<'a>(word: &mut impl Iterator<Item = &'a str>) -> Option<Step> {
    let name = word.next()?;
    let a = word.next()?.parse().ok()?;
    let b = word.next()?.parse().ok()?;
    let secs = word.next()?;
    let seconds = if secs == "inf" { f64::INFINITY } else { secs.parse().ok()? };
    Some(Step { action: Action::spell(name, [a, b])?, seconds })
}

/// The rest of a `mark` line, after the word `mark`.
fn read_mark<'a>(word: &mut impl Iterator<Item = &'a str>) -> Option<Mark> {
    let mut next = || word.next();
    let number = |w: Option<&str>| w?.parse::<f64>().ok();

    let nib = match next()? {
        "round" => Nib::Round(number(next())?),
        "quill" => Nib::Quill { slow: number(next())?, fast: number(next())?, pace: number(next())? },
        "broad" => Nib::Broad { width: number(next())?, angle: number(next())? },
        _ => return None,
    };

    let mut m = Mark::new(Vec::new(), nib, 0xFFFFFF);
    // The rest is keyword-and-value, so a later version can add a field
    // without a reader of this one falling over.
    while let Some(key) = next() {
        match key {
            "taper" => m.taper = number(next())?,
            "colour" => m.colour = u32::from_str_radix(next()?, 16).ok()?,
            "fill" => m.filled = true,
            "line" => m.filled = false,
            "closed" => m.closed = true,
            "open" => m.closed = false,
            // Something a later version wrote. Skip its value and carry on
            // rather than losing the whole mark.
            _ => {
                let _ = next();
            }
        }
    }
    Some(m)
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Act;
    use std::f64::consts::TAU;

    fn ring(n: usize, r: f64, at: Cx) -> Vec<Cx> {
        (0..n).map(|k| at + Cx::polar(r, k as f64 / n as f64 * TAU)).collect()
    }

    fn an_act() -> Act {
        Act::still()
            .then(Action::Walk(Cx::new(1.5, 0.0)), 2.0)
            .then(Action::Jump { height: 1.2, rate: 1.5 }, 1.5)
            .looping(true)
    }

    fn a_sheet() -> Sheet {
        let mut s = Sheet::new();
        s.add(Mark::new(ring(24, 2.0, Cx::ZERO), Nib::Round(0.35), 0xE3E9EF).closed(true).taper(0.25).doing(an_act()));
        s.add(Mark::new(ring(30, 1.0, Cx::new(5.0, 1.0)), Nib::Broad { width: 0.42, angle: 0.785 }, 0xE585AC));
        s.add(Mark::new(vec![Cx::new(-3.0, -3.0), Cx::new(3.0, -3.0)], Nib::Round(0.1), 0x4FBCD4).outlined());
        s
    }

    /// ★ Written and read back must be the same drawing. Everything else in
    /// this module is in service of that one sentence, and a format that
    /// almost round-trips silently loses somebody's work.
    #[test]
    fn a_drawing_survives_being_written_and_read() {
        let before = a_sheet();
        let (after, confused) = Sheet::from_text(&before.to_text());
        assert_eq!(confused, 0, "nothing it wrote should confuse it");
        assert_eq!(after.len(), before.len());

        for (a, b) in after.marks.iter().zip(&before.marks) {
            assert_eq!(a.nib, b.nib);
            assert_eq!(a.colour, b.colour);
            assert_eq!(a.filled, b.filled);
            assert_eq!(a.closed, b.closed);
            assert!((a.taper - b.taper).abs() < 1e-4);
            assert_eq!(a.act, b.act, "what it does must survive too");
            assert_eq!(a.group, b.group, "and which figure it belongs to");
            assert_eq!(a.place, b.place, "and what it follows");
            assert_eq!(a.spin, b.spin, "and which way it is turned");
            assert_eq!(a.track, b.track, "and every keyframe");
            assert_eq!(a.pts.len(), b.pts.len());
            for (p, q) in a.pts.iter().zip(&b.pts) {
                // Four decimal places is the format's promise, and at the
                // scale a hand draws at, a ten-thousandth is far below a pixel.
                assert!((*p - *q).abs() < 1e-4, "a point moved: {p:?} vs {q:?}");
            }
        }
    }

    /// ★ It is text a person can read, and the version is the first word — a
    /// format with no version is a format that can never change.
    #[test]
    fn the_file_says_what_it_is_before_anything_else() {
        let text = a_sheet().to_text();
        assert!(text.starts_with("easel 1\n"), "it should announce itself: {:?}", &text[..20.min(text.len())]);
        assert!(text.contains("round 0.3500"), "and be readable");
        assert!(text.contains("colour E3E9EF"));
        assert!(text.lines().count() > 6, "points should wrap rather than being one huge line");
    }

    /// ★ A bad line loses that line, not the drawing. Refusing to open a file
    /// because one number in it is wrong throws away everything somebody did.
    #[test]
    fn one_broken_line_does_not_lose_the_others() {
        let mut text = a_sheet().to_text();
        text.push_str("mark spatula 3 4\n");
        text.push_str("p not a number at all\n");
        text.push_str("what even is this line\n");

        let (sheet, confused) = Sheet::from_text(&text);
        assert_eq!(sheet.len(), 3, "the three good marks should all be there");
        assert!(confused >= 2, "and it should say it was confused: {confused}");
    }

    /// ★ A field a later version writes must not lose the mark that carries
    /// it. Unknown keywords are skipped with their value, so version 2 can add
    /// something and version 1 still opens the file.
    #[test]
    fn a_field_from_the_future_is_stepped_over() {
        let text = "easel 2\nmark round 0.30 texture chalk taper 0.10 colour 00FF00 fill open\np 0 0 1 1\n";
        let (sheet, confused) = Sheet::from_text(text);
        assert_eq!(confused, 0, "an unknown field is not a broken line");
        assert_eq!(sheet.len(), 1);
        assert_eq!(sheet.marks[0].colour, 0x00FF00, "the fields after it must still be read");
        assert!((sheet.marks[0].taper - 0.10).abs() < 1e-9);
        assert_eq!(sheet.marks[0].pts.len(), 2);
    }

    /// ★ An animation is part of the drawing, so it has to survive the file
    /// too. A drawing that reopened as a set of motionless shapes would have
    /// lost the part that took longest to get right.
    #[test]
    fn what_a_mark_does_survives_the_file() {
        let mut s = Sheet::new();
        s.add(Mark::new(ring(12, 1.0, Cx::ZERO), Nib::Round(0.2), 0xFFFFFF).doing(an_act()));
        s.add(Mark::new(ring(12, 1.0, Cx::new(4.0, 0.0)), Nib::Round(0.2), 0xFFFFFF).doing(Act::just(Action::Spin(0.5))));

        let (back, confused) = Sheet::from_text(&s.to_text());
        assert_eq!(confused, 0);
        assert_eq!(back.marks[0].act, s.marks[0].act);
        assert_eq!(back.marks[1].act.steps[0].seconds, f64::INFINITY, "forever must survive as forever");
    }

    /// An action word this version does not know loses that step, not the
    /// mark and not the drawing.
    #[test]
    fn an_action_from_the_future_loses_only_itself() {
        let text = "easel 1\nmark round 0.3 colour FFFFFF fill open\np 0 0 1 1\nact loop\ndo cartwheel 1 2 3\ndo spin 0.5 0 4\n";
        let (sheet, confused) = Sheet::from_text(text);
        assert_eq!(sheet.len(), 1, "the mark should survive");
        assert_eq!(confused, 1, "and it should say one line was lost");
        assert_eq!(sheet.marks[0].act.steps.len(), 1, "the step it understood should be there");
        assert_eq!(sheet.marks[0].act.steps[0].action, Action::Spin(0.5));
    }

    /// ★ A figure is several strokes that belong together, and that belonging
    /// has to survive the file — or a drawing reopens as a heap of unrelated
    /// lines and every figure has to be gathered up again by hand.
    #[test]
    fn belonging_to_a_figure_survives_the_file() {
        let mut s = Sheet::new();
        for k in 0..3 {
            let mut m = Mark::new(ring(12, 0.5, Cx::new(k as f64, 0.0)), Nib::Round(0.2), 0xFFFFFF);
            m.group = if k < 2 { 7 } else { 0 };
            s.add(m);
        }
        let (back, confused) = Sheet::from_text(&s.to_text());
        assert_eq!(confused, 0);
        assert_eq!(back.marks[0].group, 7);
        assert_eq!(back.marks[1].group, 7);
        assert_eq!(back.marks[2].group, 0, "and belonging to nothing must survive as nothing");
    }

    /// ★ Keyframes are the part of a drawing that takes longest to get right,
    /// so they had better survive the file exactly — a pose that came back a
    /// hair different at every key would show as a wobble nobody could find.
    #[test]
    fn keyframes_survive_the_file_exactly() {
use crate::track::Ease;
        let mut m = Mark::new(ring(12, 1.0, Cx::ZERO), Nib::Round(0.2), 0xFFFFFF);
        m.track.set(0.0, Pose::STILL, Ease::Smooth);
        m.track.set(1.25, Pose::new(Cx::polar(1.5, 0.9), Cx::new(3.0, -2.0)), Ease::Linear);
        m.track.set(2.5, Pose::new(Cx::polar(0.6, -2.2), Cx::new(-1.0, 4.0)), Ease::Hold);
        m.track.looping = false;
        let mut s = Sheet::new();
        s.add(m.clone());

        let (back, confused) = Sheet::from_text(&s.to_text());
        assert_eq!(confused, 0);
        let got = &back.marks[0].track;
        assert_eq!(got.len(), 3);
        assert!(!got.looping, "and whether it repeats");
        for (a, b) in got.keys.iter().zip(&m.track.keys) {
            assert!((a.at - b.at).abs() < 1e-4);
            assert!((a.pose.a - b.pose.a).abs() < 1e-4, "the turn moved: {:?} vs {:?}", a.pose.a, b.pose.a);
            assert!((a.pose.b - b.pose.b).abs() < 1e-4);
            assert_eq!(a.ease, b.ease);
        }
    }

    /// A key with no ease named is the default, not a lost key — an earlier
    /// version of this format might not have written one.
    #[test]
    fn a_key_with_no_ease_named_still_reads() {
        let text = "easel 1\nmark round 0.3 colour FFFFFF fill open\np 0 0 1 1\nkey 1.0 1 0 2 3\n";
        let (sheet, confused) = Sheet::from_text(text);
        assert_eq!(confused, 0);
        assert_eq!(sheet.marks[0].track.len(), 1);
    }

    /// ★ The written half is saved with the drawn half, because it is the
    /// same drawing. And a row comes back **verbatim** -- split it into words
    /// and join them again and `circle(0,  1)` comes back differently spaced,
    /// which is somebody's formatting quietly rewritten under them.
    #[test]
    fn the_script_is_saved_with_the_drawing_exactly_as_written() {
        let mut s = Sheet::new();
        s.script.add("r = 2");
        s.script.add("circle(0,  r)   # two spaces, on purpose");
        s.script.rows.push(Row::new("ngon(0, r, 5)").off());
        s.add(Mark::new(ring(12, 1.0, Cx::ZERO), Nib::Round(0.2), 0xFFFFFF));

        let (back, confused) = Sheet::from_text(&s.to_text());
        assert_eq!(confused, 0);
        assert_eq!(back.script.rows, s.script.rows, "every row, verbatim");
        assert_eq!(back.marks.len(), 1, "and the drawn half too");
    }

    /// An empty drawing writes and reads as an empty drawing rather than as
    /// nothing at all — saving before you have drawn is an ordinary thing.
    #[test]
    fn an_empty_drawing_is_still_a_drawing() {
        let (back, confused) = Sheet::from_text(&Sheet::new().to_text());
        assert_eq!(confused, 0);
        assert!(back.is_empty());
    }

    /// Nonsense is not a drawing, but it is not a crash either.
    #[test]
    fn opening_something_that_is_not_a_drawing_is_survivable() {
        let (sheet, confused) = Sheet::from_text("\u{0}\u{1}binary rubbish\n\n\nmore rubbish");
        assert!(sheet.is_empty());
        assert!(confused > 0);
    }

    /// ★ The mark you point at is the one you can **see** — the last one
    /// drawn. Searching from the bottom up picks whatever happens to be
    /// underneath, which feels broken in a way people cannot describe.
    #[test]
    fn pointing_at_an_overlap_picks_the_one_on_top() {
        let mut s = Sheet::new();
        s.add(Mark::new(ring(40, 2.0, Cx::ZERO), Nib::Round(0.2), 0x111111).closed(true));
        s.add(Mark::new(ring(40, 2.0, Cx::ZERO), Nib::Round(0.2), 0x222222).closed(true));
        assert_eq!(s.at(Cx::new(2.0, 0.0), 0.1, 0.0), Some(1), "the top one, by its edge");
        // And by its middle, which is a closed shape's inside. This used to
        // say the opposite: the nib sweeps a ring and the middle is the hole
        // in it, which is even-odd being right and is also completely useless
        // for tapping a box.
        assert_eq!(s.at(Cx::new(0.0, 0.0), 0.05, 0.0), Some(1), "and by its middle");
    }

    /// ★ **A closed shape is hit anywhere inside it**, and this is the fix for
    /// a game that could not be played.
    ///
    /// Draw a box: the nib sweeps a thin rectangular *ring*, and the middle of
    /// the box is the hole in that ring — outside it by the even-odd rule, and
    /// correctly so. But nobody tapping a box means "the two pixels of its
    /// edge". So the question asked is whether the point is inside the line
    /// the hand drew, not inside the ink.
    #[test]
    fn the_inside_of_a_closed_shape_can_be_tapped() {
        let mut s = Sheet::new();
        s.add(Mark::new(ring(40, 2.0, Cx::ZERO), Nib::Round(0.1), 0xFFFFFF).closed(true));
        assert_eq!(s.at(Cx::new(0.0, 0.0), 0.05, 0.0), Some(0), "the middle of a box is the box");
        assert_eq!(s.at(Cx::new(2.0, 0.0), 0.15, 0.0), Some(0), "and so is its edge");
        assert_eq!(s.at(Cx::new(9.0, 0.0), 0.15, 0.0), None, "but outside is outside");
    }

    /// An **open** stroke is only its line — it encloses nothing, so there is
    /// no inside for a tap to be in.
    #[test]
    fn an_open_stroke_is_only_its_line() {
        let mut s = Sheet::new();
        s.add(Mark::new(vec![Cx::new(-2.0, 0.0), Cx::new(2.0, 0.0)], Nib::Round(0.1), 0xFFFFFF));
        assert_eq!(s.at(Cx::new(0.0, 1.5), 0.05, 0.0), None);
        assert_eq!(s.at(Cx::new(0.0, 0.0), 0.15, 0.0), Some(0));
    }

    /// And a closed shape that has walked away is hit where it has walked to.
    #[test]
    fn a_moving_box_is_tapped_where_it_has_got_to() {
        use crate::track::Ease;
        let mut s = Sheet::new();
        let mut m = Mark::new(ring(40, 1.0, Cx::ZERO), Nib::Round(0.1), 0xFFFFFF).closed(true);
        m.track.looping = false;
        m.track.set(0.0, Pose::STILL, Ease::Linear);
        m.track.set(1.0, Pose::new(Cx::ONE, Cx::new(6.0, 0.0)), Ease::Linear);
        s.add(m);
        assert_eq!(s.at(Cx::new(6.0, 0.0), 0.05, 1.0), Some(0), "inside it, where it now is");
        assert_eq!(s.at(Cx::new(0.0, 0.0), 0.05, 1.0), None, "and not where it used to be");
    }

    /// ★ A moving mark must be catchable **where it is**, not where it was
    /// drawn. Without the time, grabbing an animated shape means finding an
    /// invisible copy of it somewhere else on the page.
    #[test]
    fn a_moving_mark_is_caught_where_it_has_moved_to() {
use crate::track::Ease;
        let mut s = Sheet::new();
        let mut m = Mark::new(ring(40, 0.6, Cx::ZERO), Nib::Round(0.2), 0xFFFFFF).closed(true);
        m.track.looping = false;
        m.track.set(0.0, Pose::STILL, Ease::Linear);
        m.track.set(1.0, Pose::new(Cx::ONE, Cx::new(5.0, 0.0)), Ease::Linear);
        s.add(m);

        assert_eq!(s.at(Cx::new(0.6, 0.0), 0.1, 0.0), Some(0), "at the start, where it was drawn");
        assert_eq!(s.at(Cx::new(5.6, 0.0), 0.1, 1.0), Some(0), "at one second, where it now is");
        assert_eq!(s.at(Cx::new(0.6, 0.0), 0.1, 1.0), None, "and not where it used to be");
    }

    /// Pointing at nothing is nothing.
    #[test]
    fn pointing_at_empty_paper_finds_nothing() {
        assert_eq!(a_sheet().at(Cx::new(40.0, 40.0), 0.1, 0.0), None);
        assert_eq!(Sheet::new().at(Cx::ZERO, 0.1, 0.0), None);
    }

    /// Bounds cover every mark, so the view can be framed on the work.
    #[test]
    fn bounds_cover_the_whole_drawing() {
        let (lo, hi) = a_sheet().bounds().expect("it has marks");
        // The traced line runs to -3 exactly: an unfilled mark is its
        // centreline, so it has no nib width to add.
        assert!(lo.re <= -3.0 && hi.re > 5.0, "from {lo:?} to {hi:?}");
        assert!(Sheet::new().bounds().is_none(), "empty paper has no bounds");
    }
}
