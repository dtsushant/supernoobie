//! The die as a **foundation**: five expression functions, one command, and a
//! mark that can be turned. Any drawing can have a thrown die in five rows.

use easel::Board;
use plotkit::Cx;

fn one(src: &str) -> Board {
    let mut b = Board::new();
    for row in src.trim().lines() {
        b.sheet.script.add(row.trim());
    }
    b
}

fn v(b: &Board, name: &str) -> f64 {
    b.written().vars.iter().find(|(n, _)| n == name).map(|(_, x)| x.re).unwrap_or(f64::NAN)
}

/// ★ **A die in five rows, in a drawing that knows nothing about Ludo.** This
/// is the whole point of moving it out of the board's rows: the next game gets
/// it for nothing.
#[test]
fn any_drawing_can_have_a_thrown_die() {
    let b = one(
        "
        seed = 41
        rolls = 3
        age = 0.4
        span = 5
        face = dieface(seed, rolls, age, span)
        done = diedone(seed, rolls, age, span)
        dice(face, diex(seed, rolls, age, span), diey(seed, rolls, age, span), 0.6, dieturn(seed, rolls, age, span))
        ",
    );
    let made = b.written();
    assert!(made.errors.is_empty(), "{:?}", made.errors);
    let face = v(&b, "face");
    assert!((1.0..=6.0).contains(&face), "a real face: {face}");
    assert_eq!(v(&b, "done"), 0.0, "still going at 0.4 seconds");
    // A body and one pip per spot, drawn filled -- pips as hairline rings do
    // not read as pips at any size.
    assert_eq!(made.solid.len(), 1 + face as usize, "a body and {face} pips");
}

/// ★ The five agree with each other, because they are one throw asked five
/// questions rather than five sums that could drift apart.
#[test]
fn the_five_describe_one_throw() {
    let b = one(
        "
        seed = 137
        rolls = 2
        age = 0.7
        span = 6
        x = diex(seed, rolls, age, span)
        y = diey(seed, rolls, age, span)
        a = dieturn(seed, rolls, age, span)
        f = dieface(seed, rolls, age, span)
        ",
    );
    let r = plotkit::dice::thrown(137.0, 2.0, 0.7, 6.0);
    assert!((v(&b, "x") - r.at.re).abs() < 1e-9);
    assert!((v(&b, "y") - r.at.im).abs() < 1e-9);
    assert!((v(&b, "a") - r.turn).abs() < 1e-9);
    assert_eq!(v(&b, "f"), r.face as f64);
}

/// ★ It stays on the board it was thrown across, at every moment — the
/// property a detect-and-push-back scheme fails on the frame it is fastest.
#[test]
fn the_die_stays_within_its_span() {
    for age in 0..60 {
        let b = one(&format!(
            "seed = 9\nrolls = 5\nage = {}\nspan = 4\nx = diex(seed, rolls, age, span)\ny = diey(seed, rolls, age, span)",
            age as f64 * 0.06
        ));
        assert!(v(&b, "x").abs() <= 4.0 + 1e-9, "x {} at {age}", v(&b, "x"));
        assert!(v(&b, "y").abs() <= 4.0 + 1e-9, "y {} at {age}", v(&b, "y"));
    }
}

/// ★ **`placea` turns a mark about its own anchor**, so a shape spins in place
/// rather than orbiting the origin. Without it a mark can be moved by the game
/// but only ever sits the way it was drawn, which reads as sliding.
#[test]
fn a_mark_can_be_turned_by_an_expression() {
    let mut b = Board::new();
    b.sheet.script.add("a = 0");
    // A short bar along x, so a quarter turn is unmistakable.
    let pts: Vec<Cx> = (0..=10).map(|k| Cx::new(k as f64 * 0.1, 0.0)).collect();
    b.sheet.add(easel::Mark {
        pts,
        nib: shapes::Nib::Round(0.05),
        taper: 0.0,
        colour: 0xFFFFFF,
        filled: false,
        closed: false,
        act: easel::Act::still(),
        track: easel::Track::new(),
        place: Some(("2".into(), "3".into())),
        spin: Some("a".into()),
        group: 0,
    });

    let ends = |b: &Board| {
        let env = b.sheet.script.env(b.clock, &b.tally);
        let s = b.sheet.marks[0].at_in(b.clock, &env);
        let runs = s.polylines(Cx::new(-9.0, -9.0), Cx::new(9.0, 9.0), 600);
        let all: Vec<Cx> = runs.into_iter().flatten().collect();
        let far = all.iter().cloned().fold(all[0], |acc, p| {
            if (p - Cx::new(2.0, 3.0)).abs() > (acc - Cx::new(2.0, 3.0)).abs() { p } else { acc }
        });
        far - Cx::new(2.0, 3.0)
    };

    let flat = ends(&b);
    assert!(flat.re.abs() > flat.im.abs() * 3.0, "it lies along x: {flat:?}");

    b.sheet.script.set_dial("a", std::f64::consts::FRAC_PI_2);
    let turned = ends(&b);
    assert!(turned.im.abs() > turned.re.abs() * 3.0, "and a quarter turn stands it up: {turned:?}");
    assert!(
        (turned.abs() - flat.abs()).abs() < 0.05,
        "the same length, turned -- not stretched: {} then {}",
        flat.abs(),
        turned.abs()
    );
}

/// And a mark that is placed but not turned is unaffected, so `placea` is
/// genuinely optional.
#[test]
fn a_placed_mark_without_a_turn_is_unchanged() {
    let mut b = Board::new();
    let pts: Vec<Cx> = (0..=6).map(|k| Cx::new(k as f64 * 0.2, 0.0)).collect();
    b.sheet.add(easel::Mark {
        pts,
        nib: shapes::Nib::Round(0.05),
        taper: 0.0,
        colour: 0xFFFFFF,
        filled: false,
        closed: false,
        act: easel::Act::still(),
        track: easel::Track::new(),
        place: Some(("1".into(), "1".into())),
        spin: None,
        group: 0,
    });
    let env = b.sheet.script.env(0.0, &b.tally);
    let pose = b.sheet.marks[0].pose_in(0.0, &env);
    assert_eq!(pose.apply(Cx::new(1.0, 0.0)) - pose.apply(Cx::ZERO), Cx::new(1.0, 0.0), "no turn");
}

/// ★ `placea` survives a save and a load, like everything else here — a thing
/// that only works until you close the file is not a feature.
#[test]
fn a_turn_is_saved_with_the_drawing() {
    let mut b = Board::new();
    let pts: Vec<Cx> = (0..=4).map(|k| Cx::new(k as f64 * 0.2, 0.0)).collect();
    b.sheet.add(easel::Mark {
        pts,
        nib: shapes::Nib::Round(0.05),
        taper: 0.0,
        colour: 0xFFFFFF,
        filled: false,
        closed: false,
        act: easel::Act::still(),
        track: easel::Track::new(),
        place: Some(("sin(t)".into(), "0".into())),
        spin: Some("2*t + 1".into()),
        group: 0,
    });
    let text = b.sheet.to_text();
    let (back, muddle) = easel::Sheet::from_text(&text);
    assert_eq!(muddle, 0, "nothing it could not read");
    assert_eq!(back.marks[0].spin.as_deref(), Some("2*t + 1"));
    assert_eq!(back.marks[0].place, b.sheet.marks[0].place);
}
