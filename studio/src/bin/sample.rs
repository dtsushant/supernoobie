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
        ("adding", adding as fn() -> Board),
        ("ludo", ludo as fn() -> Board),
        ("ludogame", ludogame as fn() -> Board),
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

/// **A game.** Two numbers to add, three answers to tap, and a score.
///
/// The whole of it is nine rows and three grouped shapes. Nothing about it is
/// special-cased anywhere: the questions are arithmetic, the answers are
/// ordinary figures, and tapping one runs a rule.
///
/// ## Where the questions come from
///
/// ```text
///     a = 1 + floor(5*abs(sin(37*(score+1))))
/// ```
///
/// `sin` of a big multiple of a whole number wanders about the interval
/// without ever settling into a pattern a child would spot, and `floor` makes
/// it a whole number. **No random number generator** — the same game replays
/// identically, which is what the tapes in this repository have always needed,
/// and it means a wrong answer can be looked at again rather than lost.
///
/// ## Where the wrong answers come from
///
/// One too many and one too few. That is deliberate: an obviously wrong answer
/// teaches a child to spot obviously wrong answers, and a near-miss makes them
/// count. It also means the three boxes never collide, since they are always
/// three consecutive numbers.
///
/// ## Saying well done, and saying oh dear
///
/// A rule notes **when** it happened — `cheer = time` — and a face is drawn at
/// a size that runs down from there: `max(0, 1.1 - 1.4*(time - cheer))`. On
/// the instant it is full size; a second later it is nothing, and a face of no
/// size is a face that is not there. No notion of visibility anywhere, and no
/// code that knows what "temporarily" means.
///
/// ## What is still missing, honestly
///
/// The answers are in a fixed order — right one in the middle — because a rule
/// can change a number but cannot yet move a shape. That wants a deed which
/// sets a mark's pose, and it is the obvious next thing.
fn adding() -> Board {
    let mut b = Board::new();
    b.nib = Nib::Round(0.1);
    b.taper = 0.0;

    // Three boxes to tap. Each is one figure, so a rule can name it.
    for (k, x) in [-3.4f64, 0.0, 3.4].into_iter().enumerate() {
        b.colour = [0xE0704A, 0x6FCF97, 0x4FBCD4][k];
        let at = Cx::new(x, -2.2);
        let corners = [(-1.2, -1.0), (1.2, -1.0), (1.2, 1.0), (-1.2, 1.0), (-1.2, -1.0)];
        let box_path: Vec<Cx> = corners
            .windows(2)
            .flat_map(|w| line(at + Cx::new(w[0].0, w[0].1), at + Cx::new(w[1].0, w[1].1)))
            .collect();
        stroke(&mut b, &box_path);
        b.selected = vec![b.sheet.len() - 1];
        // Each box is its own figure, numbered 1, 2, 3 -- which is what the
        // rules below tap on.
        b.group_alone();
        b.selected.clear();
    }

    let rows = [
        "# ADDING. press PLAY, then tap the box with the right answer.",
        "score = 0",
        "",
        "# the question. no random numbers anywhere: sin of a big multiple of a",
        "# whole number wanders without ever settling into a pattern, and floor",
        "# makes it whole. So the same game replays exactly.",
        "a = 1 + floor(5*abs(sin(37*(score+1))))",
        "b = 1 + floor(5*abs(sin(53*(score+1))))",
        "",
        "digits(a, -3.0, 1.6, 0.9)",
        "digits(b, 0.4, 1.6, 0.9)",
        "color(0x46525E)",
        "line(-1.9 + 1.6i, -1.1 + 1.6i)",
        "line(-1.5 + 1.2i, -1.5 + 2.0i)",
        "line(1.4 + 1.4i, 2.2 + 1.4i)",
        "line(1.4 + 1.8i, 2.2 + 1.8i)",
        "",
        "# the three answers: one too few, the right one, one too many",
        "digits(a + b - 1, -3.4, -2.2, 0.8)",
        "digits(a + b, 0, -2.2, 0.8)",
        "digits(a + b + 1, 3.4, -2.2, 0.8)",
        "",
        "# the score, up in the corner",
        "digits(score, 5.0, 3.2, 0.55)",
        "",
        "# saying well done, and saying oh dear.",
        "#",
        "# `cheer` and `boo` are the moment it happened. A face is drawn at a",
        "# size, so a size of nothing is a face that is not there -- and",
        "# max(0, 1 - (time - cheer)) grows one on the instant and shrinks it",
        "# away over a second. There is no notion of visibility anywhere.",
        "cheer = -9",
        "boo = -9",
        "smiley(3.4, 1.5, max(0, 1.0 - 1.4*(time - cheer)))",
        "ghost(3.4, 1.5, max(0, 1.0 - 0.9*(time - boo)))",
        "",
        "# and the rules. the middle box is the right one.",
        "#",
        "# each one puts the OTHER face into the past, or a fading smile hangs",
        "# about inside the ghost that follows it.",
        "when tap 1: score = score - 1, boo = time, cheer = -9",
        "when tap 2: score = score + 1, cheer = time, boo = -9",
        "when tap 3: score = score - 1, boo = time, cheer = -9",
    ];
    for r in rows {
        b.sheet.script.add(r);
    }
    b
}

/// **A board.** Not a game yet — the geometry, with tokens that walk when you
/// press play, so the path each seat takes can be watched rather than trusted.
///
/// Everything moving here is `time` and arithmetic. `mod` keeps a token on the
/// board when it runs past the end, and the four seats walk at different rates
/// so they separate instead of moving as one.
///
/// What is missing is the rules, and they are missing for a reason: a rule can
/// change a number but cannot yet say "send home whichever token is on this
/// square". That wants writing to a computed name, which is the next thing the
/// language grows.
fn ludo() -> Board {
    let mut b = Board::new();
    let rows = [
        "# THE BOARD. press play and watch the four paths.",
        "ludo()",
        "",
        "# how fast the tokens walk. drag it.",
        "pace = 3",
        "",
        "# each seat walks its own way round, from its own start.",
        "# mod keeps them on the board when they run past the end.",
        "token(0, mod(floor(time*pace), 58), 0.34)",
        "token(1, mod(floor(time*pace*0.8), 58), 0.34)",
        "token(2, mod(floor(time*pace*0.6), 58), 0.34)",
        "token(3, mod(floor(time*pace*0.4), 58), 0.34)",
        "",
        "# and one of each still waiting. a NEGATIVE step is the yard:",
        "# -1 to -4 are the four places a token waits in.",
        "token(0, -1, 0.26)",
        "token(1, -2, 0.26)",
        "token(2, -3, 0.26)",
        "token(3, -4, 0.26)",
    ];
    for r in rows {
        b.sheet.script.add(r);
    }
    b
}

/// **Four-player Ludo, hot seat.** Tap the die, then tap a token.
///
/// ## Two tokens a seat, not four
///
/// Every capture is a written line — "is that one standing where this one just
/// landed?" — so four tokens each is 16 × 15 = 240 of them. Two each is 8 × 7 =
/// 56, which is a game you can read. The repeat that would make four each
/// bearable is the next thing the language wants, and this is the reason.
///
/// ## The die is thrown, and it settles
///
/// ```text
///     slow    = exp(-age/relax)        1 at the throw, nothing when it is over
///     tumbles = spin*(1 - slow)        faces turned through: the RATE decays
///     slid    = 5.5*(1 - slow)         and it slides, and folds off the walls
/// ```
///
/// One number does all three — `e^{-t/τ}`, the same decay as a branch settling
/// after a gust or a note dying away. It whirls, eases and stops; it does not
/// run at one speed and then halt.
///
/// **Nothing is stepped frame by frame.** `flung` is *when* it was thrown, so
/// everything above is a function of the clock — which means the throw can be
/// replayed, scrubbed, or watched at half speed, and looks the same every time.
///
/// The face it lands on is `spin`, worked out from the seed and the roll
/// number rather than drawn from anywhere. No random number generator: it is
/// unpredictable in play and **exactly repeatable** from its seed, which is
/// what lets a match be replayed and makes "he cheated" answerable.
///
/// A move is refused until the die has stopped — four time constants, which is
/// the usual answer to *when has an exponential finished*.
///
/// ## Whether a token may move is named once
///
/// `can0 … can7`, and three things read it: the move itself, the ring round
/// the tokens you may tap, and the rule that notices you may tap **none** of
/// them. Three copies of one rule is three chances for them to disagree.
///
/// That last one matters more than it sounds. Roll a three with both tokens in
/// the yard and nothing at all is legal — so the turn has to pass by itself,
/// or the game simply stops. It did.
///
/// ## Safe squares in one `mod`
///
/// The four starts are at 0, 13, 26 and 39, and the four starred squares eight
/// further on. So "safe" is `here` being a multiple of thirteen, or eight past
/// one — the whole rule in one expression rather than a list of eight numbers
/// to mistype.
///
/// ## A house rule is a dial
///
/// No two tables play Ludo the same way, and the differences are all *numbers*:
/// what opens the gate, whether you must cut before coming home, what earns
/// another turn, whether two together block a square. So they are numbers —
/// plain bindings at the top of the file:
///
/// ```text
///     opens = 6     alsoone = 0     mustcut = 0     blockade = 1
///     again6 = 1    againcut = 0    againhome = 0   stars = 1
/// ```
///
/// A plain number gets a slider, so these are set before the game starts by
/// dragging them; and they are saved in the file, so **two tables differ by a
/// file** rather than by a build. The rules already written just read them —
/// `if(at < 0, or(die == opens, and(alsoone == 1, die == 1)), …)` is the same
/// line it always was with a name where the six used to be.
///
/// This is why the settings needed no new machinery. Once behaviour is rows,
/// configuring behaviour is editing rows, which the file format already does.
///
/// ## Squares, not steps
///
/// Two seats at the same *step* are on different *squares*: they start a
/// quarter of the loop apart. So a capture compares `sq`, not `at`:
///
/// ```text
///     sq = mod(13*seat + at, 52)      on the track
///          -100 - k                   in the yard: its own, so never shared
///          200 + 10*seat + at         in the home column: private to a seat
/// ```
///
/// The yard and the home column get numbers no other token can hold, which is
/// how "nobody can be captured there" is said without a rule saying it.
fn ludogame() -> Board {
    let mut b = Board::new();
    let tokens = 8usize; // two each
    let seat_of = |k: usize| k / 2;

    let mut rows: Vec<String> = Vec::new();
    macro_rules! add {
        ($t:expr) => {
            rows.push(($t).to_string())
        };
    }

    add!("# LUDO, four players, one screen. tap the DIE, then tap a token.");
    add!("ludo()");
    add!("");
    add!("# --- HOUSE RULES ------------------------------------------------");
    add!("# Set these before you start. Each is a plain number, so each gets a");
    add!("# slider -- a house rule is a dial, and two tables differ by a file.");
    add!("#");
    add!("# what brings a token out of the yard");
    add!("opens = 6");
    add!("# ...and whether a one does as well");
    add!("alsoone = 0");
    add!("# must a seat have cut somebody before it may go home?");
    add!("mustcut = 0");
    add!("# what earns another turn: a six, a capture, getting one home");
    add!("again6 = 1");
    add!("againcut = 0");
    add!("againhome = 0");
    add!("# may you land on a square where two of one seat already stand?");
    add!("blockade = 1");
    add!("# show the eight safe squares");
    add!("stars = 1");
    add!("");
    add!("# --- the state -------------------------------------------------");
    add!("seed = 137");
    add!("rolls = 0");
    add!("rolled = 0");
    add!("# when the die was last thrown. the whole throw is a function of the");
    add!("# clock and this one number, so nothing is stepped frame by frame.");
    add!("flung = -99");
    add!("turn = 0");
    for k in 0..tokens {
        rows.push(format!("seat{k} = {}", seat_of(k)));
        rows.push(format!("at{k} = -{}", k % 2 + 1));
    }
    for seat in 0..4 {
        rows.push(format!("cuts{seat} = 0"));
    }
    add!("");
    add!("# where each token counts as standing. the yard and the home column");
    add!("# get numbers no other token can hold, so nobody can be caught there.");
    for k in 0..tokens {
        rows.push(format!(
            "sq{k} = if(at{k} < 0, -100 - {k}, if(at{k} > 50, 200 + 10*seat{k} + at{k}, mod(13*seat{k} + at{k}, 52)))"
        ));
    }
    add!("");
    add!("# --- the die, thrown -------------------------------------------");
    add!("#");
    add!("# A die is flung, tumbles fast, slows, and settles. All three are one");
    add!("# number: exp(-age/relax), which is 1 at the throw and nothing when it");
    add!("# is over -- the same e^(-t/tau) as a branch settling after a gust or a");
    add!("# note dying away. Laplace, doing the only thing it ever does.");
    add!("relax = 0.42");
    add!("age = max(0, time - flung)");
    add!("slow = exp(-0 - age/relax)");
    add!("");
    add!("# how hard it was thrown. different every roll, and worked out rather");
    add!("# than drawn from anywhere, so the same match replays exactly.");
    add!("spin = 11 + mod(floor(seed + 29*rolls), 13)");
    add!("");
    add!("# faces turned through so far. the RATE decays, so it whirls, eases");
    add!("# and stops -- it does not run at one speed and then halt.");
    add!("tumbles = spin*(1 - slow)");
    add!("");
    add!("# four time constants is 98 percent of the way there, which is the");
    add!("# usual answer to `when has an exponential finished`.");
    add!("settled = age > 4*relax");
    add!("die = if(settled, 1 + mod(floor(spin), 6), 1 + mod(floor(tumbles), 6))");
    add!("");
    add!("# and it slides, and bounces off the walls of its box. a reflection is");
    add!("# a triangle wave: go a distance, fold it back at each edge.");
    add!("box = 1.15");
    add!("slid = 5.5*(1 - slow)");
    add!("dx = box - abs(mod(slid, 2*box) - box)");
    add!("dy = 0.55*box - abs(mod(1.7*slid, 1.1*box) - 0.55*box)");
    add!("");
    add!("# --- what you can see ------------------------------------------");
    add!("color(0xE3E9EF)");
    add!("digits(die, 8.6 - box + dx, 4.2 - 0.3 + dy, 0.9)");
    add!("");
    add!("# whose turn it is, in that seat's own colour, beside the die.");
    add!("color(pick(turn, 0xE0704A, 0x6FCF97, 0x4FBCD4, 0xE0A44A))");
    add!("param(0.62*exp(i*t) + 8.6 + 1.5i, 0, tau)");
    add!("digits(turn + 1, 8.6, 1.5, 0.42)");
    add!("");
    add!("# a ring round the die while it is still going, so it is plain that");
    add!("# the number is not yet the number.");
    add!("color(0x46525E)");
    add!("param(if(settled, 0, 1.5)*exp(i*t) + 8.6 + 4.2i, 0, tau)");

    add!("");
    add!("# --- throwing it ------------------------------------------------");
    add!("# `flung` is WHEN, so everything above is a function of the clock and");
    add!("# nothing has to be stepped frame by frame.");
    add!("when tap 9: rolls = rolls + if(rolled == 1, 0, 1), \
         flung = if(rolled == 1, flung, time), \
         rolled = 1");

    add!("");
    add!("# how many of ONE seat stand on each square a token might land on.");
    add!("# A blockade is two of a colour together; you may not land there.");
    add!("");
    add!("# --- may it move? ----------------------------------------------");
    add!("# Named once and used three times: to allow the move, to ring the");
    add!("# tokens you may tap, and to notice that you may tap none of them.");
    add!("# Three copies of a rule is three chances for them to disagree.");
    for k in 0..tokens {
        // Out of the yard on whatever the house says opens it; and into the
        // home column only if the house does not ask for a cut first.
        let lands = format!("at{k} + die");
        let blocked = (0..tokens)
            .step_by(2)
            .map(|j| {
                format!(
                    "and(and(seat{j} != seat{k}, at{j} >= 0), and(at{j} == at{jj}, \
                     mod(13*seat{j} + at{j}, 52) == mod(13*seat{k} + {lands}, 52)))",
                    jj = j + 1
                )
            })
            .fold(String::from("0"), |acc, t| format!("or({acc}, {t})"));
        rows.push(format!(
            "onto{k} = if(blockade == 0, 0, {blocked})"
        ));
        rows.push(format!(
            "can{k} = and(and(and(seat{k} == turn, rolled == 1), settled), \
             if(at{k} < 0, or(die == opens, and(alsoone == 1, die == 1)), \
             and(and({lands} <= 57, not(onto{k})), \
             or(or(mustcut == 0, cuts[seat{k}] > 0), {lands} <= 50))))"
        ));
    }
    rows.push(format!(
        "anycan = {}",
        (0..tokens).map(|k| format!("can{k}")).fold(String::new(), |acc, t| if acc.is_empty() {
            t
        } else {
            format!("or({acc}, {t})")
        })
    ));
    add!("");
    add!("# a ring round each token you may move. drawn at no size when you may");
    add!("# not, which is how a thing is hidden here.");
    add!("color(0xE3E9EF)");
    for k in 0..tokens {
        rows.push(format!(
            "param(if(can{k}, 0.52, 0)*exp(i*t) + ludox(seat{k}, at{k}) + i*ludoy(seat{k}, at{k}), 0, tau)"
        ));
    }

    add!("");
    add!("# --- a turn nobody can play -------------------------------------");
    add!("# Roll a three with both tokens in the yard and nothing is legal. The");
    add!("# turn has to pass by itself, or the game simply stops -- which it did.");
    add!("when and(and(rolled == 1, settled), not(anycan)): \
         turn = mod(turn + 1, 4), rolled = 0, passed = passed + 1");
    add!("passed = 0");

    add!("");
    add!("# --- three sixes forfeit ----------------------------------------");
    add!("# The face is not known until the die stops, so this waits for that");
    add!("# rather than firing when it is thrown.");
    add!("sixes = 0");
    add!("when and(rolled == 1, settled): sixes = if(die == 6, sixes + 1, 0)");
    add!("when sixes > 2: turn = mod(turn + 1, 4), rolled = 0, sixes = 0");

    add!("");
    add!("# --- moving a token ---------------------------------------------");
    for k in 0..tokens {
        let mut deeds: Vec<String> = Vec::new();
        // May this token move at all? Its seat's turn, a die rolled, and
        // either it is out or the die is a six.
        deeds.push(format!("ok = can{k}"));
        // Out of the yard on a six goes to the start; otherwise walk on.
        deeds.push(format!("was = at{k}"));
        for j in 0..tokens {
            if j != k {
                deeds.push(format!("was{j} = at{j}"));
            }
        }
        deeds.push(format!("at[{k}] = if(ok, if(at{k} < 0, 0, at{k} + die), at{k})"));
        deeds.push(format!(
            "here = if(ok, if(was < 0, mod(13*seat{k}, 52), \
             if(at[{k}] > 50, 200 + 10*seat{k} + at[{k}], mod(13*seat{k} + at[{k}], 52))), -999)"
        ));
        for j in 0..tokens {
            if j == k {
                continue;
            }
            // Not on a safe square. The four starts are at 0, 13, 26 and 39 and
            // the four starred ones eight further on -- so "safe" is `here`
            // being a multiple of thirteen, or eight past one. The whole rule
            // in one `mod`, rather than a list of eight numbers to mistype.
            deeds.push(format!(
                "at[{j}] = if(and(and(and(seat{j} != seat{k}, sq{j} == here), and(here >= 0, here < 52)), \
                 not(or(mod(here, 13) == 0, mod(here, 13) == 8))), -{}, at{j})",
                j % 2 + 1
            ));
        }
        // Did this move cut anybody? Counted, because a house rule may ask
        // for a cut before a seat is allowed home.
        let cut = (0..tokens)
            .filter(|j| *j != k)
            .map(|j| format!("and(at{j} < 0, was{j} >= 0)"))
            .fold(String::from("0"), |acc, t| format!("or({acc}, {t})"));
        deeds.push(format!("cut = if(ok, {cut}, 0)"));
        deeds.push(format!("cuts[seat{k}] = cuts[seat{k}] + cut"));
        // Another turn, on whatever the house says earns one.
        deeds.push(format!(
            "turn = if(ok, if(or(or(and(again6 == 1, die == 6), and(againcut == 1, cut)), \
             and(againhome == 1, at[{k}] == 57)), turn, mod(turn + 1, 4)), turn)"
        ));
        deeds.push("rolled = if(ok, 0, rolled)".into());
        rows.push(format!("when tap {}: {}", k + 1, deeds.join(", \\\n         ")));
    }

    add!("");
    add!("# --- the safe stars ---------------------------------------------");
    add!("# Drawn at no size when the house does not use them, which is how a");
    add!("# thing is hidden here.");
    add!("color(0x6B7987)");
    for k in (0..52).step_by(13) {
        for off in [0usize, 8] {
            rows.push(format!(
                "param(if(stars == 1, 0.3, 0)*exp(i*t) + ludox(0, {}) + i*ludoy(0, {}), 0, tau)",
                k + off,
                k + off
            ));
        }
    }

    add!("");
    add!("# --- how many are in --------------------------------------------");
    for seat in 0..4 {
        let mine: Vec<String> =
            (0..tokens).filter(|k| seat_of(*k) == seat).map(|k| format!("(at{k} == 57)")).collect();
        rows.push(format!("home{seat} = {}", mine.join(" + ")));
        rows.push(format!("color({})", ["0xE0704A", "0x6FCF97", "0x4FBCD4", "0xE0A44A"][seat]));
        rows.push(format!("digits(home{seat}, -8.4, {}, 0.4)", 4.0 - seat as f64 * 1.1));
    }

    add!("");
    add!("# --- winning ----------------------------------------------------");
    for seat in 0..4 {
        let mine: Vec<String> = (0..tokens).filter(|k| seat_of(*k) == seat).map(|k| format!("at{k} == 57")).collect();
        rows.push(format!("when and({}, {}): won = {}", mine[0], mine[1], seat + 1));
    }
    add!("won = 0");
    add!("color(0x6FCF97)");
    add!("digits(won, 8.6, -1.2, 0.5)");

    for r in &rows {
        b.sheet.script.add(r.replace(" \\\n         ", " ").replace("\\\n         ", " "));
    }

    // The tokens and the die are MARKS, because only a mark can be tapped --
    // and each follows its own numbers, which is what `place` is for.
    for k in 0..tokens {
        let seat = seat_of(k);
        let at = plotkit::ludo::waiting(seat, k % 2 + 1);
        let ring: Vec<Cx> = (0..=28).map(|j| at + Cx::polar(0.36, j as f64 / 28.0 * TAU)).collect();
        b.nib = Nib::Round(0.1);
        b.colour = shapes::ludo::SEATS[seat];
        stroke(&mut b, &ring);
        let m = b.sheet.marks.last_mut().expect("a token");
        m.closed = true;
        m.place = Some((format!("ludox(seat{k}, at{k})"), format!("ludoy(seat{k}, at{k})")));
        b.selected = vec![b.sheet.len() - 1];
        b.group_alone();
    }
    // The die: figure 9, in the corner.
    b.colour = 0xE3E9EF;
    b.nib = Nib::Round(0.08);
    let at = Cx::new(8.6, 4.2);
    let box_path: Vec<Cx> = [(-0.9, -0.9), (0.9, -0.9), (0.9, 0.9), (-0.9, 0.9), (-0.9, -0.9)]
        .windows(2)
        .flat_map(|w| line(at + Cx::new(w[0].0, w[0].1), at + Cx::new(w[1].0, w[1].1)))
        .collect();
    stroke(&mut b, &box_path);
    b.selected = vec![b.sheet.len() - 1];
    b.group_alone();
    b.selected.clear();
    b
}
