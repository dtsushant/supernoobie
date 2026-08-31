//! # waves — two sine waves, and what happens when you add them
//!
//! Run:  `cargo run -p studio --release --bin waves`
//!
//! Three strips on the right: wave one, wave two, and their sum. A column of
//! rotating arrows on the left. The connector between an arrow and its wave is
//! always exactly horizontal, and that is not a coincidence — it is the whole
//! point:
//!
//! ```text
//!     a sin(kx + φ)  =  Im( a e^{i(kx+φ)} )
//! ```
//!
//! The wave *is* the shadow of the arrow. Once you believe that, addition
//! becomes easy, because arrows add head-to-tail and shadows add with them:
//!
//! ```text
//!     Im(A) + Im(B) = Im(A + B)
//! ```
//!
//! ## The theorem this file exists to show
//!
//! When the two frequencies **agree**, pull the common rotation `e^{ikx}` out:
//!
//! ```text
//!     a₁ sin(kx+φ₁) + a₂ sin(kx+φ₂)
//!       = Im( a₁e^{iφ₁} e^{ikx} ) + Im( a₂e^{iφ₂} e^{ikx} )
//!       = Im( (a₁e^{iφ₁} + a₂e^{iφ₂}) · e^{ikx} )
//!       = Im( A e^{ikx} )                            where A = a₁e^{iφ₁} + a₂e^{iφ₂}
//!       = |A| sin(kx + arg A)
//! ```
//!
//! **A sine plus a sine of the same frequency is another sine of that same
//! frequency.** Its amplitude is `|A|` and its phase is `arg A`, and `A` is one
//! complex addition. Every "sum-to-product" identity in a trigonometry
//! textbook is this one line, written out in real numbers so it looks hard.
//!
//! Press `2` and watch it cancel to nothing — `e^{iπ} = −1`, so the arrows are
//! back to back and `A = 0`. Press `3`: `sin + cos = √2 sin(x + π/4)`, and the
//! √2 is visibly the diagonal of a unit square.
//!
//! When the frequencies **differ**, the step above is illegal — there is no
//! common `e^{ikx}` to pull out — and the sum is genuinely a new shape. Press
//! `4`, `5`, `6`. That failure is not a defect; it is where Fourier series
//! live, and it is what draws the digits in `studio/src/main.rs`.
//!
//! ## Controls
//!
//! ```text
//!   1..6   presets      W/S  amplitude 1      Space  pause
//!   ← →    frequency 2  A/D  phase 2          R      reset
//!   ↑ ↓    amplitude 2  G    grid on/off      Esc    quit
//! ```

use minifb::{Key, KeyRepeat, Window, WindowOptions};
use plotkit::{plot, raster::Canvas, Cx, Frame, Shape, View};
use std::f64::consts::PI;

// ===========================================================================
//  THE MATHEMATICS
//
//  No canvas, no view, no colours below this line. Everything here could be
//  worked out on paper, and the tests at the bottom check that it was.
// ===========================================================================

/// A sinusoid `a·sin(kx + φ)`.
///
/// Three numbers: how tall, how fast, and where it starts.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Wave {
    a: f64,
    k: f64,
    phi: f64,
}

impl Wave {
    const fn new(a: f64, k: f64, phi: f64) -> Wave {
        Wave { a, k, phi }
    }

    /// The height of the wave at `x`.
    fn at(self, x: f64) -> f64 {
        self.a * (self.k * x + self.phi).sin()
    }

    /// The rotating arrow the wave is the shadow of.
    ///
    /// Length `a`, turning at `k` radians per unit of `x`, starting at angle
    /// `φ`. By construction `arrow(x).im == at(x)`, which is the identity the
    /// whole picture rests on.
    fn arrow(self, x: f64) -> Cx {
        Cx::polar(self.a, self.k * x + self.phi)
    }

    /// The arrow at `x = 0` — amplitude and phase in one complex number.
    ///
    /// Engineers call this the *phasor*. It is the wave with the boring part
    /// (the spinning) factored out.
    fn phasor(self) -> Cx {
        Cx::polar(self.a, self.phi)
    }
}

/// Add two waves **of the same frequency** and get the single wave that
/// results — by adding their phasors.
///
/// Returns `None` when the frequencies differ, because then no single sine
/// wave is the answer and pretending otherwise would be a lie. That refusal is
/// half the lesson.
fn combine(u: Wave, w: Wave) -> Option<Wave> {
    if (u.k - w.k).abs() > 1e-9 {
        return None;
    }
    let a = u.phasor() + w.phasor(); // <- the entire computation
    Some(Wave::new(a.abs(), u.k, a.arg()))
}

/// Named starting points, each chosen to make one thing obvious.
const PRESETS: [(&str, Wave, Wave); 6] = [
    // Same arrow twice: they point together, so the sum is twice as tall.
    ("same frequency, in phase -> amplitudes just add", Wave::new(1.0, 1.0, 0.0), Wave::new(1.0, 1.0, 0.0)),
    // e^{i pi} = -1. Back to back. A = 0. The sum is a flat line.
    ("same frequency, half a turn apart -> total cancellation", Wave::new(1.0, 1.0, 0.0), Wave::new(1.0, 1.0, PI)),
    // sin + cos. A = 1 + i, so |A| = sqrt(2) and arg A = pi/4.
    ("sin + cos = sqrt(2) sin(x + pi/4) -- read it off the arrows", Wave::new(1.0, 1.0, 0.0), Wave::new(1.0, 1.0, PI / 2.0)),
    // Different k. No common e^{ikx}. The sum is not a sine any more.
    ("an octave up -> the sum is a NEW shape, not a sine", Wave::new(1.0, 1.0, 0.0), Wave::new(0.6, 2.0, 0.0)),
    // Nearly equal k: the two drift in and out of step. Beats.
    ("nearly equal -> beats, as they drift in and out of step", Wave::new(1.0, 1.0, 0.0), Wave::new(1.0, 1.15, 0.0)),
    // sin(x) + sin(3x)/3. Keep going with 5, 7, 9... and you get a square wave.
    ("sin(x) + sin(3x)/3 -> the first two terms of a square wave", Wave::new(1.0, 1.0, 0.0), Wave::new(1.0 / 3.0, 3.0, 0.0)),
];

// ===========================================================================
//  THE SCENE
//
//  Where the numbers get placed on the page. Still no pixels — every position
//  here is in the same units the mathematics uses.
// ===========================================================================

const X0: f64 = -3.0; // left edge of the wave strips
const X1: f64 = 10.3; // right edge
const COL: f64 = -7.3; // centre of the arrow column
const B1: f64 = 4.8; // baseline of wave one
const B2: f64 = 1.9; // baseline of wave two
const BS: f64 = -2.6; // baseline of the sum

const C1: u32 = 0x4FBCD4;
const C2: u32 = 0xE0A44A;
const CS: u32 = 0x6FCF97;
const DIM: u32 = 0x33414F;
const INK: u32 = 0x9AA7B4;

struct State {
    w1: Wave,
    w2: Wave,
    preset: usize,
    grid: bool,
    running: bool,
    t: f64,
}

/// One strip: a baseline, a circle of radius `a`, the wave, and the arrow and
/// dot at the current `x = t`.
fn strip(f: &mut Frame, w: Wave, base: f64, t: f64, col: u32) {
    let c = Cx::new(COL, base);

    f.add(Shape::path(vec![Cx::new(X0, base), Cx::new(X1, base)])).color(DIM).width(1);
    f.add(Shape::circle(c, w.a.max(1e-3))).color(DIM).width(1);

    // y = a sin(kx + phi), sampled along the strip only.
    f.add(Shape::param(move |x| Cx::new(x, base + w.at(x)), X0, X1, 700)).color(col).width(2);

    // The arrow, and the dot on the wave it casts.
    let tip = c + w.arrow(t);
    f.add(Shape::path(vec![c, tip])).color(col).width(2);
    f.add(Shape::point(tip)).color(col).dot(4.0);
    f.add(Shape::point(Cx::new(t, base + w.at(t)))).color(col).dot(5.0);
}

fn scene(st: &State) -> Frame {
    let (w1, w2, t) = (st.w1, st.w2, st.t);
    let mut f = Frame::new();

    strip(&mut f, w1, B1, t, C1);
    strip(&mut f, w2, B2, t, C2);

    // --- the sum ----------------------------------------------------------
    let c = Cx::new(COL, BS);
    f.add(Shape::path(vec![Cx::new(X0, BS), Cx::new(X1, BS)])).color(DIM).width(1);

    // The path the summed tip traces. A circle when the frequencies agree,
    // something more interesting when they do not.
    f.add(Shape::param(move |x| c + w1.arrow(x) + w2.arrow(x), X0, X1, 900)).color(DIM).width(1);

    f.add(Shape::param(move |x| Cx::new(x, BS + w1.at(x) + w2.at(x)), X0, X1, 900)).color(CS).width(2);

    // Head to tail: this is the addition, drawn.
    let a1 = c + w1.arrow(t);
    let a2 = a1 + w2.arrow(t);
    f.add(Shape::path(vec![c, a1])).color(C1).width(2);
    f.add(Shape::path(vec![a1, a2])).color(C2).width(2);
    f.add(Shape::path(vec![c, a2])).color(CS).width(3);
    f.add(Shape::point(a2)).color(CS).dot(4.0);
    f.add(Shape::point(Cx::new(t, BS + w1.at(t) + w2.at(t)))).color(CS).dot(5.0);

    // The horizontal connectors. Each one is level because the wave's height
    // and the arrow tip's imaginary part are the same number.
    for (base, tip, col) in [(B1, Cx::new(COL, B1) + w1.arrow(t), C1), (B2, Cx::new(COL, B2) + w2.arrow(t), C2), (BS, a2, CS)] {
        let _ = base;
        dashes(&mut f, tip, Cx::new(t, tip.im), col);
    }

    f.label(Cx::new(COL, B1 + 1.15), "wave 1", C1, 2);
    f.label(Cx::new(COL, B2 + 1.45), "wave 2", C2, 2);
    f.label(Cx::new(COL, BS + 2.75), "wave 1 + wave 2", CS, 2);
    f
}

/// A dotted line, because a solid one would read as part of the mathematics.
fn dashes(f: &mut Frame, a: Cx, b: Cx, col: u32) {
    let n = 26;
    for k in 0..n {
        if k % 2 == 1 {
            continue;
        }
        let (u, v) = (k as f64 / n as f64, (k + 1) as f64 / n as f64);
        f.add(Shape::path(vec![a + (b - a).scale(u), a + (b - a).scale(v)])).color(col).width(1);
    }
}

// ===========================================================================
//  THE WINDOW
//
//  The boring part. Twenty lines that turn `scene()` into something you can
//  look at. This is the only code in the file that knows a screen exists.
// ===========================================================================

const W: usize = 1100;
const H: usize = 700;

fn main() {
    let mut st = State { w1: PRESETS[0].1, w2: PRESETS[0].2, preset: 0, grid: false, running: true, t: X0 };

    let v = View::centred(W, H, 52.0);
    let mut c = Canvas::new(W, H);
    let mut win = Window::new("WAVES  -  adding sines", W, H, WindowOptions::default()).expect("no window");
    win.set_target_fps(60);

    while win.is_open() && !win.is_key_down(Key::Escape) {
        // --- input --------------------------------------------------------
        for k in win.get_keys_pressed(KeyRepeat::No) {
            match k {
                Key::Space => st.running = !st.running,
                Key::G => st.grid = !st.grid,
                Key::R => {
                    let p = PRESETS[st.preset];
                    st = State { w1: p.1, w2: p.2, preset: st.preset, grid: st.grid, running: true, t: X0 };
                }
                Key::Key1 | Key::Key2 | Key::Key3 | Key::Key4 | Key::Key5 | Key::Key6 => {
                    let i = match k {
                        Key::Key1 => 0,
                        Key::Key2 => 1,
                        Key::Key3 => 2,
                        Key::Key4 => 3,
                        Key::Key5 => 4,
                        _ => 5,
                    };
                    let p = PRESETS[i];
                    st = State { w1: p.1, w2: p.2, preset: i, grid: st.grid, running: true, t: X0 };
                }
                _ => {}
            }
        }
        let step = |d: f64| if win.is_key_down(Key::LeftShift) { d * 0.2 } else { d };
        if win.is_key_down(Key::Up) {
            st.w2.a = (st.w2.a + step(0.012)).min(1.2);
        }
        if win.is_key_down(Key::Down) {
            st.w2.a = (st.w2.a - step(0.012)).max(0.0);
        }
        if win.is_key_down(Key::W) {
            st.w1.a = (st.w1.a + step(0.012)).min(1.2);
        }
        if win.is_key_down(Key::S) {
            st.w1.a = (st.w1.a - step(0.012)).max(0.0);
        }
        if win.is_key_down(Key::Right) {
            st.w2.k = (st.w2.k + step(0.01)).min(8.0);
        }
        if win.is_key_down(Key::Left) {
            st.w2.k = (st.w2.k - step(0.01)).max(0.0);
        }
        if win.is_key_down(Key::D) {
            st.w2.phi += step(0.02);
        }
        if win.is_key_down(Key::A) {
            st.w2.phi -= step(0.02);
        }
        if st.running {
            st.t += 0.035;
            if st.t > X1 {
                st.t = X0;
            }
        }

        // --- draw ---------------------------------------------------------
        c.clear(0x0B1017);
        if st.grid {
            plot::grid(&mut c, &v, &plot::GridStyle { labels: false, ..Default::default() });
        }
        scene(&st).draw(&mut c, &v);
        hud(&mut c, &st);
        win.update_with_buffer(&c.buf, W, H).expect("present failed");
    }
}

fn hud(c: &mut Canvas, st: &State) {
    let f = |w: Wave| format!("{:.2} sin({:.2}x {} {:.2})", w.a, w.k, if w.phi < 0.0 { "-" } else { "+" }, w.phi.abs());
    c.text(14, 12, PRESETS[st.preset].0, INK, 2);

    c.text(14, 618, &format!("wave 1  =  {}", f(st.w1)), C1, 2);
    c.text(14, 638, &format!("wave 2  =  {}", f(st.w2)), C2, 2);

    // The readout that proves the theorem, or admits it does not apply.
    let sum = match combine(st.w1, st.w2) {
        Some(w) => format!("sum     =  {}      <- still one sine wave", f(w)),
        None => "sum     =  not a sine wave. the frequencies differ, so no common e^(ikx) factors out.".to_string(),
    };
    c.text(14, 658, &sum, CS, 2);
    c.text(14, 680, "1-6 presets   arrows: amp/freq of wave 2   A D phase   W S amp of wave 1   G grid   space pause   R reset", 0x5A6774, 1);
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    /// The identity the entire picture is built on. If this ever failed, the
    /// horizontal connectors would slope and the drawing would be a lie.
    #[test]
    fn a_wave_is_the_shadow_of_its_arrow() {
        let w = Wave::new(0.7, 2.3, 1.1);
        for k in 0..40 {
            let x = -4.0 + 0.31 * k as f64;
            assert!((w.arrow(x).im - w.at(x)).abs() < 1e-12);
        }
    }

    /// Same frequency in, same frequency out — and the combined wave agrees
    /// with the pointwise sum everywhere, not just at the sample points that
    /// happen to be convenient.
    #[test]
    fn same_frequency_sines_add_to_one_sine() {
        let u = Wave::new(1.0, 1.0, 0.0);
        let w = Wave::new(0.6, 1.0, 0.9);
        let s = combine(u, w).expect("same frequency");
        for k in 0..200 {
            let x = -6.0 + 0.07 * k as f64;
            assert!((s.at(x) - (u.at(x) + w.at(x))).abs() < 1e-12, "disagreed at x = {x}");
        }
    }

    /// sin + cos = sqrt(2) sin(x + pi/4). The famous one, falling out of
    /// `1 + i` and nothing else.
    #[test]
    fn sin_plus_cos_is_root_two_at_a_slant() {
        let s = combine(Wave::new(1.0, 1.0, 0.0), Wave::new(1.0, 1.0, PI / 2.0)).unwrap();
        assert!((s.a - 2f64.sqrt()).abs() < 1e-12);
        assert!((s.phi - PI / 4.0).abs() < 1e-12);
    }

    /// Half a turn apart is `e^{i pi} = -1`, so the phasors annihilate.
    #[test]
    fn opposite_phase_cancels_exactly() {
        let s = combine(Wave::new(1.0, 1.0, 0.0), Wave::new(1.0, 1.0, PI)).unwrap();
        assert!(s.a < 1e-15, "amplitude was {}", s.a);
    }

    /// ★ The refusal. Different frequencies have no single-sine answer, and
    /// `combine` says so rather than inventing one.
    #[test]
    fn different_frequencies_have_no_single_sine_answer() {
        assert!(combine(Wave::new(1.0, 1.0, 0.0), Wave::new(1.0, 2.0, 0.0)).is_none());
    }

    /// And here is why the refusal is honest: the sum of an octave pair is not
    /// a sine of *any* amplitude or phase, because a sine of frequency 1 takes
    /// each value twice per period and this sum does not.
    #[test]
    fn an_octave_pair_is_genuinely_a_new_shape() {
        let (u, w) = (Wave::new(1.0, 1.0, 0.0), Wave::new(0.6, 2.0, 0.0));
        let f = |x: f64| u.at(x) + w.at(x);
        // A pure sine of frequency 1 is odd-symmetric about its peak. Find the
        // peak numerically and show this sum is not symmetric about it.
        let n = 20_000;
        let mut peak = 0.0;
        let mut best = f64::NEG_INFINITY;
        for k in 0..n {
            let x = TAU * k as f64 / n as f64;
            if f(x) > best {
                best = f(x);
                peak = x;
            }
        }
        let d = 0.6;
        assert!((f(peak + d) - f(peak - d)).abs() > 1e-3, "the sum was suspiciously sine-like");
    }

    /// Adding shadows and shadowing sums are the same operation. This is what
    /// lets the arrows be drawn head to tail.
    #[test]
    fn arrows_add_head_to_tail_the_way_the_waves_do() {
        let (u, w) = (Wave::new(0.9, 1.0, 0.3), Wave::new(0.4, 2.7, -1.2));
        for k in 0..50 {
            let x = 0.13 * k as f64;
            assert!(((u.arrow(x) + w.arrow(x)).im - (u.at(x) + w.at(x))).abs() < 1e-12);
        }
    }
}

