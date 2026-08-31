//! # waves — adding sine waves, however many of them
//!
//! Run:  `cargo run -p studio --release --bin waves`
//!
//! One strip per wave, and one more for their sum. Beside each strip, the
//! rotating arrow the wave is the shadow of. The connector between an arrow
//! and its wave is always exactly level, because `a sin(kx+φ)` **is**
//! `Im(a e^{i(kx+φ)})` — the height and the imaginary part are the same number.
//!
//! The arrows for the sum are laid tip to tail. That picture is not an
//! illustration of the addition; it *is* the addition, because taking the
//! shadow is linear: `Im(A) + Im(B) = Im(A+B)`.
//!
//! ## What to look for
//!
//! Press `1`–`7`. When every frequency agrees the sum is still one sine wave
//! and the readout names it — `|A| sin(kx + arg A)`, where `A` is the sum of
//! the phasors and nothing more. When the frequencies differ the readout
//! refuses, because no single sine is the answer, and that refusal is where
//! Fourier series start. Preset `6` is the first three terms of a square wave;
//! keep adding odd harmonics with `=` and it squares off.
//!
//! ## Controls
//!
//! ```text
//!   1..7   presets              Tab  choose which wave you are editing
//!   ↑ ↓    its amplitude        =    add a wave      Space  pause
//!   ← →    its frequency        -    drop one        G      graph paper
//!   A D    its phase            R    reset           Esc    quit
//! ```
//!
//! The mathematics is in [`shapes::wave`]; this file only decides where on the
//! page things go. Nothing here counts the waves — [`Layout`] adds up how much
//! room they need — which is why a third wave is a key press rather than an
//! edit in seven places.

use minifb::{Key, KeyRepeat, Window, WindowOptions};
use plotkit::{plot, raster::Canvas, Cx, Frame, Shape, View};
use shapes::wave::{chain, combine, total, Wave};
use std::f64::consts::PI;

// ===========================================================================
//  THE SCENE
//
//  Where things go on the page. Every number here is in the units the
//  mathematics uses; not one of them is a pixel.
// ===========================================================================

const W: usize = 1100;
const H: usize = 700;
const MAX_WAVES: usize = 5;

const IN: [u32; MAX_WAVES] = [0x4FBCD4, 0xE0A44A, 0xE585AC, 0x9B7BD4, 0xE0704A];
const SUM: u32 = 0x6FCF97;
const DIM: u32 = 0x33414F;
const INK: u32 = 0x9AA7B4;

/// Where every strip sits, worked out from the waves themselves.
struct Layout {
    /// One baseline per wave, top to bottom.
    bases: Vec<f64>,
    /// The baseline of the sum, which needs room for every amplitude at once.
    sum: f64,
    /// Centre of the column of arrows, and the ends of the strips.
    col: f64,
    x0: f64,
    x1: f64,
    view: View,
}

impl Layout {
    fn of(ws: &[Wave]) -> Layout {
        const PAD: f64 = 0.55;
        let tall: Vec<f64> = ws.iter().map(|w| 2.0 * w.a.max(0.25)).collect();
        let reach: f64 = ws.iter().map(|w| w.a).sum::<f64>().max(0.25);
        let total_h: f64 = tall.iter().sum::<f64>() + 2.0 * reach + PAD * (ws.len() as f64 + 1.0);

        // Leave room under the picture for the readout, then fit what is left.
        // Adding a wave zooms out rather than overflowing, which is honest:
        // the picture gets smaller because there is more of it.
        let hud = 46.0 + 20.0 * (ws.len() as f64 + 1.0);
        let usable = H as f64 - hud;
        let scale = usable / total_h;

        // Stack downwards from the top of the usable band.
        let mut y = total_h / 2.0;
        let mut bases = Vec::new();
        for t in &tall {
            y -= PAD + t / 2.0;
            bases.push(y);
            y -= t / 2.0;
        }
        y -= PAD + reach;

        // The arrow column is as wide as the sum arrows can reach; the strips
        // take whatever is left.
        let half = W as f64 / (2.0 * scale);
        let col = -half + reach + 0.4;
        Layout {
            bases,
            sum: y,
            col,
            x0: col + reach + 0.6,
            x1: half - 0.3,
            view: View::centred(W, H, scale).with_origin(W as f64 / 2.0, usable / 2.0),
        }
    }

    fn x_at(&self, u: f64) -> f64 {
        self.x0 + (self.x1 - self.x0) * u
    }
}

struct State {
    ws: Vec<Wave>,
    /// Which wave the arrow keys are editing.
    sel: usize,
    preset: usize,
    grid: bool,
    running: bool,
    /// How far along the strips the marker is, as a fraction.
    u: f64,
}

/// One strip: baseline, the circle the arrow sweeps, the wave, the arrow, and
/// the level connector between the arrow tip and the point on the wave.
fn strip(f: &mut Frame, l: &Layout, w: Wave, base: f64, x: f64, col: u32, thick: bool) {
    let c = Cx::new(l.col, base);
    let (x0, x1) = (l.x0, l.x1);

    f.add(Shape::path(vec![Cx::new(x0, base), Cx::new(x1, base)])).color(DIM).width(1);
    f.add(Shape::circle(c, w.a.max(1e-3))).color(DIM).width(1);
    f.add(Shape::param(move |t| Cx::new(t, base + w.at(t)), x0, x1, 800)).color(col).width(if thick { 3 } else { 2 });

    let tip = c + w.arrow(x);
    f.add(Shape::path(vec![c, tip])).color(col).width(2);
    f.add(Shape::point(tip)).color(col).dot(4.0);
    f.add(Shape::point(Cx::new(x, base + w.at(x)))).color(col).dot(5.0);
    dashes(f, tip, Cx::new(x, tip.im), col);
}

fn scene(st: &State, l: &Layout) -> Frame {
    let mut f = Frame::new();
    let x = l.x_at(st.u);

    for (k, w) in st.ws.iter().enumerate() {
        strip(&mut f, l, *w, l.bases[k], x, IN[k % IN.len()], k == st.sel);
        f.label(Cx::new(l.col, l.bases[k] + w.a.max(0.25) + 0.3), format!("wave {}", k + 1), IN[k % IN.len()], 2);
    }

    // --- the sum ----------------------------------------------------------
    let c = Cx::new(l.col, l.sum);
    let (x0, x1, base) = (l.x0, l.x1, l.sum);

    f.add(Shape::path(vec![Cx::new(x0, base), Cx::new(x1, base)])).color(DIM).width(1);

    // The path the summed tip traces: a circle when the frequencies agree,
    // something more interesting when they do not.
    let trace = st.ws.clone();
    f.add(Shape::param(move |t| c + *chain(&trace, t).last().expect("a chain always ends somewhere"), x0, x1, 900))
        .color(DIM)
        .width(1);

    let curve = st.ws.clone();
    f.add(Shape::param(move |t| Cx::new(t, base + total(&curve, t)), x0, x1, 900)).color(SUM).width(2);

    // Head to tail. One segment per wave, however many there are.
    let links = chain(&st.ws, x);
    for (k, seg) in links.windows(2).enumerate() {
        f.add(Shape::path(vec![c + seg[0], c + seg[1]])).color(IN[k % IN.len()]).width(2);
    }
    let end = c + *links.last().expect("a chain always ends somewhere");
    f.add(Shape::path(vec![c, end])).color(SUM).width(3);
    f.add(Shape::point(end)).color(SUM).dot(4.0);
    f.add(Shape::point(Cx::new(x, base + total(&st.ws, x)))).color(SUM).dot(5.0);
    dashes(&mut f, end, Cx::new(x, end.im), SUM);

    let reach: f64 = st.ws.iter().map(|w| w.a).sum::<f64>().max(0.25);
    f.label(Cx::new(l.col, base + reach + 0.3), "sum", SUM, 2);
    f
}

/// A dotted line — a solid one would read as part of the mathematics.
fn dashes(f: &mut Frame, a: Cx, b: Cx, col: u32) {
    let n = 26;
    for k in (0..n).step_by(2) {
        let (u, v) = (k as f64 / n as f64, (k + 1) as f64 / n as f64);
        f.add(Shape::path(vec![a + (b - a).scale(u), a + (b - a).scale(v)])).color(col).width(1);
    }
}

// ===========================================================================
//  THE PRESETS
//
//  Each one chosen to make exactly one thing obvious.
// ===========================================================================

// Same arrow twice: they point together, so the amplitudes add.
const TOGETHER: [Wave; 2] = [Wave::new(1.0, 1.0, 0.0), Wave::new(1.0, 1.0, 0.0)];
// e^{iπ} = −1. Back to back. The sum is a flat line.
const AGAINST: [Wave; 2] = [Wave::new(1.0, 1.0, 0.0), Wave::new(1.0, 1.0, PI)];
// sin + cos. A = 1 + i, so |A| = √2 and arg A = π/4.
const QUARTER: [Wave; 2] = [Wave::new(1.0, 1.0, 0.0), Wave::new(1.0, 1.0, PI / 2.0)];
// Different k. No common e^{ikx} to pull out, so not a sine any more.
const OCTAVE: [Wave; 2] = [Wave::new(1.0, 1.0, 0.0), Wave::new(0.6, 2.0, 0.0)];
// Nearly equal: the two drift in and out of step. Beats.
const BEATS: [Wave; 2] = [Wave::new(1.0, 1.0, 0.0), Wave::new(1.0, 1.15, 0.0)];
// Odd harmonics at 1/n. Keep going with 7, 9, 11 and it squares off.
const SQUARE: [Wave; 3] = [Wave::new(1.0, 1.0, 0.0), Wave::new(1.0 / 3.0, 3.0, 0.0), Wave::new(0.2, 5.0, 0.0)];
// Three arrows a third of a turn apart — the cube roots of unity, which sum to
// zero. Nothing about the two-wave cancellation was special.
const THIRDS: [Wave; 3] = [
    Wave::new(1.0, 1.0, 0.0),
    Wave::new(1.0, 1.0, 2.0943951023931953),
    Wave::new(1.0, 1.0, 4.1887902047863905),
];

const PRESETS: [(&str, &[Wave]); 7] = [
    ("same frequency, in phase -> the amplitudes just add", &TOGETHER),
    ("same frequency, half a turn apart -> total cancellation", &AGAINST),
    ("sin + cos = sqrt(2) sin(x + pi/4) -- read it off the arrows", &QUARTER),
    ("an octave up -> the sum is a NEW shape, not a sine", &OCTAVE),
    ("nearly equal -> beats, as they drift in and out of step", &BEATS),
    ("sin(x) + sin(3x)/3 + sin(5x)/5 -> a square wave, three terms in", &SQUARE),
    ("three arrows 120 degrees apart -> the cube roots of unity, summing to zero", &THIRDS),
];

// ===========================================================================
//  THE WINDOW
// ===========================================================================

fn main() {
    let mut st = State { ws: PRESETS[0].1.to_vec(), sel: 0, preset: 0, grid: false, running: true, u: 0.0 };

    let mut c = Canvas::new(W, H);
    let mut win = Window::new("WAVES  -  adding sines", W, H, WindowOptions::default()).expect("no window");
    win.set_target_fps(60);

    while win.is_open() && !win.is_key_down(Key::Escape) {
        for k in win.get_keys_pressed(KeyRepeat::No) {
            match k {
                Key::Space => st.running = !st.running,
                Key::G => st.grid = !st.grid,
                Key::R => {
                    let p = st.preset;
                    load(&mut st, p);
                }
                Key::Tab => st.sel = (st.sel + 1) % st.ws.len(),
                Key::Equal => {
                    if st.ws.len() < MAX_WAVES {
                        // The next term of a harmonic series, ready to edit.
                        let n = st.ws.len() as f64 + 1.0;
                        st.ws.push(Wave::new(1.0 / n, n, 0.0));
                        st.sel = st.ws.len() - 1;
                    }
                }
                Key::Minus => {
                    if st.ws.len() > 1 {
                        st.ws.pop();
                        st.sel = st.sel.min(st.ws.len() - 1);
                    }
                }
                Key::Key1 | Key::Key2 | Key::Key3 | Key::Key4 | Key::Key5 | Key::Key6 | Key::Key7 => {
                    let i = [Key::Key1, Key::Key2, Key::Key3, Key::Key4, Key::Key5, Key::Key6, Key::Key7]
                        .iter()
                        .position(|&p| p == k)
                        .expect("matched just above");
                    load(&mut st, i);
                }
                _ => {}
            }
        }

        // Every adjustment lands on the selected wave, so the controls do not
        // grow when the number of waves does.
        let fine = if win.is_key_down(Key::LeftShift) { 0.2 } else { 1.0 };
        let w = &mut st.ws[st.sel];
        if win.is_key_down(Key::Up) {
            w.a = (w.a + 0.012 * fine).min(1.2);
        }
        if win.is_key_down(Key::Down) {
            w.a = (w.a - 0.012 * fine).max(0.0);
        }
        if win.is_key_down(Key::Right) {
            w.k = (w.k + 0.01 * fine).min(9.0);
        }
        if win.is_key_down(Key::Left) {
            w.k = (w.k - 0.01 * fine).max(0.0);
        }
        if win.is_key_down(Key::D) {
            w.phi += 0.02 * fine;
        }
        if win.is_key_down(Key::A) {
            w.phi -= 0.02 * fine;
        }

        if st.running {
            st.u = (st.u + 0.0035) % 1.0;
        }

        let l = Layout::of(&st.ws);
        c.clear(0x0B1017);
        if st.grid {
            plot::grid(&mut c, &l.view, &plot::GridStyle { labels: false, ..Default::default() });
        }
        scene(&st, &l).draw(&mut c, &l.view);
        hud(&mut c, &st);
        win.update_with_buffer(&c.buf, W, H).expect("present failed");
    }
}

fn load(st: &mut State, i: usize) {
    st.ws = PRESETS[i].1.to_vec();
    st.preset = i;
    st.sel = 0;
    st.u = 0.0;
    st.running = true;
}

fn hud(c: &mut Canvas, st: &State) {
    let say =
        |w: &Wave| format!("{:.2} sin({:.2}x {} {:.2})", w.a, w.k, if w.phi < 0.0 { "-" } else { "+" }, w.phi.abs());
    // The title lives in the readout block, not at the top of the screen —
    // the top belongs to the first wave's label, and the two collided there.
    let top = H as i32 - 46 - 20 * (st.ws.len() as i32 + 1);
    for (k, w) in st.ws.iter().enumerate() {
        let mark = if k == st.sel { ">" } else { " " };
        c.text(14, top + 20 * k as i32, &format!("{mark} wave {}  =  {}", k + 1, say(w)), IN[k % IN.len()], 2);
    }

    // The readout that proves the theorem, or admits it does not apply.
    let line = match combine(&st.ws) {
        Some(w) => format!("  sum     =  {}      <- still one sine wave", say(&w)),
        None => "  sum     =  not a sine wave. the frequencies differ, so no common e^(ikx) factors out.".to_string(),
    };
    c.text(14, top + 20 * st.ws.len() as i32, &line, SUM, 2);
    c.text(14, top + 20 * (st.ws.len() as i32 + 1), PRESETS[st.preset].0, INK, 2);
    c.text(
        14,
        H as i32 - 20,
        "1-7 presets   TAB pick a wave   arrows amp/freq   A D phase   = add   - drop   G grid   space pause   R reset",
        0x5A6774,
        1,
    );
}

