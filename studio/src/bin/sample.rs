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
/// ## Four tokens a seat, because a rule may now repeat
///
/// The capture is one sentence — *anybody standing where I just landed goes
/// back to the yard* — and written a token at a time it was one line per pair:
/// 8 × 7 = 56 lines for two tokens a seat, and 16 × 15 = 240 for four. So the
/// board had two.
///
/// `each j in 0..16 (…)` collapses all of it to one deed, and the whole
/// tap rule went from **1706 characters to 730 while the board doubled** — it no
/// longer grows with the number of tokens at all. That is the entire reason the
/// loop exists, and the board is the proof it was worth adding.
///
/// ## The die is thrown across the whole board
///
/// The throw is not written here at all. It is [`plotkit::dice`], so any game
/// can have one, and the board asks for it in five rows that differ only in the
/// name at the front:
///
/// ```text
///     die     = dieface(seed, rolls, age, span)
///     settled = diedone(seed, rolls, age, span)
///     dice(die, diex(…), diey(…), 0.62, dieturn(…))
/// ```
///
/// ## The old die is worth remembering
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
    let tokens = 16usize; // four each, which `each` is what made possible
    let seat_of = |k: usize| k / 4;

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
    add!("# The `# rule:` at the end of a line is what puts it on the setup");
    add!("# screen, with that text as its name. Any game can do it; nothing here");
    add!("# knows the word `ludo`.");
    add!("opens = 6                # rule: what brings a token out");
    add!("alsoone = 0              # rule: a one brings one out as well");
    add!("mustcut = 0              # rule: no way home until you have cut somebody");
    add!("mercy = 1                # rule: ...but your farthest token may go home anyway");
    add!("again6 = 1               # rule: a six earns another turn");
    add!("againcut = 0             # rule: a capture earns another turn");
    add!("againhome = 0            # rule: getting one home earns another turn");
    add!("blockade = 1             # rule: two together block the square");
    add!("stars = 1                # rule: show the safe stars");
    add!("starback = 2             # rule: squares the star sits before the home turn");
    add!("");
    add!("# Worked out HERE, beside the rule it comes from, and not down with the");
    add!("# drawing that shows it -- which is thirty rows below the tap rules that");
    add!("# read it. A name bound after its reader is not an error, it is an");
    add!("# unknown name: the comparison quietly fails and every square becomes");
    add!("# capturable. That is the second time this has happened in this file.");
    add!("starstep = 51 - starback");
    add!("starmod = mod(starstep, 13)");
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
        rows.push(format!("at{k} = -{}", k % 4 + 1));
    }
    for seat in 0..4 {
        rows.push(format!("cuts{seat} = 0"));
    }
    add!("");
    add!("# --- walking, rather than teleporting ---------------------------");
    add!("# A token that goes from square 10 to square 14 has TELEPORTED. What");
    add!("# it should do is walk, and walking needs two more numbers: where it");
    add!("# set off from, and when.");
    add!("#");
    add!("# Then where it is drawn is a function of the clock, like everything");
    add!("# else here -- no stepping, no frame counter, and a move that can be");
    add!("# scrubbed backwards and looks the same.");
    add!("pace = 7");
    for k in 0..tokens {
        rows.push(format!("from{k} = at{k}"));
        rows.push(format!("moved{k} = -99"));
    }
    add!("");
    add!("# how far along it has got. `min` is what stops it: it walks at `pace`");
    add!("# squares a second and simply stops when it arrives, so no rule has to");
    add!("# notice that it has.");
    add!("#");
    add!("# A token sent home does not walk -- `at` goes negative and the min");
    add!("# takes it at once, which is right: it was carried, not walked.");
    for k in 0..tokens {
        rows.push(format!("walk{k} = min(at{k}, from{k} + pace*max(0, time - moved{k}))"));
    }

    add!("");
    add!("# --- the mercy rule ---------------------------------------------");
    add!("# `no way home until you have cut somebody` can leave a seat that has");
    add!("# cut nobody unable to finish at all, which is not a rule, it is a");
    add!("# deadlock. So its FARTHEST token is let home anyway -- one of them,");
    add!("# the one that has earned it.");
    for seat in 0..4 {
        let mine: Vec<String> =
            (0..tokens).filter(|k| seat_of(*k) == seat).map(|k| format!("at{k}")).collect();
        let far = mine.iter().skip(1).fold(mine[0].clone(), |acc, t| format!("max({acc}, {t})"));
        rows.push(format!("far{seat} = {far}"));
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
    add!("# --- the die, thrown across the board ---------------------------");
    add!("#");
    add!("# Five rows, because the throw itself lives in `plotkit::dice` where");
    add!("# any game can have one. All five take the same four things -- the");
    add!("# seed, which throw this is, how long ago it left the hand, and how");
    add!("# big the board is -- so they differ only in the name at the front.");
    add!("#");
    add!("# It slides with its speed dying away, so the distance it covers");
    add!("# approaches a limit and never passes it; it folds off the walls,");
    add!("# which IS the reflection, at the angle it struck; and it turns more");
    add!("# slowly as it goes, landing square on a right angle.");
    add!("span = 6.4");
    add!("age = max(0, time - flung)");
    add!("die = dieface(seed, rolls, age, span)");
    add!("settled = diedone(seed, rolls, age, span)");
    add!("");
    add!("# --- what you can see ------------------------------------------");
    add!("# The die itself is a MARK, further down, so it can be tapped -- this");
    add!("# is only its face, which has to be redrawn every frame because it");
    add!("# changes, and a mark\'s points do not.");
    add!("dice(die, diex(seed, rolls, age, span), diey(seed, rolls, age, span), \
         0.78, dieturn(seed, rolls, age, span), \
         diesquash(seed, rolls, age, span),          dienext(seed, rolls, age, span))");
    add!("");
    add!("# whose turn it is, in that seat\'s own colour, in the middle of the");
    add!("# board where the four home paths meet.");
    add!("color(pick(turn, 0xE0704A, 0x6FCF97, 0x4FBCD4, 0xE0A44A))");
    add!("param(0.62*exp(i*t) + 8.6 + 1.5i, 0, tau)");
    add!("digits(turn + 1, 8.6, 1.5, 0.42)");
    add!("");
    add!("# a ring that shrinks onto the die as it settles, so it is plain that");
    add!("# the number is not yet the number. it follows the die about.");
    add!("color(0x46525E)");
    add!("param(if(settled, 0, 1.2)*exp(i*t) + diex(seed, rolls, age, span) \
         + i*diey(seed, rolls, age, span), 0, tau)");

    add!("# --- throwing it ------------------------------------------------");
    add!("# `flung` is WHEN, so everything above is a function of the clock and");
    add!("# nothing has to be stepped frame by frame.");
    add!("when tap 17: rolls = rolls + if(rolled == 1, 0, 1), \
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
        // Two of ONE other seat standing together on the square this token
        // would land on. `sq` already carries the seat offset, so a pair is
        // two tokens of a seat with the same `sq` -- and the yard and the home
        // column hold numbers no track square can, so they cannot make a wall.
        let mut pairs: Vec<String> = Vec::new();
        for (a, b) in (0..tokens).flat_map(|a| (a + 1..tokens).map(move |b| (a, b))) {
            if seat_of(a) == seat_of(b) && seat_of(a) != seat_of(k) {
                pairs.push(format!("and(sq{a} == sq{b}, sq{a} == land{k})"));
            }
        }
        let blocked = pairs.into_iter().fold(String::from("0"), |acc, t| format!("or({acc}, {t})"));
        rows.push(format!("land{k} = mod(13*seat{k} + {lands}, 52)"));
        rows.push(format!(
            "onto{k} = if(blockade == 0, 0, {blocked})"
        ));
        rows.push(format!(
            "can{k} = and(and(and(seat{k} == turn, rolled == 1), settled), \
             if(at{k} < 0, or(die == opens, and(alsoone == 1, die == 1)), \
             and(and({lands} <= 57, not(onto{k})), \
             or(or(or(mustcut == 0, cuts[seat{k}] > 0), {lands} <= 50), \
             and(mercy == 1, at{k} >= far[seat{k}])))))"
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
            "param(if(can{k}, 0.52, 0)*exp(i*t) + ludox(seat{k}, walk{k}) + i*ludoy(seat{k}, walk{k}), 0, tau)"
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
        // Where everybody stood, so a capture can be noticed afterwards.
        deeds.push(format!("each j in 0..{tokens} (was[j] = at[j])"));
        // Where it set off from and when, so it can walk there. Coming out of
        // the yard it does not walk: it is placed on its start square, and a
        // token sliding out from the yard would be walking a path that is not
        // part of the board.
        deeds.push(format!("from[{k}] = if(ok, if(at{k} < 0, 0, at{k}), from{k})"));
        deeds.push(format!("moved[{k}] = if(ok, time, moved{k})"));
        deeds.push(format!("at[{k}] = if(ok, if(at{k} < 0, 0, at{k} + die), at{k})"));
        deeds.push(format!(
            "here = if(ok, if(was < 0, mod(13*seat{k}, 52), \
             if(at[{k}] > 50, 200 + 10*seat{k} + at[{k}], mod(13*seat{k} + at[{k}], 52))), -999)"
        ));
        // ANYBODY standing where this one just landed goes back to the yard.
        // One line, for sixteen tokens -- written a token at a time it was
        // sixteen rules of fifteen lines, which is why `each` exists and why
        // there used to be two tokens a seat instead of four.
        //
        // Not on a safe square, though. The four starts are at 0, 13, 26 and 39
        // and the four starred ones eight further on -- so "safe" is `here`
        // being a multiple of thirteen, or eight past one. The whole rule in one
        // `mod`, rather than a list of eight numbers to mistype.
        //
        // Back to `-1 - mod(j, 4)`: its own yard place, worked out from the
        // index rather than written down sixteen times.
        deeds.push(format!(
            "each j in 0..{tokens} (at[j] = if(and(and(and(seat[j] != seat{k}, sq[j] == here), \
             and(here >= 0, here < 52)), \
             not(or(mod(here, 13) == 0, mod(here, 13) == starmod))), \
             0 - 1 - mod(j, 4), at[j]))"
        ));
        // Did this move cut anybody? Counted, because a house rule may ask for
        // a cut before a seat is allowed home. A loop adds it up, so this is
        // two lines rather than a fifteen-term `or`.
        deeds.push("cut = 0".into());
        deeds.push(format!(
            "each j in 0..{tokens} (cut = cut + if(and(at[j] < 0, was[j] >= 0), 1, 0))"
        ));
        deeds.push("cut = if(ok, cut > 0, 0)".into());
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
    add!("# A star, because that is what a safe square is called. The four");
    add!("# starts are in each seat's own colour -- they are that seat's square,");
    add!("# and drawing them all in grey was why they looked misplaced rather");
    add!("# than looking like starts.");
    add!("#");
    add!("# The other one a seat has sits `starback` squares before it turns into");
    add!("# its home column. A token turns in off step 50, so the star is on");
    add!("# step 51 - starback -- and that is a DIAL, because which square this");
    add!("# is differs from board to board and is far easier to point at than to");
    add!("# describe. Slide it and watch the star move.");
    add!("#");
    add!("# Whatever it is set to, all four land on the same `mod 13`, since the");
    add!("# seats are thirteen apart. So `safe` stays two comparisons however far");
    add!("# round it is moved.");
    for seat in 0..4 {
        // The seat's own start square, in its own colour.
        rows.push(format!("color({})", ["0xE0704A", "0x6FCF97", "0x4FBCD4", "0xE0A44A"][seat]));
        rows.push(format!(
            "star(ludox({seat}, 0), ludoy({seat}, 0), if(stars == 1, 0.34, 0), 5)"
        ));
        rows.push("color(0x8A97A5)".into());
        rows.push(format!(
            "star(ludox({seat}, starstep), ludoy({seat}, starstep), if(stars == 1, 0.3, 0), 5)"
        ));
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
        let at = plotkit::ludo::waiting(seat, k % 4);
        let ring: Vec<Cx> = (0..=28).map(|j| at + Cx::polar(0.36, j as f64 / 28.0 * TAU)).collect();
        b.nib = Nib::Round(0.1);
        b.colour = shapes::ludo::SEATS[seat];
        stroke(&mut b, &ring);
        let m = b.sheet.marks.last_mut().expect("a token");
        m.closed = true;
        // Filled, so a token reads as a piece on a board rather than as a
        // circle drawn on one. Everything here was outlines, and outlines on
        // outlines is why it looked like a diagram.
        m.filled = true;
        m.place = Some((format!("ludox(seat{k}, walk{k})"), format!("ludoy(seat{k}, walk{k})")));
        b.selected = vec![b.sheet.len() - 1];
        b.group_alone();
    }
    // The die. A MARK, because only a mark can be tapped -- and placed AND
    // TURNED by the throw, so the square you tap is the square you can see
    // wherever it has slid to and however it is lying. That is what `placea`
    // is for, and it is the piece that was missing: a thing that follows a
    // number nearly always has to face somewhere too.
    // Round, and the same ivory as the die, because it cannot squash: a
    // square hit target stayed square while the die foreshortened, and read as
    // a second box drifting out of the first. A circle has no corners to give
    // that away, and the die body is drawn over it besides.
    b.colour = 0xEDE6D6;
    b.nib = Nib::Round(0.07);
    let at = Cx::ZERO;
    let ring: Vec<Cx> = (0..=28).map(|j| at + Cx::polar(0.34, j as f64 / 28.0 * TAU)).collect();
    stroke(&mut b, &ring);
    let m = b.sheet.marks.last_mut().expect("the die");
    m.closed = true;
    m.place =
        Some(("diex(seed, rolls, age, span)".into(), "diey(seed, rolls, age, span)".into()));
    m.spin = Some("dieturn(seed, rolls, age, span)".into());
    b.selected = vec![b.sheet.len() - 1];
    b.group_alone();
    b.selected.clear();
    b
}
