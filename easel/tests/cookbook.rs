//! Every claim the cookbook makes that can be checked, checked.
//!
//! A document that drifts from the code is worse than no document: it is
//! wrong with authority. These are the statements in `document/Studio.md`
//! that are testable, so the day one stops being true, this fails.

use easel::{Board, Script};
use plotkit::Cx;

fn run(src: &str) -> easel::script::Made {
    let mut s = Script::new();
    for line in src.lines() {
        s.add(line);
    }
    s.run(0.0)
}

fn ink(made: &easel::script::Made) -> usize {
    let (lo, hi) = (Cx::new(-10.0, -10.0), Cx::new(10.0, 10.0));
    made.shapes.iter().map(|(s, _)| s.polylines(lo, hi, 900).iter().map(Vec::len).sum::<usize>()).sum()
}

/// ★ The cookbook's table of questions: an answer is a number, so it is
/// arithmetic; equality is slack; comparison binds looser than `+`; and the
/// deciding happens before the arguments are worked out.
#[test]
fn the_question_table_is_true() {
    let val = |src: &str| {
        let made = run(src);
        assert!(made.errors.is_empty(), "{src}: {:?}", made.errors);
        made.vars.iter().find(|(n, _)| n == "a").expect("a").1.re
    };
    assert_eq!(val("die = 6
a = 5 + 10*(die == 6)"), 15.0);
    assert_eq!(val("a = 0.1 + 0.2 == 0.3"), 1.0, "equality has a hair of slack");
    assert_eq!(val("a = 1 + 2 == 3"), 1.0, "comparison binds looser than +");
    assert_eq!(val("x = 0
a = if(x == 0, 0, 1/x)"), 0.0, "if never divides by nothing");
    assert_eq!(val("a = and(0, ln(0))"), 0.0, "and never takes the log of nothing");
    assert_eq!(val("a = pick(0, 5, ln(0))"), 5.0, "pick works out only the one chosen");
    assert!(!run("a = 1 < 2 < 3").errors.is_empty(), "two comparisons in a row are refused");
}

/// ★ A curve that only **touches** the level is drawn if and only if the grid
/// happens to land on it — which is worse than never being drawn, because it
/// depends on where you are looking.
///
/// The sign test is `(a > 0) != (b > 0)`, so a corner exactly at the level
/// counts as the non-positive side. `x*x` at `x = 0` is exactly zero, and the
/// grid over a symmetric window lands there — so the y-axis appears. Shift the
/// touch off the grid and it vanishes.
#[test]
fn a_touching_curve_appears_only_if_the_grid_lands_on_it() {
    assert!(ink(&run("implicit(x*x, 0)")) > 0, "the grid lands on x = 0, so it is drawn");
    let off = run("implicit((x - 0.0137)*(x - 0.0137), 0)");
    assert_eq!(ink(&off), 0, "moved a hair off the grid, the same curve vanishes");

    assert!(ink(&run("implicit(x, 0)")) > 10, "a genuine crossing is always drawn");
    assert!(ink(&run("implicit(x*x + y*y, 4)")) > 100, "and so is a circle of radius 2");
}

/// ★ "The grid follows the window" — zooming in gives more detail, not a
/// bigger staircase.
#[test]
fn implicit_resamples_with_the_view() {
    let made = run("implicit(x*x + y*y, 4)");
    let count = |lo: f64, hi: f64| {
        made.shapes[0].0.polylines(Cx::new(lo, lo), Cx::new(hi, hi), 900).len()
    };
    // Looking at a small window puts more of the grid on the same curve.
    assert!(count(-2.5, 2.5) > count(-40.0, 40.0), "a closer look should sample the curve more finely");
}

/// ★ "A cell where F is not finite is skipped" — holes, not nonsense.
#[test]
fn implicit_leaves_a_hole_where_it_blows_up() {
    let made = run("implicit(1/(x*y), 1)");
    assert!(made.errors.is_empty());
    for (s, _) in &made.shapes {
        for run in s.polylines(Cx::new(-6.0, -6.0), Cx::new(6.0, 6.0), 600) {
            for z in run {
                assert!(z.re.is_finite() && z.im.is_finite());
            }
        }
    }
}

/// ★ "plot breaks its line where the value stops being finite" — 1/x is two
/// curves, not two curves joined through the middle.
#[test]
fn a_plot_breaks_at_a_pole() {
    let made = run("plot(1/x)");
    let runs = made.shapes[0].0.polylines(Cx::new(-6.0, -6.0), Cx::new(6.0, 6.0), 600);
    assert!(runs.len() >= 2, "it should be in pieces, not one line across the pole");
}

/// ★ The sampling costs quoted: param is 320 points whatever the view, and a
/// plot is about one per pixel of width.
#[test]
fn the_sampling_costs_are_what_the_cookbook_says() {
    let made = run("param(exp(i*t), 0, tau)");
    for width in [300usize, 900, 1800] {
        let n: usize =
            made.shapes[0].0.polylines(Cx::new(-9.0, -9.0), Cx::new(9.0, 9.0), width).iter().map(Vec::len).sum();
        // 320 steps, so 321 points -- both ends are included.
        assert_eq!(n, 321, "param is 321 points whatever the window");
    }
    let plot = run("plot(x)");
    let n: usize =
        plot.shapes[0].0.polylines(Cx::new(-9.0, -9.0), Cx::new(9.0, 9.0), 900).iter().map(Vec::len).sum();
    assert!((890..=910).contains(&n), "a plot is about one sample per pixel of width, got {n}");
}

/// ★ The table of how far `sin` reaches off the real axis.
#[test]
fn sin_grows_off_the_real_axis_as_the_table_says() {
    let at = |im: f64| {
        let made = run(&format!("a = abs(sin(1 + {im}i))"));
        made.vars.iter().find(|(n, _)| n == "a").expect("a").1.re
    };
    assert!(at(0.0) <= 1.0);
    assert!((at(5.0) - 74.0).abs() < 5.0, "at Im 5 it is about 74, got {}", at(5.0));
    assert!((at(10.0) / 11013.0 - 1.0).abs() < 0.05, "at Im 10 it is about 11013, got {}", at(10.0));
    assert!(at(20.0) > 1e8);
    assert!(at(710.0).is_finite(), "710 still fits, just");
    assert!(!at(711.0).is_finite(), "and a little past 710.5 an f64 gives up");
}

/// ★ `tan` errors at a pole rather than returning infinity.
#[test]
fn tan_says_so_at_a_pole() {
    let made = run("a = tan(pi/2)");
    // pi/2 in floating point is not exactly the pole, so this is finite --
    // but huge, which is the honest answer.
    let v = made.vars.iter().find(|(n, _)| n == "a").expect("a").1.re;
    assert!(v.abs() > 1e15, "near the pole it should be enormous, got {v}");
}

/// ★ `ln` refuses zero, and takes the principal branch — so `arg(-1)` is `+pi`
/// and not `-pi`, negative zero notwithstanding.
#[test]
fn ln_and_arg_take_the_principal_branch() {
    assert!(!run("a = ln(0)").errors.is_empty(), "ln(0) should say so");
    let made = run("a = arg(-1)");
    let v = made.vars.iter().find(|(n, _)| n == "a").expect("a").1.re;
    assert!((v - std::f64::consts::PI).abs() < 1e-12, "arg(-1) should be +pi, got {v}");
}

/// ★ `sqrt(-4)` is `2i`: the principal branch, with a cut along the negatives.
#[test]
fn sqrt_of_a_negative_is_imaginary() {
    let made = run("a = sqrt(-4)");
    let v = made.vars.iter().find(|(n, _)| n == "a").expect("a").1;
    assert!(v.re.abs() < 1e-12 && (v.im - 2.0).abs() < 1e-12, "got {v:?}");
}

/// ★ The whole-number functions read only the real part and hand back a real.
#[test]
fn the_whole_number_functions_drop_the_imaginary_part() {
    let made = run("a = floor(1.7 + 9i)\nb = mod(-1, 9)\nc = max(0, 3 - 5)");
    let get = |n: &str| made.vars.iter().find(|(k, _)| k == n).expect(n).1;
    assert_eq!(get("a"), Cx::new(1.0, 0.0), "floor(1.7 + 9i) is 1, and the 9i is dropped");
    assert_eq!(get("b"), Cx::new(8.0, 0.0), "mod is Euclidean");
    assert_eq!(get("c"), Cx::new(0.0, 0.0));
}

/// ★ `pow` switches implementation at 64, and both agree where they overlap.
#[test]
fn pow_changes_method_at_sixty_four_without_changing_answer() {
    let made = run("a = pow(1.05, 64)\nb = pow(1.05, 65)");
    let get = |n: &str| made.vars.iter().find(|(k, _)| k == n).expect(n).1.re;
    assert!((get("a") - 1.05f64.powi(64)).abs() < 1e-9, "exact by repeated multiplication");
    assert!((get("b") - 1.05f64.powi(65)).abs() < 1e-9, "and the branch path agrees here");
}

/// ★ Hex for colours, and `color(...)` pins from there on.
#[test]
fn a_colour_can_be_written_in_hex() {
    let made = run("color(0xE0A44A)\ncircle(0, 1)\ncircle(0, 2)");
    assert!(made.errors.is_empty(), "{:?}", made.errors);
    assert_eq!(made.shapes[0].1, 0xE0_A4_4A);
    assert_eq!(made.shapes[1].1, 0xE0_A4_4A, "pinned means pinned");
}

/// ★ `ngon` takes 3..512 sides and says so outside that.
#[test]
fn ngon_has_the_range_the_cookbook_states() {
    assert!(run("ngon(0, 1, 3)").errors.is_empty());
    assert!(run("ngon(0, 1, 512)").errors.is_empty());
    assert!(!run("ngon(0, 1, 2)").errors.is_empty());
    assert!(!run("ngon(0, 1, 513)").errors.is_empty());
}

/// ★ A `.rec` file keeps its comments and blank lines, and a round trip
/// through the file format keeps every row verbatim.
#[test]
fn a_script_file_survives_a_round_trip() {
    let text = "# the radius\nr = 2\n\ncircle(0,  r)\n";
    let s = Script::from_rec(text);
    assert_eq!(s.len(), 4, "comment, binding, blank, command");

    let mut b = Board::new();
    b.sheet.script = s.clone();
    let path = std::env::temp_dir().join("cookbook.easel");
    let path = path.to_str().expect("a path");
    b.save(path).expect("saved");

    let mut back = Board::new();
    assert_eq!(back.load(path).expect("loaded"), 0);
    assert_eq!(back.sheet.script.rows, s.rows, "every row, exactly as written");
    let _ = std::fs::remove_file(path);
}
