//! A scratchpad for the complex-number type. Edit anything, re-run, no tests
//! to keep green:
//!
//!     cargo run --example play_complex
//!
//! Change the two numbers in EDIT ME below and everything downstream follows.

use recursion1::complex::Cx;
use std::f64::consts::PI;

fn main() {
    // ================= EDIT ME =================
    let z = Cx::new(3.0, 2.0); // 3 + 2i
    let w = Cx::new(1.0, -2.0); // 1 - 2i
    let spiral_r = 1.08; // >1 grows, <1 decays, =1 circles
    let spiral_a = 0.55; // radians turned per step
    let root_n = 5; // how many roots of unity
    // ===========================================

    rule("the two numbers");
    describe("z", z);
    describe("w", w);

    rule("arithmetic");
    println!("  z + w  = {}", z + w);
    println!("  z - w  = {}", z - w);
    println!("  z * w  = {}   |z||w| = {:.4}", z * w, z.abs() * w.abs());
    println!("  z / w  = {}", z / w);
    println!("  conj z = {}", z.conj());
    println!("  z*conj = {}   <- always a positive real = |z|^2", z * z.conj());

    rule("multiplication ADDS the angles");
    println!("  arg(z)     = {:+.4} rad", z.arg());
    println!("  arg(w)     = {:+.4} rad", w.arg());
    println!("  arg(z)+arg(w) = {:+.4}", z.arg() + w.arg());
    println!("  arg(z*w)   = {:+.4} rad   (same, modulo 2pi)", (z * w).arg());

    rule("the 4-cycle of i");
    let mut p = Cx::ONE;
    for k in 1..=8 {
        p = p * Cx::I;
        print!("  i^{k} = {:>16}", format!("{p}"));
        if k % 4 == 0 {
            println!("   <- full turn, home again");
        } else {
            println!();
        }
    }

    rule("Euler: e^(i theta) round the circle");
    for k in 0..=8 {
        let t = PI * k as f64 / 4.0;
        let e = Cx::expi(t);
        println!(
            "  theta = {:>5.2} rad ({:>4.0} deg)   e^(i theta) = {}   |.| = {:.3}",
            t,
            t.to_degrees(),
            e,
            e.abs()
        );
    }

    rule("powers of one number = a spiral");
    println!("  base: r = {spiral_r}, angle = {spiral_a} rad");
    let base = Cx::polar(spiral_r, spiral_a);
    let mut pts = Vec::new();
    let mut cur = Cx::ONE;
    for k in 0..=18 {
        if k > 0 {
            cur = cur * base;
        }
        pts.push(cur);
        if k % 3 == 0 {
            println!("  z^{:<2} = {:>20}   |z^{}| = {:.4}", k, format!("{cur}"), k, cur.abs());
        }
    }
    println!(
        "  {}",
        if spiral_r > 1.0 {
            "|z| > 1 -> spirals OUT (a system that blows up)"
        } else if spiral_r < 1.0 {
            "|z| < 1 -> spirals IN (a system that settles)"
        } else {
            "|z| = 1 -> circles forever (pure oscillation)"
        }
    );
    plot(&pts, "the spiral");

    rule("roots of unity");
    let roots: Vec<Cx> = (0..root_n)
        .map(|k| Cx::expi(2.0 * PI * k as f64 / root_n as f64))
        .collect();
    for (k, r) in roots.iter().enumerate() {
        // raise it back to the nth power - every one must return to 1
        let back = (0..root_n).fold(Cx::ONE, |acc, _| acc * *r);
        println!("  root {k} = {r}   raised to the {root_n}th -> {back}");
    }
    println!("  multiply any two and you land on another one - the set is closed.");
    plot(&roots, &format!("{root_n} roots of unity"));

    rule("nth roots of z (there are always n of them)");
    let n = 3;
    let r = z.abs().powf(1.0 / n as f64);
    for k in 0..n {
        let ang = (z.arg() + 2.0 * PI * k as f64) / n as f64;
        let root = Cx::polar(r, ang);
        println!("  root {k} = {}   cubed -> {}", root, root * root * root);
    }
    println!("  \"the\" cube root does not exist - you pick a branch.");
}

fn rule(title: &str) {
    println!("\n== {title} {}", "=".repeat(66usize.saturating_sub(title.len())));
}

fn describe(name: &str, z: Cx) {
    println!(
        "  {name} = {}   |{name}| = {:.4}   arg = {:+.4} rad ({:+.1} deg)",
        z,
        z.abs(),
        z.arg(),
        z.arg().to_degrees()
    );
}

/// A tiny ASCII plot of the complex plane, so a list of numbers becomes a shape.
fn plot(pts: &[Cx], title: &str) {
    const W: usize = 61;
    const H: usize = 23;
    let m = pts.iter().map(|p| p.abs()).fold(1e-9, f64::max) * 1.15;
    let mut grid = vec![vec![' '; W]; H];

    let col = |x: f64| (((x / m + 1.0) / 2.0) * (W - 1) as f64).round() as usize;
    let row = |y: f64| (((1.0 - y / m) / 2.0) * (H - 1) as f64).round() as usize;

    for c in 0..W {
        grid[row(0.0)][c] = '-';
    }
    for r in 0..H {
        grid[r][col(0.0)] = '|';
    }
    grid[row(0.0)][col(0.0)] = '+';

    for (k, p) in pts.iter().enumerate() {
        let (r, c) = (row(p.im), col(p.re));
        if r < H && c < W {
            // number the first ten so the ORDER is visible, then dots
            grid[r][c] = if k < 10 {
                char::from_digit(k as u32, 10).unwrap()
            } else {
                '*'
            };
        }
    }

    println!("\n  {title}   (real ->, imaginary ^, scale {:.2})", m);
    for r in grid {
        println!("  {}", r.into_iter().collect::<String>());
    }
}
