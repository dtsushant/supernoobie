//! # studio — a maths game, drawn out of sine waves
//!
//! Run:  `cargo run -p studio --release`
//!
//! A sum appears. Type the answer. That is the whole game.
//!
//! **Nothing on the screen is a font.** Every digit is a closed curve run
//! through a Fourier transform and redrawn as a sum of rotating arrows, so the
//! numbers are literally made by adding waves together. Press `-` and `=` to
//! change how many waves are in the sum and watch a `7` melt back into a circle
//! and reform. Press `E` to see the arrows doing it.
//!
//! Every shape here — the digits, the tally marks, the plus, the smiley, the
//! ghost — comes from the [`shapes`] crate, and each one knows how to explain
//! itself:
//!
//! ```text
//!     cargo run -p shapes -- seven --steps
//!     cargo run -p shapes -- ghost
//! ```
//!
//! This file contains no geometry at all. It holds the state of the game, says
//! where things go on the page, and runs a window. Delete it and the
//! mathematics is untouched.
//!
//! ## Controls
//!
//! ```text
//!   0-9    type your answer      -  =   fewer / more waves
//!   Enter  check it              E      show the spinning arrows
//!   Bksp   rub one out           N      a new sum
//!   Esc    quit
//! ```

use minifb::{Key, KeyRepeat, Window, WindowOptions};
use plotkit::{raster::Canvas, Cx, Frame, Shape, View};
use shapes::{count, digit, face, glyph, Draw, Place, Series};
use std::sync::Arc;

// ===========================================================================
//  THE GAME
// ===========================================================================

/// Whatever random means without a crate. A linear congruential generator —
/// the one in the back of Knuth — is plenty for picking sums.
struct Rng(u64);
impl Rng {
    fn upto(&mut self, n: u64) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) % n
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Mood {
    Asking,
    Right,
    Wrong,
}

struct Game {
    a: u32,
    b: u32,
    typed: String,
    mood: Mood,
    /// Seconds since the mood last changed.
    since: f64,
    terms: usize,
    show_arrows: bool,
    t: f64,
    rng: Rng,
    /// The waves of each digit, worked out once at startup.
    waves: Vec<Arc<Series>>,
}

impl Game {
    fn new(seed: u64) -> Game {
        let mut g = Game {
            a: 3,
            b: 4,
            typed: String::new(),
            mood: Mood::Asking,
            since: 0.0,
            terms: 40,
            show_arrows: false,
            t: 0.0,
            rng: Rng(seed | 1),
            waves: (0..10).map(|d| Arc::new(digit::series(d))).collect(),
        };
        g.ask();
        g
    }

    fn answer(&self) -> u32 {
        self.a + self.b
    }

    fn ask(&mut self) {
        self.a = 1 + self.rng.upto(5) as u32;
        self.b = 1 + self.rng.upto(5) as u32;
        self.typed.clear();
        self.mood = Mood::Asking;
        self.since = 0.0;
    }

    fn check(&mut self) {
        if self.typed.is_empty() {
            return;
        }
        let right = self.typed.parse::<u32>().map(|n| n == self.answer()).unwrap_or(false);
        self.mood = if right { Mood::Right } else { Mood::Wrong };
        self.since = 0.0;
    }

    /// A digit, built from however many waves are currently switched on.
    fn digit(&self, d: u32) -> Shape {
        self.waves[d as usize].curve(self.terms)
    }
}

// ===========================================================================
//  THE SCENE
//
//  Where things go on the page. Every shape arrives from `shapes` already
//  centred on its own origin, so placing one is a single call.
// ===========================================================================

const CYAN: u32 = 0x4FBCD4;
const AMBER: u32 = 0xE0A44A;
const PINK: u32 = 0xE585AC;
const GREEN: u32 = 0x6FCF97;
const VIOLET: u32 = 0x9B7BD4;
const GREY: u32 = 0x5A6774;
const DIM: u32 = 0x2C3742;

const ROW: f64 = 3.7; // the sum
const STICKS: f64 = 0.9;
const FACE: f64 = -4.1; // how it went
const AX: f64 = -6.5; // where the first digit sits
const BX: f64 = -1.3;

fn scene(g: &Game) -> Frame {
    let mut f = Frame::new();

    // --- the sum ----------------------------------------------------------
    f.place(g.digit(g.a).sized(1.55), Cx::new(AX, ROW)).color(CYAN).width(3);
    f.place(glyph::plus(), Cx::new(-3.9, ROW)).color(GREY).width(3);
    f.place(g.digit(g.b).sized(1.55), Cx::new(BX, ROW)).color(AMBER).width(3);
    f.place(glyph::equals(), Cx::new(1.4, ROW)).color(GREY).width(3);

    // --- the answer box ---------------------------------------------------
    let box_at = Cx::new(5.3, ROW);
    f.place(Shape::rect(Cx::new(-1.9, -1.7), Cx::new(1.9, 1.7)), box_at).color(DIM).width(2);
    if g.typed.is_empty() {
        // A blinking caret, so it is obvious the box wants typing.
        if (g.t * 2.0) as i64 % 2 == 0 {
            f.place(Shape::path(vec![Cx::new(0.0, -1.1), Cx::new(0.0, 1.1)]), box_at).color(PINK).width(3);
        }
    } else {
        let ds: Vec<u32> = g.typed.chars().filter_map(|ch| ch.to_digit(10)).collect();
        for (k, d) in ds.iter().enumerate() {
            let x = box_at.re + (k as f64 - (ds.len() as f64 - 1.0) / 2.0) * 1.15;
            f.place(g.digit(*d).sized(1.25), Cx::new(x, ROW)).color(PINK).width(3);
        }
    }

    // --- the sticks, centred under the digit they count -------------------
    f.place(count::tally(g.a), Cx::new(AX, STICKS)).color(CYAN).width(3);
    f.place(count::tally(g.b), Cx::new(BX, STICKS)).color(AMBER).width(3);
    f.label(Cx::new(AX, -0.1), "this many", GREY, 2);
    f.label(Cx::new(BX, -0.1), "and this many", GREY, 2);

    // --- the spinning arrows ----------------------------------------------
    if g.show_arrows {
        let machine = g.waves[g.a as usize].machine(g.terms, g.t * 0.9);
        f.place(machine.sized(1.55), Cx::new(AX, ROW)).color(0x3B4A59).width(1);
    }

    // --- how it went ------------------------------------------------------
    let spot = Cx::new(-3.4, FACE);
    match g.mood {
        Mood::Asking => {}
        Mood::Right => {
            f.place(confetti(28, (g.since * 0.55).min(1.0)), spot).color(GREEN).dot(4.0);
            f.place(face::smiley(1.1), spot + bounce(g.t)).color(GREEN).width(3);
            f.label(Cx::new(1.8, -3.9), "GOOD JOB!", GREEN, 5);
            f.label(Cx::new(1.8, -5.3), "press N for another one", GREY, 2);
        }
        Mood::Wrong => {
            f.place(face::ghost(0.95), spot + sway(g.t)).color(VIOLET).width(3);
            // The o's grow as the boo goes on. Cute, and free.
            let n = 3 + ((g.since * 4.0) as usize).min(5);
            f.label(Cx::new(1.8, -3.9), format!("b{}...", "o".repeat(n)), VIOLET, 5);
            f.label(Cx::new(1.8, -5.3), "not quite! backspace and try again", GREY, 2);
        }
    }
    f
}

/// Dots flung outwards. The `k`-th sits at angle `2πk/n`, which is the `n`-th
/// roots of unity doing party duty.
fn confetti(n: usize, age: f64) -> Shape {
    use std::f64::consts::TAU;
    let r = 0.9 + 3.2 * age;
    Shape::points(
        (0..n)
            .map(|k| Cx::polar(r * (1.0 - 0.25 * ((k * 7) % 5) as f64 / 5.0), TAU * k as f64 / n as f64 + age * 1.4))
            .collect::<Vec<_>>(),
    )
}

fn bounce(t: f64) -> Cx {
    Cx::new(0.0, 0.10 * (t * 6.0).sin())
}

fn sway(t: f64) -> Cx {
    Cx::new(0.22 * (t * 2.2).sin(), 0.12 * (t * 3.1).cos())
}

// ===========================================================================
//  THE WINDOW
//
//  The only part that knows there is a screen.
// ===========================================================================

const W: usize = 1200;
const H: usize = 740;

fn main() {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x5EED_1234);

    let mut g = Game::new(seed);
    let v = View::centred(W, H, 52.0).with_origin(W as f64 * 0.5, H as f64 * 0.42);
    let mut c = Canvas::new(W, H);
    let mut win = Window::new("STUDIO  -  numbers made of waves", W, H, WindowOptions::default()).expect("no window");
    win.set_target_fps(60);

    while win.is_open() && !win.is_key_down(Key::Escape) {
        for k in win.get_keys_pressed(KeyRepeat::No) {
            match k {
                Key::Enter | Key::NumPadEnter => g.check(),
                Key::Backspace => {
                    g.typed.pop();
                    g.mood = Mood::Asking;
                }
                Key::N => g.ask(),
                Key::E => g.show_arrows = !g.show_arrows,
                Key::Minus => g.terms = g.terms.saturating_sub(1).max(1),
                Key::Equal => g.terms = (g.terms + 1).min(80),
                _ => {
                    if let Some(d) = digit_key(k) {
                        if g.mood != Mood::Right && g.typed.len() < 2 {
                            g.typed.push(d);
                            g.mood = Mood::Asking;
                        }
                    }
                }
            }
        }

        g.t += 1.0 / 60.0;
        g.since += 1.0 / 60.0;

        c.clear(0x0B1017);
        scene(&g).draw(&mut c, &v);
        c.text(
            14,
            H as i32 - 26,
            &format!("waves per digit: {}    ( - fewer   = more   E arrows   Enter check   N new   Esc quit )", g.terms),
            GREY,
            2,
        );
        win.update_with_buffer(&c.buf, W, H).expect("present failed");
    }
}

fn digit_key(k: Key) -> Option<char> {
    use Key::*;
    Some(match k {
        Key0 | NumPad0 => '0',
        Key1 | NumPad1 => '1',
        Key2 | NumPad2 => '2',
        Key3 | NumPad3 => '3',
        Key4 | NumPad4 => '4',
        Key5 | NumPad5 => '5',
        Key6 | NumPad6 => '6',
        Key7 | NumPad7 => '7',
        Key8 | NumPad8 => '8',
        Key9 | NumPad9 => '9',
        _ => return None,
    })
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checking_an_answer_says_yes_or_no() {
        let mut g = Game::new(1);
        (g.a, g.b) = (3, 4);
        g.typed = "7".into();
        g.check();
        assert!(g.mood == Mood::Right);
        g.typed = "8".into();
        g.check();
        assert!(g.mood == Mood::Wrong);
    }

    /// An empty box is not a wrong answer — pressing Enter with nothing typed
    /// should not summon the ghost.
    #[test]
    fn an_empty_answer_is_not_an_answer() {
        let mut g = Game::new(2);
        g.typed.clear();
        g.mood = Mood::Asking;
        g.check();
        assert!(g.mood == Mood::Asking);
    }

    /// Sums stay inside what a small child can count on sticks.
    #[test]
    fn the_sums_stay_small() {
        let mut g = Game::new(99);
        for _ in 0..500 {
            g.ask();
            assert!((1..=5).contains(&g.a) && (1..=5).contains(&g.b));
            assert!(g.answer() <= 10);
        }
    }

    /// Both digits vary. A generator that returned the same number twice would
    /// pass every other test here while making a very boring game.
    #[test]
    fn the_questions_actually_change() {
        let mut g = Game::new(7);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..200 {
            g.ask();
            seen.insert((g.a, g.b));
        }
        assert!(seen.len() > 10, "only {} different sums in 200 draws", seen.len());
        assert!(seen.iter().any(|(a, b)| a != b), "the two digits are always equal");
    }
}

