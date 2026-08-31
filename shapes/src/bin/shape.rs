//! # shape — draw a shape in the terminal, and show how it was drawn
//!
//! ```text
//!   cargo run -p shapes -- list
//!   cargo run -p shapes -- smiley
//!   cargo run -p shapes -- seven --steps
//!   cargo run -p shapes -- ghost --big
//!   cargo run -p shapes -- hexagon --png hex.png
//! ```
//!
//! No window and no image viewer: the canvas is dumped as Unicode braille,
//! which packs a 2×4 block of pixels into one character, so a curve in a
//! terminal still looks like a curve.
//!
//! `--steps` walks the construction one line at a time — what you would say
//! aloud while drawing it, and the geometry that sentence produces.

use plotkit::{plot, raster::Canvas, view::View, Cx, Frame};
use shapes::{catalogue, find, Recipe};

struct Args {
    name: String,
    steps: bool,
    grid: bool,
    cols: usize,
    png: Option<String>,
}

fn main() {
    let a = match parse(std::env::args().skip(1).collect()) {
        Some(a) => a,
        None => return usage(),
    };
    let Some(r) = find(&a.name) else {
        eprintln!("no shape called '{}'.\n", a.name);
        return usage();
    };

    println!("\n\x1b[1m{}\x1b[0m", r.name.to_uppercase());
    println!("\x1b[38;2;110;125;140m{}\x1b[0m\n", r.maths);

    if a.steps {
        for n in 1..=r.len() {
            println!("\x1b[1m  step {n} of {}\x1b[0m  \x1b[38;2;150;165;180m{}\x1b[0m", r.len(), r.steps[n - 1].says);
            print!("{}", render(&r, n, &a));
            println!();
        }
    } else {
        print!("{}", render(&r, r.len(), &a));
        println!();
        for (n, s) in r.steps.iter().enumerate() {
            let c = s.colour;
            println!("  \x1b[38;2;{};{};{}m{}.\x1b[0m {}", (c >> 16) & 255, (c >> 8) & 255, c & 255, n + 1, s.says);
        }
        println!("\n  (add --steps to watch it being drawn)");
    }
    println!();
}

/// Draw the first `n` steps and hand back the terminal picture.
///
/// The canvas is four times taller than the character grid and twice as wide,
/// because that is what one braille cell holds.
fn render(r: &Recipe, n: usize, a: &Args) -> String {
    // A square canvas. One braille cell is 2 pixels wide and 4 tall, so a
    // square picture becomes cols x cols/2 characters — and a terminal
    // character is about twice as tall as it is wide, which puts it back
    // square on screen. A canvas twice as wide as tall would come out flat.
    let (w, h) = (a.cols * 2, a.cols * 2);
    let mut c = Canvas::new(w, h);
    c.clear(0x000000);

    // Fit the shape to the canvas, so a big shape and a small one both fill it.
    let reach = reach(r).max(0.2);
    let v = View::centred(w, h, (h as f64 * 0.42) / reach);

    if a.grid {
        plot::grid(&mut c, &v, &plot::GridStyle { minor: 0x000000, major: 0x101820, axis: 0x203040, labels: false, ..Default::default() });
    }

    let mut f = Frame::new();
    for (k, s) in r.steps.iter().take(n).enumerate() {
        // While stepping, everything before the newest line fades back so the
        // eye lands on what just happened. In the finished picture every step
        // keeps its own colour, which is what the numbered key below refers to.
        let dim = a.steps && k + 1 < n;
        f.add(s.shape.clone()).color(if dim { 0x2E3A46 } else { s.colour }).width(1).dot(2.0);
    }
    f.draw(&mut c, &v);
    c.braille(0x000000, true)
}

/// How far the shape reaches from the origin, so the view can be fitted to it.
fn reach(r: &Recipe) -> f64 {
    r.shape()
        .polylines(Cx::new(-99.0, -99.0), Cx::new(99.0, 99.0), 600)
        .into_iter()
        .flatten()
        .fold(0.0f64, |m, p| m.max(p.re.abs()).max(p.im.abs()))
}

fn parse(mut args: Vec<String>) -> Option<Args> {
    // Tolerate the way it is natural to type this: `shape one`, `lib shape one`.
    while matches!(args.first().map(|s| s.as_str()), Some("lib") | Some("shape") | Some("shapes") | Some("run")) {
        args.remove(0);
    }
    let mut a = Args { name: String::new(), steps: false, grid: false, cols: 46, png: None };
    let mut it = args.into_iter();
    while let Some(t) = it.next() {
        match t.as_str() {
            "--steps" | "-s" => a.steps = true,
            "--grid" | "-g" => a.grid = true,
            "--big" | "-b" => a.cols = 70,
            "--small" => a.cols = 30,
            "--png" => a.png = it.next(),
            "list" | "--list" | "-l" => {
                println!("\n  {}\n", catalogue().join("  "));
                println!("  digits also answer to numerals: 0 1 2 ... 9");
                println!("  tally marks take a count: tally3, tally7, tally12\n");
                // Done, and not a failure to parse — so do not fall through to
                // printing the usage block as well.
                std::process::exit(0);
            }
            _ if t.starts_with('-') => return None,
            _ => a.name = t,
        }
    }
    if a.name.is_empty() {
        return None;
    }
    if let Some(path) = &a.png {
        write_png(&a, path);
    }
    Some(a)
}

fn write_png(a: &Args, path: &str) {
    let Some(r) = find(&a.name) else { return };
    let (w, h) = (900, 900);
    let mut c = Canvas::new(w, h);
    c.clear(0x0B1017);
    let v = View::centred(w, h, (h as f64 * 0.40) / reach(&r).max(0.2));
    let mut f = Frame::new();
    for s in &r.steps {
        f.add(s.shape.clone()).color(s.colour).width(3);
    }
    f.draw(&mut c, &v);
    match c.write_png(path) {
        Ok(()) => println!("wrote {path}"),
        Err(e) => eprintln!("could not write {path}: {e}"),
    }
}

fn usage() {
    println!(
        "\n  shape — draw a shape in the terminal, and show how it was drawn\n\
         \n\
         \x20   cargo run -p shapes -- <name> [options]\n\
         \n\
         \x20   --steps  -s   draw it one construction line at a time\n\
         \x20   --grid   -g   put graph paper behind it\n\
         \x20   --big    -b   larger  (--small for smaller)\n\
         \x20   --png <file>  also write a full-size picture\n\
         \x20   list          every name it knows\n\
         \n\
         \x20   cargo run -p shapes -- seven --steps\n\
         \x20   cargo run -p shapes -- ghost --big\n"
    );
}
