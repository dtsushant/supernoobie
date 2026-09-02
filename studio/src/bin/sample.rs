//! # sample — write the example drawings
//!
//! ```text
//!     cargo run -p studio --bin sample
//! ```
//!
//! Writes `samples/*.easel`, then:
//!
//! ```text
//!     cargo run -p studio --release --bin draw -- samples/dials.easel
//! ```
//!
//! ## Why they are generated rather than typed out
//!
//! A hand-written sample is a copy of the file format, and the day the format
//! gains a field the sample quietly stops exercising it — or worse, stops
//! loading, and the first thing anybody tries is broken. These are built
//! through the same [`Board`] the studio uses, so they cannot describe a
//! drawing the program could not have made.
//!
//! It also means this file doubles as the shortest description of the API
//! there is: everything the studio can do, done in code.

use easel::{Action, Board, Ease};
use plotkit::Cx;
use shapes::{Nib, Pose};
use std::f64::consts::TAU;

fn main() {
    let dir = std::path::Path::new("samples");
    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!("could not make {}: {e}", dir.display());
        return;
    }

    for (name, build) in [
        ("dials", dials as fn() -> Board),
        ("bouncing", bouncing as fn() -> Board),
        ("walker", walker as fn() -> Board),
    ] {
        let path = dir.join(format!("{name}.easel"));
        let board = build();
        match board.save(path.to_str().expect("a path")) {
            Ok(()) => println!(
                "{:<28} {} marks, {} rows",
                path.display(),
                board.sheet.len(),
                board.sheet.script.len()
            ),
            Err(e) => eprintln!("could not write {}: {e}", path.display()),
        }
    }

    println!();
    println!("  cargo run -p studio --release --bin draw -- samples/dials.easel");
}

/// Drag the pen along a path, as a hand would, so the mark is a real one —
/// tapered, quilled, with the spring on it.
fn stroke(b: &mut Board, path: &[Cx]) {
    for z in path {
        b.pointer(*z, true);
    }
    b.pointer(*path.last().expect("a path"), false);
}

fn ring(r: f64, at: Cx) -> Vec<Cx> {
    (0..=72).map(|k| at + Cx::polar(r, k as f64 / 72.0 * TAU)).collect()
}

fn line(from: Cx, to: Cx) -> Vec<Cx> {
    (0..=24).map(|k| from + (to - from).scale(k as f64 / 24.0)).collect()
}

/// **The written half.** Move a slider and everything that mentions it moves.
fn dials() -> Board {
    let mut b = Board::new();
    b.sheet.script.add("# move r and n -- everything below follows");
    b.sheet.script.add("r = 3");
    b.sheet.script.add("n = 6");
    b.sheet.script.add("circle(0, r)");
    b.sheet.script.add("ngon(0, r, n)");
    b.sheet.script.add("");
    b.sheet.script.add("# a rose. time is the clock, so this turns when you play");
    b.sheet.script.add("color(0xE585AC)");
    b.sheet.script.add("param(r * cos(n*t) * exp(i*(t + 0.3*time)), 0, tau)");
    b.sheet.script.add("");
    b.sheet.script.add("# x^2 + y^2 = r^2, marched over a grid. switch it on with the tick");
    let off = easel::Row::new("implicit(x*x + y*y, r*r)").off();
    b.sheet.script.rows.push(off);
    b
}

/// **Keyframes.** A ball dropped, with the timing said rather than computed —
/// which is what an animator does.
fn bouncing() -> Board {
    let mut b = Board::new();
    b.nib = Nib::Round(0.12);
    b.colour = 0xE0A44A;
    b.taper = 0.0;
    stroke(&mut b, &ring(0.6, Cx::new(-4.0, 3.0)));

    let ball = &mut b.sheet.marks[0];
    ball.track.looping = true;

    // Down, land, up again -- and it swells slightly on impact rather than
    // squashing.
    //
    // It CANNOT squash, and that is worth knowing. A pose is `z -> az + b`
    // with `a` a complex number, which is a **similarity**: it can turn and it
    // can scale, but only by the same amount in every direction. Squash and
    // stretch -- wide-and-short on impact, tall-and-thin in flight, the first
    // thing any animator reaches for -- is not a similarity, and no complex
    // `a` expresses it. It needs a full 2x2 matrix, which is a real extension
    // and not a tweak.
    let impact = Pose::new(Cx::new(1.2, 0.0), Cx::new(2.0, -3.0));
    ball.track.set(0.0, Pose::STILL, Ease::Smooth);
    // Linear on the way down, because gravity does not ease off.
    ball.track.set(0.55, Pose::new(Cx::ONE, Cx::new(1.0, -1.6)), Ease::Linear);
    ball.track.set(0.7, impact, Ease::Smooth);
    ball.track.set(0.85, Pose::new(Cx::ONE, Cx::new(3.0, -1.4)), Ease::Smooth);
    ball.track.set(1.4, Pose::new(Cx::ONE, Cx::new(4.5, 0.6)), Ease::Smooth);
    ball.track.set(2.2, Pose::STILL, Ease::Smooth);

    // The ground, so there is something to land on.
    b.colour = 0x46525E;
    b.nib = Nib::Round(0.06);
    stroke(&mut b, &line(Cx::new(-6.0, -0.6), Cx::new(6.0, -0.6)));

    b.sheet.script.add("# press PLAY. the ball is keyed; this line is written");
    b.sheet.script.add("color(0x2A3542)");
    b.sheet.script.add("plot(0.15*sin(3*x + time))");
    b
}

/// **Verbs and groups.** Six strokes bound into a figure, told to walk and
/// then jump — one press each, because it is one figure.
fn walker() -> Board {
    let mut b = Board::new();
    b.nib = Nib::Quill { slow: 0.14, fast: 0.02, pace: 0.16 };
    b.taper = 0.12;
    b.colour = 0x6FCF97;

    let head = Cx::new(-4.0, 1.4);
    stroke(&mut b, &ring(0.42, head));
    stroke(&mut b, &line(Cx::new(-4.0, 0.95), Cx::new(-4.0, -0.2)));
    stroke(&mut b, &line(Cx::new(-4.0, 0.7), Cx::new(-4.6, 0.2)));
    stroke(&mut b, &line(Cx::new(-4.0, 0.7), Cx::new(-3.4, 0.2)));
    stroke(&mut b, &line(Cx::new(-4.0, -0.2), Cx::new(-4.35, -1.1)));
    stroke(&mut b, &line(Cx::new(-4.0, -0.2), Cx::new(-3.65, -1.1)));

    b.selected = (0..b.sheet.len()).collect();
    b.group();
    b.give(Action::Walk(Cx::new(1.4, 0.0)), Some(2.0));
    b.give(Action::Jump { height: 1.0, rate: 1.5 }, Some(2.0));
    b.give(Action::Walk(Cx::new(-1.4, 0.0)), Some(2.0));
    b.selected.clear();

    b.sheet.script.add("# tap any part of the figure and the whole of it is chosen");
    b.sheet.script.add("color(0x46525E)");
    b.sheet.script.add("line(-7 - 2i, 7 - 2i)");
    b
}
