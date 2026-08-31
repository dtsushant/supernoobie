//! # studio — a maths game, drawn out of sine waves
//!
//! Run:  `cargo run -p studio --release`
//!
//! A sum appears. Type the answer. That is the whole game.
//!
//! The part worth knowing is that **nothing on the screen is a font.** Every
//! digit is a closed curve, and every closed curve is a sum of sine waves —
//! so the numbers are literally drawn by adding waves together. Press `-` and
//! `=` to change how many waves are in the sum and watch a `7` melt back into
//! a circle and reform.
//!
//! ## How a wiggly curve becomes a sum of waves
//!
//! Walk around the outline of a digit at a steady pace and write down where
//! you are. You get a function `z(θ)` from an angle to a point in the plane —
//! a complex number in, a complex number out. Any such function that comes
//! back where it started can be written
//!
//! ```text
//!     z(θ)  =  Σ  c_n · e^{i n θ}
//!             n
//! ```
//!
//! Each term is an arrow of length `|c_n|` spinning at `n` turns per lap. Stack
//! them tip to tail and the last tip traces the digit. Because
//! `e^{inθ} = cos nθ + i sin nθ`, that sum *is* a sum of sines and cosines, one
//! pair per term.
//!
//! Getting the `c_n` back out is one line, and it is the same trick as pulling
//! a phasor out in `bin/waves.rs` — multiply by the conjugate wave and average,
//! which kills every term except the one you asked for:
//!
//! ```text
//!     c_n  =  (1/N) Σ  z_k · e^{-i n θ_k}
//!                   k
//! ```
//!
//! Keep the biggest few `c_n` and you have a recognisable digit out of a
//! handful of waves. That is the entire drawing engine.
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
use std::f64::consts::TAU;
use std::sync::Arc;

// ===========================================================================
//  THE MATHEMATICS
//
//  Nothing below this line knows what a pixel is. It is all curves, sums and
//  complex numbers, and the tests at the bottom check it on its own terms.
// ===========================================================================

/// A slice of an ellipse, as points. The building block every digit is made of.
fn arc(c: Cx, rx: f64, ry: f64, a0: f64, a1: f64, n: usize) -> Vec<Cx> {
    (0..=n)
        .map(|k| {
            let a = a0 + (a1 - a0) * k as f64 / n as f64;
            c + Cx::new(rx * a.cos(), ry * a.sin())
        })
        .collect()
}

/// The outline of a digit, as one continuous closed loop in a box roughly
/// `x ∈ [-0.4, 0.4]`, `y ∈ [-0.9, 0.9]`.
///
/// The loop has to *close* — the Fourier series only represents periodic
/// things — so open strokes are walked out and walked back. The return trip
/// lands exactly on the outward one, so it is invisible, and it costs nothing
/// but a factor of two in samples.
fn digit_outline(d: u32) -> Vec<Cx> {
    let p = |x: f64, y: f64| Cx::new(x, y);
    let stroke: Vec<Cx> = match d {
        // Already closed: a plain ellipse.
        0 => return arc(Cx::ZERO, 0.40, 0.88, 0.0, TAU, 120),

        1 => vec![p(-0.30, 0.50), p(0.02, 0.92), p(0.02, -0.90)],

        2 => {
            let mut v = vec![p(-0.36, 0.50)];
            v.extend(arc(p(0.00, 0.48), 0.36, 0.40, 2.6, -0.2, 34));
            v.extend([p(-0.34, -0.88), p(0.38, -0.88)]);
            v
        }

        3 => {
            let mut v = arc(p(0.00, 0.48), 0.34, 0.40, 2.3, -1.4, 32);
            v.extend(arc(p(0.00, -0.42), 0.38, 0.46, 1.5, -2.5, 36));
            v
        }

        // Up the stem, down the diagonal, across the bar, back to the stem.
        4 => vec![p(0.20, 0.92), p(-0.40, -0.14), p(0.40, -0.14), p(0.20, -0.14), p(0.20, -0.90)],

        5 => {
            let mut v = vec![p(0.36, 0.90), p(-0.28, 0.90), p(-0.32, 0.10)];
            v.extend(arc(p(0.02, -0.40), 0.38, 0.48, 1.7, -2.4, 36));
            v
        }

        // A tail curling into a closed bowl.
        6 => {
            let mut v = arc(p(0.02, 0.30), 0.36, 0.58, 0.9, 3.1416, 30);
            v.extend(arc(p(0.00, -0.40), 0.38, 0.46, 3.1416, 3.1416 - TAU, 46));
            v
        }

        7 => vec![p(-0.38, 0.90), p(0.38, 0.90), p(-0.06, -0.90)],

        // A figure of eight, traced through the junction at (0, 0.04).
        8 => {
            let mut v = arc(p(0.00, 0.44), 0.32, 0.40, -1.5708, -1.5708 + TAU, 52);
            v.extend(arc(p(0.00, -0.42), 0.38, 0.46, 1.5708, 1.5708 - TAU, 56));
            return v;
        }

        // The mirror of 6: a closed bowl on top, with a tail falling away.
        _ => {
            let mut v = arc(p(0.00, 0.42), 0.36, 0.44, 0.0, TAU, 46);
            v.extend([p(0.34, -0.28), p(0.28, -0.62), p(0.10, -0.84), p(-0.20, -0.90)]);
            v
        }
    };
    // Out and back, so the path is a loop.
    let mut loop_ = stroke.clone();
    loop_.extend(stroke.into_iter().rev().skip(1));
    loop_
}

/// Re-space a path so consecutive points are equally far apart along the curve.
///
/// Without this the pen would dawdle where the original points were dense and
/// sprint where they were sparse. The series would still be correct, but a
/// truncated one would waste its few terms describing the dawdling instead of
/// the shape.
fn resample(path: &[Cx], n: usize) -> Vec<Cx> {
    let mut cum = vec![0.0];
    for w in path.windows(2) {
        cum.push(cum.last().unwrap() + (w[1] - w[0]).abs());
    }
    let total = *cum.last().unwrap();
    if total <= 0.0 {
        return vec![path[0]; n];
    }
    let mut out = Vec::with_capacity(n);
    let mut j = 0;
    for k in 0..n {
        let want = total * k as f64 / n as f64;
        while j + 2 < path.len() && cum[j + 1] < want {
            j += 1;
        }
        let span = (cum[j + 1] - cum[j]).max(1e-12);
        let u = ((want - cum[j]) / span).clamp(0.0, 1.0);
        out.push(path[j] + (path[j + 1] - path[j]).scale(u));
    }
    out
}

/// The discrete Fourier transform: `c_n = (1/N) Σ z_k e^{-i n θ_k}`.
///
/// Multiplying by `e^{-inθ}` un-spins the term you want so it stops moving,
/// while every other term keeps spinning and averages to nothing. What
/// survives the average is exactly `c_n`.
///
/// Returned sorted by `|c_n|`, largest first, so taking the first `m` keeps the
/// `m` waves that matter most.
fn dft(z: &[Cx]) -> Vec<(i32, Cx)> {
    let n = z.len();
    let half = (n / 2) as i32;
    let mut out: Vec<(i32, Cx)> = (-half..half)
        .map(|f| {
            let mut acc = Cx::ZERO;
            for (k, zk) in z.iter().enumerate() {
                acc = acc + *zk * Cx::expi(-TAU * f as f64 * k as f64 / n as f64);
            }
            (f, acc.scale(1.0 / n as f64))
        })
        .collect();
    out.sort_by(|a, b| b.1.abs().partial_cmp(&a.1.abs()).unwrap());
    out
}

/// Add the first `terms` waves back up: `Σ c_n e^{inθ}`.
///
/// One term is a point. Two is a circle. Six is a wobbly digit. Forty is the
/// digit.
fn series(c: &[(i32, Cx)], terms: usize, theta: f64) -> Cx {
    c.iter().take(terms.min(c.len())).fold(Cx::ZERO, |acc, (f, cn)| acc + *cn * Cx::expi(*f as f64 * theta))
}

/// Every partial sum, so the arrows can be drawn tip to tail.
fn epicycles(c: &[(i32, Cx)], terms: usize, theta: f64) -> Vec<Cx> {
    let mut z = Cx::ZERO;
    let mut out = vec![z];
    for (f, cn) in c.iter().take(terms.min(c.len())) {
        z = z + *cn * Cx::expi(*f as f64 * theta);
        out.push(z);
    }
    out
}

/// A digit's waves, worked out once at startup.
fn digit_waves(d: u32) -> Vec<(i32, Cx)> {
    dft(&resample(&digit_outline(d), 256))
}

/// Whatever random means without a crate. A linear congruential generator —
/// the same one in the back of Knuth — is plenty for picking sums.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn upto(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

// ===========================================================================
//  THE SCENE
//
//  Placing the mathematics on the page. Still in the units the mathematics
//  uses — not one of these numbers is a pixel.
// ===========================================================================

const CYAN: u32 = 0x4FBCD4;
const AMBER: u32 = 0xE0A44A;
const PINK: u32 = 0xE585AC;
const GREEN: u32 = 0x6FCF97;
const VIOLET: u32 = 0x9B7BD4;
const GREY: u32 = 0x5A6774;
const DIM: u32 = 0x2C3742;


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
    since: f64, // seconds since the mood changed
    terms: usize,
    show_arrows: bool,
    t: f64,
    rng: Rng,
    waves: Vec<Arc<Vec<(i32, Cx)>>>,
}

impl Game {
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
        let ok = self.typed.parse::<u32>().map(|n| n == self.answer()).unwrap_or(false);
        self.mood = if ok { Mood::Right } else { Mood::Wrong };
        self.since = 0.0;
    }
}

/// A digit, drawn as the sum of its first `terms` waves.
fn digit(g: &Game, d: u32, at: Cx, size: f64, terms: usize) -> Shape {
    let c = g.waves[d as usize].clone();
    Shape::param(move |th| series(&c, terms, th), 0.0, TAU, 420).scaled(size).shift(at)
}

const GAP: f64 = 0.34;

/// How wide `n` tally sticks come out, so they can be centred under a digit.
fn tally_width(n: u32) -> f64 {
    if n == 0 {
        return 0.0;
    }
    let last = ((n - 1) / 5) as f64;
    let i = ((n - 1) % 5) as f64;
    let right = last * (GAP * 6.0) + if i < 4.0 { i * GAP } else { 3.0 * GAP + 0.16 };
    right + if n % 5 == 0 { 0.16 } else { 0.0 }
}

/// `n` tally sticks **centred on `at`**, with every fifth one struck through
/// the previous four — the number written the way a child counts it.
fn tally(n: u32, at: Cx) -> Shape {
    let h = 0.9;
    let x0 = at.re - tally_width(n) / 2.0;
    let mut parts = Vec::new();
    for k in 0..n {
        let (grp, i) = ((k / 5) as f64, (k % 5) as f64);
        let x = x0 + grp * (GAP * 6.0);
        if i < 4.0 {
            let x = x + i * GAP;
            parts.push(Shape::path(vec![Cx::new(x, at.im - h / 2.0), Cx::new(x, at.im + h / 2.0)]));
        } else {
            parts.push(Shape::path(vec![
                Cx::new(x - 0.16, at.im - h / 2.0 - 0.06),
                Cx::new(x + 3.0 * GAP + 0.16, at.im + h / 2.0 + 0.06),
            ]));
        }
    }
    Shape::group(parts)
}

/// A `+`, a `=`, and a `?`. Plain strokes — these are punctuation, not
/// mathematics, so they get no Fourier treatment.
fn plus(at: Cx, s: f64) -> Shape {
    Shape::group(vec![
        Shape::path(vec![at + Cx::new(-s, 0.0), at + Cx::new(s, 0.0)]),
        Shape::path(vec![at + Cx::new(0.0, -s), at + Cx::new(0.0, s)]),
    ])
}

fn equals(at: Cx, s: f64) -> Shape {
    Shape::group(vec![
        Shape::path(vec![at + Cx::new(-s, 0.34), at + Cx::new(s, 0.34)]),
        Shape::path(vec![at + Cx::new(-s, -0.34), at + Cx::new(s, -0.34)]),
    ])
}

/// A face that means well.
fn smiley(at: Cx, r: f64, t: f64) -> Shape {
    let bounce = Cx::new(0.0, 0.10 * (t * 6.0).sin());
    Shape::group(vec![
        Shape::circle(Cx::ZERO, r),
        Shape::circle(Cx::new(-r * 0.36, r * 0.28), r * 0.09),
        Shape::circle(Cx::new(r * 0.36, r * 0.28), r * 0.09),
        Shape::param(move |a| Cx::polar(r * 0.58, a), 3.6, 5.8, 40),
    ])
    .shift(at + bounce)
}

/// A ghost. Half a circle on top, three scallops underneath, and a wobble.
///
/// The scallops are `sin(3πu)` along the bottom edge, which is the only reason
/// a ghost belongs in a file about sine waves at all.
fn ghost(at: Cx, r: f64, t: f64) -> Shape {
    let foot = -1.05;
    let mut body = arc(Cx::ZERO, r, r * 1.05, 0.0, std::f64::consts::PI, 40); // dome, right to left
    body.push(Cx::new(-r, foot));
    for k in 0..=48 {
        let u = k as f64 / 48.0;
        // `.abs()` matters: plain sin flips sign each lobe, which would make
        // every other scallop bulge *upward* into the ghost.
        body.push(Cx::new(-r + 2.0 * r * u, foot - 0.24 * (std::f64::consts::PI * 3.0 * u).sin().abs()));
    }
    body.push(Cx::new(r, 0.0));

    let sway = Cx::new(0.22 * (t * 2.2).sin(), 0.12 * (t * 3.1).cos());
    Shape::group(vec![
        Shape::path(body),
        Shape::circle(Cx::new(-r * 0.40, r * 0.30), r * 0.15),
        Shape::circle(Cx::new(r * 0.40, r * 0.30), r * 0.15),
        Shape::circle(Cx::new(0.0, -r * 0.34), r * 0.20),
    ])
    .shift(at + sway)
}

/// Dots flung outwards. `k`-th dot at angle `2πk/n`, which is the `n`-th roots
/// of unity doing party duty.
fn confetti(at: Cx, n: usize, age: f64) -> Shape {
    let r = 0.9 + 3.2 * age;
    Shape::points((0..n).map(|k| at + Cx::polar(r * (1.0 - 0.25 * ((k * 7) % 5) as f64 / 5.0), TAU * k as f64 / n as f64 + age * 1.4)).collect::<Vec<_>>())
}

fn scene(g: &Game) -> Frame {
    let mut f = Frame::new();
    let terms = g.terms;
    let row = 3.7;
    let s = 1.55;
    let (ax, bx) = (-6.5, -1.3);

    // --- the sum ----------------------------------------------------------
    f.add(digit(g, g.a, Cx::new(ax, row), s, terms)).color(CYAN).width(3);
    f.add(plus(Cx::new(-3.9, row), 0.5)).color(GREY).width(3);
    f.add(digit(g, g.b, Cx::new(bx, row), s, terms)).color(AMBER).width(3);
    f.add(equals(Cx::new(1.4, row), 0.55)).color(GREY).width(3);

    // --- the answer box ---------------------------------------------------
    let box_at = Cx::new(5.3, row);
    f.add(Shape::rect(box_at + Cx::new(-1.9, -1.7), box_at + Cx::new(1.9, 1.7))).color(DIM).width(2);
    if g.typed.is_empty() {
        // A blinking caret, so it is obvious the box wants typing.
        if (g.t * 2.0) as i64 % 2 == 0 {
            f.add(Shape::path(vec![box_at + Cx::new(0.0, -1.1), box_at + Cx::new(0.0, 1.1)])).color(PINK).width(3);
        }
    } else {
        let ds: Vec<u32> = g.typed.chars().filter_map(|ch| ch.to_digit(10)).collect();
        let w = 1.15;
        for (k, d) in ds.iter().enumerate() {
            let x = box_at.re + (k as f64 - (ds.len() as f64 - 1.0) / 2.0) * w;
            f.add(digit(g, *d, Cx::new(x, row), 1.25, terms)).color(PINK).width(3);
        }
    }

    // --- the sticks, centred under the digit they count -------------------
    f.add(tally(g.a, Cx::new(ax, 0.9))).color(CYAN).width(3);
    f.add(tally(g.b, Cx::new(bx, 0.9))).color(AMBER).width(3);
    f.label(Cx::new(ax, -0.1), "this many", GREY, 2);
    f.label(Cx::new(bx, -0.1), "and this many", GREY, 2);

    // --- the spinning arrows ----------------------------------------------
    if g.show_arrows {
        let c = g.waves[g.a as usize].clone();
        let chain: Vec<Cx> = epicycles(&c, terms, g.t * 0.9).iter().map(|z| z.scale(s) + Cx::new(ax, row)).collect();
        for w in chain.windows(2) {
            let r = (w[1] - w[0]).abs();
            if r > 0.03 {
                f.add(Shape::circle(w[0], r)).color(0x3B4A59).width(1);
            }
        }
        f.add(Shape::path(chain.clone())).color(0xFFFFFF).width(1);
        f.add(Shape::points(chain)).color(0xFFFFFF).dot(2.0);
    }

    // --- how it went ------------------------------------------------------
    let face = Cx::new(-3.4, -4.1);
    match g.mood {
        Mood::Asking => {}
        Mood::Right => {
            f.add(confetti(face, 28, (g.since * 0.55).min(1.0))).color(GREEN).dot(4.0);
            f.add(smiley(face, 1.1, g.t)).color(GREEN).width(3);
            f.label(Cx::new(1.8, -3.9), "GOOD JOB!", GREEN, 5);
            f.label(Cx::new(1.8, -5.3), "press N for another one", GREY, 2);
        }
        Mood::Wrong => {
            f.add(ghost(face, 0.95, g.t)).color(VIOLET).width(3);
            // The o's grow as the boo goes on. Cute, and free.
            let n = 3 + ((g.since * 4.0) as usize).min(5);
            f.label(Cx::new(1.8, -3.9), format!("b{}...", "o".repeat(n)), VIOLET, 5);
            f.label(Cx::new(1.8, -5.3), "not quite! backspace and try again", GREY, 2);
        }
    }
    f
}

// ===========================================================================
//  THE WINDOW
//
//  The only part that knows there is a screen.
// ===========================================================================

const W: usize = 1200;
const H: usize = 740;

fn main() {
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(12345);

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
        waves: (0..10).map(|d| Arc::new(digit_waves(d))).collect(),
    };
    g.ask();

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

    /// The series has to reproduce the curve it came from. With every term
    /// kept, this is not an approximation — it is an identity, and it is the
    /// claim the whole game rests on.
    #[test]
    fn all_the_waves_add_back_up_to_the_digit() {
        for d in 0..10 {
            let pts = resample(&digit_outline(d), 128);
            let c = dft(&pts);
            for (k, want) in pts.iter().enumerate() {
                let got = series(&c, c.len(), TAU * k as f64 / 128.0);
                assert!((got - *want).abs() < 1e-9, "digit {d} sample {k}: {got:?} vs {want:?}");
            }
        }
    }

    /// Fewer waves means a rounder, wronger digit — but never a *worse* one
    /// than the round before. Error has to fall as terms are added.
    #[test]
    fn adding_waves_only_ever_helps() {
        let pts = resample(&digit_outline(7), 128);
        let c = dft(&pts);
        let err = |m: usize| {
            pts.iter()
                .enumerate()
                .map(|(k, w)| (series(&c, m, TAU * k as f64 / 128.0) - *w).abs())
                .fold(0.0f64, f64::max)
        };
        let (mut prev, mut checked) = (err(1), 0);
        for m in [2, 4, 8, 16, 32, 64, 128] {
            let e = err(m);
            assert!(e <= prev + 1e-12, "error grew from {prev} to {e} at {m} terms");
            prev = e;
            checked += 1;
        }
        assert_eq!(checked, 7);
        assert!(prev < 1e-9, "128 terms should be exact, got {prev}");
    }

    /// ★ One wave is a point; two are a circle. The simplest possible picture
    /// of what a Fourier series is doing.
    ///
    /// Done on an actual circle, because a circle is the one shape whose whole
    /// series is `c_0 + c_1 e^{iθ}` and nothing else.
    #[test]
    fn one_wave_is_a_point_and_two_are_a_circle() {
        let c = dft(&resample(&arc(Cx::new(2.0, 0.0), 1.0, 1.0, 0.0, TAU, 128), 128));
        for k in 0..40 {
            let th = TAU * k as f64 / 40.0;
            assert!((series(&c, 1, th) - Cx::new(2.0, 0.0)).abs() < 1e-9, "one term should sit still at the centre");
            assert!(((series(&c, 2, th) - Cx::new(2.0, 0.0)).abs() - 1.0).abs() < 1e-9, "two terms should trace radius 1");
        }
        // And there is genuinely nothing else in there.
        assert!(c[2].1.abs() < 1e-12, "a circle needs exactly two terms, found a third of size {}", c[2].1.abs());
    }

    /// An ellipse, by contrast, needs **two counter-rotating** waves — `n = +1`
    /// and `n = −1`. One circle spinning forwards and one backwards add to an
    /// ellipse, which is why digit 0 is not a one-wave shape.
    #[test]
    fn an_ellipse_is_two_circles_spinning_opposite_ways() {
        let (rx, ry) = (0.40, 0.88);
        let c = dft(&arc(Cx::ZERO, rx, ry, 0.0, TAU, 256)[..256].to_vec());
        let pick = |n: i32| c.iter().find(|(f, _)| *f == n).unwrap().1;
        assert!((pick(1) - Cx::new((rx + ry) / 2.0, 0.0)).abs() < 1e-9);
        assert!((pick(-1) - Cx::new((rx - ry) / 2.0, 0.0)).abs() < 1e-9);
        assert!(c[2].1.abs() < 1e-9, "and nothing else");
    }

    /// Every digit closes up. An open path would make the series ring at the
    /// seam, and the digit would grow a whisker.
    #[test]
    fn every_digit_is_a_closed_loop() {
        for d in 0..10 {
            let o = digit_outline(d);
            assert!((o[0] - *o.last().unwrap()).abs() < 0.06, "digit {d} does not close");
        }
    }

    /// Resampling puts the points at even spacing, so the pen moves at a
    /// steady pace and the early terms describe shape rather than pacing.
    ///
    /// Measured on the ellipse, where the fault is obvious and has nothing to
    /// do with corners: stepping an ellipse at even *angles* covers 0.88 of a
    /// unit near the poles and 0.40 near the equator, so the raw path is more
    /// than twice as fast in one place as another.
    #[test]
    fn resampling_evens_out_the_pace() {
        let spread = |p: &[Cx]| {
            let g: Vec<f64> = p.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
            let lo = g.iter().cloned().fold(f64::MAX, f64::min);
            let hi = g.iter().cloned().fold(0.0, f64::max);
            hi / lo
        };
        let raw = arc(Cx::ZERO, 0.40, 0.88, 0.0, TAU, 240);
        assert!(spread(&raw) > 2.0, "even angles really are uneven pace: ratio {}", spread(&raw));
        assert!(spread(&resample(&raw, 240)) < 1.01, "after resampling: ratio {}", spread(&resample(&raw, 240)));
    }

    /// Why the test above avoids digit 4: a walked-out-and-back stroke turns
    /// around at the far end, and two samples straddling that reversal sit
    /// almost on top of each other however evenly they are spaced *along* the
    /// curve. Chord distance is the wrong ruler at a reversal, not evidence of
    /// bad resampling.
    #[test]
    fn a_reversal_squashes_chords_without_squashing_arclength() {
        let out = vec![Cx::ZERO, Cx::new(1.0, 0.0)];
        let there_and_back = {
            let mut v = out.clone();
            v.extend(out.into_iter().rev().skip(1));
            v
        };
        // 201, not 200: the turn is at arclength 1.0, so an even count would
        // land a sample exactly on it and nothing would straddle.
        let p = resample(&there_and_back, 201);
        let gaps: Vec<f64> = p.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
        let tiny = gaps.iter().filter(|g| **g < 0.005).count();
        assert_eq!(tiny, 1, "exactly one chord — the one over the turn — should collapse");
    }

    /// The epicycle chain ends exactly where the series says it should — the
    /// arrows and the curve are the same computation drawn two ways.
    #[test]
    fn the_arrow_chain_ends_on_the_curve() {
        let c = dft(&resample(&digit_outline(3), 128));
        for k in 0..20 {
            let th = TAU * k as f64 / 20.0;
            assert!((*epicycles(&c, 12, th).last().unwrap() - series(&c, 12, th)).abs() < 1e-12);
        }
    }

    /// Sorting by size is what makes truncation sensible: the first term kept
    /// must be the biggest one.
    #[test]
    fn the_loudest_waves_come_first() {
        let c = dft(&resample(&digit_outline(8), 128));
        for w in c.windows(2) {
            assert!(w[0].1.abs() >= w[1].1.abs() - 1e-15);
        }
    }

    #[test]
    fn tally_marks_count_what_they_should() {
        // Five sticks is four uprights and one stroke through them.
        assert_eq!(tally(5, Cx::ZERO).polylines(Cx::new(-9.0, -9.0), Cx::new(9.0, 9.0), 400).len(), 5);
        assert_eq!(tally(3, Cx::ZERO).polylines(Cx::new(-9.0, -9.0), Cx::new(9.0, 9.0), 400).len(), 3);
    }

    #[test]
    fn checking_an_answer_says_yes_or_no() {
        let mut g = Game {
            a: 3,
            b: 4,
            typed: "7".into(),
            mood: Mood::Asking,
            since: 0.0,
            terms: 20,
            show_arrows: false,
            t: 0.0,
            rng: Rng(1),
            waves: (0..10).map(|d| Arc::new(digit_waves(d))).collect(),
        };
        g.check();
        assert!(g.mood == Mood::Right);
        g.typed = "8".into();
        g.check();
        assert!(g.mood == Mood::Wrong);
    }

    /// Sums stay inside what a small child can count on sticks.
    #[test]
    fn the_sums_stay_small() {
        let mut g = Game {
            a: 1,
            b: 1,
            typed: String::new(),
            mood: Mood::Asking,
            since: 0.0,
            terms: 20,
            show_arrows: false,
            t: 0.0,
            rng: Rng(99),
            waves: Vec::new(),
        };
        for _ in 0..500 {
            g.ask();
            assert!((1..=5).contains(&g.a) && (1..=5).contains(&g.b));
            assert!(g.answer() <= 10);
        }
    }
}

