//! # tape — everything that happened, so it can happen again
//!
//! A scene is already `f(t, state) -> Frame`, and [`Sketch::step`] funnels
//! every input through one place. So the whole run is
//!
//! ```text
//!     state  =  f(seed, the list of inputs)
//! ```
//!
//! Write that list down and you have saving, loading, undo, replay and a
//! scrubbable timeline out of **one** mechanism — the same trick as making a
//! `Frame` a layer so that animation is `f(t)`, one level up.
//!
//! ```text
//!     cargo run -p studio --release -- --record run.tape
//!     cargo run -p studio --release -- --replay run.tape
//! ```
//!
//! (That resolves to the game because `studio` sets `default-run`. The other
//! binaries in the package need naming: `--bin sketch`, `--bin stage`,
//! `--bin waves`.)
//!
//! ## The rule that makes it work
//!
//! **A state persists; an edge happens once.**
//!
//! Holding a key down is a *state* — if the tape skips a frame, the key is
//! still down. Pressing it is an *edge* — it happened at one instant and must
//! not happen again. So a snapshot is written only when the input changes, and
//! when replay reuses an earlier snapshot it keeps the held keys and the
//! pointer while dropping the presses, clicks and scrolls.
//!
//! Get that backwards and a single click replays as sixty clicks a second.
//!
//! ## What has to be in the tape
//!
//! Anything the run depends on that is not the inputs:
//!
//! * **The seed.** The game seeds its generator from the clock, so without
//!   recording it a replay would ask different questions.
//! * **The frame rate.** Time must come from counting frames, not from the
//!   wall clock, or a replay on a busier machine drifts. [`crate::Graph`]
//!   already advances `t` by a fixed step, which was accidentally right.
//!
//! ## The format
//!
//! Plain text, one snapshot per line, so a tape can be read, diffed and edited
//! by hand:
//!
//! ```text
//!     tape 1
//!     seed 8134922175
//!     dt 0.0166666666666667
//!     12 n n 3.1000 -1.4000 517.0 289.0 do 0.000
//!     13 * n 3.1000 -1.4000 517.0 289.0 do 0.000
//!     ^  ^ ^ ^______________ ^_________  ^   ^
//!     |  | |     world          screen   |   scroll
//!     |  | held keys                   flags
//!     |  pressed this frame ( * means none )
//!     frame
//! ```

use crate::{Keys, KEY_CODES};
use plotkit::Cx;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};

/// The input at one frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub frame: u32,
    pub keys: Keys,
}

/// A recording of a run: the seed, the frame rate, and every input.
pub struct Tape {
    pub seed: u64,
    pub dt: f64,
    snaps: Vec<Snapshot>,
    /// Written as it goes, so a crash still leaves a usable tape.
    sink: Option<BufWriter<File>>,
    last: Option<Keys>,
}

impl Tape {
    /// An empty tape, held in memory.
    pub fn new(seed: u64, dt: f64) -> Tape {
        Tape { seed, dt, snaps: Vec::new(), sink: None, last: None }
    }

    /// An empty tape that writes each snapshot to `path` as it is recorded.
    ///
    /// Appending as it goes rather than saving at the end means a run that
    /// crashes — or that you close by killing the window — still leaves a tape
    /// of everything up to that moment. Which is usually the run you most
    /// wanted to keep.
    pub fn to(path: &str, seed: u64, dt: f64) -> std::io::Result<Tape> {
        let mut w = BufWriter::new(File::create(path)?);
        writeln!(w, "tape 1")?;
        writeln!(w, "seed {seed}")?;
        writeln!(w, "dt {dt:.17}")?;
        w.flush()?;
        Ok(Tape { seed, dt, snaps: Vec::new(), sink: Some(w), last: None })
    }

    /// Note the input at `frame`, if it differs from the frame before.
    ///
    /// Most frames of most runs have nothing happening in them, so this is
    /// where a tape stops being one line per frame and becomes small.
    pub fn record(&mut self, frame: u32, k: &Keys) {
        if self.last.as_ref() == Some(k) {
            return;
        }
        self.last = Some(k.clone());
        let line = encode(frame, k);
        self.snaps.push(Snapshot { frame, keys: k.clone() });
        if let Some(w) = &mut self.sink {
            let _ = writeln!(w, "{line}");
            let _ = w.flush();
        }
    }

    /// The input at `frame`.
    ///
    /// The most recent snapshot at or before it — **with the edges stripped**
    /// if that snapshot belongs to an earlier frame, because a press happened
    /// once and holding it forward would replay it every frame since.
    pub fn at(&self, frame: u32) -> Keys {
        match self.snaps.partition_point(|s| s.frame <= frame) {
            0 => Keys::none(),
            n => {
                let s = &self.snaps[n - 1];
                if s.frame == frame {
                    s.keys.clone()
                } else {
                    s.keys.clone().steady()
                }
            }
        }
    }

    pub fn len(&self) -> usize {
        self.snaps.len()
    }
    pub fn is_empty(&self) -> bool {
        self.snaps.is_empty()
    }

    /// The last frame anything happened on — how long the run was.
    pub fn last_frame(&self) -> u32 {
        self.snaps.last().map_or(0, |s| s.frame)
    }

    /// Write the whole tape out in one go. [`Tape::to`] is usually better.
    pub fn save(&self, path: &str) -> std::io::Result<()> {
        let mut w = BufWriter::new(File::create(path)?);
        writeln!(w, "tape 1")?;
        writeln!(w, "seed {}", self.seed)?;
        writeln!(w, "dt {:.17}", self.dt)?;
        for s in &self.snaps {
            writeln!(w, "{}", encode(s.frame, &s.keys))?;
        }
        w.flush()
    }

    pub fn load(path: &str) -> std::io::Result<Tape> {
        let mut t = Tape::new(0, 1.0 / 60.0);
        for line in BufReader::new(File::open(path)?).lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(v) = line.strip_prefix("seed ") {
                t.seed = v.trim().parse().unwrap_or(0);
            } else if let Some(v) = line.strip_prefix("dt ") {
                t.dt = v.trim().parse().unwrap_or(1.0 / 60.0);
            } else if line.starts_with("tape ") {
                // version; only one so far
            } else if let Some(s) = decode(line) {
                t.snaps.push(s);
            }
        }
        Ok(t)
    }
}

/// How an empty field is written, so that every line keeps the same number of
/// columns and `split_whitespace` can never lose count.
///
/// `*` and not `-`, because `-` is the tape code for the Minus key: a snapshot
/// whose only keypress was `-` encoded as an empty field and read back as
/// nothing at all.
const EMPTY: &str = "*";

fn encode(frame: u32, k: &Keys) -> String {
    let letters = |v: &str| if v.is_empty() { EMPTY.to_string() } else { v.to_string() };
    let mut flags = String::new();
    for (on, c) in [
        (k.down(), 'd'),
        (k.clicked(), 'c'),
        (k.right_down(), 'r'),
        (k.middle(), 'm'),
        (k.shift(), 's'),
        (k.over(), 'o'),
        (k.home_pressed(), 'h'),
    ] {
        if on {
            flags.push(c);
        }
    }
    format!(
        "{frame} {} {} {:.4} {:.4} {:.1} {:.1} {} {:.3}",
        letters(&k.pressed_codes()),
        letters(&k.held_codes()),
        k.at().re,
        k.at().im,
        k.at_px().0,
        k.at_px().1,
        letters(&flags),
        k.scroll()
    )
}

fn decode(line: &str) -> Option<Snapshot> {
    let f: Vec<&str> = line.split_whitespace().collect();
    if f.len() != 9 {
        return None;
    }
    fn un(s: &str) -> &str {
        if s == EMPTY {
            ""
        } else {
            s
        }
    }
    let flags = un(f[7]);
    Some(Snapshot {
        frame: f[0].parse().ok()?,
        keys: Keys::from_parts(
            un(f[1]),
            un(f[2]),
            Cx::new(f[3].parse().ok()?, f[4].parse().ok()?),
            (f[5].parse().ok()?, f[6].parse().ok()?),
            flags.contains('d'),
            flags.contains('c'),
            flags.contains('r'),
            flags.contains('m'),
            flags.contains('s'),
            flags.contains('o'),
            flags.contains('h'),
            f[8].parse().ok()?,
        ),
    })
}

/// Every key a tape can carry, for the doc and for tests.
pub fn recordable_keys() -> &'static str {
    KEY_CODES
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> String {
        format!("{}/{name}", std::env::temp_dir().display())
    }

    #[test]
    fn a_tape_only_notes_what_changed() {
        let mut t = Tape::new(1, 1.0 / 60.0);
        for f in 0..100 {
            t.record(f, &Keys::none()); // nothing happening, all frame long
        }
        assert_eq!(t.len(), 1, "a hundred identical frames is one snapshot");

        t.record(100, &Keys::pressing("n"));
        t.record(101, &Keys::none());
        assert_eq!(t.len(), 3);
    }

    /// ★ The rule the whole thing turns on. A held key is a state and survives
    /// being held forward; a press is an edge and must not. Get this backwards
    /// and one click replays as sixty clicks a second.
    #[test]
    fn a_state_persists_and_an_edge_happens_once() {
        let mut t = Tape::new(1, 1.0 / 60.0);
        t.record(10, &Keys::pressing("n")); // pressed AND held at frame 10

        let at_press = t.at(10);
        assert!(at_press.just('n') && at_press.held('n'));

        for f in 11..40 {
            let later = t.at(f);
            assert!(later.held('n'), "the key is still down at frame {f}");
            assert!(!later.just('n'), "but it was only pressed once, not again at {f}");
        }
    }

    /// The same rule for the mouse: the button stays down, the click does not
    /// repeat, and a flick of the wheel is not an endless scroll.
    #[test]
    fn a_click_and_a_scroll_do_not_repeat() {
        let mut t = Tape::new(1, 1.0 / 60.0);
        t.record(5, &Keys::clicking(Cx::new(1.0, 2.0)));

        assert!(t.at(5).clicked());
        let later = t.at(9);
        assert!(!later.clicked(), "a click is an edge");
        assert!(later.down(), "but the button is still held");
        assert_eq!(later.at(), Cx::new(1.0, 2.0), "and the pointer has not moved");
    }

    #[test]
    fn before_the_first_snapshot_nothing_is_happening() {
        let mut t = Tape::new(1, 1.0 / 60.0);
        t.record(50, &Keys::pressing("a"));
        assert_eq!(t.at(0), Keys::none());
        assert_eq!(t.at(49), Keys::none());
    }

    /// ★ A tape written out and read back is the same tape. If it were not,
    /// a replay would diverge from the run it came from — silently, and worse
    /// the longer it ran.
    #[test]
    fn a_tape_survives_the_round_trip() {
        let path = tmp("plotkit-roundtrip.tape");
        let mut t = Tape::new(8_134_922_175, 1.0 / 60.0);
        t.record(3, &Keys::pressing("n7"));
        t.record(9, &Keys::holding("w"));
        t.record(11, &Keys::clicking(Cx::new(3.25, -1.5)));
        t.record(20, &Keys::none());
        t.save(&path).expect("write");

        let back = Tape::load(&path).expect("read");
        assert_eq!(back.seed, t.seed);
        assert!((back.dt - t.dt).abs() < 1e-15);
        assert_eq!(back.len(), t.len());
        for f in 0..30 {
            assert_eq!(back.at(f), t.at(f), "frame {f} came back different");
        }
        let _ = std::fs::remove_file(&path);
    }

    /// Writing as it goes means a run that is killed still leaves a usable
    /// tape — usually the run you most wanted to keep.
    #[test]
    fn a_tape_is_readable_before_it_is_finished() {
        let path = tmp("plotkit-partial.tape");
        {
            let mut t = Tape::to(&path, 42, 1.0 / 60.0).expect("create");
            t.record(1, &Keys::pressing("a"));
            t.record(2, &Keys::none());
            // deliberately not dropped tidily — read it while it is open
            let back = Tape::load(&path).expect("read while open");
            assert_eq!(back.seed, 42);
            assert_eq!(back.len(), 2);
        }
        let _ = std::fs::remove_file(&path);
    }

    /// The seed has to be in the tape. The game seeds itself from the clock,
    /// so a replay without it would ask completely different questions.
    #[test]
    fn the_seed_travels_with_the_tape() {
        let path = tmp("plotkit-seed.tape");
        Tape::new(0xDEAD_BEEF, 1.0 / 60.0).save(&path).expect("write");
        assert_eq!(Tape::load(&path).expect("read").seed, 0xDEAD_BEEF);
        let _ = std::fs::remove_file(&path);
    }

    /// Every key that can be pressed can be recorded. A key missing from the
    /// table would replay as nothing at all — and only for that one key, which
    /// is the sort of bug that takes an afternoon.
    #[test]
    fn every_recordable_key_survives_the_round_trip() {
        for c in recordable_keys().chars() {
            let k = Keys::pressing(&c.to_string());
            let line = encode(7, &k);
            let back = decode(&line).unwrap_or_else(|| panic!("{c:?} did not decode: {line}"));
            assert_eq!(back.keys, k, "{c:?} did not survive");
        }
    }

    #[test]
    fn a_rubbish_line_is_skipped_rather_than_fatal() {
        assert!(decode("this is not a snapshot").is_none());
        assert!(decode("").is_none());
    }
}
